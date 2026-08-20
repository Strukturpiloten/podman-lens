//! M6-A transport rendering contracts; no test contacts or mutates Podman.

#![allow(clippy::expect_used)]

use podman_lens::{
    AbsoluteContainerPath, ArgumentArray, ContainerHostname, ContainerIntent, ContainerUser, ContainerWorkdir,
    DeploymentConnectionReference, DeploymentEnvironmentValue, DeploymentIntent, DeploymentResource,
    DeploymentResourceId, EnvironmentAssignment, EnvironmentName, ExternalPrecondition, ImageIntent, Label, LabelKey,
    NamedVolumeCopyMode, NamedVolumeMount, NetworkIntent, ObservedApiVersion, ObservedPodmanVersion, PodIntent,
    PublicLabelValue, RenderStatus, RenderedHttpBody, ResourceKind, RestartPolicy, SecretIntent,
    SensitiveInlineEnvironmentValue, SensitiveInputReference, TargetProfile, VolumeIntent, artifact::deployment_v1,
    plan_deployment, render_deployment,
};

fn id(kind: ResourceKind, name: &str) -> DeploymentResourceId {
    DeploymentResourceId::new(kind, name).expect("valid identity")
}

fn mount(volume: DeploymentResourceId, destination: &str) -> NamedVolumeMount {
    NamedVolumeMount::new(
        volume,
        AbsoluteContainerPath::new(destination).expect("destination"),
        false,
        NamedVolumeCopyMode::Copy,
    )
    .expect("mount")
}

fn target(engine: &str, api: &str) -> TargetProfile {
    TargetProfile::new(
        ObservedPodmanVersion::parse(engine).expect("engine"),
        ObservedApiVersion::parse(api).expect("api"),
    )
    .expect("target")
}

fn complete_plan(version: &str) -> podman_lens::DeploymentPlan {
    let network = id(ResourceKind::Network, "network one");
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let pod = id(ResourceKind::Pod, "pod-one");
    let container = id(ResourceKind::Container, "container-one");
    let mut pod_intent = PodIntent::new(pod.clone()).expect("pod");
    pod_intent.add_network(network.clone()).expect("network");
    pod_intent.add_member(container.clone()).expect("member");
    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
    container_intent.set_pod(pod).expect("pod assignment");
    let mut intent = DeploymentIntent::new(target(version, version));
    intent.set_connection(DeploymentConnectionReference::new("remote-one").expect("connection"));
    intent.add_resource(DeploymentResource::Network(
        NetworkIntent::new(network).expect("network"),
    ));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/team/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(container_intent));
    plan_deployment(&intent).plan().cloned().expect("semantic plan")
}

fn all_operation_plan(version: &str) -> podman_lens::DeploymentPlan {
    let network = id(ResourceKind::Network, "network");
    let volume = id(ResourceKind::Volume, "volume");
    let secret = id(ResourceKind::Secret, "secret");
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let pod = id(ResourceKind::Pod, "pod");
    let member = id(ResourceKind::Container, "member");
    let standalone = id(ResourceKind::Container, "standalone");
    let mut pod_intent = PodIntent::new(pod.clone()).expect("pod");
    pod_intent.add_network(network.clone()).expect("network");
    pod_intent.add_member(member.clone()).expect("member");
    let mut member_intent = ContainerIntent::new(member, image.clone()).expect("member");
    member_intent.set_pod(pod).expect("pod assignment");
    let mut intent = DeploymentIntent::new(target(version, version));
    intent.add_resource(DeploymentResource::Network(
        NetworkIntent::new(network).expect("network"),
    ));
    intent.add_resource(DeploymentResource::Volume(VolumeIntent::new(volume).expect("volume")));
    intent.add_resource(DeploymentResource::Secret(
        SecretIntent::new(
            secret,
            SensitiveInputReference::new("vault/app-password").expect("secret reference"),
        )
        .expect("secret"),
    ));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image.clone(), "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(member_intent));
    intent.add_resource(DeploymentResource::Container(
        ContainerIntent::new(standalone, image).expect("standalone"),
    ));
    plan_deployment(&intent).plan().cloned().expect("semantic plan")
}

fn external_precondition_plan() -> podman_lens::DeploymentPlan {
    let mut intent = DeploymentIntent::new(target("6.1.0", "6.1.0"));
    intent.set_connection(DeploymentConnectionReference::new("review-remote").expect("connection"));
    for (kind, name) in [
        (ResourceKind::Network, "outside-network"),
        (ResourceKind::Volume, "outside-volume"),
        (ResourceKind::Image, "registry.example.invalid/outside:1"),
        (ResourceKind::Secret, "outside-secret"),
    ] {
        intent.add_resource(DeploymentResource::ExternalPrecondition(
            ExternalPrecondition::new(id(kind, name)).expect("external precondition"),
        ));
    }
    plan_deployment(&intent).plan().cloned().expect("semantic plan")
}

#[test]
fn reviewed_versions_render_the_complete_m5_surface_deterministically() {
    for version in ["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"] {
        let plan = complete_plan(version);
        let first = render_deployment(&plan).rendering().cloned().expect("rendering");
        let second = render_deployment(&plan).rendering().cloned().expect("rendering");
        assert_eq!(first, second);
        assert_eq!(first.status(), RenderStatus::Exact);
        assert_eq!(first.operations().len(), 5);
        assert!(
            first
                .operations()
                .iter()
                .all(|operation| operation.cli().program() == "podman")
        );
        assert!(
            first
                .operations()
                .iter()
                .all(|operation| operation.cli().argv()[0] == "--connection")
        );
        assert!(first.operations().iter().all(|operation| {
            operation
                .libpod()
                .path_and_query()
                .starts_with(&format!("/v{version}/libpod/"))
        }));
        assert!(
            first
                .operations()
                .iter()
                .all(|operation| !operation.libpod().path_and_query().contains("remote%20one"))
        );
        let pod = first
            .operations()
            .iter()
            .find(|operation| {
                operation.cli().argv().contains(&"pod".to_owned())
                    && operation.cli().argv().contains(&"create".to_owned())
            })
            .expect("pod create");
        assert!(
            pod.cli()
                .argv()
                .windows(2)
                .any(|arguments| { arguments == ["--network", "network one"] })
        );
        assert_eq!(
            pod.libpod().body(),
            &RenderedHttpBody::Json(serde_json::json!({
                "name": "pod-one",
                "networks": {"network one": {}}
            }))
        );
        let container = first
            .operations()
            .iter()
            .find(|operation| {
                operation.cli().argv().contains(&"container".to_owned())
                    && operation.cli().argv().contains(&"create".to_owned())
            })
            .expect("container create");
        assert!(
            container
                .cli()
                .argv()
                .windows(2)
                .any(|arguments| { arguments == ["--pod", "pod-one"] })
        );
        assert!(!container.cli().argv().contains(&"--network".to_owned()));
        assert_eq!(
            container.libpod().body(),
            &RenderedHttpBody::Json(serde_json::json!({
                "image": "registry.example.invalid/team/app:1",
                "pod": "pod-one"
            }))
        );
    }
}

#[test]
fn unpodded_container_uses_its_distinct_start_container_operation() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let container = id(ResourceKind::Container, "standalone");
    let mut intent = DeploymentIntent::new(target("6.1.0", "6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image.clone(), "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(
        ContainerIntent::new(container.clone(), image).expect("container"),
    ));
    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    let rendering = render_deployment(&plan).rendering().cloned().expect("rendering");
    assert!(
        rendering
            .operations()
            .iter()
            .any(|operation| operation.cli().argv().contains(&"start".to_owned())
                && operation.cli().argv().contains(&container.name().to_owned()))
    );
}

#[test]
fn rendering_uses_explicit_pull_never_encodes_once_and_defers_secret_payloads() {
    let rendering = render_deployment(&complete_plan("6.1.0"))
        .rendering()
        .cloned()
        .expect("rendering");
    let image = rendering
        .operations()
        .iter()
        .find(|operation| operation.cli().argv().contains(&"image".to_owned()))
        .expect("image");
    assert!(image.cli().argv().contains(&"--policy=missing".to_owned()));
    assert!(
        image
            .libpod()
            .path_and_query()
            .contains("registry.example.invalid%2Fteam%2Fapp%3A1")
    );
    let container = rendering
        .operations()
        .iter()
        .find(|operation| operation.cli().argv().contains(&"container".to_owned()))
        .expect("container");
    assert!(container.cli().argv().contains(&"--pull=never".to_owned()));
    let json = serde_json::to_string(&deployment_v1::deployment(&rendering)).expect("deployment artifact JSON");
    assert!(!json.contains("vault/app-password"));
}

#[test]
fn renderer_covers_every_evidenced_operation_for_every_reviewed_release() {
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("../catalogue/v1/podman-deployment-rendering.json"))
            .expect("renderer evidence");
    for version in evidence["reviewed_lines"].as_array().expect("reviewed lines") {
        let version = version["version"].as_str().expect("version");
        let rendering = render_deployment(&all_operation_plan(version))
            .rendering()
            .cloned()
            .expect("rendering");
        let categories = rendering
            .operations()
            .iter()
            .map(|operation| {
                format!(
                    "{}-{}",
                    match operation.operation().resource_intent() {
                        DeploymentResource::Network(_) => "network",
                        DeploymentResource::Volume(_) => "volume",
                        DeploymentResource::Secret(_) => "secret",
                        DeploymentResource::Image(_) => "image",
                        DeploymentResource::Pod(_) => "pod",
                        DeploymentResource::Container(_) => "container",
                        DeploymentResource::ExternalPrecondition(_) => "external",
                        _ => "unknown",
                    },
                    match operation.operation().id().action() {
                        podman_lens::SemanticOperationAction::EnsureImage => "pull",
                        podman_lens::SemanticOperationAction::Create => "create",
                        podman_lens::SemanticOperationAction::StartPod
                        | podman_lens::SemanticOperationAction::StartContainer => "start",
                        _ => "unknown",
                    }
                )
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            categories,
            std::collections::BTreeSet::from([
                "container-create".to_owned(),
                "container-start".to_owned(),
                "image-pull".to_owned(),
                "network-create".to_owned(),
                "pod-create".to_owned(),
                "pod-start".to_owned(),
                "secret-create".to_owned(),
                "volume-create".to_owned(),
            ])
        );
    }
}

#[test]
fn every_reviewed_release_has_exact_cli_and_libpod_renderings_for_all_operation_categories() {
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("../catalogue/v1/podman-deployment-rendering.json"))
            .expect("renderer evidence");
    for release in evidence["reviewed_lines"].as_array().expect("reviewed lines") {
        let version = release["version"].as_str().expect("version");
        let rendering = render_deployment(&all_operation_plan(version))
            .rendering()
            .cloned()
            .expect("rendering");
        let cases = [
            (
                vec!["network", "create", "network"],
                format!("/v{version}/libpod/networks/create"),
                RenderedHttpBody::Json(serde_json::json!({"name": "network"})),
            ),
            (
                vec!["volume", "create", "volume"],
                format!("/v{version}/libpod/volumes/create"),
                RenderedHttpBody::Json(serde_json::json!({"Name": "volume"})),
            ),
            (
                vec!["secret", "create", "secret", "-"],
                format!("/v{version}/libpod/secrets/create?name=secret"),
                RenderedHttpBody::ExternalSensitiveInput(
                    SensitiveInputReference::new("expected-only").expect("reference"),
                ),
            ),
            (
                vec!["image", "pull", "--policy=missing", "registry.example.invalid/app:1"],
                format!("/v{version}/libpod/images/pull?reference=registry.example.invalid%2Fapp%3A1&policy=missing"),
                RenderedHttpBody::Empty,
            ),
            (
                vec!["pod", "create", "--name", "pod", "--network", "network"],
                format!("/v{version}/libpod/pods/create"),
                RenderedHttpBody::Json(serde_json::json!({
                    "name": "pod",
                    "networks": {"network": {}}
                })),
            ),
            (
                vec![
                    "container",
                    "create",
                    "--name",
                    "member",
                    "--pull=never",
                    "--pod",
                    "pod",
                    "registry.example.invalid/app:1",
                ],
                format!("/v{version}/libpod/containers/create?name=member"),
                RenderedHttpBody::Json(serde_json::json!({
                    "image": "registry.example.invalid/app:1",
                    "pod": "pod"
                })),
            ),
            (
                vec!["pod", "start", "pod"],
                format!("/v{version}/libpod/pods/pod/start"),
                RenderedHttpBody::Empty,
            ),
            (
                vec!["container", "start", "standalone"],
                format!("/v{version}/libpod/containers/standalone/start"),
                RenderedHttpBody::Empty,
            ),
        ];
        for (expected_argv, expected_path, expected_body) in cases {
            let expected_argv = expected_argv.into_iter().map(str::to_owned).collect::<Vec<_>>();
            let operation = rendering
                .operations()
                .iter()
                .find(|operation| operation.cli().argv() == expected_argv)
                .expect("operation category");
            assert_eq!(operation.libpod().method(), podman_lens::RenderedHttpMethod::Post);
            assert_eq!(operation.libpod().path_and_query(), expected_path);
            match expected_body {
                RenderedHttpBody::ExternalSensitiveInput(_) => {
                    assert!(matches!(
                        operation.libpod().body(),
                        RenderedHttpBody::ExternalSensitiveInput(_)
                    ));
                    assert!(operation.cli().external_input().is_some());
                }
                expected_body => assert_eq!(operation.libpod().body(), &expected_body),
            }
        }
    }
}

#[test]
fn rendering_preserves_connection_and_safely_discloses_every_external_precondition_in_the_review_script() {
    let rendering = render_deployment(&external_precondition_plan())
        .rendering()
        .cloned()
        .expect("rendering");
    assert_eq!(
        rendering.connection().map(DeploymentConnectionReference::as_str),
        Some("review-remote")
    );
    let script = rendering.shell_script();
    for prerequisite in [
        "# Requires external network: 'outside-network'",
        "# Requires external volume: 'outside-volume'",
        "# Requires external image: 'registry.example.invalid/outside:1'",
        "# Requires external secret: 'outside-secret'",
    ] {
        assert!(
            script.contains(prerequisite),
            "missing prerequisite comment: {prerequisite}"
        );
    }
    assert!(!script.contains("vault/app-password"));
    let artifact = serde_json::to_value(deployment_v1::deployment(&rendering)).expect("artifact");
    assert_eq!(artifact["connection"], "review-remote");

    let unconnected = render_deployment(&all_operation_plan("6.1.0"))
        .rendering()
        .cloned()
        .expect("rendering");
    let artifact = serde_json::to_value(deployment_v1::deployment(&unconnected)).expect("artifact");
    assert!(artifact["connection"].is_null());
}

#[test]
fn endpoint_credential_and_path_connection_sentinels_cannot_reach_debug_or_deployment_artifacts() {
    let rendering = render_deployment(&complete_plan("6.1.0"))
        .rendering()
        .cloned()
        .expect("rendering");
    let artifact = serde_json::to_string(&deployment_v1::deployment(&rendering)).expect("artifact");
    for sentinel in [
        "ssh://user:password@example.invalid/run/user/1000/podman/podman.sock",
        "unix:///run/user/1000/podman/podman.sock",
        "tcp://token@example.invalid:8080",
        "/run/user/1000/podman/podman.sock",
    ] {
        let error = DeploymentConnectionReference::new(sentinel).expect_err("unsafe detail");
        assert_eq!(error.code().as_str(), "PLN0034");
        assert!(!format!("{error:?}").contains(sentinel));
        assert!(
            !artifact.contains(sentinel),
            "an unconstructable connection detail cannot serialize"
        );
    }
}

#[test]
fn unpodded_networks_and_resolved_managed_or_external_images_are_rendered_exactly() {
    let network = id(ResourceKind::Network, "standalone-network");
    let managed_image = id(ResourceKind::Image, "managed-image-id");
    let container = id(ResourceKind::Container, "standalone");
    let mut managed = DeploymentIntent::new(target("6.1.0", "6.1.0"));
    managed.add_resource(DeploymentResource::Network(
        NetworkIntent::new(network.clone()).expect("network"),
    ));
    managed.add_resource(DeploymentResource::Image(
        ImageIntent::new(managed_image.clone(), "registry.example.invalid/actual-source:1").expect("image"),
    ));
    let mut container_intent = ContainerIntent::new(container, managed_image).expect("container");
    container_intent.add_network(network).expect("network");
    managed.add_resource(DeploymentResource::Container(container_intent));
    let managed_plan = plan_deployment(&managed).plan().cloned().expect("plan");
    let managed_rendering = render_deployment(&managed_plan)
        .rendering()
        .cloned()
        .expect("rendering");
    let managed_container = managed_rendering
        .operations()
        .iter()
        .find(|operation| {
            operation.cli().argv().contains(&"container".to_owned())
                && operation.cli().argv().contains(&"create".to_owned())
        })
        .expect("container");
    assert!(
        managed_container
            .cli()
            .argv()
            .windows(2)
            .any(|arguments| { arguments == ["--network", "standalone-network"] })
    );
    assert_eq!(
        managed_container.cli().argv().last(),
        Some(&"registry.example.invalid/actual-source:1".to_owned())
    );
    assert_eq!(
        managed_container.libpod().body(),
        &RenderedHttpBody::Json(serde_json::json!({
            "image": "registry.example.invalid/actual-source:1",
            "networks": {"standalone-network": {}}
        }))
    );

    let external_image = id(ResourceKind::Image, "registry.example.invalid/external:1");
    let external_container = id(ResourceKind::Container, "external-image-container");
    let mut external = DeploymentIntent::new(target("6.1.0", "6.1.0"));
    external.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(external_image.clone()).expect("external image"),
    ));
    external.add_resource(DeploymentResource::Container(
        ContainerIntent::new(external_container, external_image.clone()).expect("container"),
    ));
    let external_plan = plan_deployment(&external).plan().cloned().expect("plan");
    let external_rendering = render_deployment(&external_plan)
        .rendering()
        .cloned()
        .expect("rendering");
    let external_container = external_rendering
        .operations()
        .iter()
        .find(|operation| {
            operation.cli().argv().contains(&"container".to_owned())
                && operation.cli().argv().contains(&"create".to_owned())
        })
        .expect("container");
    assert_eq!(
        external_container.cli().argv().last(),
        Some(&external_image.name().to_owned())
    );
}

#[test]
fn renderer_reports_every_unrepresentable_topology_field_without_partial_output() {
    let volume = id(ResourceKind::Volume, "data");
    let secret = id(ResourceKind::Secret, "credential");
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let pod = id(ResourceKind::Pod, "pod");
    let container = id(ResourceKind::Container, "container");
    let mut pod_intent = PodIntent::new(pod).expect("pod");
    pod_intent.add_infra_mount(mount(volume.clone(), "/pod-data"));
    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
    container_intent.add_mount(mount(volume.clone(), "/container-data"));
    container_intent.add_secret(secret.clone()).expect("secret");
    let mut intent = DeploymentIntent::new(target("6.1.0", "6.1.0"));
    intent.add_resource(DeploymentResource::Volume(VolumeIntent::new(volume).expect("volume")));
    intent.add_resource(DeploymentResource::Secret(
        SecretIntent::new(
            secret,
            SensitiveInputReference::new("vault/app-password").expect("secret reference"),
        )
        .expect("secret"),
    ));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(container_intent));
    let planning = plan_deployment(&intent);
    let plan = planning.plan().expect("valid semantic plan");
    let outcome = render_deployment(plan);
    assert!(!outcome.is_success());
    assert_eq!(
        outcome
            .findings()
            .iter()
            .map(|finding| (
                finding.code().as_str(),
                finding.subject().expect("subject").name(),
                finding.field()
            ))
            .collect::<Vec<_>>(),
        vec![
            ("PLN0046", "container", Some("mounts")),
            ("PLN0046", "container", Some("secrets")),
            ("PLN0046", "pod", Some("infra_mounts")),
        ]
    );
}

#[test]
fn renderer_rejects_each_unrendered_container_setting_without_leaking_sensitive_values() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let container = id(ResourceKind::Container, "container");
    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
    let sensitive_sentinel = "must-not-reach-rendering";
    {
        let settings = container_intent.settings_mut();
        settings
            .set_command(ArgumentArray::new(["serve"]).expect("command"))
            .expect("command");
        settings
            .set_entrypoint(ArgumentArray::new(["/entrypoint"]).expect("entrypoint"))
            .expect("entrypoint");
        settings
            .set_user(ContainerUser::new("1000:1000").expect("user"))
            .expect("user");
        settings
            .set_workdir(ContainerWorkdir::new(
                AbsoluteContainerPath::new("/srv/application").expect("workdir"),
            ))
            .expect("workdir");
        settings
            .set_hostname(ContainerHostname::new("app.example").expect("hostname"))
            .expect("hostname");
        settings
            .add_label(Label::new(
                LabelKey::new("org.example.mode").expect("label key"),
                PublicLabelValue::new("production").expect("public label value"),
            ))
            .expect("label");
        settings
            .add_environment(EnvironmentAssignment::new(
                EnvironmentName::new("PASSWORD").expect("environment name"),
                DeploymentEnvironmentValue::SensitiveInline(
                    SensitiveInlineEnvironmentValue::new(sensitive_sentinel).expect("sensitive value"),
                ),
            ))
            .expect("environment");
        settings
            .set_restart_policy(RestartPolicy::UnlessStopped)
            .expect("restart policy");
    }
    let mut intent = DeploymentIntent::new(target("6.1.0", "6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container_intent));
    let planning = plan_deployment(&intent);
    let plan = planning.plan().expect("valid semantic plan");
    let outcome = render_deployment(plan);
    assert!(!outcome.is_success());
    assert_eq!(
        outcome
            .findings()
            .iter()
            .map(podman_lens::RenderingFinding::field)
            .collect::<Vec<_>>(),
        vec![
            Some("command"),
            Some("entrypoint"),
            Some("environment"),
            Some("hostname"),
            Some("labels"),
            Some("restart_policy"),
            Some("user"),
            Some("workdir"),
        ]
    );
    let debug = format!("{outcome:?}");
    assert!(!debug.contains(sensitive_sentinel));
}

#[test]
fn renderer_rejects_api_engine_pairs_without_exact_wire_evidence() {
    for (engine, api) in [
        ("6.1.0", "4.0.0"),
        ("6.1.1", "6.1.1"),
        ("6.1.0+build.1", "6.1.0+build.1"),
    ] {
        let mut intent = DeploymentIntent::new(target(engine, api));
        intent.add_resource(DeploymentResource::ExternalPrecondition(
            ExternalPrecondition::new(id(ResourceKind::Network, "outside")).expect("precondition"),
        ));
        let planned = plan_deployment(&intent);
        let plan = planned.plan().expect("empty plan");
        let outcome = render_deployment(plan);
        assert!(!outcome.is_success());
        assert_eq!(outcome.findings()[0].code().as_str(), "PLN0045");
    }
}

#[test]
fn deployment_artifact_schema_is_strict_and_redacts_sensitive_values() -> Result<(), Box<dyn std::error::Error>> {
    let rendering = render_deployment(&complete_plan("6.1.0"))
        .rendering()
        .cloned()
        .expect("rendering");
    let value = serde_json::to_value(deployment_v1::deployment(&rendering))?;
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../docs/schemas/podman-lens-deployment-v1.schema.json"))?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)?;
    assert!(validator.is_valid(&value));
    let mut missing = value.clone();
    missing.as_object_mut().expect("object").remove("operations");
    assert!(!validator.is_valid(&missing));
    let mut missing_connection = value.clone();
    missing_connection.as_object_mut().expect("object").remove("connection");
    assert!(!validator.is_valid(&missing_connection));
    let mut invalid_connection = value.clone();
    invalid_connection["connection"] = serde_json::json!(false);
    assert!(!validator.is_valid(&invalid_connection));
    for unsafe_connection in [
        "ssh://user@example.invalid/run/user/1000/podman/podman.sock",
        "remote connection",
        "/run/user/1000/podman/podman.sock",
    ] {
        let mut invalid_connection = value.clone();
        invalid_connection["connection"] = serde_json::json!(unsafe_connection);
        assert!(
            !validator.is_valid(&invalid_connection),
            "schema must reject unsafe connection detail: {unsafe_connection}"
        );
    }
    let mut extra = value.clone();
    extra
        .as_object_mut()
        .expect("object")
        .insert("unexpected".to_owned(), serde_json::json!(true));
    assert!(!validator.is_valid(&extra));
    let mut wrong = value.clone();
    wrong["operations"][0]["cli"]["argv"] = serde_json::json!(true);
    assert!(!validator.is_valid(&wrong));
    let mut enum_value = value;
    enum_value["status"] = serde_json::json!("invalid");
    assert!(!validator.is_valid(&enum_value));
    let mut missing_json = serde_json::to_value(deployment_v1::deployment(&rendering))?;
    missing_json["operations"][0]["libpod"]["body"] = serde_json::json!({"kind": "json"});
    assert!(!validator.is_valid(&missing_json));
    let mut unexpected_json = serde_json::to_value(deployment_v1::deployment(&rendering))?;
    unexpected_json["operations"][1]["libpod"]["body"] = serde_json::json!({"kind": "empty", "json": {}});
    assert!(!validator.is_valid(&unexpected_json));
    let mut wrong_action_body = serde_json::to_value(deployment_v1::deployment(&rendering))?;
    wrong_action_body["operations"][0]["action"] = serde_json::json!("start_container");
    assert!(!validator.is_valid(&wrong_action_body));
    let mut wrong_method = serde_json::to_value(deployment_v1::deployment(&rendering))?;
    wrong_method["operations"][0]["libpod"]["method"] = serde_json::json!("GET");
    assert!(!validator.is_valid(&wrong_method));
    let mut wrong_path = serde_json::to_value(deployment_v1::deployment(&rendering))?;
    wrong_path["operations"][0]["libpod"]["path_and_query"] = serde_json::json!("/v6.1.0/libpod/volumes/create");
    assert!(!validator.is_valid(&wrong_path));
    let secret_rendering = render_deployment(&all_operation_plan("6.1.0"));
    let mut external_with_json = serde_json::to_value(deployment_v1::deployment(
        secret_rendering.rendering().expect("rendering"),
    ))?;
    let secret = external_with_json["operations"]
        .as_array_mut()
        .expect("operations")
        .iter_mut()
        .find(|operation| operation["resource"]["kind"] == "secret")
        .expect("secret");
    secret["libpod"]["body"]["json"] = serde_json::json!({});
    assert!(!validator.is_valid(&external_with_json));
    let mut secret_without_input = serde_json::to_value(deployment_v1::deployment(
        secret_rendering.rendering().expect("rendering"),
    ))?;
    let secret = secret_without_input["operations"]
        .as_array_mut()
        .expect("operations")
        .iter_mut()
        .find(|operation| operation["resource"]["kind"] == "secret")
        .expect("secret");
    secret["cli"]["external_sensitive_input_required"] = serde_json::json!(false);
    assert!(!validator.is_valid(&secret_without_input));
    Ok(())
}

#[test]
fn committed_renderer_evidence_covers_every_operation_and_reviewed_line() -> Result<(), Box<dyn std::error::Error>> {
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("../catalogue/v1/podman-deployment-rendering.json"))?;
    assert_eq!(evidence["schema_version"], 2);
    let lines = evidence["reviewed_lines"].as_array().expect("lines");
    assert_eq!(lines.len(), 7);
    assert_eq!(
        lines
            .iter()
            .map(|line| line["version"].as_str().expect("version"))
            .collect::<Vec<_>>(),
        vec!["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"]
    );
    assert!(
        lines
            .iter()
            .all(|line| line["revision"].as_str().is_some_and(|value| value.len() == 40))
    );
    for line in lines {
        let revision = line["revision"].as_str().expect("revision");
        let operations = line["operations"].as_array().expect("operations");
        assert_eq!(operations.len(), 8);
        for operation in operations {
            for source in ["cli_source", "libpod_endpoint_source"] {
                assert!(
                    operation[source]
                        .as_str()
                        .is_some_and(|source| source.contains(revision)),
                    "{source} must use the immutable release revision"
                );
            }
            if let Some(source) = operation["body_source"].as_str() {
                assert!(
                    source.contains(revision),
                    "body source must use the immutable release revision"
                );
            } else {
                assert!(operation["body_source"].is_null(), "body source must be a URL or null");
            }
        }
    }
    Ok(())
}

#[test]
fn checked_in_deployment_artifacts_are_byte_exact_and_never_expose_sensitive_input_references() {
    let rendering = render_deployment(&complete_plan("6.1.0"))
        .rendering()
        .cloned()
        .expect("rendering");
    let rendered_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&deployment_v1::deployment(&rendering)).expect("deployment artifact JSON")
    );
    let expected_json = include_str!("../fixtures/deployment/deployment-plan-v1.json");
    let rendered_script = rendering.shell_script();
    let expected_script = include_str!("../fixtures/deployment/deployment.sh");

    assert_eq!(rendered_json, expected_json);
    assert_eq!(rendered_script, expected_script);
    for artifact in [expected_json, expected_script] {
        assert!(
            !artifact.contains("vault/app-password"),
            "checked-in artifact must not expose the sensitive external-input reference sentinel"
        );
    }
}
