//! Compile-time public API contract for the stable M4 native input boundary.

#[cfg(unix)]
use std::time::Duration;

use podman_lens::{
    AbsoluteContainerPath, ArgumentArray, CgroupCapabilityEvidence, CgroupController, CgroupVersion,
    ConfiguredHealthCheck, ContainerHostname, ContainerIntent, ContainerUser, ContainerWorkdir,
    DeploymentConnectionReference, DeploymentEnvironmentValue, DeploymentIntent, DeploymentPlan, DeploymentResource,
    DeploymentResourceId, DnsConfiguration, EnvironmentAssignment, EnvironmentName, ExternalPrecondition, HealthCheck,
    HealthCommand, HostAlias, ImageIntent, ImagePullPolicy, Label, LabelKey, LinuxCapability, LogDriver, LogSize,
    NamedVolumeCopyMode, NamedVolumeMount, NetworkAttachment, NetworkCidr, NetworkIntent, NetworkRoute, NetworkSubnet,
    ObservedApiVersion, ObservedPodmanVersion, PlanningFinding, PlanningOutcome, PodIntent, PortMapping, PortProtocol,
    PublicEnvironmentValue, PublicHealthArgumentArray, PublicHealthCommand, PublicLabelValue, RestartPolicy, Rlimit,
    RlimitKind, RlimitValue, RouteType, SemanticOperationAction, SensitiveInlineEnvironmentValue, StartupDependency,
    StartupHealthCheck, StaticMacAddress, TargetExecutionContext, TargetProfile, VolumeIntent, plan_deployment,
};
use podman_lens::{
    AcquisitionOptions, ConnectionSpec, DiscoveryRequest, LabelSelector, LibpodHeaders, LibpodPath, LibpodRequest,
    LibpodResponse, LibpodTransport, LibpodTransportFuture, NativeFieldCoverageClassification,
    NativeFieldCoveragePlane, OpaqueReference, ResourceInventory, ResourceKind, ResourceSelector, SshConnection,
    TransportError, UnixConnection, acquire_inventory, artifact::deployment_v1, discover, probe_libpod_service,
    snapshot::v1,
};
#[cfg(unix)]
use podman_lens::{ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts, TransportLimits};

struct FixtureTransport;

impl LibpodTransport for FixtureTransport {
    fn send<'a>(&'a self, _request: &'a LibpodRequest) -> LibpodTransportFuture<'a> {
        Box::pin(async {
            LibpodResponse::new(200, LibpodHeaders::default(), Vec::new()).map_err(|_| TransportError::unavailable())
        })
    }
}

fn consume_inventory_snapshot(source: &ResourceInventory) {
    let snapshot = v1::inventory(source);
    assert_eq!(snapshot.schema_version(), v1::SCHEMA_VERSION);
    drop(serde_json::to_string(&snapshot));
}

fn consume_graph_contract(
    source: &ResourceInventory,
    request: &DiscoveryRequest,
) -> Result<(), podman_lens::Diagnostic> {
    let graph = discover(source, request)?;
    let _ = graph.requested_roots();
    let _ = graph.requested_label_roots();
    let _ = graph.all_requested();
    let _ = graph.resolved_roots();
    let _ = graph.groups();
    let _ = graph.shared_prerequisites();
    let _ = graph.dependencies();
    let _ = graph.grouping_edges();
    let _ = graph.findings();
    let _ = graph.explanations();
    let snapshot = v1::graph(&graph);
    assert_eq!(snapshot.schema_version(), v1::SCHEMA_VERSION);
    drop(serde_json::to_string(&snapshot));
    Ok(())
}

#[test]
fn external_consumer_can_construct_explicit_connections_and_an_object_safe_transport()
-> Result<(), Box<dyn std::error::Error>> {
    let unix = UnixConnection::parse("unix:///run/user/1000/podman/podman.sock")?;
    let connection = ConnectionSpec::Unix(unix);
    assert!(matches!(connection, ConnectionSpec::Unix(_)));

    let ssh = SshConnection::parse(
        "ssh://podman@example.invalid:2222/run/user/1000/podman/podman.sock",
        OpaqueReference::new("host-key-reference")?,
        OpaqueReference::new("authentication-reference")?,
    )?;
    assert_eq!(ssh.port(), 2222);

    let transport: &dyn LibpodTransport = &FixtureTransport;
    let request = LibpodRequest::new(
        podman_lens::LibpodMethod::Get,
        LibpodPath::parse("/libpod/_ping")?,
        Vec::new(),
    )?;
    drop(transport.send(&request));
    Ok(())
}

#[test]
fn crate_can_be_linked_by_an_external_consumer() {
    assert_eq!(env!("CARGO_PKG_NAME"), "podman-lens");
}

#[test]
fn external_consumer_can_select_the_redacted_inventory_contract() {
    let options = AcquisitionOptions::redacted();
    assert_eq!(options, AcquisitionOptions::default());
    assert_eq!(ResourceKind::Container, ResourceKind::Container);
    let transport: &dyn LibpodTransport = &FixtureTransport;
    drop(acquire_inventory(
        transport,
        AcquisitionOptions::include_environment_values(),
    ));
}

#[test]
fn external_consumer_can_inspect_the_strict_two_plane_coverage_ledger() -> Result<(), Box<dyn std::error::Error>> {
    let entries = podman_lens::native_field_coverage_catalogue()?;
    assert!(entries.iter().any(|entry| {
        entry.id() == "PLN-FLD-0035"
            && entry.plane() == NativeFieldCoveragePlane::InputObservation
            && entry.field_path() == "$.Spec.Driver"
            && entry.classification() == NativeFieldCoverageClassification::ObservationOnly
            && entry.observation() == "inventory::decode_secret"
            && entry.cli_renderer() == "not_applicable"
            && entry.libpod_renderer() == "not_applicable"
            && entry.target_versions().is_empty()
    }));
    assert!(entries.iter().any(|entry| {
        entry.id() == "PLN-FLD-0038"
            && entry.classification() == NativeFieldCoverageClassification::UnknownIncomplete
            && entry.public_contract() == "ResourceRecord::unknown_fields_complete"
    }));
    assert!(entries.iter().any(|entry| {
        entry.id() == "PLN-OUT-0015"
            && entry.plane() == NativeFieldCoveragePlane::OutputIntent
            && entry.field_path() == "runtime.logging.journald_labels"
            && entry.classification() == NativeFieldCoverageClassification::TargetGated
            && entry.planner() == "deployment::validate_runtime_settings"
            && entry.cli_renderer() == "render::append_container_runtime_arguments"
            && entry.libpod_renderer() == "render::append_container_runtime_json"
            && entry.target_versions() == ["6.0.0", "6.1.0"]
    }));
    assert!(entries.iter().any(|entry| {
        entry.id() == "PLN-OUT-0034"
            && entry.classification() == NativeFieldCoverageClassification::Manual
            && entry.field_path() == "runtime.startup_health.command.sensitive"
            && entry.public_contract() == "HealthCommand"
            && entry.target_versions().len() == 7
    }));
    Ok(())
}

#[test]
fn external_consumer_can_declare_bounded_runtime_intent_with_explicit_cgroup_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    let mut target = TargetProfile::new(
        ObservedPodmanVersion::parse("6.1.0")?,
        ObservedApiVersion::parse("6.1.0")?,
    )?;
    target.set_cgroup_capabilities(CgroupCapabilityEvidence::new(
        CgroupVersion::V2,
        [CgroupController::Cpu, CgroupController::Memory, CgroupController::Pids],
    ));
    let image = DeploymentResourceId::new(ResourceKind::Image, "registry.example.invalid/runtime:1")?;
    let mut container = ContainerIntent::new(
        DeploymentResourceId::new(ResourceKind::Container, "runtime")?,
        image.clone(),
    )?;
    container
        .runtime_mut()
        .set_health(HealthCheck::Command(ConfiguredHealthCheck::new(HealthCommand::Shell(
            PublicHealthCommand::new("true")?,
        ))))?;
    container
        .runtime_mut()
        .set_startup_health(StartupHealthCheck::new(HealthCommand::Exec(
            PublicHealthArgumentArray::new(["/usr/bin/true"])?,
        )))?;
    container.runtime_mut().logging_mut().set_driver(LogDriver::K8sFile)?;
    container
        .runtime_mut()
        .logging_mut()
        .set_max_size(LogSize::new(1024)?)?;
    container
        .runtime_mut()
        .security_mut()
        .add_capability(LinuxCapability::new("CHOWN")?)?;
    container.runtime_mut().resources_mut().set_cpu_shares(250)?;
    container.runtime_mut().resources_mut().set_memory_bytes(1024)?;
    container.runtime_mut().resources_mut().set_pids(32)?;
    container.runtime_mut().resources_mut().add_rlimit(Rlimit::new(
        RlimitKind::NoFile,
        RlimitValue::finite(128),
        RlimitValue::Unlimited,
    )?)?;
    assert!(container.runtime().health().is_some());
    assert!(container.runtime().startup_health().is_some());
    Ok(())
}

#[test]
fn external_consumer_can_use_the_stable_input_and_snapshot_contracts() -> Result<(), Box<dyn std::error::Error>> {
    let mut request = DiscoveryRequest::new();
    for (kind, reference) in [
        (ResourceKind::Container, "container"),
        (ResourceKind::Pod, "pod"),
        (ResourceKind::Network, "network"),
        (ResourceKind::Volume, "volume"),
        (ResourceKind::Image, "example.invalid/image:1"),
        (ResourceKind::Secret, "secret"),
    ] {
        request.add_root(ResourceSelector::exact(kind, reference)?);
    }
    request.add_label_root(LabelSelector::presence("example.invalid/present")?);
    request.add_label_root(LabelSelector::exact("example.invalid/exact", "redacted")?);
    request.add_network_boundary_override("network")?;
    request.select_all();
    assert_eq!(request.roots().len(), 6);
    assert_eq!(request.label_roots().len(), 2);
    assert!(request.all());
    assert_eq!(request.network_boundary_overrides().count(), 1);

    let _ = consume_inventory_snapshot;
    let _ = consume_graph_contract;
    Ok(())
}

#[test]
fn external_consumer_can_create_a_transport_neutral_semantic_deployment_plan() -> Result<(), Box<dyn std::error::Error>>
{
    let target = TargetProfile::new(
        ObservedPodmanVersion::parse("6.1.0")?,
        ObservedApiVersion::parse("4.0.0")?,
    )?;
    let image = DeploymentResourceId::new(ResourceKind::Image, "registry.example.invalid/web:1")?;
    let container = DeploymentResourceId::new(ResourceKind::Container, "web")?;
    let volume = DeploymentResourceId::new(ResourceKind::Volume, "web-data")?;
    let mut pod_intent = PodIntent::new(DeploymentResourceId::new(ResourceKind::Pod, "web-pod")?)?;
    pod_intent.add_infra_mount(NamedVolumeMount::new(
        volume.clone(),
        AbsoluteContainerPath::new("/var/lib/infra")?,
        false,
        NamedVolumeCopyMode::Copy,
    )?);
    assert_eq!(pod_intent.infra_mounts().len(), 1);
    let mut container_intent = ContainerIntent::new(container.clone(), image.clone())?;
    container_intent.add_mount(NamedVolumeMount::new(
        volume.clone(),
        AbsoluteContainerPath::new("/var/lib/web")?,
        false,
        NamedVolumeCopyMode::NoCopy,
    )?);
    {
        let settings = container_intent.settings_mut();
        settings.set_command(ArgumentArray::new(["serve"])?)?;
        settings.set_entrypoint(ArgumentArray::new(["/entrypoint"])?)?;
        settings.set_user(ContainerUser::new("1000:1000")?)?;
        settings.set_workdir(ContainerWorkdir::new(AbsoluteContainerPath::new("/srv/web")?))?;
        settings.set_hostname(ContainerHostname::new("web.example")?)?;
        settings.add_label(Label::new(
            LabelKey::new("org.example.role")?,
            PublicLabelValue::new("web")?,
        ))?;
        settings.add_environment(EnvironmentAssignment::new(
            EnvironmentName::new("MODE")?,
            DeploymentEnvironmentValue::Public(PublicEnvironmentValue::new("production")?),
        ))?;
        settings.add_environment(EnvironmentAssignment::new(
            EnvironmentName::new("PASSWORD")?,
            DeploymentEnvironmentValue::SensitiveInline(SensitiveInlineEnvironmentValue::new("redacted")?),
        ))?;
        settings.set_restart_policy(RestartPolicy::Always)?;
    }
    let mut intent = DeploymentIntent::new(target);
    intent.set_connection(DeploymentConnectionReference::new("production")?);
    intent.add_resource(DeploymentResource::ExternalPrecondition(ExternalPrecondition::new(
        image.clone(),
    )?));
    intent.add_resource(DeploymentResource::Volume(VolumeIntent::new(volume)?));
    intent.add_resource(DeploymentResource::Container(container_intent));
    intent.add_startup_dependency(StartupDependency::new(container.clone(), container.clone())?);
    let outcome = plan_deployment(&intent);
    let _: &PlanningOutcome = &outcome;
    let _: &[PlanningFinding] = outcome.findings();
    let _: Option<&DeploymentPlan> = outcome.plan();
    assert!(!outcome.is_success());
    let finding = &outcome.findings()[0];
    assert_eq!(finding.code().as_str(), "PLN0039");
    assert!(!finding.message().is_empty());
    let _ = finding.subject();
    let _ = finding.related();
    let _ = finding.field();
    let _ = finding.occurrence();
    let _ = finding.count();
    if let Some(plan) = outcome.plan() {
        let _ = plan.external_preconditions();
        for operation in plan.operations() {
            let _ = operation.resource_intent();
        }
    }
    if let Some(plan) = outcome.plan() {
        let rendering = podman_lens::render_deployment(plan);
        if let Some(rendering) = rendering.rendering() {
            let artifact = deployment_v1::deployment(rendering);
            assert_eq!(artifact.schema_version(), deployment_v1::SCHEMA_VERSION);
            drop(serde_json::to_string(&artifact));
        }
    }
    assert_eq!(ImagePullPolicy::Missing, ImagePullPolicy::default());
    let _ = ImageIntent::new(
        DeploymentResourceId::new(ResourceKind::Image, "registry.example.invalid/managed:1")?,
        "registry.example.invalid/managed:1",
    )?;
    let _ = SemanticOperationAction::StartPod;
    let _ = SemanticOperationAction::StartContainer;
    Ok(())
}

#[test]
fn external_consumer_can_declare_typed_networking_output() -> Result<(), Box<dyn std::error::Error>> {
    let network = DeploymentResourceId::new(ResourceKind::Network, "application-network")?;
    let image = DeploymentResourceId::new(ResourceKind::Image, "registry.example.invalid/application:1")?;
    let container = DeploymentResourceId::new(ResourceKind::Container, "application")?;
    let mut attachment = NetworkAttachment::new(network.clone())?;
    attachment.add_alias("application")?;
    attachment.set_static_ipv4("192.0.2.10".parse()?)?;
    attachment.set_static_mac(StaticMacAddress::new("02:42:ac:11:00:02")?)?;
    let mut network_intent = NetworkIntent::new(network.clone())?;
    let mut subnet = NetworkSubnet::new(NetworkCidr::new("192.0.2.0/24")?);
    subnet.set_gateway("192.0.2.1".parse()?)?;
    assert!(subnet.subnet().contains("192.0.2.2".parse()?));
    network_intent.add_subnet(subnet)?;
    network_intent.add_route(NetworkRoute::new(
        NetworkCidr::new("198.51.100.0/24")?,
        None,
        RouteType::Blackhole,
    )?)?;
    let mut container_intent = ContainerIntent::new(container, image.clone())?;
    container_intent.add_network(attachment)?;
    container_intent.set_network_order(vec![network.clone()])?;
    container_intent.add_port(PortMapping::new(None, 8080, 80, PortProtocol::Tcp)?)?;
    container_intent.add_host_alias(HostAlias::new("192.0.2.53".parse()?, "database.test")?)?;
    let dns: &mut DnsConfiguration = container_intent.dns_mut();
    dns.add_server("192.0.2.53".parse()?)?;
    dns.add_search("example.test")?;
    dns.add_option("ndots:1")?;

    let _ = network_intent.subnets();
    let _ = network_intent.routes();
    let _ = container_intent.networks();
    let _ = container_intent.network_order();
    let _ = container_intent.ports();
    let _ = container_intent.dns();
    let _ = container_intent.host_aliases();
    Ok(())
}

#[test]
fn external_consumer_can_set_explicit_target_execution_context() -> Result<(), Box<dyn std::error::Error>> {
    let mut target = TargetProfile::new(
        ObservedPodmanVersion::parse("6.1.0")?,
        ObservedApiVersion::parse("4.0.0")?,
    )?;
    assert_eq!(target.execution_context(), TargetExecutionContext::Unknown);
    target.set_execution_context(TargetExecutionContext::Rootful);
    assert_eq!(target.execution_context(), TargetExecutionContext::Rootful);
    Ok(())
}

#[tokio::test]
#[cfg(unix)]
async fn external_consumer_can_use_the_fixed_read_only_probe_contract() -> Result<(), Box<dyn std::error::Error>> {
    let transport: &dyn LibpodTransport = &FixtureTransport;
    let error = probe_libpod_service(transport)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("empty fixture response unexpectedly probed"))?;
    assert_eq!(error.code().as_str(), "PLN0008");

    let unix = UnixConnection::new("/run/user/1000/podman/podman.sock")?;
    let timeouts = ReadOnlyUnixTransportTimeouts::new(
        Duration::from_secs(1),
        Duration::from_secs(1),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )?;
    let transport = ReadOnlyUnixTransport::new(unix, TransportLimits::default(), timeouts)?;
    assert_eq!(transport.timeouts(), timeouts);
    Ok(())
}
