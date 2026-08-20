//! M5 public deployment-planning contracts.

#![allow(clippy::expect_used)] // Test-only construction keeps each semantic scenario legible.

use podman_lens::{
    ContainerIntent, DeploymentConnectionReference, DeploymentIntent, DeploymentResource, DeploymentResourceId,
    ExternalPrecondition, ImageIntent, ImagePullPolicy, NetworkIntent, ObservedApiVersion, ObservedPodmanVersion,
    PodIntent, ResourceKind, SecretIntent, SemanticOperationAction, SensitiveInputReference, StartupDependency,
    TargetProfile, VolumeIntent, plan_deployment,
};

fn target(version: &str) -> TargetProfile {
    TargetProfile::new(
        ObservedPodmanVersion::parse(version).expect("reviewed Podman version"),
        ObservedApiVersion::parse("4.0.0").expect("reviewed Libpod API version"),
    )
    .expect("compatible target profile")
}

fn id(kind: ResourceKind, name: &str) -> DeploymentResourceId {
    DeploymentResourceId::new(kind, name).expect("valid resource identity")
}

fn complete_pod_intent(version: &str) -> DeploymentIntent {
    let network = id(ResourceKind::Network, "application-network");
    let volume = id(ResourceKind::Volume, "application-data");
    let secret = id(ResourceKind::Secret, "application-password");
    let image = id(ResourceKind::Image, "registry.example.invalid/application:1.0");
    let pod = id(ResourceKind::Pod, "application");
    let container = id(ResourceKind::Container, "application-web");

    let mut pod_intent = PodIntent::new(pod).expect("pod identity");
    pod_intent.add_network(network.clone()).expect("network identity");
    pod_intent.add_member(container.clone()).expect("container identity");

    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container identity");
    container_intent
        .set_pod(pod_intent.identity().clone())
        .expect("pod identity");
    container_intent.add_volume(volume.clone()).expect("volume identity");
    container_intent.add_secret(secret.clone()).expect("secret identity");

    let mut intent = DeploymentIntent::new(target(version));
    intent.add_resource(DeploymentResource::Container(container_intent));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/application:1.0").expect("strict image reference"),
    ));
    intent.add_resource(DeploymentResource::Secret(
        SecretIntent::new(
            secret,
            SensitiveInputReference::new("vault/application-password").expect("external reference"),
        )
        .expect("secret identity"),
    ));
    intent.add_resource(DeploymentResource::Volume(
        VolumeIntent::new(volume).expect("volume identity"),
    ));
    intent.add_resource(DeploymentResource::Network(
        NetworkIntent::new(network).expect("network identity"),
    ));
    intent
}

#[test]
fn supported_target_profiles_produce_the_same_semantic_plan() {
    for version in ["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"] {
        let outcome = plan_deployment(&complete_pod_intent(version));
        assert!(outcome.is_success(), "{version}: {:?}", outcome.findings());
        let plan = outcome.plan().expect("plan");
        assert_eq!(plan.target().podman_version().original(), version);
        assert_eq!(plan.operations().len(), 7);
        assert_eq!(
            plan.operations()
                .iter()
                .map(|operation| operation.id().action())
                .collect::<Vec<_>>(),
            vec![
                SemanticOperationAction::Create,
                SemanticOperationAction::Create,
                SemanticOperationAction::Create,
                SemanticOperationAction::EnsureImage,
                SemanticOperationAction::Create,
                SemanticOperationAction::Create,
                SemanticOperationAction::StartPod,
            ]
        );
        assert_eq!(plan.operations()[3].image_pull_policy(), Some(ImagePullPolicy::Missing));
        assert_eq!(plan.operations()[6].depends_on().len(), 2);
    }
}

#[test]
fn independent_resource_kinds_have_stable_order_and_shared_prerequisites_are_emitted_once() {
    let first = complete_pod_intent("6.1.0");
    let mut second = DeploymentIntent::new(target("6.1.0"));
    for resource in first.resources().iter().rev().cloned() {
        second.add_resource(resource);
    }
    let first_plan = plan_deployment(&first).plan().cloned().expect("first plan");
    let second_plan = plan_deployment(&second).plan().cloned().expect("second plan");
    assert_eq!(first_plan, second_plan);
    assert_eq!(
        first_plan
            .operations()
            .iter()
            .filter(|operation| operation.id().resource().kind() == ResourceKind::Network)
            .count(),
        1
    );
}

#[test]
fn external_preconditions_are_explicit_and_emit_no_operation() {
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let container = id(ResourceKind::Container, "web");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(image.clone()).expect("external image"),
    ));
    intent.add_resource(DeploymentResource::Container(
        ContainerIntent::new(container, image).expect("container"),
    ));
    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    assert_eq!(plan.operations().len(), 2);
    assert!(
        plan.operations()
            .iter()
            .all(|operation| operation.id().resource().kind() != ResourceKind::Image)
    );
    assert_eq!(
        plan.operations()[1].id().action(),
        SemanticOperationAction::StartContainer
    );
}

#[test]
fn semantic_plan_retains_managed_intent_and_every_external_precondition_without_secret_leaks() {
    let managed_image = id(ResourceKind::Image, "managed-web");
    let source = "registry.example.invalid/team/web:1.2.3";
    let secret = id(ResourceKind::Secret, "managed-secret");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(managed_image, source).expect("managed image"),
    ));
    intent.add_resource(DeploymentResource::Secret(
        SecretIntent::new(
            secret,
            SensitiveInputReference::new("vault/production/web-password").expect("external material"),
        )
        .expect("managed secret"),
    ));
    for (kind, name) in [
        (ResourceKind::Network, "external-network"),
        (ResourceKind::Volume, "external-volume"),
        (ResourceKind::Image, "external-image"),
        (ResourceKind::Secret, "external-secret"),
    ] {
        intent.add_resource(DeploymentResource::ExternalPrecondition(
            ExternalPrecondition::new(id(kind, name)).expect("allowed external prerequisite"),
        ));
    }

    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    assert_eq!(
        plan.external_preconditions()
            .iter()
            .map(|precondition| precondition.identity().name())
            .collect::<Vec<_>>(),
        vec![
            "external-network",
            "external-volume",
            "external-image",
            "external-secret"
        ]
    );
    let image = plan
        .operations()
        .iter()
        .find_map(|operation| match operation.resource_intent() {
            DeploymentResource::Image(image) => Some(image),
            _ => None,
        })
        .expect("managed image operation");
    assert_eq!(image.source(), source);
    let secret = plan
        .operations()
        .iter()
        .find_map(|operation| match operation.resource_intent() {
            DeploymentResource::Secret(secret) => Some(secret),
            _ => None,
        })
        .expect("managed secret operation");
    assert_eq!(secret.material().as_str(), "vault/production/web-password");
    let debug = format!("{plan:?}");
    assert!(!debug.contains("vault/production/web-password"));
    assert!(debug.contains("SensitiveInputReference([redacted])"));
}

#[test]
fn missing_and_duplicate_or_conflicting_resources_are_structured_findings() {
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let container = id(ResourceKind::Container, "web");
    let mut missing = DeploymentIntent::new(target("6.1.0"));
    missing.add_resource(DeploymentResource::Container(
        ContainerIntent::new(container, image).expect("container"),
    ));
    let missing = plan_deployment(&missing);
    assert_eq!(missing.findings()[0].code().as_str(), "PLN0037");

    let network = NetworkIntent::new(id(ResourceKind::Network, "network")).expect("network");
    let mut duplicate = DeploymentIntent::new(target("6.1.0"));
    duplicate.add_resource(DeploymentResource::Network(network.clone()));
    duplicate.add_resource(DeploymentResource::Network(network));
    let duplicate = plan_deployment(&duplicate);
    assert_eq!(duplicate.findings()[0].code().as_str(), "PLN0035");
    assert_eq!(duplicate.findings()[0].occurrence(), None);
    assert_eq!(duplicate.findings()[0].count(), Some(2));

    let same = id(ResourceKind::Network, "conflict");
    let mut conflict = DeploymentIntent::new(target("6.1.0"));
    conflict.add_resource(DeploymentResource::Network(
        NetworkIntent::new(same.clone()).expect("network"),
    ));
    conflict.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(same).expect("external network"),
    ));
    let conflict = plan_deployment(&conflict);
    assert_eq!(conflict.findings()[0].code().as_str(), "PLN0036");
}

#[test]
fn pod_members_start_once_and_cross_pod_dependencies_are_lifted() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let first_pod = id(ResourceKind::Pod, "first");
    let second_pod = id(ResourceKind::Pod, "second");
    let first = id(ResourceKind::Container, "first-member");
    let second = id(ResourceKind::Container, "second-member");
    let mut first_pod_intent = PodIntent::new(first_pod.clone()).expect("pod");
    first_pod_intent.add_member(first.clone()).expect("member");
    let mut second_pod_intent = PodIntent::new(second_pod.clone()).expect("pod");
    second_pod_intent.add_member(second.clone()).expect("member");
    let mut first_container = ContainerIntent::new(first.clone(), image.clone()).expect("container");
    first_container.set_pod(first_pod.clone()).expect("pod");
    let mut second_container = ContainerIntent::new(second.clone(), image.clone()).expect("container");
    second_container.set_pod(second_pod.clone()).expect("pod");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(first_pod_intent));
    intent.add_resource(DeploymentResource::Pod(second_pod_intent));
    intent.add_resource(DeploymentResource::Container(first_container));
    intent.add_resource(DeploymentResource::Container(second_container));
    intent.add_startup_dependency(StartupDependency::new(first.clone(), second.clone()).expect("dependency"));
    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    assert_eq!(
        plan.operations()
            .iter()
            .filter(|operation| operation.id().action() == SemanticOperationAction::StartPod)
            .count(),
        2
    );
    assert!(
        plan.operations()
            .iter()
            .all(|operation| operation.id().action() != SemanticOperationAction::StartContainer)
    );
    let second_start = plan
        .operations()
        .iter()
        .find(|operation| {
            operation.id().resource() == &second_pod && operation.id().action() == SemanticOperationAction::StartPod
        })
        .expect("second pod start");
    assert!(
        second_start
            .depends_on()
            .iter()
            .any(|dependency| dependency.resource() == &first_pod
                && dependency.action() == SemanticOperationAction::StartPod)
    );
    assert!(matches!(second_start.resource_intent(), DeploymentResource::Pod(_)));
}

#[test]
fn same_pod_order_and_start_cycles_are_rejected() {
    let mut same_pod = complete_pod_intent("6.1.0");
    let member = id(ResourceKind::Container, "application-web");
    same_pod.add_startup_dependency(StartupDependency::new(member.clone(), member).expect("dependency"));
    let same_pod = plan_deployment(&same_pod);
    assert_eq!(same_pod.findings()[0].code().as_str(), "PLN0044");

    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let left = id(ResourceKind::Container, "left");
    let right = id(ResourceKind::Container, "right");
    let mut cycle = DeploymentIntent::new(target("6.1.0"));
    cycle.add_resource(DeploymentResource::Image(
        ImageIntent::new(image.clone(), "registry.example.invalid/app:1").expect("image"),
    ));
    cycle.add_resource(DeploymentResource::Container(
        ContainerIntent::new(left.clone(), image.clone()).expect("left"),
    ));
    cycle.add_resource(DeploymentResource::Container(
        ContainerIntent::new(right.clone(), image).expect("right"),
    ));
    cycle.add_startup_dependency(StartupDependency::new(left.clone(), right.clone()).expect("edge"));
    cycle.add_startup_dependency(StartupDependency::new(right, left).expect("edge"));
    let cycle = plan_deployment(&cycle);
    assert_eq!(cycle.findings()[0].code().as_str(), "PLN0039");
}

#[test]
fn incomplete_pod_membership_is_rejected_before_operation_ordering() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let pod = id(ResourceKind::Pod, "application");
    let container = id(ResourceKind::Container, "application-member");
    let mut member = ContainerIntent::new(container.clone(), image.clone()).expect("container");
    member.set_pod(pod.clone()).expect("pod");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(PodIntent::new(pod.clone()).expect("pod")));
    intent.add_resource(DeploymentResource::Container(member));
    let outcome = plan_deployment(&intent);
    assert!(!outcome.is_success());
    let finding = outcome
        .findings()
        .iter()
        .find(|finding| finding.code().as_str() == "PLN0043")
        .expect("pod membership finding");
    assert_eq!(finding.subject(), Some(&container));
    assert_eq!(finding.related(), &[pod]);
    assert_eq!(finding.field(), Some("pod"));
}

#[test]
fn constructors_reject_wrong_kinds_invalid_images_and_sensitive_payload_spelling() {
    assert_eq!(
        DeploymentResourceId::new(ResourceKind::Network, "")
            .expect_err("empty name")
            .code()
            .as_str(),
        "PLN0034"
    );
    assert_eq!(
        NetworkIntent::new(id(ResourceKind::Volume, "not-a-network"))
            .expect_err("wrong kind")
            .code()
            .as_str(),
        "PLN0034"
    );
    assert_eq!(
        ContainerIntent::new(
            id(ResourceKind::Container, "wrong-image"),
            id(ResourceKind::Network, "not-an-image"),
        )
        .expect_err("wrong prerequisite kind")
        .code()
        .as_str(),
        "PLN0034"
    );
    assert_eq!(
        ExternalPrecondition::new(id(ResourceKind::Pod, "not-external"))
            .expect_err("pods need managed membership")
            .code()
            .as_str(),
        "PLN0042"
    );
    assert_eq!(
        PodIntent::new(id(ResourceKind::Network, "not-a-pod"))
            .expect_err("wrong pod kind")
            .code()
            .as_str(),
        "PLN0034"
    );
    assert_eq!(
        VolumeIntent::new(id(ResourceKind::Network, "not-a-volume"))
            .expect_err("wrong volume kind")
            .code()
            .as_str(),
        "PLN0034"
    );
    assert_eq!(
        SecretIntent::new(
            id(ResourceKind::Network, "not-a-secret"),
            SensitiveInputReference::new("vault/test").expect("reference"),
        )
        .expect_err("wrong secret kind")
        .code()
        .as_str(),
        "PLN0034"
    );
    assert_eq!(
        ImageIntent::new(
            id(ResourceKind::Network, "not-an-image"),
            "registry.example.invalid/team/image:1",
        )
        .expect_err("wrong image kind")
        .code()
        .as_str(),
        "PLN0034"
    );
    assert_eq!(
        SensitiveInputReference::new("literal:not-safe")
            .expect_err("literal payload")
            .code()
            .as_str(),
        "PLN0040"
    );
    assert_eq!(
        ImageIntent::new(
            id(ResourceKind::Image, "bad"),
            "registry.example.invalid/app:1 $(whoami)"
        )
        .expect_err("shell syntax is not an image reference")
        .code()
        .as_str(),
        "PLN0041"
    );
    let sensitive = SensitiveInputReference::new("vault/test-value").expect("reference");
    assert!(!format!("{sensitive:?}").contains("vault/test-value"));
}

#[test]
fn validation_collects_independent_missing_resources_and_startup_errors() {
    let container = id(ResourceKind::Container, "broken");
    let image = id(ResourceKind::Image, "registry.example.invalid/broken:1");
    let pod = id(ResourceKind::Pod, "missing-pod");
    let network = id(ResourceKind::Network, "missing-network");
    let volume = id(ResourceKind::Volume, "missing-volume");
    let secret = id(ResourceKind::Secret, "missing-secret");
    let missing_container = id(ResourceKind::Container, "missing-container");
    let mut broken = ContainerIntent::new(container.clone(), image).expect("container");
    broken.set_pod(pod.clone()).expect("pod identity");
    broken.add_network(network).expect("network identity");
    broken.add_volume(volume).expect("volume identity");
    broken.add_secret(secret).expect("secret identity");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Container(broken));
    intent.add_startup_dependency(StartupDependency::new(container.clone(), missing_container).expect("edge"));
    let outcome = plan_deployment(&intent);
    assert!(!outcome.is_success());
    assert!(
        outcome
            .findings()
            .iter()
            .any(|finding| finding.code().as_str() == "PLN0038" && finding.field() == Some("networks"))
    );
    assert!(outcome.findings().len() >= 6, "{:?}", outcome.findings());
    assert!(
        outcome
            .findings()
            .iter()
            .any(|finding| finding.field() == Some("networks") && !finding.related().is_empty())
    );
    assert!(outcome.findings().iter().any(|finding| {
        finding.field() == Some("pod") && finding.subject() == Some(&container) && finding.related() == [pod.clone()]
    }));
    assert!(
        outcome
            .findings()
            .iter()
            .any(|finding| finding.field() == Some("startup_dependencies") && finding.occurrence() == Some(1))
    );
    assert!(outcome.findings().iter().all(|finding| !finding.message().is_empty()));
}

#[test]
fn pod_assignment_rejects_duplicate_and_conflicting_second_values_without_overwrite() {
    let container = id(ResourceKind::Container, "web");
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let first = id(ResourceKind::Pod, "first");
    let second = id(ResourceKind::Pod, "second");
    let mut intent = ContainerIntent::new(container, image).expect("container");
    intent.set_pod(first.clone()).expect("first pod");
    assert_eq!(
        intent.set_pod(first.clone()).expect_err("duplicate").code().as_str(),
        "PLN0035"
    );
    assert_eq!(intent.set_pod(second).expect_err("conflict").code().as_str(), "PLN0038");
    assert_eq!(intent.pod(), Some(&first));
}

#[test]
fn invalid_declaration_permutations_produce_identical_sorted_findings() {
    let network = id(ResourceKind::Network, "conflict");
    let image = id(ResourceKind::Image, "registry.example.invalid/missing:1");
    let container = id(ResourceKind::Container, "broken");
    let resources = vec![
        DeploymentResource::Network(NetworkIntent::new(network.clone()).expect("network")),
        DeploymentResource::ExternalPrecondition(ExternalPrecondition::new(network).expect("external network")),
        DeploymentResource::Container(ContainerIntent::new(container, image).expect("container")),
    ];
    let mut first = DeploymentIntent::new(target("6.1.0"));
    let mut second = DeploymentIntent::new(target("6.1.0"));
    for resource in &resources {
        first.add_resource(resource.clone());
    }
    for resource in resources.into_iter().rev() {
        second.add_resource(resource);
    }
    assert_eq!(plan_deployment(&first).findings(), plan_deployment(&second).findings());
}

#[test]
fn pod_volume_prerequisites_support_managed_external_and_duplicate_boundaries() {
    let pod = id(ResourceKind::Pod, "application");
    let volume = id(ResourceKind::Volume, "application-data");
    let mut managed_pod = PodIntent::new(pod.clone()).expect("pod");
    managed_pod.add_volume(volume.clone()).expect("volume");
    let mut managed = DeploymentIntent::new(target("6.1.0"));
    managed.add_resource(DeploymentResource::Pod(managed_pod));
    managed.add_resource(DeploymentResource::Volume(
        VolumeIntent::new(volume.clone()).expect("volume"),
    ));
    let managed_plan = plan_deployment(&managed).plan().cloned().expect("managed plan");
    assert_eq!(managed_plan.operations()[0].id().resource(), &volume);

    let mut external_pod = PodIntent::new(pod).expect("pod");
    external_pod.add_volume(volume.clone()).expect("volume");
    let mut external = DeploymentIntent::new(target("6.1.0"));
    external.add_resource(DeploymentResource::Pod(external_pod));
    external.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(volume.clone()).expect("external volume"),
    ));
    assert_eq!(
        plan_deployment(&external)
            .plan()
            .expect("external plan")
            .operations()
            .len(),
        1
    );

    let mut duplicate_pod = PodIntent::new(id(ResourceKind::Pod, "duplicate")).expect("pod");
    duplicate_pod.add_volume(volume.clone()).expect("volume");
    duplicate_pod.add_volume(volume).expect("volume");
    let mut duplicate = DeploymentIntent::new(target("6.1.0"));
    duplicate.add_resource(DeploymentResource::Pod(duplicate_pod));
    assert_eq!(plan_deployment(&duplicate).findings()[0].code().as_str(), "PLN0035");
}

#[test]
fn two_distinct_members_of_one_pod_cannot_have_a_startup_order() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let pod = id(ResourceKind::Pod, "application");
    let left = id(ResourceKind::Container, "left");
    let right = id(ResourceKind::Container, "right");
    let mut pod_intent = PodIntent::new(pod.clone()).expect("pod");
    pod_intent.add_member(left.clone()).expect("left member");
    pod_intent.add_member(right.clone()).expect("right member");
    let mut left_intent = ContainerIntent::new(left.clone(), image.clone()).expect("left");
    left_intent.set_pod(pod.clone()).expect("pod");
    let mut right_intent = ContainerIntent::new(right.clone(), image.clone()).expect("right");
    right_intent.set_pod(pod).expect("pod");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(left_intent));
    intent.add_resource(DeploymentResource::Container(right_intent));
    intent.add_startup_dependency(StartupDependency::new(left.clone(), right.clone()).expect("edge"));
    let outcome = plan_deployment(&intent);
    let finding = outcome
        .findings()
        .iter()
        .find(|finding| finding.code().as_str() == "PLN0044")
        .expect("same-pod finding");
    assert_eq!(finding.subject(), Some(&right));
    assert_eq!(finding.related(), &[left]);
    assert_eq!(finding.occurrence(), Some(1));
}

#[test]
fn portable_pull_reference_grammar_accepts_host_qualified_tag_and_digest_only() {
    let digest = format!("registry.example.invalid/team/app@sha256:{}", "a".repeat(64));
    for source in [
        "registry.example.invalid/team/app:1.2.3".to_owned(),
        "registry.example.invalid:5000/team/app:stable".to_owned(),
        digest,
    ] {
        assert!(ImageIntent::new(id(ResourceKind::Image, &source), source).is_ok());
    }
    for source in [
        "app:1",
        "localhost/app:1",
        "registry.example.invalid/team/app",
        "registry.example.invalid/team/app:-tag",
        "Registry.example.invalid/team/app:1",
        "registry_example.invalid/team/app:1",
        "registry.example.invalid/Team/app:1",
        "registry.example.invalid/team/app@sha256:abc",
        "registry.example.invalid/team/app:tag@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert_eq!(
            ImageIntent::new(id(ResourceKind::Image, "invalid"), source)
                .expect_err(source)
                .code()
                .as_str(),
            "PLN0041"
        );
    }
}

#[test]
fn public_constructors_reject_wrong_kinds_and_invalid_non_sensitive_references() {
    assert_eq!(
        DeploymentConnectionReference::new("")
            .expect_err("empty")
            .code()
            .as_str(),
        "PLN0034"
    );
    assert_eq!(
        DeploymentConnectionReference::new("bad\nconnection")
            .expect_err("control")
            .code()
            .as_str(),
        "PLN0034"
    );
    for reference in [
        "",
        "bad\nreference",
        "literal:value",
        "plaintext:value",
        "base64:value",
        "LITERAL:value",
        "PlainText:value",
        "BASE64:value",
    ] {
        assert!(SensitiveInputReference::new(reference).is_err(), "{reference}");
    }
    assert_eq!(
        ExternalPrecondition::new(id(ResourceKind::Pod, "pod"))
            .expect_err("pod")
            .code()
            .as_str(),
        "PLN0042"
    );
    assert_eq!(
        ExternalPrecondition::new(id(ResourceKind::Container, "container"))
            .expect_err("container")
            .code()
            .as_str(),
        "PLN0042"
    );
    let mut pod = PodIntent::new(id(ResourceKind::Pod, "pod")).expect("pod");
    assert!(pod.add_network(id(ResourceKind::Volume, "wrong")).is_err());
    assert!(pod.add_volume(id(ResourceKind::Network, "wrong")).is_err());
    assert!(pod.add_member(id(ResourceKind::Pod, "wrong")).is_err());
    let mut container = ContainerIntent::new(
        id(ResourceKind::Container, "container"),
        id(ResourceKind::Image, "registry.example.invalid/image:1"),
    )
    .expect("container");
    assert!(container.add_network(id(ResourceKind::Volume, "wrong")).is_err());
    assert!(container.add_volume(id(ResourceKind::Network, "wrong")).is_err());
    assert!(container.add_secret(id(ResourceKind::Volume, "wrong")).is_err());
    assert!(StartupDependency::new(id(ResourceKind::Pod, "wrong"), id(ResourceKind::Container, "container")).is_err());
}

#[test]
fn duplicate_startup_dependencies_report_the_second_edge_position() {
    let image = id(ResourceKind::Image, "registry.example.invalid/team/app:1");
    let first = id(ResourceKind::Container, "first");
    let second = id(ResourceKind::Container, "second");
    let dependency = StartupDependency::new(first.clone(), second.clone()).expect("dependency");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image.clone(), "registry.example.invalid/team/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(
        ContainerIntent::new(first, image.clone()).expect("first container"),
    ));
    intent.add_resource(DeploymentResource::Container(
        ContainerIntent::new(second.clone(), image).expect("second container"),
    ));
    intent.add_startup_dependency(dependency.clone());
    intent.add_startup_dependency(dependency);
    let outcome = plan_deployment(&intent);
    let finding = outcome
        .findings()
        .iter()
        .find(|finding| finding.code().as_str() == "PLN0035" && finding.field() == Some("startup_dependencies"))
        .expect("duplicate startup finding");
    assert_eq!(finding.subject(), Some(&second));
    assert_eq!(finding.occurrence(), Some(2));
    assert_eq!(finding.count(), None);
}
