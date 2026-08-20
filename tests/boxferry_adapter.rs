//! Public-only BoxFerry-style input-to-output adapter acceptance scenario.
#![allow(clippy::expect_used)]

use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Write as _,
    fs, io,
    path::PathBuf,
    sync::Mutex,
};

use podman_lens::{
    AbsoluteContainerPath, AcquisitionOptions, ArgumentArray, ContainerIntent, ContainerMountKind,
    ContainerMountSource, ContainerUser, ContainerWorkdir, DeploymentIntent, DeploymentResource, DeploymentResourceId,
    DiscoveryRequest, ExternalPrecondition, ImageIntent, ImagePullPolicy, ImageSource, LibpodHeader, LibpodHeaders,
    LibpodRequest, LibpodResponse, LibpodTransport, LibpodTransportFuture, MountAccess, NamedVolumeCopyMode,
    NamedVolumeMount, NetworkAttachment, NetworkIntent, ObservationField, ObservationOrigin, ObservedApiVersion,
    ObservedPodmanVersion, PodIntent, RenderedHttpBody, RenderedHttpMethod, ResourceDetails, ResourceInventory,
    ResourceKind, ResourceObservation, SecretGrant, SemanticOperationAction, StartupDependency, TargetProfile,
    TransportError, VolumeIntent, acquire_inventory, discover, plan_deployment, render_deployment,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug)]
struct FixtureTransport {
    responses: Mutex<VecDeque<LibpodResponse>>,
}

impl FixtureTransport {
    fn new(responses: Vec<LibpodResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }
}

impl LibpodTransport for FixtureTransport {
    fn send<'a>(&'a self, _request: &'a LibpodRequest) -> LibpodTransportFuture<'a> {
        Box::pin(async move {
            let mut responses = self.responses.lock().map_err(|_| TransportError::unavailable())?;
            responses.pop_front().ok_or_else(TransportError::unavailable)
        })
    }
}

#[derive(Serialize)]
struct AdapterGolden {
    engine_version: String,
    api_version: String,
    inventory_counts: BTreeMap<&'static str, usize>,
    discovery: DiscoveryGolden,
    neutral_application: NeutralApplication,
    decisions: Vec<MappingDecision>,
    plan_operations: Vec<String>,
    render_status: String,
    rendered_operation_count: usize,
    rendered_cli_sha256: String,
    rendered_libpod_sha256: String,
}

#[derive(Serialize)]
struct DiscoveryGolden {
    resolved_roots: Vec<String>,
    groups: Vec<GroupGolden>,
    shared_prerequisites: Vec<String>,
    dependency_count: usize,
    finding_count: usize,
}

#[derive(Serialize)]
struct GroupGolden {
    id: String,
    members: Vec<String>,
    prerequisites: Vec<String>,
}

#[derive(Serialize)]
struct NeutralApplication {
    services: Vec<NeutralService>,
    groups: Vec<NeutralGroup>,
    networks: Vec<NeutralResource>,
    volumes: Vec<NeutralResource>,
    images: Vec<NeutralImage>,
    secrets: Vec<NeutralResource>,
}

#[derive(Serialize)]
struct NeutralService {
    name: String,
    native_id: String,
    image: String,
    group: Option<String>,
    networks: Vec<String>,
    mounts: Vec<NeutralMount>,
    secrets: Vec<String>,
    command: Vec<String>,
    entrypoint: Option<Vec<String>>,
    user: Option<String>,
    workdir: Option<String>,
    dependencies: Vec<String>,
}

#[derive(Serialize)]
struct NeutralMount {
    source: String,
    destination: String,
    writable: bool,
}

#[derive(Serialize)]
struct NeutralGroup {
    name: String,
    native_id: String,
    members: Vec<String>,
    networks: Vec<String>,
}

#[derive(Serialize)]
struct NeutralResource {
    name: String,
    ownership: &'static str,
}

#[derive(Serialize)]
struct NeutralImage {
    name: String,
    source: String,
    acquisition: &'static str,
}

#[derive(Serialize)]
struct MappingDecision {
    source: &'static str,
    target: &'static str,
    outcome: &'static str,
    reason: &'static str,
}

fn corpus_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/corpus")
        .join(name)
}

fn fixture_responses(name: &str) -> Result<Vec<LibpodResponse>, Box<dyn std::error::Error>> {
    let fixture: Value = serde_json::from_slice(&fs::read(corpus_path(name))?)?;
    fixture["responses"]
        .as_array()
        .ok_or_else(|| io::Error::other("fixture must contain responses"))?
        .iter()
        .map(fixture_response)
        .collect()
}

fn fixture_response(value: &Value) -> Result<LibpodResponse, Box<dyn std::error::Error>> {
    let status = value["status"]
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| io::Error::other("fixture status must be u16"))?;
    let mut headers = value["headers"]
        .as_array()
        .ok_or_else(|| io::Error::other("fixture headers must be an array"))?
        .iter()
        .map(|header| {
            let values = header
                .as_array()
                .ok_or_else(|| io::Error::other("fixture header must be a pair"))?;
            LibpodHeader::new(
                values.first().and_then(Value::as_str).unwrap_or_default(),
                values.get(1).and_then(Value::as_str).unwrap_or_default(),
            )
            .map_err(Into::into)
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let body = value.get("body").cloned().unwrap_or(Value::Null);
    if headers.is_empty() && !body.is_null() {
        headers.push(LibpodHeader::new("content-type", "application/json")?);
    }
    let body = if body.is_null() {
        Vec::new()
    } else {
        serde_json::to_vec(&body)?
    };
    Ok(LibpodResponse::new(status, LibpodHeaders::new(headers), body)?)
}

fn required_observed<'a, T>(
    field: &'a ObservationField<T>,
    label: &'static str,
) -> Result<&'a podman_lens::ObservedValue<T>, io::Error> {
    field
        .observed()
        .ok_or_else(|| io::Error::other(format!("{label} must be observed")))
}

fn require_origin<'a, T>(
    field: &'a ObservationField<T>,
    origin: ObservationOrigin,
    label: &'static str,
) -> Result<&'a T, io::Error> {
    let observed = required_observed(field, label)?;
    if observed.origin() != origin {
        return Err(io::Error::other(format!("{label} has wrong origin")));
    }
    Ok(observed.value())
}

fn observation<'a>(
    inventory: &'a ResourceInventory,
    kind: ResourceKind,
    reference: &str,
) -> Result<&'a ResourceObservation, io::Error> {
    inventory
        .section(kind)
        .and_then(|section| {
            section.observations().iter().find(|observation| {
                let identity = observation.header().identity();
                identity.id() == reference || identity.name() == Some(reference)
            })
        })
        .ok_or_else(|| io::Error::other(format!("missing {kind:?} {reference}")))
}

fn resource_name(inventory: &ResourceInventory, kind: ResourceKind, reference: &str) -> Result<String, io::Error> {
    let identity = observation(inventory, kind, reference)?.header().identity();
    Ok(identity.name().unwrap_or(identity.id()).to_owned())
}

fn resource_key(kind: ResourceKind, id: &str) -> String {
    format!("{}:{id}", kind_name(kind))
}

const fn kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Container => "container",
        ResourceKind::Pod => "pod",
        ResourceKind::Network => "network",
        ResourceKind::Volume => "volume",
        ResourceKind::Image => "image",
        ResourceKind::Secret => "secret",
        _ => "future",
    }
}

fn adapt_mounts(container: &podman_lens::ContainerObservation) -> Result<Vec<NeutralMount>, io::Error> {
    Ok(
        require_origin(container.mounts(), ObservationOrigin::Effective, "container mounts")?
            .iter()
            .filter_map(|mount| {
                if mount.kind() != ContainerMountKind::NamedVolume {
                    return None;
                }
                let source = required_observed(mount.source(), "mount source").ok()?;
                let ContainerMountSource::NamedVolume(source) = source.value() else {
                    return None;
                };
                let destination = required_observed(mount.destination(), "mount destination").ok()?;
                let writable = required_observed(mount.writable(), "mount writable").ok()?;
                Some(NeutralMount {
                    source: source.clone(),
                    destination: destination.value().clone(),
                    writable: *writable.value(),
                })
            })
            .collect(),
    )
}

fn adapt_secrets(
    inventory: &ResourceInventory,
    container: &podman_lens::ContainerObservation,
) -> Result<Vec<String>, io::Error> {
    container
        .secret_grants()
        .observed()
        .map(|value| {
            value
                .value()
                .iter()
                .filter_map(|grant| {
                    grant
                        .reference()
                        .observed()
                        .and_then(|reference| reference.value().name().or_else(|| reference.value().id()))
                        .map(|reference| resource_name(inventory, ResourceKind::Secret, reference.reference()))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn adapt_service(inventory: &ResourceInventory, reference: &str) -> Result<NeutralService, Box<dyn std::error::Error>> {
    let observation = observation(inventory, ResourceKind::Container, reference)?;
    let identity = observation.header().identity();
    let ResourceDetails::Container(container) = observation.details() else {
        return Err(io::Error::other("container identity has non-container details").into());
    };
    let image = require_origin(
        container.configured_image(),
        ObservationOrigin::Configured,
        "configured container image",
    )?
    .clone();
    let command = require_origin(container.command(), ObservationOrigin::Configured, "configured command")?
        .arguments()
        .to_vec();
    let entrypoint = container.entrypoint().observed().map(|value| {
        assert_eq!(value.origin(), ObservationOrigin::Configured);
        value.value().arguments().to_vec()
    });
    let user = container.user().observed().map(|value| {
        assert_eq!(value.origin(), ObservationOrigin::Configured);
        value.value().value().to_owned()
    });
    let workdir = container.working_directory().observed().map(|value| {
        assert_eq!(value.origin(), ObservationOrigin::Configured);
        value.value().value().to_owned()
    });
    let group = container
        .pod_membership()
        .observed()
        .map(|value| {
            assert_eq!(value.origin(), ObservationOrigin::Configured);
            resource_name(inventory, ResourceKind::Pod, value.value().reference())
        })
        .transpose()?;
    let dependencies = container
        .native_dependencies()
        .observed()
        .map(|value| {
            assert_eq!(value.origin(), ObservationOrigin::Configured);
            value
                .value()
                .iter()
                .map(|reference| resource_name(inventory, ResourceKind::Container, reference.reference()))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let networks = if group.is_some() {
        Vec::new()
    } else {
        container
            .networking()
            .observed()
            .and_then(|networking| networking.value().networks().observed())
            .map(|networks| {
                networks
                    .value()
                    .iter()
                    .map(|network| resource_name(inventory, ResourceKind::Network, network.reference()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default()
    };
    let mounts = adapt_mounts(container)?;
    let secrets = adapt_secrets(inventory, container)?;

    Ok(NeutralService {
        name: identity.name().unwrap_or(identity.id()).to_owned(),
        native_id: identity.id().to_owned(),
        image,
        group,
        networks,
        mounts,
        secrets,
        command,
        entrypoint,
        user,
        workdir,
        dependencies,
    })
}

fn validate_observation_boundaries(inventory: &ResourceInventory) -> Result<(), Box<dyn std::error::Error>> {
    let ResourceDetails::Container(api) = observation(inventory, ResourceKind::Container, "bf61-api")?.details() else {
        return Err(io::Error::other("api must be a container").into());
    };
    assert_eq!(
        require_origin(
            api.local_image_id(),
            ObservationOrigin::LocalResolution,
            "local image ID"
        )?,
        "sha256:bf61"
    );
    assert_eq!(
        require_origin(api.environment(), ObservationOrigin::Effective, "protected environment")?
            .entries()
            .len(),
        2
    );
    let bind = require_origin(api.mounts(), ObservationOrigin::Effective, "api mounts")?
        .iter()
        .find(|mount| mount.kind() == ContainerMountKind::Bind)
        .ok_or_else(|| io::Error::other("api bind mount must be retained"))?;
    let bind_source = required_observed(bind.source(), "bind source")?;
    assert_eq!(bind_source.origin(), ObservationOrigin::LocalResolution);
    assert!(matches!(bind_source.value(), ContainerMountSource::LocalBindPath(_)));

    let ResourceDetails::Network(network) = observation(inventory, ResourceKind::Network, "bf61-network")?.details()
    else {
        return Err(io::Error::other("network identity has non-network details").into());
    };
    assert_eq!(
        required_observed(network.subnets(), "network subnets")?.origin(),
        ObservationOrigin::Effective
    );
    let ResourceDetails::Volume(volume) = observation(inventory, ResourceKind::Volume, "application-data")?.details()
    else {
        return Err(io::Error::other("volume identity has non-volume details").into());
    };
    assert_eq!(
        required_observed(volume.driver(), "volume driver")?.origin(),
        ObservationOrigin::Effective
    );
    let ResourceDetails::Image(image) = observation(inventory, ResourceKind::Image, "sha256:bf61")?.details() else {
        return Err(io::Error::other("image identity has non-image details").into());
    };
    assert_eq!(
        require_origin(
            image.repo_tags(),
            ObservationOrigin::LocalResolution,
            "image repository tags"
        )?
        .len(),
        1
    );
    let ResourceDetails::Secret(secret) = observation(inventory, ResourceKind::Secret, "bf61-secret")?.details() else {
        return Err(io::Error::other("secret identity has non-secret details").into());
    };
    let driver = require_origin(secret.driver(), ObservationOrigin::Effective, "secret driver")?;
    assert_eq!(
        require_origin(driver.name(), ObservationOrigin::Effective, "secret driver name")?,
        "file"
    );
    assert_eq!(
        require_origin(driver.options(), ObservationOrigin::Effective, "secret driver options")?.len(),
        1
    );
    Ok(())
}

fn mapping_decisions() -> Vec<MappingDecision> {
    vec![
        MappingDecision {
            source: "Container.Config.ImageName",
            target: "Service.image",
            outcome: "promoted",
            reason: "configured evidence has exact neutral semantics",
        },
        MappingDecision {
            source: "Container.Image",
            target: "Service.image",
            outcome: "observation-only",
            reason: "local-resolution IDs are not desired state",
        },
        MappingDecision {
            source: "Container.Config.Env",
            target: "Service.environment",
            outcome: "manual",
            reason: "values remain protected and require explicit authorization",
        },
        MappingDecision {
            source: "Container.Mounts.volume",
            target: "Service.mounts",
            outcome: "promoted",
            reason: "named source and container destination are portable",
        },
        MappingDecision {
            source: "Container.Mounts.bind",
            target: "Service.mounts",
            outcome: "manual",
            reason: "host source is local-resolution evidence",
        },
        MappingDecision {
            source: "Network.subnets",
            target: "Network.ipam",
            outcome: "observation-only",
            reason: "effective inspect state needs explicit authoring policy",
        },
        MappingDecision {
            source: "Image.RepoTags",
            target: "ImageAcquisition.reference",
            outcome: "observation-only",
            reason: "repository tags are local-resolution evidence",
        },
        MappingDecision {
            source: "Secret.Spec.Driver.Options",
            target: "Secret.material",
            outcome: "unavailable",
            reason: "only option count is retained and payload is never acquired",
        },
    ]
}

fn adapt_application(
    inventory: &ResourceInventory,
) -> Result<(NeutralApplication, Vec<MappingDecision>), Box<dyn std::error::Error>> {
    let api = adapt_service(inventory, "bf61-api")?;
    let worker = adapt_service(inventory, "bf61-worker")?;
    let pod = observation(inventory, ResourceKind::Pod, "bf61-pod")?;
    let ResourceDetails::Pod(pod_details) = pod.details() else {
        return Err(io::Error::other("pod identity has non-pod details").into());
    };
    let pod_networks = required_observed(pod_details.networking(), "pod networking")?
        .value()
        .networks()
        .observed()
        .ok_or_else(|| io::Error::other("pod networks must be observed"))?
        .value()
        .iter()
        .map(|network| resource_name(inventory, ResourceKind::Network, network.reference()))
        .collect::<Result<Vec<_>, _>>()?;
    validate_observation_boundaries(inventory)?;

    let group_name = pod.header().identity().name().unwrap_or("bf61-pod").to_owned();
    let application = NeutralApplication {
        services: vec![api, worker],
        groups: vec![NeutralGroup {
            name: group_name,
            native_id: pod.header().identity().id().to_owned(),
            members: vec!["api".to_owned()],
            networks: pod_networks,
        }],
        networks: vec![NeutralResource {
            name: resource_name(inventory, ResourceKind::Network, "bf61-network")?,
            ownership: "uncertain",
        }],
        volumes: vec![NeutralResource {
            name: resource_name(inventory, ResourceKind::Volume, "application-data")?,
            ownership: "uncertain",
        }],
        images: vec![NeutralImage {
            name: "application-image".to_owned(),
            source: "registry.example.invalid/boxferry/application:6.1".to_owned(),
            acquisition: "pull-if-missing",
        }],
        secrets: vec![NeutralResource {
            name: resource_name(inventory, ResourceKind::Secret, "bf61-secret")?,
            ownership: "external",
        }],
    };
    Ok((application, mapping_decisions()))
}

fn deployment_id(kind: ResourceKind, name: &str) -> Result<DeploymentResourceId, io::Error> {
    DeploymentResourceId::new(kind, name).map_err(|error| io::Error::other(error.to_string()))
}

fn build_intent(
    application: &NeutralApplication,
    target: TargetProfile,
) -> Result<DeploymentIntent, Box<dyn std::error::Error>> {
    let image = deployment_id(ResourceKind::Image, &application.images[0].name)?;
    let network = deployment_id(ResourceKind::Network, &application.networks[0].name)?;
    let volume = deployment_id(ResourceKind::Volume, &application.volumes[0].name)?;
    let secret = deployment_id(ResourceKind::Secret, &application.secrets[0].name)?;
    let pod = deployment_id(ResourceKind::Pod, &application.groups[0].name)?;
    let mut intent = DeploymentIntent::new(target);
    intent.add_resource(DeploymentResource::Image(ImageIntent::new(
        image.clone(),
        ImageSource::new(&application.images[0].source)?,
        ImagePullPolicy::Missing,
    )?));
    intent.add_resource(DeploymentResource::Network(NetworkIntent::new(network.clone())?));
    intent.add_resource(DeploymentResource::Volume(VolumeIntent::new(volume)?));
    intent.add_resource(DeploymentResource::ExternalPrecondition(ExternalPrecondition::new(
        secret.clone(),
    )?));

    let api_id = deployment_id(ResourceKind::Container, "api")?;
    let mut pod_intent = PodIntent::new(pod.clone())?;
    pod_intent.add_network(NetworkAttachment::new(network.clone())?)?;
    pod_intent.add_member(api_id.clone())?;
    intent.add_resource(DeploymentResource::Pod(pod_intent));

    for service in &application.services {
        let service_id = deployment_id(ResourceKind::Container, &service.name)?;
        let mut container = ContainerIntent::new(service_id.clone(), image.clone())?;
        if service.group.is_some() {
            container.set_pod(pod.clone())?;
        } else {
            for _ in &service.networks {
                container.add_network(NetworkAttachment::new(network.clone())?)?;
            }
        }
        for mount in &service.mounts {
            container.add_mount(NamedVolumeMount::new(
                deployment_id(ResourceKind::Volume, &mount.source)?,
                AbsoluteContainerPath::new(&mount.destination)?,
                if mount.writable {
                    MountAccess::ReadWrite
                } else {
                    MountAccess::ReadOnly
                },
                NamedVolumeCopyMode::Copy,
            )?);
        }
        for secret_name in &service.secrets {
            container.add_secret_grant(SecretGrant::mount(deployment_id(ResourceKind::Secret, secret_name)?)?);
        }
        container
            .settings_mut()
            .set_command(ArgumentArray::new(service.command.clone())?)?;
        if let Some(entrypoint) = &service.entrypoint {
            container
                .settings_mut()
                .set_entrypoint(ArgumentArray::new(entrypoint.clone())?)?;
        }
        if let Some(user) = &service.user {
            container.settings_mut().set_user(ContainerUser::new(user)?)?;
        }
        if let Some(workdir) = &service.workdir {
            container
                .settings_mut()
                .set_workdir(ContainerWorkdir::new(AbsoluteContainerPath::new(workdir)?))?;
        }
        intent.add_resource(DeploymentResource::Container(container));
        for prerequisite in &service.dependencies {
            intent.add_startup_dependency(StartupDependency::new(
                service_id.clone(),
                deployment_id(ResourceKind::Container, prerequisite)?,
            )?);
        }
    }
    Ok(intent)
}

fn action_name(action: SemanticOperationAction) -> &'static str {
    match action {
        SemanticOperationAction::EnsureImage => "ensure-image",
        SemanticOperationAction::Create => "create",
        SemanticOperationAction::StartPod => "start-pod",
        SemanticOperationAction::StartContainer => "start-container",
        _ => "future",
    }
}

fn plan_operation_key(operation: &podman_lens::DeploymentOperation) -> String {
    let id = operation.id();
    format!(
        "{}:{}:{}",
        action_name(id.action()),
        kind_name(id.resource().kind()),
        id.resource().name()
    )
}

fn rendered_body_name(operation: &podman_lens::RenderedOperation) -> &'static str {
    match (operation.libpod().method(), operation.libpod().body()) {
        (RenderedHttpMethod::Get, RenderedHttpBody::Empty) => ("GET", "empty"),
        (RenderedHttpMethod::Post, RenderedHttpBody::Empty) => ("POST", "empty"),
        (RenderedHttpMethod::Post, RenderedHttpBody::Json(_)) => ("POST", "json"),
        (RenderedHttpMethod::Post, RenderedHttpBody::ExternalSensitiveInput(_)) => ("POST", "external-sensitive-input"),
        (RenderedHttpMethod::Get, _) => ("GET", "unexpected"),
    }
    .1
}

fn sha256_lines(lines: impl IntoIterator<Item = String>) -> String {
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update([0]);
    }
    hasher.finalize().iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
        output
    })
}

#[tokio::test]
async fn public_boxferry_adapter_scenario_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
    let inventory = acquire_inventory(
        &FixtureTransport::new(fixture_responses("boxferry-6.1.responses.json")?),
        AcquisitionOptions::redacted(),
    )
    .await?;
    let mut request = DiscoveryRequest::new();
    request.select_all();
    let graph = discover(&inventory, &request)?;
    let (application, decisions) = adapt_application(&inventory)?;
    let target = TargetProfile::new(
        ObservedPodmanVersion::parse("6.1.0")?,
        ObservedApiVersion::parse("6.1.0")?,
    )?;
    let intent = build_intent(&application, target)?;
    let planning = plan_deployment(&intent);
    assert!(planning.findings().is_empty());
    let plan = planning
        .plan()
        .ok_or_else(|| io::Error::other("adapter intent must produce a plan"))?;
    let rendering = render_deployment(plan);
    assert!(rendering.findings().is_empty());
    let rendered = rendering
        .rendering()
        .ok_or_else(|| io::Error::other("adapter plan must render exactly"))?;

    let inventory_counts = [
        ResourceKind::Container,
        ResourceKind::Pod,
        ResourceKind::Network,
        ResourceKind::Volume,
        ResourceKind::Image,
        ResourceKind::Secret,
    ]
    .into_iter()
    .map(|kind| {
        (
            kind_name(kind),
            inventory
                .section(kind)
                .map_or(0, |section| section.observations().len()),
        )
    })
    .collect();
    let golden = AdapterGolden {
        engine_version: inventory.service().engine_version().original().to_owned(),
        api_version: inventory.service().api_version().original().to_owned(),
        inventory_counts,
        discovery: DiscoveryGolden {
            resolved_roots: graph
                .resolved_roots()
                .iter()
                .map(|identity| resource_key(identity.kind(), identity.id()))
                .collect(),
            groups: graph
                .groups()
                .iter()
                .map(|group| GroupGolden {
                    id: resource_key(group.id().kind(), group.id().id()),
                    members: group
                        .members()
                        .iter()
                        .map(|identity| resource_key(identity.kind(), identity.id()))
                        .collect(),
                    prerequisites: group
                        .prerequisites()
                        .iter()
                        .map(|identity| resource_key(identity.kind(), identity.id()))
                        .collect(),
                })
                .collect(),
            shared_prerequisites: graph
                .shared_prerequisites()
                .iter()
                .map(|identity| resource_key(identity.kind(), identity.id()))
                .collect(),
            dependency_count: graph.dependencies().len(),
            finding_count: graph.findings().len(),
        },
        neutral_application: application,
        decisions,
        plan_operations: plan.operations().iter().map(plan_operation_key).collect(),
        render_status: format!("{:?}", rendered.status()).to_lowercase(),
        rendered_operation_count: rendered.operations().len(),
        rendered_cli_sha256: sha256_lines(rendered.operations().iter().map(|operation| {
            let mut values = vec![operation.cli().program().to_owned()];
            values.extend(operation.cli().argv().iter().cloned());
            values.join("\u{1f}")
        })),
        rendered_libpod_sha256: sha256_lines(rendered.operations().iter().map(|operation| {
            format!(
                "{:?}\u{1f}{}\u{1f}{}",
                operation.libpod().method(),
                operation.libpod().path_and_query(),
                rendered_body_name(operation)
            )
        })),
    };
    let actual = serde_json::to_value(golden)?;
    let expected: Value = serde_json::from_slice(&fs::read(corpus_path("boxferry-adapter-6.1.expected.json"))?)?;
    assert_eq!(actual, expected);
    Ok(())
}
