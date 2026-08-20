//! M6-A transport rendering contracts; no test contacts or mutates Podman.

#![allow(clippy::expect_used)]

use podman_lens::{
    AbsoluteContainerPath, ArgumentArray, ContainerHostname, ContainerIntent, ContainerUser, ContainerWorkdir,
    DeploymentConnectionReference, DeploymentEnvironmentValue, DeploymentIntent, DeploymentResource,
    DeploymentResourceId, DnsConfiguration, EnvironmentAssignment, EnvironmentName, ExternalPrecondition, HostAlias,
    ImageIntent, Label, LabelKey, NamedVolumeCopyMode, NamedVolumeMount, NetworkAttachment, NetworkCidr, NetworkIntent,
    NetworkRoute, NetworkSubnet, ObservedApiVersion, ObservedPodmanVersion, PodIntent, PortMapping, PortProtocol,
    PublicEnvironmentValue, PublicLabelValue, RenderStatus, RenderedHttpBody, ResourceKind, RestartPolicy, RouteType,
    SecretIntent, SensitiveInlineEnvironmentValue, SensitiveInputReference, TargetExecutionContext, TargetProfile,
    VolumeIntent, artifact::deployment_v1, plan_deployment, render_deployment,
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
    pod_intent
        .add_network(NetworkAttachment::new(network.clone()).expect("network attachment"))
        .expect("network");
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
    pod_intent
        .add_network(NetworkAttachment::new(network.clone()).expect("network attachment"))
        .expect("network");
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
    let mut selected_target = target("6.1.0", "6.1.0");
    selected_target.set_execution_context(TargetExecutionContext::Rootful);
    let mut intent = DeploymentIntent::new(selected_target);
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

fn core_settings_plan(version: &str) -> podman_lens::DeploymentPlan {
    core_settings_plan_with_restart(version, RestartPolicy::No)
}

fn core_settings_plan_with_restart(version: &str, restart_policy: RestartPolicy) -> podman_lens::DeploymentPlan {
    let volume = id(ResourceKind::Volume, "application-data");
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let pod = id(ResourceKind::Pod, "infra-pod");
    let container = id(ResourceKind::Container, "application");
    let mut pod_intent = PodIntent::new(pod).expect("pod");
    pod_intent.add_infra_mount(
        NamedVolumeMount::new(
            volume.clone(),
            AbsoluteContainerPath::new("/var/lib/infra").expect("destination"),
            true,
            NamedVolumeCopyMode::NoCopy,
        )
        .expect("infra mount"),
    );
    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
    container_intent.add_mount(
        NamedVolumeMount::new(
            volume.clone(),
            AbsoluteContainerPath::new("/var/lib/application").expect("destination"),
            false,
            NamedVolumeCopyMode::Copy,
        )
        .expect("mount"),
    );
    let settings = container_intent.settings_mut();
    settings
        .set_command(ArgumentArray::new(["serve", "--foreground"]).expect("command"))
        .expect("command");
    settings
        .set_entrypoint(ArgumentArray::new(["/init", "--safe"]).expect("entrypoint"))
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
        .set_hostname(ContainerHostname::new("application.example").expect("hostname"))
        .expect("hostname");
    for (key, value) in [("org.example.role", "web"), ("org.example.tier", "frontend")] {
        settings
            .add_label(Label::new(
                LabelKey::new(key).expect("label key"),
                PublicLabelValue::new(value).expect("public label value"),
            ))
            .expect("label");
    }
    for (name, value) in [("MODE", "production"), ("LOG_LEVEL", "info")] {
        settings
            .add_environment(EnvironmentAssignment::new(
                EnvironmentName::new(name).expect("environment name"),
                DeploymentEnvironmentValue::Public(PublicEnvironmentValue::new(value).expect("public value")),
            ))
            .expect("environment");
    }
    settings.set_restart_policy(restart_policy).expect("restart policy");
    let mut intent = DeploymentIntent::new(target(version, version));
    intent.add_resource(DeploymentResource::Volume(VolumeIntent::new(volume).expect("volume")));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(container_intent));
    plan_deployment(&intent).plan().cloned().expect("semantic plan")
}

#[test]
fn renderer_reports_one_pod_member_restart_boundary() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let pod = id(ResourceKind::Pod, "application");
    let container = id(ResourceKind::Container, "application");
    let mut pod_intent = PodIntent::new(pod.clone()).expect("pod");
    pod_intent.add_member(container.clone()).expect("member");
    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
    container_intent.set_pod(pod).expect("pod assignment");
    container_intent
        .settings_mut()
        .set_restart_policy(RestartPolicy::Always)
        .expect("restart policy");
    let mut intent = DeploymentIntent::new(target("6.1.0", "6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(container_intent));
    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    let outcome = render_deployment(&plan);
    assert!(!outcome.is_success());
    assert_eq!(outcome.findings()[0].field(), Some("restart_policy.pod_member"));
}

#[test]
fn renderer_rejects_cli_ambiguous_named_volume_spelling_without_partial_output() {
    let volume = id(ResourceKind::Volume, "data:ambiguous");
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let container = id(ResourceKind::Container, "application");
    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
    container_intent.add_mount(
        NamedVolumeMount::new(
            volume.clone(),
            AbsoluteContainerPath::new("/var/lib/application").expect("destination"),
            false,
            NamedVolumeCopyMode::Copy,
        )
        .expect("mount"),
    );
    let mut intent = DeploymentIntent::new(target("6.1.0", "6.1.0"));
    intent.add_resource(DeploymentResource::Volume(VolumeIntent::new(volume).expect("volume")));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container_intent));
    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    let outcome = render_deployment(&plan);
    assert!(!outcome.is_success());
    assert_eq!(outcome.findings()[0].field(), Some("mounts.cli_ambiguous"));
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
                "Networks": {"network one": {}}
            }))
        );
        assert!(matches!(
            pod.libpod().body(),
            RenderedHttpBody::Json(pod_body)
                if pod_body.get("Networks").is_some() && pod_body.get("networks").is_none()
        ));
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
fn reviewed_versions_render_every_core_setting_exactly_and_in_declaration_order() {
    for version in ["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"] {
        let rendering = render_deployment(&core_settings_plan(version))
            .rendering()
            .cloned()
            .expect("exact rendering");
        assert_eq!(rendering.status(), RenderStatus::Exact);
        let pod = rendering
            .operations()
            .iter()
            .find(|operation| operation.cli().argv().windows(2).any(|args| args == ["pod", "create"]))
            .expect("pod create");
        assert!(
            pod.cli()
                .argv()
                .windows(2)
                .any(|args| args == ["--volume", "application-data:/var/lib/infra:ro,nocopy"])
        );
        assert_eq!(
            pod.libpod().body(),
            &RenderedHttpBody::Json(serde_json::json!({
                "name": "infra-pod",
                "Networks": {},
                "volumes": [{"Name": "application-data", "Dest": "/var/lib/infra", "Options": ["ro", "nocopy"]}],
            }))
        );
        let container = rendering
            .operations()
            .iter()
            .find(|operation| {
                operation
                    .cli()
                    .argv()
                    .windows(2)
                    .any(|args| args == ["container", "create"])
            })
            .expect("container create");
        let arguments = container.cli().argv();
        let image_index = arguments
            .iter()
            .position(|argument| argument == "registry.example.invalid/app:1")
            .expect("image");
        assert!(arguments[..image_index].contains(&"--entrypoint".to_owned()));
        assert_eq!(&arguments[image_index + 1..], ["serve", "--foreground"]);
        for pair in [
            ["--entrypoint", r#"["/init","--safe"]"#],
            ["--user", "1000:1000"],
            ["--workdir", "/srv/application"],
            ["--hostname", "application.example"],
            ["--label", "org.example.role=web"],
            ["--label", "org.example.tier=frontend"],
            ["--env", "MODE=production"],
            ["--env", "LOG_LEVEL=info"],
            ["--restart", "no"],
            ["--volume", "application-data:/var/lib/application:rw,copy"],
        ] {
            assert!(arguments.windows(2).any(|actual| actual == pair), "missing {pair:?}");
        }
        assert_eq!(
            container.libpod().body(),
            &RenderedHttpBody::Json(serde_json::json!({
                "image": "registry.example.invalid/app:1",
                "Networks": {},
                "command": ["serve", "--foreground"],
                "entrypoint": ["/init", "--safe"],
                "user": "1000:1000",
                "work_dir": "/srv/application",
                "hostname": "application.example",
                "labels": {"org.example.role": "web", "org.example.tier": "frontend"},
                "env": {"MODE": "production", "LOG_LEVEL": "info"},
                "restart_policy": "no",
                "volumes": [{"Name": "application-data", "Dest": "/var/lib/application", "Options": ["rw", "copy"]}],
            }))
        );
        let artifact = serde_json::to_string(&deployment_v1::deployment(&rendering)).expect("artifact JSON");
        let script = rendering.shell_script();
        for public_value in [
            "production",
            "application.example",
            "application-data:/var/lib/application:rw,copy",
        ] {
            assert!(artifact.contains(public_value));
            assert!(script.contains(public_value));
        }
    }
}

#[test]
fn every_restart_policy_has_exact_cli_and_libpod_forms_for_every_reviewed_target() {
    let policies = [
        (RestartPolicy::No, "no"),
        (RestartPolicy::OnFailure, "on-failure"),
        (RestartPolicy::Always, "always"),
        (RestartPolicy::UnlessStopped, "unless-stopped"),
    ];
    for version in ["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"] {
        for (policy, expected) in policies {
            let rendering = render_deployment(&core_settings_plan_with_restart(version, policy))
                .rendering()
                .cloned()
                .expect("exact rendering");
            let container = rendering
                .operations()
                .iter()
                .find(|operation| {
                    operation
                        .cli()
                        .argv()
                        .windows(2)
                        .any(|arguments| arguments == ["container", "create"])
                })
                .expect("container create");
            assert!(
                container
                    .cli()
                    .argv()
                    .windows(2)
                    .any(|arguments| arguments == ["--restart", expected]),
                "missing {expected:?} CLI restart policy for Podman {version}"
            );
            assert!(matches!(container.libpod().body(), RenderedHttpBody::Json(_)));
            if let RenderedHttpBody::Json(body) = container.libpod().body() {
                assert_eq!(
                    body["restart_policy"],
                    serde_json::Value::String(expected.to_owned()),
                    "missing {expected:?} Libpod restart policy for Podman {version}"
                );
            }
        }
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
                    "Networks": {"network": {}}
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
    container_intent
        .add_network(NetworkAttachment::new(network).expect("network attachment"))
        .expect("network");
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
            "Networks": {"standalone-network": {}}
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

#[allow(clippy::too_many_lines)] // One byte-exact end-to-end matrix keeps every B2 wire value together.
#[test]
fn populated_networking_intent_is_rendered_exactly_with_cli_and_libpod_forms() {
    let network = id(ResourceKind::Network, "application-network");
    let image = id(ResourceKind::Image, "registry.example.invalid/application:1");
    let container = id(ResourceKind::Container, "application");
    let mut network_intent = NetworkIntent::new(network.clone()).expect("network");
    let mut subnet = NetworkSubnet::new(NetworkCidr::new("192.0.2.0/24").expect("subnet"));
    subnet
        .set_gateway("192.0.2.1".parse().expect("gateway"))
        .expect("gateway");
    subnet
        .set_range(
            "192.0.2.10".parse().expect("range start"),
            "192.0.2.20".parse().expect("range end"),
        )
        .expect("range");
    network_intent.add_subnet(subnet).expect("subnet");
    let mut route = NetworkRoute::new(
        NetworkCidr::new("198.51.100.0/24").expect("destination"),
        Some("192.0.2.1".parse().expect("gateway")),
        RouteType::Unicast,
    )
    .expect("route");
    route.set_metric(42).expect("metric");
    network_intent.add_route(route).expect("route");
    let mut attachment = NetworkAttachment::new(network.clone()).expect("attachment");
    attachment.add_alias("application").expect("alias");
    attachment
        .set_static_ipv4("192.0.2.50".parse().expect("static IPv4"))
        .expect("static IPv4");
    attachment
        .set_static_ipv6("2001:db8::50".parse().expect("static IPv6"))
        .expect("static IPv6");
    attachment
        .set_static_mac(podman_lens::StaticMacAddress::new("02:42:ac:11:00:02").expect("mac"))
        .expect("mac");
    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
    container_intent.add_network(attachment).expect("network");
    container_intent
        .add_port(PortMapping::new(None, 8080, 80, PortProtocol::Tcp).expect("port"))
        .expect("port");
    container_intent
        .add_port(
            PortMapping::new(
                Some("192.0.2.10".parse().expect("IPv4 bind")),
                5353,
                5353,
                PortProtocol::Udp,
            )
            .expect("port"),
        )
        .expect("port");
    container_intent
        .add_port(
            PortMapping::new(
                Some("2001:db8::10".parse().expect("IPv6 bind")),
                9899,
                9899,
                PortProtocol::Sctp,
            )
            .expect("port"),
        )
        .expect("port");
    container_intent
        .add_host_alias(HostAlias::new("192.0.2.53".parse().expect("host"), "database.test").expect("alias"))
        .expect("host alias");
    container_intent
        .add_host_alias(HostAlias::new("2001:db8::53".parse().expect("host"), "database-v6.test").expect("alias"))
        .expect("host alias");
    let dns: &mut DnsConfiguration = container_intent.dns_mut();
    dns.add_server("192.0.2.53".parse().expect("dns")).expect("dns");
    dns.add_server("2001:db8::53".parse().expect("dns")).expect("dns");
    dns.add_search("example.test").expect("search");
    dns.add_option("ndots:2").expect("option");
    container_intent
        .set_network_order(vec![network.clone()])
        .expect("network order");
    let mut selected_target = target("6.1.0", "6.1.0");
    selected_target.set_execution_context(TargetExecutionContext::Rootful);
    let mut intent = DeploymentIntent::new(selected_target);
    intent.add_resource(DeploymentResource::Network(network_intent));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/team/application:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container_intent));
    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    let outcome = render_deployment(&plan);
    let rendering = outcome.rendering().expect("exact rendering");
    let network = rendering
        .operations()
        .iter()
        .find(|operation| {
            operation
                .cli()
                .argv()
                .windows(2)
                .any(|args| args == ["network", "create"])
        })
        .expect("network create");
    assert_eq!(
        network.cli().argv(),
        [
            "network",
            "create",
            "--subnet",
            "192.0.2.0/24",
            "--gateway",
            "192.0.2.1",
            "--ip-range",
            "192.0.2.10-192.0.2.20",
            "--route",
            "198.51.100.0/24,192.0.2.1,42",
            "application-network"
        ]
    );
    assert_eq!(
        network.libpod().body(),
        &RenderedHttpBody::Json(serde_json::json!({
            "name": "application-network",
            "subnets": [{"subnet": "192.0.2.0/24", "gateway": "192.0.2.1", "lease_range": {"start_ip": "192.0.2.10", "end_ip": "192.0.2.20"}}],
            "routes": [{"destination": "198.51.100.0/24", "gateway": "192.0.2.1", "metric": 42, "route_type": "unicast"}],
        }))
    );
    let container = rendering
        .operations()
        .iter()
        .find(|operation| {
            operation
                .cli()
                .argv()
                .windows(2)
                .any(|args| args == ["container", "create"])
        })
        .expect("container create");
    assert!(container.cli().argv().windows(2).any(|args| {
        args == [
            "--network",
            "application-network:alias=application,ip=192.0.2.50,ip6=2001:db8::50,mac=02:42:ac:11:00:02",
        ]
    }));
    assert!(
        container
            .cli()
            .argv()
            .windows(2)
            .any(|args| args == ["--publish", "[2001:db8::10]:9899:9899/sctp"])
    );
    assert_eq!(
        container.libpod().body(),
        &RenderedHttpBody::Json(serde_json::json!({
            "image": "registry.example.invalid/team/application:1",
            "Networks": {"application-network": {"aliases": ["application"], "static_ips": ["192.0.2.50", "2001:db8::50"], "static_mac": "02:42:ac:11:00:02"}},
            "portmappings": [
                {"host_ip": "", "host_port": 8080, "container_port": 80, "range": 1, "protocol": "tcp"},
                {"host_ip": "192.0.2.10", "host_port": 5353, "container_port": 5353, "range": 1, "protocol": "udp"},
                {"host_ip": "2001:db8::10", "host_port": 9899, "container_port": 9899, "range": 1, "protocol": "sctp"},
            ],
            "dns_server": ["192.0.2.53", "2001:db8::53"], "dns_search": ["example.test"], "dns_option": ["ndots:2"],
            "hostadd": ["database.test:192.0.2.53", "database-v6.test:2001:db8::53"],
            "networkOrder": ["application-network"],
        }))
    );
}

#[allow(clippy::too_many_lines)] // The target matrix is intentionally visible in one test.
#[test]
fn basic_unpodded_networking_is_exact_for_every_reviewed_release() {
    for version in ["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"] {
        let network = id(ResourceKind::Network, "network");
        let image = id(ResourceKind::Image, "registry.example.invalid/application:1");
        let container = id(ResourceKind::Container, "application");
        let mut attachment = NetworkAttachment::new(network.clone()).expect("attachment");
        attachment.add_alias("application").expect("alias");
        attachment
            .set_static_ipv4("192.0.2.50".parse().expect("IPv4"))
            .expect("IPv4");
        attachment
            .set_static_ipv6("2001:db8::50".parse().expect("IPv6"))
            .expect("IPv6");
        attachment
            .set_static_mac(podman_lens::StaticMacAddress::new("02:42:ac:11:00:02").expect("MAC"))
            .expect("MAC");
        let mut network_intent = NetworkIntent::new(network.clone()).expect("network");
        let mut subnet = NetworkSubnet::new(NetworkCidr::new("192.0.2.0/24").expect("subnet"));
        subnet
            .set_gateway("192.0.2.1".parse().expect("gateway"))
            .expect("gateway");
        subnet
            .set_range(
                "192.0.2.10".parse().expect("range start"),
                "192.0.2.20".parse().expect("range end"),
            )
            .expect("range");
        network_intent.add_subnet(subnet).expect("subnet");
        let mut route = NetworkRoute::new(
            NetworkCidr::new("198.51.100.0/24").expect("destination"),
            Some("192.0.2.1".parse().expect("gateway")),
            RouteType::Unicast,
        )
        .expect("route");
        route.set_metric(42).expect("metric");
        network_intent.add_route(route).expect("route");
        let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
        container_intent.add_network(attachment).expect("network");
        container_intent
            .add_port(PortMapping::new(None, 8080, 80, PortProtocol::Tcp).expect("port"))
            .expect("port");
        container_intent
            .add_port(
                PortMapping::new(
                    Some("192.0.2.10".parse().expect("IPv4 bind")),
                    5353,
                    5353,
                    PortProtocol::Udp,
                )
                .expect("port"),
            )
            .expect("port");
        container_intent
            .add_port(
                PortMapping::new(
                    Some("2001:db8::10".parse().expect("IPv6 bind")),
                    9899,
                    9899,
                    PortProtocol::Sctp,
                )
                .expect("port"),
            )
            .expect("port");
        container_intent
            .add_host_alias(HostAlias::new("2001:db8::53".parse().expect("host"), "database.test").expect("host"))
            .expect("host");
        let dns = container_intent.dns_mut();
        dns.add_server("2001:db8::53".parse().expect("DNS")).expect("DNS");
        dns.add_search("example.test").expect("search");
        dns.add_option("ndots:2").expect("option");
        let mut selected_target = target(version, version);
        selected_target.set_execution_context(TargetExecutionContext::Rootful);
        let mut intent = DeploymentIntent::new(selected_target);
        intent.add_resource(DeploymentResource::Network(network_intent));
        intent.add_resource(DeploymentResource::Image(
            ImageIntent::new(image, "registry.example.invalid/team/application:1").expect("image"),
        ));
        intent.add_resource(DeploymentResource::Container(container_intent));
        let plan = plan_deployment(&intent).plan().cloned().expect("plan");
        let rendering = render_deployment(&plan).rendering().cloned().expect("exact rendering");
        let container = rendering
            .operations()
            .iter()
            .find(|operation| {
                operation
                    .cli()
                    .argv()
                    .windows(2)
                    .any(|args| args == ["container", "create"])
            })
            .expect("container create");
        assert!(container.cli().argv().windows(2).any(|args| {
            args == [
                "--network",
                "network:alias=application,ip=192.0.2.50,ip6=2001:db8::50,mac=02:42:ac:11:00:02",
            ]
        }));
        for args in [
            ["--publish", "8080:80/tcp"],
            ["--publish", "192.0.2.10:5353:5353/udp"],
            ["--publish", "[2001:db8::10]:9899:9899/sctp"],
            ["--dns", "2001:db8::53"],
            ["--dns-search", "example.test"],
            ["--dns-option", "ndots:2"],
            ["--add-host", "database.test:2001:db8::53"],
        ] {
            assert!(container.cli().argv().windows(2).any(|observed| observed == args));
        }
        assert!(
            matches!(container.libpod().body(), RenderedHttpBody::Json(body) if body["Networks"] == serde_json::json!({"network": {"aliases": ["application"], "static_ips": ["192.0.2.50", "2001:db8::50"], "static_mac": "02:42:ac:11:00:02"}}) && body["portmappings"] == serde_json::json!([
            {"host_ip": "", "host_port": 8080, "container_port": 80, "range": 1, "protocol": "tcp"},
            {"host_ip": "192.0.2.10", "host_port": 5353, "container_port": 5353, "range": 1, "protocol": "udp"},
            {"host_ip": "2001:db8::10", "host_port": 9899, "container_port": 9899, "range": 1, "protocol": "sctp"},
        ]) && body["dns_server"] == serde_json::json!(["2001:db8::53"]) && body["dns_search"] == serde_json::json!(["example.test"]) && body["dns_option"] == serde_json::json!(["ndots:2"]) && body["hostadd"] == serde_json::json!(["database.test:2001:db8::53"]))
        );
        let network = rendering.operations().first().expect("network create");
        assert!(
            network
                .cli()
                .argv()
                .windows(2)
                .any(|args| args == ["--subnet", "192.0.2.0/24"])
        );
        assert!(
            network
                .cli()
                .argv()
                .windows(2)
                .any(|args| args == ["--gateway", "192.0.2.1"])
        );
        assert!(
            network
                .cli()
                .argv()
                .windows(2)
                .any(|args| args == ["--ip-range", "192.0.2.10-192.0.2.20"])
        );
        assert!(
            network
                .cli()
                .argv()
                .windows(2)
                .any(|args| args == ["--route", "198.51.100.0/24,192.0.2.1,42"])
        );
        assert!(
            matches!(network.libpod().body(), RenderedHttpBody::Json(body) if body["subnets"] == serde_json::json!([{"subnet": "192.0.2.0/24", "gateway": "192.0.2.1", "lease_range": {"start_ip": "192.0.2.10", "end_ip": "192.0.2.20"}}]) && body["routes"][0]["destination"] == "198.51.100.0/24" && body["routes"][0]["gateway"] == "192.0.2.1" && body["routes"][0]["metric"] == 42 && if matches!(version, "6.0.0" | "6.1.0") { body["routes"][0]["route_type"] == "unicast" } else { body["routes"][0].get("route_type").is_none() })
        );
    }
}

#[allow(clippy::too_many_lines)] // Pod namespace ownership and its wire output are one invariant.
#[test]
fn pod_owned_networking_renders_on_the_infra_container() {
    for version in ["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"] {
        let network = id(ResourceKind::Network, "network");
        let image = id(ResourceKind::Image, "registry.example.invalid/application:1");
        let pod = id(ResourceKind::Pod, "application-pod");
        let member = id(ResourceKind::Container, "application");
        let mut attachment = NetworkAttachment::new(network.clone()).expect("attachment");
        attachment.add_alias("application").expect("alias");
        attachment
            .set_static_ipv4("192.0.2.50".parse().expect("IPv4"))
            .expect("IPv4");
        attachment
            .set_static_ipv6("2001:db8::50".parse().expect("IPv6"))
            .expect("IPv6");
        attachment
            .set_static_mac(podman_lens::StaticMacAddress::new("02:42:ac:11:00:02").expect("MAC"))
            .expect("MAC");
        let mut pod_intent = PodIntent::new(pod.clone()).expect("pod");
        pod_intent.add_network(attachment).expect("network");
        pod_intent
            .add_port(
                PortMapping::new(
                    Some("2001:db8::10".parse().expect("bind")),
                    8443,
                    443,
                    PortProtocol::Sctp,
                )
                .expect("port"),
            )
            .expect("port");
        pod_intent
            .add_host_alias(HostAlias::new("2001:db8::53".parse().expect("host"), "database.test").expect("host"))
            .expect("host");
        let dns = pod_intent.dns_mut();
        dns.add_server("2001:db8::53".parse().expect("DNS")).expect("DNS");
        dns.add_search("example.test").expect("search");
        dns.add_option("ndots:2").expect("option");
        pod_intent.add_member(member.clone()).expect("member");
        let mut member_intent = ContainerIntent::new(member, image.clone()).expect("container");
        member_intent.set_pod(pod.clone()).expect("pod");
        let mut selected_target = target(version, version);
        selected_target.set_execution_context(TargetExecutionContext::Rootful);
        let mut intent = DeploymentIntent::new(selected_target);
        intent.add_resource(DeploymentResource::Network(
            NetworkIntent::new(network).expect("network"),
        ));
        intent.add_resource(DeploymentResource::Image(
            ImageIntent::new(image, "registry.example.invalid/team/application:1").expect("image"),
        ));
        intent.add_resource(DeploymentResource::Pod(pod_intent));
        intent.add_resource(DeploymentResource::Container(member_intent));
        let plan = plan_deployment(&intent).plan().cloned().expect("plan");
        let rendering = render_deployment(&plan).rendering().cloned().expect("exact rendering");
        let pod = rendering
            .operations()
            .iter()
            .find(|operation| operation.cli().argv().windows(2).any(|args| args == ["pod", "create"]))
            .expect("pod create");
        assert!(pod.cli().argv().windows(2).any(|args| {
            args == [
                "--network",
                "network:alias=application,ip=192.0.2.50,ip6=2001:db8::50,mac=02:42:ac:11:00:02",
            ]
        }));
        for args in [
            ["--publish", "[2001:db8::10]:8443:443/sctp"],
            ["--dns", "2001:db8::53"],
            ["--dns-search", "example.test"],
            ["--dns-option", "ndots:2"],
            ["--add-host", "database.test:2001:db8::53"],
        ] {
            assert!(pod.cli().argv().windows(2).any(|observed| observed == args));
        }
        assert!(
            matches!(pod.libpod().body(), RenderedHttpBody::Json(body) if body["Networks"] == serde_json::json!({"network": {"aliases": ["application"], "static_ips": ["192.0.2.50", "2001:db8::50"], "static_mac": "02:42:ac:11:00:02"}}) && body["portmappings"] == serde_json::json!([{"host_ip": "2001:db8::10", "host_port": 8443, "container_port": 443, "range": 1, "protocol": "sctp"}]) && body["dns_server"] == serde_json::json!(["2001:db8::53"]) && body["dns_search"] == serde_json::json!(["example.test"]) && body["dns_option"] == serde_json::json!(["ndots:2"]) && body["hostadd"] == serde_json::json!(["database.test:2001:db8::53"]))
        );
    }
}

#[test]
fn every_non_unicast_route_type_is_exact_from_podman_six() {
    for version in ["6.0.0", "6.1.0"] {
        for (route_type, spelling) in [
            (RouteType::Blackhole, "blackhole"),
            (RouteType::Unreachable, "unreachable"),
            (RouteType::Prohibit, "prohibit"),
        ] {
            let network = id(ResourceKind::Network, "network");
            let mut network_intent = NetworkIntent::new(network.clone()).expect("network");
            network_intent
                .add_route(
                    NetworkRoute::new(
                        NetworkCidr::new("198.51.100.0/24").expect("destination"),
                        None,
                        route_type,
                    )
                    .expect("route"),
                )
                .expect("route");
            let mut intent = DeploymentIntent::new(target(version, version));
            intent.add_resource(DeploymentResource::Network(network_intent));
            let plan = plan_deployment(&intent).plan().cloned().expect("plan");
            let rendering = render_deployment(&plan).rendering().cloned().expect("exact rendering");
            let network = rendering.operations().first().expect("network create");
            assert!(
                network
                    .cli()
                    .argv()
                    .windows(2)
                    .any(|args| args == ["--route", &format!("198.51.100.0/24,{spelling}")])
            );
            assert!(
                matches!(network.libpod().body(), RenderedHttpBody::Json(body) if body["routes"][0]["route_type"] == spelling)
            );
        }
    }
}

#[test]
fn networking_target_boundaries_block_inexact_rendering() {
    for version in ["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6"] {
        let network = id(ResourceKind::Network, "network");
        let image = id(ResourceKind::Image, "registry.example.invalid/image:1");
        let container = id(ResourceKind::Container, "container");
        let mut network_intent = NetworkIntent::new(network.clone()).expect("network");
        network_intent
            .add_route(
                NetworkRoute::new(
                    NetworkCidr::new("198.51.100.0/24").expect("destination"),
                    None,
                    RouteType::Unreachable,
                )
                .expect("route"),
            )
            .expect("route");
        let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
        container_intent
            .add_network(NetworkAttachment::new(network.clone()).expect("attachment"))
            .expect("network");
        container_intent
            .set_network_order(vec![network.clone()])
            .expect("order");
        let mut intent = DeploymentIntent::new(target(version, version));
        intent.add_resource(DeploymentResource::Network(network_intent));
        intent.add_resource(DeploymentResource::Image(
            ImageIntent::new(image, "registry.example.invalid/team/image:1").expect("image"),
        ));
        intent.add_resource(DeploymentResource::Container(container_intent));
        let plan = plan_deployment(&intent).plan().cloned().expect("plan");
        let outcome = render_deployment(&plan);
        assert!(!outcome.is_success());
        assert_eq!(
            outcome
                .findings()
                .iter()
                .filter_map(podman_lens::RenderingFinding::field)
                .collect::<Vec<_>>(),
            ["network_order", "routes.route_type"]
        );
    }
}

#[test]
fn renderer_rejects_unmodelled_secret_attachments_without_partial_output() {
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
        vec![("PLN0046", "container", Some("secrets"))]
    );
}

#[test]
fn renderer_rejects_sensitive_environment_values_without_leaking_names_or_values() {
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
            .add_environment(EnvironmentAssignment::new(
                EnvironmentName::new("SECOND_PASSWORD").expect("environment name"),
                DeploymentEnvironmentValue::SensitiveInline(
                    SensitiveInlineEnvironmentValue::new("another-sensitive-value").expect("sensitive value"),
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
        vec![Some("environment.sensitive_inline"),]
    );
    let debug = format!("{outcome:?}");
    assert!(!debug.contains(sensitive_sentinel));
    assert!(!debug.contains("PASSWORD"));
}

#[test]
fn renderer_rejects_external_environment_values_without_leaking_names_or_references() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let container = id(ResourceKind::Container, "container");
    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container");
    container_intent
        .settings_mut()
        .add_environment(EnvironmentAssignment::new(
            EnvironmentName::new("TOKEN_FILE").expect("environment name"),
            DeploymentEnvironmentValue::External(
                SensitiveInputReference::new("vault/external-token").expect("external reference"),
            ),
        ))
        .expect("environment");
    let mut intent = DeploymentIntent::new(target("6.1.0", "6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/app:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container_intent));
    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    let outcome = render_deployment(&plan);
    assert!(!outcome.is_success());
    assert_eq!(outcome.findings()[0].field(), Some("environment.external"));
    let debug = format!("{outcome:?}");
    assert!(!debug.contains("TOKEN_FILE"));
    assert!(!debug.contains("vault/external-token"));
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
#[allow(clippy::too_many_lines)] // This is the integration-level evidence contract audit.
fn committed_renderer_evidence_covers_every_operation_and_reviewed_line() -> Result<(), Box<dyn std::error::Error>> {
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("../catalogue/v1/podman-deployment-rendering.json"))?;
    assert_eq!(evidence["schema_version"], 7);
    let runtime_claims = evidence["runtime_field_claims"]
        .as_array()
        .expect("runtime field claims");
    assert_eq!(runtime_claims.len(), 32);
    assert!(runtime_claims.iter().all(|claim| {
        claim["field"].is_string()
            && claim["cli"]["flag"].as_str().is_some_and(|flag| flag.starts_with("--"))
            && claim["cli"]["value_shape"].is_string()
            && claim["libpod"]["json_member"].is_string()
            && claim["libpod"]["value_shape"].is_string()
    }));
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
        let fields = line["field_evidence"].as_array().expect("field evidence");
        let runtime = &line["runtime_evidence"];
        assert_eq!(operations.len(), 8);
        assert_eq!(fields.len(), 41);
        let exact_runtime = runtime["exact_fields"].as_array().expect("exact runtime fields");
        let target_gated_runtime = runtime["target_gated_fields"]
            .as_array()
            .expect("target-gated runtime fields");
        assert_eq!(exact_runtime.len() + target_gated_runtime.len(), runtime_claims.len());
        assert_eq!(runtime["cli_flag_source"]["path"], "cmd/podman/common/create.go");
        assert_eq!(runtime["cli_transform_source"]["path"], "pkg/specgenutil/specgen.go");
        assert_eq!(
            runtime["command_route_source"]["path"],
            "cmd/podman/containers/create.go"
        );
        assert_eq!(runtime["route_source"]["path"], "pkg/api/server/register_containers.go");
        assert_eq!(
            runtime["handler_source"]["path"],
            "pkg/api/handlers/libpod/containers_create.go"
        );
        for source in [
            &runtime["cli_flag_source"],
            &runtime["cli_transform_source"],
            &runtime["command_route_source"],
            &runtime["route_source"],
            &runtime["handler_source"],
        ] {
            assert_eq!(source["repository"], "containers-podman");
            assert_eq!(source["revision"], revision);
            assert!(source["module"].is_null());
        }
        assert_eq!(runtime["model_sources"].as_array().map(Vec::len), Some(3));
        assert_eq!(
            runtime["model_sources"]
                .as_array()
                .expect("runtime model sources")
                .iter()
                .map(|source| source["path"].as_str().expect("model path"))
                .collect::<Vec<_>>(),
            vec![
                "pkg/specgen/specgen.go",
                "pkg/specgen/namespaces.go",
                "libpod/define/healthchecks.go",
            ]
        );
        for operation in operations {
            for source in ["cli_source", "libpod_endpoint_source"] {
                assert_eq!(operation[source]["repository"], "containers-podman");
                assert_eq!(operation[source]["revision"], revision);
                assert!(operation[source]["module"].is_null());
            }
            if operation["body_source"].is_object() {
                assert_eq!(operation["body_source"]["repository"], "containers-podman");
                assert_eq!(operation["body_source"]["revision"], revision);
            } else {
                assert!(operation["body_source"].is_null());
            }
        }
        for field in fields {
            assert!(matches!(field["availability"].as_str(), Some("exact" | "unsupported")));
            for source in ["cli_source", "handler_source"] {
                assert_eq!(field[source]["repository"], "containers-podman");
                assert_eq!(field[source]["revision"], revision);
                assert!(field[source]["module"].is_null());
            }
            assert!(field["model_sources"].as_array().is_some_and(|sources| {
                sources.iter().all(|source| {
                    (source["repository"] == "containers-podman"
                        && source["revision"] == revision
                        && source["module"].is_null())
                        || (source["repository"] == line["common_module"]["repository"]
                            && source["revision"] == line["common_module"]["revision"]
                            && source["module"] == line["common_module"])
                })
            }));
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
