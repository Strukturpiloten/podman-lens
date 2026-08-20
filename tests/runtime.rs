//! M6-B3a semantic planning boundaries; rendering remains intentionally blocked.
#![allow(clippy::expect_used)] // Concise fixtures isolate the behavior under test.

use podman_lens::{
    CgroupCapabilityEvidence, CgroupController, CgroupVersion, ConfiguredHealthCheck, ContainerHostname,
    ContainerIntent, DeploymentIntent, DeploymentResource, DeploymentResourceId, ExternalPrecondition, HealthCheck,
    HealthCommand, HealthDuration, HealthInterval, HealthRetries, HealthStartPeriod, HealthTimeout, ImageIntent,
    IpcNamespaceMode, Label, LabelKey, LinuxCapability, LogDriver, LogSize, NamespaceMode, ObservedApiVersion,
    ObservedPodmanVersion, PublicHealthArgumentArray, PublicHealthCommand, PublicLabelValue, ResourceKind, Rlimit,
    RlimitKind, RlimitValue, SensitiveInlineHealthArgumentArray, SensitiveInlineHealthCommand, SensitiveInputReference,
    StartupHealthCheck, StartupHealthRetries, StartupHealthSuccesses, TargetExecutionContext, TargetProfile,
    plan_deployment, render_deployment,
};

fn target_for(version: &str) -> TargetProfile {
    TargetProfile::new(
        ObservedPodmanVersion::parse(version).expect("reviewed engine"),
        ObservedApiVersion::parse(version).expect("reviewed API"),
    )
    .expect("profile")
}

fn target() -> TargetProfile {
    let mut target = target_for("6.1.0");
    target.set_cgroup_capabilities(CgroupCapabilityEvidence::new(
        CgroupVersion::V2,
        [CgroupController::Cpu, CgroupController::Memory, CgroupController::Pids],
    ));
    target
}

fn id(kind: ResourceKind, name: &str) -> DeploymentResourceId {
    DeploymentResourceId::new(kind, name).expect("identity")
}

fn complete_intent() -> (DeploymentIntent, DeploymentResourceId) {
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let container_id = id(ResourceKind::Container, "web");
    let mut container = ContainerIntent::new(container_id.clone(), image.clone()).expect("container");
    container
        .runtime_mut()
        .set_health(HealthCheck::Command(ConfiguredHealthCheck::new(HealthCommand::Shell(
            PublicHealthCommand::new("curl -f http://127.0.0.1/health").expect("health"),
        ))))
        .expect("health");
    container
        .runtime_mut()
        .set_startup_health(StartupHealthCheck::new(HealthCommand::Exec(
            PublicHealthArgumentArray::new(["/usr/local/bin/wait-ready"]).expect("exec"),
        )))
        .expect("startup health");
    let runtime = container.runtime_mut();
    runtime.logging_mut().set_driver(LogDriver::Journald).expect("driver");
    runtime
        .logging_mut()
        .add_journald_label(Label::new(
            LabelKey::new("org.example.service").expect("key"),
            PublicLabelValue::new("web").expect("value"),
        ))
        .expect("journald label");
    runtime.security_mut().set_privileged(false).expect("privileged");
    runtime.security_mut().set_no_new_privileges(true).expect("nnp");
    runtime.security_mut().set_read_only_filesystem(true).expect("readonly");
    runtime
        .security_mut()
        .add_capability(LinuxCapability::new("NET_BIND_SERVICE").expect("capability"))
        .expect("capability");
    runtime
        .security_mut()
        .drop_capability(LinuxCapability::new("SYS_ADMIN").expect("capability"))
        .expect("capability");
    runtime.security_mut().set_read_write_tmpfs(true).expect("tmpfs");
    runtime.resources_mut().set_cpu_shares(500).expect("cpu");
    runtime.resources_mut().set_cpu_period(100_000).expect("period");
    runtime.resources_mut().set_cpu_quota(50_000).expect("quota");
    runtime
        .resources_mut()
        .set_memory_bytes(512 * 1024 * 1024)
        .expect("memory");
    runtime.resources_mut().set_pids(256).expect("pids");
    runtime
        .resources_mut()
        .add_rlimit(Rlimit::new(RlimitKind::NoFile, RlimitValue::finite(1024), RlimitValue::Unlimited).expect("rlimit"))
        .expect("rlimit");
    let mut intent = DeploymentIntent::new(target());
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(image, "registry.example.invalid/web:1").expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container));
    (intent, container_id)
}

#[test]
fn bounded_runtime_intent_plans_but_rendering_remains_blocked() {
    let (intent, container) = complete_intent();
    let plan = plan_deployment(&intent).plan().cloned().expect("semantic plan");
    let rendering = render_deployment(&plan);
    assert!(!rendering.is_success());
    assert!(
        rendering
            .findings()
            .iter()
            .any(|finding| { finding.subject() == Some(&container) && finding.field() == Some("runtime") })
    );
}

#[test]
fn sensitive_health_commands_never_leak_from_runtime_debug() {
    let mut container = ContainerIntent::new(
        id(ResourceKind::Container, "web"),
        id(ResourceKind::Image, "registry.example.invalid/web:1"),
    )
    .expect("container");
    container
        .runtime_mut()
        .set_health(HealthCheck::Command(ConfiguredHealthCheck::new(
            HealthCommand::SensitiveInlineShell(
                SensitiveInlineHealthCommand::new("DISTINCTIVE_HEALTH_SECRET").expect("health"),
            ),
        )))
        .expect("health");
    container
        .runtime_mut()
        .set_startup_health(StartupHealthCheck::new(HealthCommand::ExternalExec(
            SensitiveInputReference::new("vault/runtime-health").expect("reference"),
        )))
        .expect("startup health");
    let debug = format!("{container:?}");
    assert!(!debug.contains("DISTINCTIVE_HEALTH_SECRET"));
    assert!(!debug.contains("vault/runtime-health"));
}

#[test]
fn bounded_runtime_values_reject_invalid_inputs_and_preserve_explicit_false() {
    assert!(PublicHealthArgumentArray::new(Vec::<String>::new()).is_err());
    assert!(PublicHealthArgumentArray::new(["", "allowed-later-argument"]).is_err());
    assert!(PublicHealthArgumentArray::new(["command", ""]).is_ok());
    assert!(HealthDuration::new(0).is_err());
    assert!(HealthTimeout::new(999_999_999).is_err());
    assert!(HealthRetries::new(u32::MAX).is_err());
    assert!(HealthRetries::new(0).is_err());
    assert!(StartupHealthSuccesses::new(0).is_ok());
    assert!(StartupHealthRetries::new(0).is_ok());
    assert!(LinuxCapability::new("CAP_CHOWN").is_err());
    assert!(LinuxCapability::new("NOT_A_CAPABILITY").is_err());
    assert!(Rlimit::new(RlimitKind::NoFile, RlimitValue::finite(2), RlimitValue::finite(1)).is_err());
    assert!(Rlimit::new(RlimitKind::NoFile, RlimitValue::Unlimited, RlimitValue::finite(1)).is_err());
    assert!(Rlimit::new(RlimitKind::NoFile, RlimitValue::finite(0), RlimitValue::Unlimited).is_ok());
    assert!(
        podman_lens::ContainerResourceControls::default()
            .set_cpu_period(999)
            .is_err()
    );
    assert!(
        podman_lens::ContainerResourceControls::default()
            .set_cpu_period(1_000_001)
            .is_err()
    );
    assert!(
        podman_lens::ContainerResourceControls::default()
            .set_cpu_quota(999)
            .is_err()
    );
    assert!(
        podman_lens::ContainerResourceControls::default()
            .set_cpu_quota(0)
            .is_err()
    );
    assert!(
        podman_lens::ContainerResourceControls::default()
            .set_cpu_quota(-1)
            .is_err()
    );
    assert!(
        podman_lens::ContainerResourceControls::default()
            .set_cpu_quota(-2)
            .is_err()
    );

    let mut health = ConfiguredHealthCheck::new(HealthCommand::Exec(
        PublicHealthArgumentArray::new(["/usr/bin/true"]).expect("command"),
    ));
    health
        .set_interval(HealthInterval::Every(HealthDuration::new(1).expect("interval")))
        .expect("first interval");
    assert!(health.set_interval(HealthInterval::Disabled).is_err());
    assert_eq!(
        health.interval(),
        Some(HealthInterval::Every(HealthDuration::new(1).expect("interval")))
    );
    health
        .set_timeout(HealthTimeout::new(1_000_000_000).expect("timeout"))
        .expect("timeout");
    health
        .set_start_period(HealthStartPeriod::new(0).expect("period"))
        .expect("period");
    let mut startup = StartupHealthCheck::new(HealthCommand::SensitiveInlineExec(
        SensitiveInlineHealthArgumentArray::new(["DISTINCTIVE_EXEC_SECRET"]).expect("secret command"),
    ));
    startup
        .set_retries(StartupHealthRetries::new(0).expect("retries"))
        .expect("retries");
    startup
        .set_successes(StartupHealthSuccesses::new(1).expect("successes"))
        .expect("successes");
    assert!(!format!("{startup:?}").contains("DISTINCTIVE_EXEC_SECRET"));

    let mut security = podman_lens::SecuritySettings::default();
    security.set_privileged(false).expect("false privileged");
    security.set_no_new_privileges(false).expect("false nnp");
    security.set_read_only_filesystem(false).expect("false readonly");
    security.set_read_write_tmpfs(false).expect("false tmpfs");
    assert!(security.set_privileged(true).is_err());
    assert_eq!(security.privileged(), Some(false));
    assert_eq!(security.no_new_privileges(), Some(false));
    assert_eq!(security.read_only_filesystem(), Some(false));
    assert_eq!(security.read_write_tmpfs(), Some(false));
}

#[test]
fn health_setters_reject_duplicates_or_conflicts_without_replacing_declared_values() {
    let command = HealthCommand::Exec(PublicHealthArgumentArray::new(["/usr/bin/true"]).expect("command"));
    let mut health = ConfiguredHealthCheck::new(command.clone());
    let interval = HealthInterval::Every(HealthDuration::new(1).expect("interval"));
    health.set_interval(interval).expect("interval");
    assert!(health.set_interval(HealthInterval::Disabled).is_err());
    assert_eq!(health.interval(), Some(interval));
    let timeout = HealthTimeout::new(1_000_000_000).expect("timeout");
    health.set_timeout(timeout).expect("timeout");
    assert!(
        health
            .set_timeout(HealthTimeout::new(2_000_000_000).expect("timeout"))
            .is_err()
    );
    assert_eq!(health.timeout(), Some(timeout));
    let retries = HealthRetries::new(1).expect("retries");
    health.set_retries(retries).expect("retries");
    assert!(health.set_retries(HealthRetries::new(2).expect("retries")).is_err());
    assert_eq!(health.retries(), Some(retries));
    let start_period = HealthStartPeriod::new(0).expect("start period");
    health.set_start_period(start_period).expect("start period");
    assert!(
        health
            .set_start_period(HealthStartPeriod::new(1).expect("start period"))
            .is_err()
    );
    assert_eq!(health.start_period(), Some(start_period));

    let mut startup = StartupHealthCheck::new(command.clone());
    startup.set_interval(HealthInterval::Disabled).expect("interval");
    assert!(
        startup
            .set_interval(HealthInterval::Every(HealthDuration::new(1).expect("interval")))
            .is_err()
    );
    assert_eq!(startup.interval(), Some(HealthInterval::Disabled));
    let startup_timeout = HealthTimeout::new(1_000_000_000).expect("timeout");
    startup.set_timeout(startup_timeout).expect("timeout");
    assert!(
        startup
            .set_timeout(HealthTimeout::new(2_000_000_000).expect("timeout"))
            .is_err()
    );
    assert_eq!(startup.timeout(), Some(startup_timeout));
    let startup_retries = StartupHealthRetries::new(0).expect("retries");
    startup.set_retries(startup_retries).expect("retries");
    assert!(
        startup
            .set_retries(StartupHealthRetries::new(1).expect("retries"))
            .is_err()
    );
    assert_eq!(startup.retries(), Some(startup_retries));
    let successes = StartupHealthSuccesses::new(0).expect("successes");
    startup.set_successes(successes).expect("successes");
    assert!(
        startup
            .set_successes(StartupHealthSuccesses::new(1).expect("successes"))
            .is_err()
    );
    assert_eq!(startup.successes(), Some(successes));
}

#[test]
fn runtime_setting_setters_reject_duplicates_or_conflicts_without_replacing_declared_values() {
    let command = HealthCommand::Exec(PublicHealthArgumentArray::new(["/usr/bin/true"]).expect("command"));
    let mut runtime = podman_lens::ContainerRuntimeSettings::default();
    runtime.set_health(HealthCheck::Disabled).expect("health");
    assert!(
        runtime
            .set_health(HealthCheck::Command(ConfiguredHealthCheck::new(command.clone())))
            .is_err()
    );
    assert!(matches!(runtime.health(), Some(HealthCheck::Disabled)));
    runtime
        .set_startup_health(StartupHealthCheck::new(command.clone()))
        .expect("startup");
    assert!(runtime.set_startup_health(StartupHealthCheck::new(command)).is_err());
    assert!(runtime.startup_health().is_some());

    let logging = runtime.logging_mut();
    logging.set_driver(LogDriver::K8sFile).expect("driver");
    assert!(logging.set_driver(LogDriver::Journald).is_err());
    assert_eq!(logging.driver(), Some(LogDriver::K8sFile));
    let max_size = LogSize::new(1024).expect("size");
    logging.set_max_size(max_size).expect("size");
    assert!(logging.set_max_size(LogSize::new(2048).expect("size")).is_err());
    assert_eq!(logging.max_size(), Some(max_size));
    let label = Label::new(
        LabelKey::new("org.example.service").expect("key"),
        PublicLabelValue::new("web").expect("value"),
    );
    logging.add_journald_label(label.clone()).expect("label");
    assert!(logging.add_journald_label(label).is_err());
    assert_eq!(logging.journald_labels().len(), 1);

    let security = runtime.security_mut();
    let cap = LinuxCapability::new("CHOWN").expect("capability");
    security.add_capability(cap.clone()).expect("capability");
    assert!(security.add_capability(cap.clone()).is_err());
    assert_eq!(security.cap_add(), std::slice::from_ref(&cap));
    security.drop_capability(cap.clone()).expect("capability");
    assert!(security.drop_capability(cap.clone()).is_err());
    assert_eq!(security.cap_drop(), [cap]);

    let resources = runtime.resources_mut();
    resources.set_cpu_shares(2).expect("shares");
    assert!(resources.set_cpu_shares(3).is_err());
    assert_eq!(resources.cpu_shares(), Some(2));
    resources.set_cpu_period(1_000).expect("period");
    assert!(resources.set_cpu_period(2_000).is_err());
    assert_eq!(resources.cpu_period(), Some(1_000));
    resources.set_cpu_quota(1_000).expect("quota");
    assert!(resources.set_cpu_quota(2_000).is_err());
    assert_eq!(resources.cpu_quota(), Some(1_000));
    resources.set_memory_bytes(1).expect("memory");
    assert!(resources.set_memory_bytes(2).is_err());
    assert_eq!(resources.memory_bytes(), Some(1));
    resources.set_pids(-1).expect("pids");
    assert!(resources.set_pids(1).is_err());
    assert_eq!(resources.pids(), Some(-1));
    let rlimit = Rlimit::new(RlimitKind::NoFile, RlimitValue::finite(1), RlimitValue::finite(1)).expect("rlimit");
    resources.add_rlimit(rlimit).expect("rlimit");
    assert!(resources.add_rlimit(rlimit).is_err());
    assert_eq!(resources.rlimits(), [rlimit]);

    let namespaces = runtime.namespaces_mut();
    namespaces.set_pid(NamespaceMode::Private).expect("pid");
    assert!(namespaces.set_pid(NamespaceMode::Host).is_err());
    assert_eq!(namespaces.pid(), Some(NamespaceMode::Private));
    namespaces.set_ipc(IpcNamespaceMode::Shareable).expect("ipc");
    assert!(namespaces.set_ipc(IpcNamespaceMode::Host).is_err());
    assert_eq!(namespaces.ipc(), Some(IpcNamespaceMode::Shareable));
    namespaces.set_uts(NamespaceMode::Private).expect("uts");
    assert!(namespaces.set_uts(NamespaceMode::Host).is_err());
    assert_eq!(namespaces.uts(), Some(NamespaceMode::Private));
    namespaces.set_cgroup(NamespaceMode::Host).expect("cgroup");
    assert!(namespaces.set_cgroup(NamespaceMode::Private).is_err());
    assert_eq!(namespaces.cgroup(), Some(NamespaceMode::Host));
}

#[test]
fn rlimit_only_intent_needs_no_cgroup_evidence() {
    let image = id(ResourceKind::Image, "registry.example.invalid/rlimit:1");
    let mut container = ContainerIntent::new(id(ResourceKind::Container, "rlimit"), image.clone()).expect("container");
    container
        .runtime_mut()
        .resources_mut()
        .add_rlimit(Rlimit::new(RlimitKind::NoFile, RlimitValue::finite(0), RlimitValue::Unlimited).expect("rlimit"))
        .expect("rlimit");
    let profile = TargetProfile::new(
        ObservedPodmanVersion::parse("6.1.0").expect("engine"),
        ObservedApiVersion::parse("6.1.0").expect("api"),
    )
    .expect("target");
    let mut intent = DeploymentIntent::new(profile);
    intent.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(image).expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container));
    assert!(plan_deployment(&intent).is_success());
}

#[test]
fn journald_labels_are_supported_from_podman_six_in_every_reviewed_target() {
    for (version, expected_success) in [
        ("5.4.0", false),
        ("5.5.0", false),
        ("5.6.0", false),
        ("5.7.0", false),
        ("5.8.6", false),
        ("6.0.0", true),
        ("6.1.0", true),
    ] {
        let image = id(ResourceKind::Image, "registry.example.invalid/journald:1");
        let mut container =
            ContainerIntent::new(id(ResourceKind::Container, "journald"), image.clone()).expect("container");
        let logging = container.runtime_mut().logging_mut();
        logging.set_driver(LogDriver::Journald).expect("driver");
        logging
            .add_journald_label(Label::new(
                LabelKey::new("org.example.service").expect("key"),
                PublicLabelValue::new("journald").expect("value"),
            ))
            .expect("label");
        let mut intent = DeploymentIntent::new(target_for(version));
        intent.add_resource(DeploymentResource::ExternalPrecondition(
            ExternalPrecondition::new(image).expect("image"),
        ));
        intent.add_resource(DeploymentResource::Container(container));
        let outcome = plan_deployment(&intent);
        assert_eq!(outcome.is_success(), expected_success, "Podman {version}");
        if !expected_success {
            assert!(
                outcome
                    .findings()
                    .iter()
                    .any(|finding| { finding.field() == Some("runtime.logging.journald_labels.target_version") })
            );
        }
    }
}

#[test]
fn unlimited_rlimits_are_supported_from_podman_five_six_in_every_reviewed_target() {
    for (version, expected_success) in [
        ("5.4.0", false),
        ("5.5.0", false),
        ("5.6.0", true),
        ("5.7.0", true),
        ("5.8.6", true),
        ("6.0.0", true),
        ("6.1.0", true),
    ] {
        let image = id(ResourceKind::Image, "registry.example.invalid/rlimit-version:1");
        let mut container =
            ContainerIntent::new(id(ResourceKind::Container, "rlimit-version"), image.clone()).expect("container");
        container
            .runtime_mut()
            .resources_mut()
            .add_rlimit(
                Rlimit::new(RlimitKind::NoFile, RlimitValue::finite(1024), RlimitValue::Unlimited).expect("rlimit"),
            )
            .expect("rlimit");
        let mut intent = DeploymentIntent::new(target_for(version));
        intent.add_resource(DeploymentResource::ExternalPrecondition(
            ExternalPrecondition::new(image).expect("image"),
        ));
        intent.add_resource(DeploymentResource::Container(container));
        let outcome = plan_deployment(&intent);
        assert_eq!(outcome.is_success(), expected_success, "Podman {version}");
        if !expected_success {
            assert!(
                outcome
                    .findings()
                    .iter()
                    .any(|finding| { finding.field() == Some("runtime.resources.rlimits.unlimited.target_version") })
            );
        }
    }
}

#[test]
fn cpu_quota_accepts_only_exact_positive_millisecond_values() {
    for (quota, expected_success) in [
        (-2, false),
        (-1, false),
        (0, false),
        (999, false),
        (1_000, true),
        (50_000, true),
    ] {
        let mut controls = podman_lens::ContainerResourceControls::default();
        assert_eq!(controls.set_cpu_quota(quota).is_ok(), expected_success, "quota {quota}");
        assert_eq!(controls.cpu_quota(), expected_success.then_some(quota));
    }
}

#[test]
fn maximum_log_size_requires_the_k8s_file_driver() {
    for (driver, expected_success) in [(LogDriver::K8sFile, true), (LogDriver::Journald, false)] {
        let image = id(ResourceKind::Image, "registry.example.invalid/logging:1");
        let mut container =
            ContainerIntent::new(id(ResourceKind::Container, "logging"), image.clone()).expect("container");
        let logging = container.runtime_mut().logging_mut();
        logging.set_driver(driver).expect("driver");
        logging.set_max_size(LogSize::new(1024).expect("size")).expect("size");
        let mut intent = DeploymentIntent::new(target());
        intent.add_resource(DeploymentResource::ExternalPrecondition(
            ExternalPrecondition::new(image).expect("image"),
        ));
        intent.add_resource(DeploymentResource::Container(container));
        let outcome = plan_deployment(&intent);
        assert_eq!(outcome.is_success(), expected_success);
        if !expected_success {
            assert!(
                outcome
                    .findings()
                    .iter()
                    .any(|finding| finding.field() == Some("runtime.logging.max_size"))
            );
        }
    }
}

#[test]
fn cgroup_private_namespace_requires_v2_and_uts_host_rejects_hostname() {
    for (evidence, expected_success) in [
        (None, false),
        (Some(CgroupCapabilityEvidence::new(CgroupVersion::V1, [])), false),
        (Some(CgroupCapabilityEvidence::new(CgroupVersion::V2, [])), true),
    ] {
        let image = id(ResourceKind::Image, "registry.example.invalid/ns:1");
        let mut container = ContainerIntent::new(id(ResourceKind::Container, "ns"), image.clone()).expect("container");
        container
            .runtime_mut()
            .namespaces_mut()
            .set_cgroup(NamespaceMode::Private)
            .expect("namespace");
        let mut profile = TargetProfile::new(
            ObservedPodmanVersion::parse("6.1.0").expect("engine"),
            ObservedApiVersion::parse("6.1.0").expect("api"),
        )
        .expect("target");
        if let Some(evidence) = evidence {
            profile.set_cgroup_capabilities(evidence);
        }
        let mut intent = DeploymentIntent::new(profile);
        intent.add_resource(DeploymentResource::ExternalPrecondition(
            ExternalPrecondition::new(image).expect("image"),
        ));
        intent.add_resource(DeploymentResource::Container(container));
        assert_eq!(plan_deployment(&intent).is_success(), expected_success);
    }
    let image = id(ResourceKind::Image, "registry.example.invalid/cgroup-host:1");
    let mut container =
        ContainerIntent::new(id(ResourceKind::Container, "cgroup-host"), image.clone()).expect("container");
    container
        .runtime_mut()
        .namespaces_mut()
        .set_cgroup(NamespaceMode::Host)
        .expect("namespace");
    let profile = TargetProfile::new(
        ObservedPodmanVersion::parse("6.1.0").expect("engine"),
        ObservedApiVersion::parse("6.1.0").expect("api"),
    )
    .expect("target");
    let mut intent = DeploymentIntent::new(profile);
    intent.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(image).expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container));
    assert!(plan_deployment(&intent).is_success());

    let image = id(ResourceKind::Image, "registry.example.invalid/uts:1");
    let mut container = ContainerIntent::new(id(ResourceKind::Container, "uts"), image.clone()).expect("container");
    container
        .settings_mut()
        .set_hostname(ContainerHostname::new("host.example").expect("hostname"))
        .expect("hostname");
    container
        .runtime_mut()
        .namespaces_mut()
        .set_uts(NamespaceMode::Host)
        .expect("uts");
    let mut intent = DeploymentIntent::new(target());
    intent.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(image).expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container));
    assert!(
        plan_deployment(&intent)
            .findings()
            .iter()
            .any(|finding| finding.field() == Some("runtime.namespaces.uts_host_with_hostname"))
    );
}

#[test]
fn cgroup_v1_resource_controls_require_rootful_context_and_each_controller() {
    for (context, expected_success) in [
        (TargetExecutionContext::Unknown, false),
        (TargetExecutionContext::Rootless, false),
        (TargetExecutionContext::Rootful, true),
    ] {
        let image = id(ResourceKind::Image, "registry.example.invalid/cgroup:1");
        let container_id = id(ResourceKind::Container, "cgroup");
        let mut container = ContainerIntent::new(container_id, image.clone()).expect("container");
        container
            .runtime_mut()
            .resources_mut()
            .set_cpu_shares(1024)
            .expect("shares");
        let mut profile = target();
        profile.set_execution_context(context);
        profile.set_cgroup_capabilities(CgroupCapabilityEvidence::new(
            CgroupVersion::V1,
            [CgroupController::Cpu],
        ));
        let mut intent = DeploymentIntent::new(profile);
        intent.add_resource(DeploymentResource::ExternalPrecondition(
            ExternalPrecondition::new(image).expect("image"),
        ));
        intent.add_resource(DeploymentResource::Container(container));
        assert_eq!(plan_deployment(&intent).is_success(), expected_success);
    }
}

#[test]
fn pod_members_accept_runtime_but_reject_namespace_intent() {
    let image = id(ResourceKind::Image, "registry.example.invalid/member:1");
    let pod = id(ResourceKind::Pod, "application");
    let container_id = id(ResourceKind::Container, "member");
    let mut container = ContainerIntent::new(container_id.clone(), image.clone()).expect("container");
    container.set_pod(pod.clone()).expect("pod");
    container
        .runtime_mut()
        .set_health(HealthCheck::Command(ConfiguredHealthCheck::new(HealthCommand::Shell(
            PublicHealthCommand::new("true").expect("health"),
        ))))
        .expect("health");
    let mut pod_intent = podman_lens::PodIntent::new(pod).expect("pod");
    pod_intent.add_member(container_id.clone()).expect("member");
    let mut intent = DeploymentIntent::new(target());
    intent.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(image.clone()).expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(container));

    let outcome = plan_deployment(&intent);
    assert!(outcome.plan().is_some());
    let mut member = match intent.resources().last().expect("member") {
        DeploymentResource::Container(container) => container.clone(),
        _ => unreachable!(),
    };
    member
        .runtime_mut()
        .namespaces_mut()
        .set_pid(NamespaceMode::Private)
        .expect("pid");
    member
        .runtime_mut()
        .namespaces_mut()
        .set_ipc(IpcNamespaceMode::Shareable)
        .expect("ipc");
    let mut rejected = DeploymentIntent::new(target());
    rejected.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(image).expect("image"),
    ));
    let pod = id(ResourceKind::Pod, "application");
    let mut pod_intent = podman_lens::PodIntent::new(pod).expect("pod");
    pod_intent.add_member(container_id.clone()).expect("member");
    rejected.add_resource(DeploymentResource::Pod(pod_intent));
    rejected.add_resource(DeploymentResource::Container(member));
    let outcome = plan_deployment(&rejected);
    assert!(outcome.plan().is_none());
    assert!(outcome.findings().iter().any(|finding| {
        finding.subject() == Some(&container_id) && finding.field() == Some("runtime.namespaces.pod_member")
    }));
}

#[test]
fn startup_health_logging_tmpfs_and_missing_cgroup_evidence_block_planning() {
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let container_id = id(ResourceKind::Container, "web");
    let mut container = ContainerIntent::new(container_id.clone(), image.clone()).expect("container");
    container
        .runtime_mut()
        .set_health(HealthCheck::Disabled)
        .expect("disabled health");
    container
        .runtime_mut()
        .set_startup_health(StartupHealthCheck::new(HealthCommand::Exec(
            PublicHealthArgumentArray::new(["check"]).expect("check"),
        )))
        .expect("startup");
    container
        .runtime_mut()
        .logging_mut()
        .add_journald_label(Label::new(
            LabelKey::new("org.example.service").expect("key"),
            PublicLabelValue::new("web").expect("value"),
        ))
        .expect("label");
    container
        .runtime_mut()
        .logging_mut()
        .set_max_size(LogSize::new(1024).expect("size"))
        .expect("size");
    container
        .runtime_mut()
        .security_mut()
        .set_read_write_tmpfs(true)
        .expect("tmpfs");
    container
        .runtime_mut()
        .resources_mut()
        .set_cpu_shares(500)
        .expect("cpu");
    container
        .runtime_mut()
        .resources_mut()
        .set_memory_bytes(128)
        .expect("memory");
    container.runtime_mut().resources_mut().set_pids(32).expect("pids");
    let mut target = target();
    target.set_cgroup_capabilities(CgroupCapabilityEvidence::new(CgroupVersion::V2, []));
    let mut intent = DeploymentIntent::new(target);
    intent.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(image).expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container));
    let outcome = plan_deployment(&intent);
    assert!(outcome.plan().is_none());
    assert_eq!(
        outcome
            .findings()
            .iter()
            .filter_map(podman_lens::PlanningFinding::field)
            .collect::<Vec<_>>(),
        [
            "runtime.logging.driver",
            "runtime.logging.journald_labels",
            "runtime.logging.max_size",
            "runtime.resources.cpu",
            "runtime.resources.memory_bytes",
            "runtime.resources.pids",
            "runtime.security.read_write_tmpfs",
            "runtime.startup_health_requires_health",
        ]
    );
}
