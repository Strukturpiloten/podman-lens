//! Cross-version complex offline acquisition, discovery, and rendering scenarios.

mod support;

use std::{collections::BTreeMap, fs, io, path::PathBuf};

use podman_lens::{
    AbsoluteContainerPath, AcquisitionOptions, ArgumentArray, ContainerIntent, DeploymentIntent, DeploymentResource,
    DeploymentResourceId, DiagnosticCode, DiscoveryRequest, ExternalPrecondition, ImageIntent, ImagePullPolicy,
    ImageSource, LabelSelector, MountAccess, NamedVolumeCopyMode, NamedVolumeMount, NativeNetworkRouteType,
    NetworkAttachment, NetworkIntent, ObservationField, ObservationOrigin, ObservedApiVersion, ObservedPodmanVersion,
    PodIntent, RenderStatus, RenderedHttpBody, RenderedHttpMethod, ResourceDetails, ResourceInventory, ResourceKind,
    ResourceObservationState, ResourceSelector, SemanticOperationAction, StartupDependency, TargetExecutionContext,
    TargetProfile, VolumeIntent, acquire_inventory, discover, plan_deployment, render_deployment, snapshot::v1,
};
use serde_json::{Value, json};

use support::cassette::{Cassette, CassetteTransport, ExecutionContext};

#[derive(Clone, Copy)]
struct VersionCase {
    version: &'static str,
    revision: &'static str,
}

const VERSIONS: [VersionCase; 7] = [
    VersionCase {
        version: "5.4.0",
        revision: "f9f7d48b24b1ca4403f189caaeab1cb8ff4a9aa2",
    },
    VersionCase {
        version: "5.5.0",
        revision: "0dbcb51477ee7ab8d3b47d30facf71fc38bb0c98",
    },
    VersionCase {
        version: "5.6.0",
        revision: "da671ef6cfa3fc9ac6225c18f1dd0a70a951e43f",
    },
    VersionCase {
        version: "5.7.0",
        revision: "0370128fc8dcae93533334324ef838db8f8da8cb",
    },
    VersionCase {
        version: "5.8.6",
        revision: "a859fc66702c23e869c282c63e92d9b6cd264229",
    },
    VersionCase {
        version: "6.0.0",
        revision: "a8ed4b6dd12992decf659cadfdfb3d0cb1937748",
    },
    VersionCase {
        version: "6.1.0",
        revision: "cade97a52ebdf9dbf9e81de8009015776837a074",
    },
];

const MODES: [ExecutionContext; 2] = [ExecutionContext::Rootless, ExecutionContext::Rootful];

fn mode_name(mode: ExecutionContext) -> &'static str {
    match mode {
        ExecutionContext::Rootless => "rootless",
        ExecutionContext::Rootful => "rootful",
    }
}

fn target_context(mode: ExecutionContext) -> TargetExecutionContext {
    match mode {
        ExecutionContext::Rootless => TargetExecutionContext::Rootless,
        ExecutionContext::Rootful => TargetExecutionContext::Rootful,
    }
}

fn cassette_path(case: VersionCase, mode: ExecutionContext) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/corpus/complex-{}-{}.cassette.json",
        case.version,
        mode_name(mode)
    ))
}

fn load_cassette(case: VersionCase, mode: ExecutionContext) -> Result<Cassette, Box<dyn std::error::Error>> {
    Ok(Cassette::from_slice(&fs::read(cassette_path(case, mode))?)?)
}

async fn replay(cassette: Cassette) -> Result<ResourceInventory, Box<dyn std::error::Error>> {
    let transport = CassetteTransport::try_new(cassette)?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    transport.assert_consumed()?;
    Ok(inventory)
}

async fn inventory(case: VersionCase, mode: ExecutionContext) -> Result<ResourceInventory, Box<dyn std::error::Error>> {
    replay(load_cassette(case, mode)?).await
}

fn observation<'a>(
    inventory: &'a ResourceInventory,
    kind: ResourceKind,
    id: &str,
) -> Result<&'a podman_lens::ResourceObservation, Box<dyn std::error::Error>> {
    inventory
        .section(kind)
        .and_then(|section| {
            section
                .observations()
                .iter()
                .find(|observation| observation.header().identity().id() == id)
        })
        .ok_or_else(|| format!("missing {kind:?} observation {id}").into())
}

fn exact_root(kind: ResourceKind, value: &str) -> Result<DiscoveryRequest, Box<dyn std::error::Error>> {
    let mut request = DiscoveryRequest::new();
    request.add_root(ResourceSelector::exact(kind, value)?);
    Ok(request)
}

fn member_ids(graph: &podman_lens::ResourceGraph) -> Vec<&str> {
    graph
        .groups()
        .iter()
        .flat_map(podman_lens::ResourceGroup::members)
        .map(podman_lens::ResourceIdentity::id)
        .collect()
}

#[tokio::test]
async fn every_complex_cassette_is_provenance_bearing_request_bound_and_fully_consumed()
-> Result<(), Box<dyn std::error::Error>> {
    for case in VERSIONS {
        for mode in MODES {
            let cassette = load_cassette(case, mode)?;
            assert_eq!(cassette.schema_version(), 1);
            assert_eq!(cassette.fixture_kind(), "libpod-cassette");
            assert_eq!(
                cassette.scenario_id(),
                format!("complex-{}-{}", case.version, mode_name(mode))
            );
            assert_eq!(cassette.scenario_revision(), 1);
            assert_eq!(cassette.engine_version(), case.version);
            assert_eq!(cassette.api_version(), case.version);
            assert_eq!(cassette.execution_context(), mode);
            assert!(cassette.synthetic());
            assert_eq!(
                cassette.provenance().evidence_kind(),
                "source-derived-synthetic-sanitized"
            );
            assert_eq!(cassette.provenance().release_tag(), format!("v{}", case.version));
            assert_eq!(cassette.provenance().revision(), case.revision);
            assert_eq!(cassette.provenance().source_urls().len(), 3);
            assert!(
                cassette
                    .provenance()
                    .source_urls()
                    .iter()
                    .all(|source| source.starts_with("https://"))
            );
            assert!(cassette.sanitization().contains("not a live export"));
            assert_eq!(cassette.interaction_count(), 30);

            let observed = replay(cassette).await?;
            assert_eq!(observed.service().engine_version().original(), case.version);
            assert_eq!(observed.service().api_version().original(), case.version);
        }
    }
    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn every_complex_inventory_has_exact_identities_and_typed_origins() -> Result<(), Box<dyn std::error::Error>> {
    let expected = [
        (
            ResourceKind::Container,
            &[
                "c-api",
                "c-backup",
                "c-db",
                "c-isolated",
                "c-metrics",
                "c-observer",
                "c-proxy",
                "c-worker",
            ][..],
        ),
        (ResourceKind::Pod, &["p-app", "p-edge"]),
        (ResourceKind::Network, &["n-app", "n-data", "n-edge", "n-isolated"]),
        (
            ResourceKind::Volume,
            &["database-data", "observer-cache", "shared-assets"],
        ),
        (ResourceKind::Image, &["sha256:api", "sha256:db", "sha256:proxy"]),
        (ResourceKind::Secret, &["s-db", "s-tls"]),
    ];

    for case in VERSIONS {
        for mode in MODES {
            let observed = inventory(case, mode).await?;
            for (kind, ids) in expected {
                let section = observed.section(kind).ok_or("missing inventory section")?;
                assert!(
                    section
                        .observations()
                        .iter()
                        .all(|observation| { observation.header().state() == ResourceObservationState::Complete })
                );
                let actual = section
                    .observations()
                    .iter()
                    .map(|observation| observation.header().identity().id())
                    .collect::<Vec<_>>();
                assert_eq!(actual, ids, "{} {} {kind:?}", case.version, mode_name(mode));
            }

            let ResourceDetails::Container(api) = observation(&observed, ResourceKind::Container, "c-api")?.details()
            else {
                return Err("c-api must decode as container".into());
            };
            assert_eq!(
                api.command().observed().map(podman_lens::ObservedValue::origin),
                Some(ObservationOrigin::Configured)
            );
            for origin in [
                api.restart_policy().observed().map(podman_lens::ObservedValue::origin),
                api.health_check().observed().map(podman_lens::ObservedValue::origin),
                api.startup_health_check()
                    .observed()
                    .map(podman_lens::ObservedValue::origin),
                api.logging().observed().map(podman_lens::ObservedValue::origin),
                api.security().observed().map(podman_lens::ObservedValue::origin),
                api.namespaces().observed().map(podman_lens::ObservedValue::origin),
                api.resource_controls()
                    .observed()
                    .map(podman_lens::ObservedValue::origin),
            ] {
                assert_eq!(origin, Some(ObservationOrigin::Effective));
            }
            assert_eq!(api.mounts().observed().map(|value| value.value().len()), Some(1));
            assert_eq!(api.secret_grants().observed().map(|value| value.value().len()), Some(1));

            let mut unmodelled = observation(&observed, ResourceKind::Container, "c-api")?
                .header()
                .unmodelled_fields()
                .iter()
                .map(podman_lens::UnmodelledField::path)
                .collect::<Vec<_>>();
            unmodelled.sort_unstable();
            assert_eq!(
                unmodelled,
                ["$.Config.Image", "$.Config.Secrets[0].Env"],
                "{} {} c-api unmodelled allowlist",
                case.version,
                mode_name(mode)
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn route_and_static_ip_observations_follow_exact_minor_boundaries() -> Result<(), Box<dyn std::error::Error>> {
    for case in VERSIONS {
        for mode in MODES {
            let observed = inventory(case, mode).await?;
            let network_observation = observation(&observed, ResourceKind::Network, "n-data")?;
            let ResourceDetails::Network(data) = network_observation.details() else {
                return Err("n-data must decode as network".into());
            };
            let routes = data.routes().observed().ok_or("n-data routes must be observed")?;
            assert_eq!(routes.origin(), ObservationOrigin::Effective);
            let route = routes.value().first().ok_or("n-data route must exist")?;
            if case.version.starts_with("6.") {
                assert!(matches!(
                    route.route_type(),
                    ObservationField::Observed(value)
                        if value.value() == &NativeNetworkRouteType::Unicast
                            && value.origin() == ObservationOrigin::Effective
                ));
                assert!(
                    !network_observation
                        .header()
                        .unmodelled_fields()
                        .iter()
                        .any(|field| field.path().starts_with("$.routes[0]")),
                    "{} {} route type must not remain stale metadata",
                    case.version,
                    mode_name(mode)
                );
            } else {
                assert!(matches!(route.route_type(), ObservationField::VersionInapplicable));
            }

            let ResourceDetails::Pod(application) = observation(&observed, ResourceKind::Pod, "p-app")?.details()
            else {
                return Err("p-app must decode as pod".into());
            };
            let networking = application
                .networking()
                .observed()
                .ok_or("p-app networking must be observed")?;
            match (case.version.starts_with("5."), mode) {
                (true, ExecutionContext::Rootful) => assert!(matches!(
                    networking.value().static_ip(),
                    ObservationField::Observed(value)
                        if value.value().to_string() == "10.42.0.20"
                            && value.origin() == ObservationOrigin::Effective
                )),
                (true, ExecutionContext::Rootless) => {
                    assert!(matches!(networking.value().static_ip(), ObservationField::Absent));
                }
                (false, _) => assert!(matches!(
                    networking.value().static_ip(),
                    ObservationField::VersionInapplicable
                )),
            }
            assert!(matches!(
                networking.value().static_mac(),
                ObservationField::VersionInapplicable
            ));
        }
    }
    Ok(())
}

#[tokio::test]
async fn exact_roots_cover_all_kinds_and_dependency_closure_preserves_pod_shared_prerequisites()
-> Result<(), Box<dyn std::error::Error>> {
    let roots = [
        (ResourceKind::Container, "c-api", "c-api"),
        (ResourceKind::Pod, "application", "p-app"),
        (ResourceKind::Network, "app-net", "n-app"),
        (ResourceKind::Volume, "database-data", "database-data"),
        (ResourceKind::Image, "sha256:api", "sha256:api"),
        (ResourceKind::Secret, "db-password", "s-db"),
    ];

    for case in VERSIONS {
        for mode in MODES {
            let observed = inventory(case, mode).await?;
            for (kind, reference, expected_id) in roots {
                let graph = discover(&observed, &exact_root(kind, reference)?)?;
                assert!(
                    graph.resolved_roots().iter().any(|root| root.id() == expected_id),
                    "{} {} {kind:?}",
                    case.version,
                    mode_name(mode)
                );
            }

            let proxy = discover(&observed, &exact_root(ResourceKind::Container, "c-proxy")?)?;
            let members = member_ids(&proxy);
            for expected_member in ["c-proxy", "c-metrics", "c-api", "c-worker", "c-db"] {
                assert!(members.contains(&expected_member), "missing {expected_member}");
            }
            assert!(proxy.groups().iter().any(|group| {
                let ids = group
                    .members()
                    .iter()
                    .map(podman_lens::ResourceIdentity::id)
                    .collect::<Vec<_>>();
                ids.contains(&"c-proxy") && ids.contains(&"c-metrics")
            }));
            for shared_id in ["n-app", "shared-assets", "sha256:proxy", "s-tls"] {
                assert!(
                    proxy
                        .groups()
                        .iter()
                        .flat_map(podman_lens::ResourceGroup::prerequisites)
                        .chain(proxy.shared_prerequisites().iter())
                        .any(|prerequisite| prerequisite.id() == shared_id),
                    "missing shared prerequisite {shared_id}"
                );
            }
            assert!(
                proxy
                    .groups()
                    .iter()
                    .flat_map(podman_lens::ResourceGroup::prerequisites)
                    .any(|prerequisite| prerequisite.id() == "shared-assets")
            );
            assert!(
                proxy
                    .groups()
                    .iter()
                    .flat_map(podman_lens::ResourceGroup::prerequisites)
                    .any(|prerequisite| prerequisite.id() == "s-tls")
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn label_presence_and_exact_value_roots_resolve_across_every_version_and_mode()
-> Result<(), Box<dyn std::error::Error>> {
    for case in VERSIONS {
        for mode in MODES {
            let observed = inventory(case, mode).await?;

            let mut presence = DiscoveryRequest::new();
            presence.add_label_root(LabelSelector::presence("org.example.complex.service")?);
            let presence = discover(&observed, &presence)?;
            assert_eq!(presence.requested_label_roots().len(), 1);
            assert_eq!(presence.resolved_roots().len(), 8);
            assert!(
                presence
                    .resolved_roots()
                    .iter()
                    .all(|root| root.kind() == ResourceKind::Container)
            );

            let mut exact = DiscoveryRequest::new();
            exact.add_label_root(LabelSelector::exact("org.example.complex.service", "api")?);
            let exact = discover(&observed, &exact)?;
            assert_eq!(exact.requested_label_roots().len(), 1);
            assert_eq!(exact.resolved_roots().len(), 1);
            assert_eq!(exact.resolved_roots()[0].id(), "c-api");
        }
    }
    Ok(())
}

#[tokio::test]
async fn shared_network_crossing_is_stopped_by_default_and_only_authorized_explicitly()
-> Result<(), Box<dyn std::error::Error>> {
    for case in VERSIONS {
        for mode in MODES {
            let observed = inventory(case, mode).await?;
            let default = discover(&observed, &exact_root(ResourceKind::Container, "c-observer")?)?;
            let default_members = member_ids(&default);
            assert!(default_members.contains(&"c-observer"));
            assert!(!default_members.contains(&"c-api"));
            assert!(!default_members.contains(&"c-isolated"));

            for boundary in ["app-net", "n-app"] {
                let mut request = exact_root(ResourceKind::Container, "c-observer")?;
                request.add_network_boundary_override(boundary)?;
                let crossed = discover(&observed, &request)?;
                let crossed_members = member_ids(&crossed);
                assert!(crossed_members.contains(&"c-api"));
                assert!(crossed_members.contains(&"c-proxy"));
                assert!(!crossed_members.contains(&"c-isolated"));
            }
        }
    }
    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Cross-version negative overlays intentionally remain one table-driven scenario.
async fn unavailable_malformed_and_ambiguous_overlays_remain_structured_for_every_cassette()
-> Result<(), Box<dyn std::error::Error>> {
    for case in VERSIONS {
        for mode in MODES {
            let container_path = format!("/v{}/libpod/containers/c-api/json", case.version);

            let mut unavailable = load_cassette(case, mode)?;
            unavailable
                .unique_interaction_mut(podman_lens::LibpodMethod::Get, &container_path)?
                .response_mut()
                .set_status(404);
            let unavailable = replay(unavailable).await?;
            let unavailable = observation(&unavailable, ResourceKind::Container, "c-api")?;
            assert_eq!(unavailable.header().state(), ResourceObservationState::Unavailable);
            assert!(
                unavailable
                    .header()
                    .findings()
                    .iter()
                    .any(|finding| { finding.code() == DiagnosticCode::ResourceUnavailable })
            );

            let mut malformed = load_cassette(case, mode)?;
            malformed
                .unique_interaction_mut(podman_lens::LibpodMethod::Get, &container_path)?
                .response_mut()
                .set_body(json!({
                    "Id": "c-api",
                    "Name": "api",
                    "Config": {"Env": ["not-an-assignment"]}
                }));
            let malformed = replay(malformed).await?;
            assert!(
                observation(&malformed, ResourceKind::Container, "c-api")?
                    .header()
                    .findings()
                    .iter()
                    .any(|finding| finding.code() == DiagnosticCode::EnvironmentMalformed)
            );

            let mut ambiguous = load_cassette(case, mode)?;
            ambiguous
                .unique_interaction_mut(
                    podman_lens::LibpodMethod::Get,
                    &format!("/v{}/libpod/images/sha256%3Adb/json", case.version),
                )?
                .response_mut()
                .set_body(json!({
                    "Id": "sha256:db",
                    "RepoTags": ["registry.example.invalid/complex/api:1"]
                }));
            let ambiguous = replay(ambiguous).await?;
            let graph = discover(
                &ambiguous,
                &exact_root(ResourceKind::Image, "registry.example.invalid/complex/api:1")?,
            )?;
            assert!(
                graph
                    .findings()
                    .iter()
                    .any(|finding| { finding.code() == DiagnosticCode::SelectorAmbiguous })
            );

            if case.version.starts_with("5.") {
                let network_path = format!("/v{}/libpod/networks/n-data/json", case.version);
                let mut version_inapplicable = load_cassette(case, mode)?;
                version_inapplicable
                    .unique_interaction_mut(podman_lens::LibpodMethod::Get, &network_path)?
                    .response_mut()
                    .set_body(json!({
                        "id": "n-data",
                        "name": "data-net",
                        "routes": [{
                            "destination": "198.51.100.0/24",
                            "gateway": "10.43.0.1",
                            "route_type": "blackhole"
                        }]
                    }));
                let version_inapplicable = replay(version_inapplicable).await?;
                let network = observation(&version_inapplicable, ResourceKind::Network, "n-data")?;
                assert!(network.header().findings().iter().any(|finding| {
                    finding.code() == DiagnosticCode::VersionInapplicableField
                        && finding.field_path() == Some("$.routes[0].route_type")
                }));
                let ResourceDetails::Network(network_details) = network.details() else {
                    return Err("n-data must decode as network".into());
                };
                let route = network_details
                    .routes()
                    .observed()
                    .ok_or("n-data routes must remain observed")?
                    .value()
                    .first()
                    .ok_or("n-data route must remain present")?;
                assert!(matches!(route.route_type(), ObservationField::VersionInapplicable));
                assert!(!serde_json::to_string(&v1::inventory(&version_inapplicable))?.contains("blackhole"));
            } else {
                let pod_path = format!("/v{}/libpod/pods/p-app/json", case.version);
                let mut version_inapplicable = load_cassette(case, mode)?;
                version_inapplicable
                    .unique_interaction_mut(podman_lens::LibpodMethod::Get, &pod_path)?
                    .response_mut()
                    .set_body(json!({
                        "Id": "p-app",
                        "Name": "application",
                        "CreateInfra": true,
                        "InfraConfig": {"StaticIP": "203.0.113.77"}
                    }));
                let version_inapplicable = replay(version_inapplicable).await?;
                let pod = observation(&version_inapplicable, ResourceKind::Pod, "p-app")?;
                assert!(pod.header().findings().iter().any(|finding| {
                    finding.code() == DiagnosticCode::VersionInapplicableField
                        && finding.field_path() == Some("$.InfraConfig.StaticIP")
                }));
                let ResourceDetails::Pod(pod_details) = pod.details() else {
                    return Err("p-app must decode as pod".into());
                };
                let networking = pod_details
                    .networking()
                    .observed()
                    .ok_or("p-app networking must remain observed")?;
                assert!(matches!(
                    networking.value().static_ip(),
                    ObservationField::VersionInapplicable
                ));
                assert!(!serde_json::to_string(&v1::inventory(&version_inapplicable))?.contains("203.0.113.77"));
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn select_all_and_selector_permutations_are_byte_deterministic() -> Result<(), Box<dyn std::error::Error>> {
    for case in VERSIONS {
        for mode in MODES {
            let original = inventory(case, mode).await?;

            let mut cassette = load_cassette(case, mode)?;
            cassette
                .unique_interaction_mut(
                    podman_lens::LibpodMethod::Get,
                    &format!("/v{}/libpod/containers/json?all=true&sync=true", case.version),
                )?
                .response_mut()
                .set_body(json!([
                    {"Id":"c-isolated","Names":["isolated-control"]},
                    {"Id":"c-backup","Names":["backup"]},
                    {"Id":"c-metrics","Names":["metrics"]},
                    {"Id":"c-observer","Names":["observer"]},
                    {"Id":"c-db","Names":["database"]},
                    {"Id":"c-proxy","Names":["proxy"]},
                    {"Id":"c-api","Names":["api"]},
                    {"Id":"c-worker","Names":["worker"]}
                ]));
            let permuted = replay(cassette).await?;

            let mut all = DiscoveryRequest::new();
            all.select_all();
            let original_all = serde_json::to_vec(&v1::graph(&discover(&original, &all)?))?;
            let permuted_all = serde_json::to_vec(&v1::graph(&discover(&permuted, &all)?))?;
            assert_eq!(original_all, permuted_all);

            let mut first = exact_root(ResourceKind::Container, "c-api")?;
            first.add_root(ResourceSelector::exact(ResourceKind::Container, "c-observer")?);
            let mut second = exact_root(ResourceKind::Container, "c-observer")?;
            second.add_root(ResourceSelector::exact(ResourceKind::Container, "c-api")?);
            assert_eq!(
                serde_json::to_vec(&v1::graph(&discover(&original, &first)?))?,
                serde_json::to_vec(&v1::graph(&discover(&original, &second)?))?
            );
        }
    }
    Ok(())
}

fn deployment_id_from_native(
    inventory: &ResourceInventory,
    kind: ResourceKind,
    native_id: &str,
) -> Result<DeploymentResourceId, Box<dyn std::error::Error>> {
    let identity = observation(inventory, kind, native_id)?.header().identity();
    Ok(DeploymentResourceId::new(
        kind,
        identity.name().unwrap_or(identity.id()),
    )?)
}

fn configured_container_intent(
    inventory: &ResourceInventory,
    native_id: &str,
    image: &DeploymentResourceId,
    expected_pod: Option<(&str, &DeploymentResourceId)>,
    authored_networks: &[&DeploymentResourceId],
    authored_mount: Option<(&DeploymentResourceId, &str, MountAccess)>,
) -> Result<ContainerIntent, Box<dyn std::error::Error>> {
    let source = observation(inventory, ResourceKind::Container, native_id)?;
    let ResourceDetails::Container(container) = source.details() else {
        return Err(format!("{native_id} must remain a container").into());
    };
    let configured_image = container
        .configured_image()
        .observed()
        .ok_or_else(|| io::Error::other(format!("{native_id} configured image must be observed")))?;
    assert_eq!(configured_image.origin(), ObservationOrigin::Configured);
    assert_eq!(configured_image.value(), image.name());

    let mut intent = ContainerIntent::new(
        DeploymentResourceId::new(
            ResourceKind::Container,
            source
                .header()
                .identity()
                .name()
                .ok_or_else(|| io::Error::other(format!("{native_id} must have a name")))?,
        )?,
        image.clone(),
    )?;
    match expected_pod {
        Some((native_pod, target_pod)) => {
            let membership = container
                .pod_membership()
                .observed()
                .ok_or_else(|| io::Error::other(format!("{native_id} pod membership must be configured")))?;
            assert_eq!(membership.origin(), ObservationOrigin::Configured);
            assert_eq!(membership.value().reference(), native_pod);
            intent.set_pod(target_pod.clone())?;
        }
        None => assert!(matches!(container.pod_membership(), ObservationField::Absent)),
    }

    // Attachments and mount access/copy behavior are explicit target authoring
    // policy. They agree with this synthetic scenario but are never converted
    // from effective inspect fields.
    for network in authored_networks {
        intent.add_network(NetworkAttachment::new((*network).clone())?)?;
    }
    if let Some((volume, destination, access)) = authored_mount {
        intent.add_mount(NamedVolumeMount::new(
            volume.clone(),
            AbsoluteContainerPath::new(destination)?,
            access,
            NamedVolumeCopyMode::Copy,
        )?);
    }

    let command = container
        .command()
        .observed()
        .ok_or_else(|| io::Error::other(format!("{native_id} configured command must be observed")))?;
    assert_eq!(command.origin(), ObservationOrigin::Configured);
    intent
        .settings_mut()
        .set_command(ArgumentArray::new(command.value().arguments().to_vec())?)?;
    Ok(intent)
}

fn assert_configured_dependencies(
    inventory: &ResourceInventory,
    native_id: &str,
    expected: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let ResourceDetails::Container(container) = observation(inventory, ResourceKind::Container, native_id)?.details()
    else {
        return Err(format!("{native_id} must remain a container").into());
    };
    if let Some(dependencies) = container.native_dependencies().observed() {
        assert_eq!(dependencies.origin(), ObservationOrigin::Configured);
        assert_eq!(
            dependencies
                .value()
                .iter()
                .map(podman_lens::NativeResourceReference::reference)
                .collect::<Vec<_>>(),
            expected
        );
    } else {
        assert!(expected.is_empty());
        assert!(matches!(container.native_dependencies(), ObservationField::Absent));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)] // The full authored topology is deliberately visible in one reviewable helper.
fn reviewed_complex_intent(
    inventory: &ResourceInventory,
    case: VersionCase,
    mode: ExecutionContext,
) -> Result<DeploymentIntent, Box<dyn std::error::Error>> {
    let mut target = TargetProfile::new(
        ObservedPodmanVersion::parse(case.version)?,
        ObservedApiVersion::parse(case.version)?,
    )?;
    target.set_execution_context(target_context(mode));
    let mut intent = DeploymentIntent::new(target);

    let app_network = deployment_id_from_native(inventory, ResourceKind::Network, "n-app")?;
    let data_network = deployment_id_from_native(inventory, ResourceKind::Network, "n-data")?;
    let edge_network = deployment_id_from_native(inventory, ResourceKind::Network, "n-edge")?;
    let isolated_network = deployment_id_from_native(inventory, ResourceKind::Network, "n-isolated")?;
    for network in [&app_network, &data_network, &edge_network, &isolated_network] {
        intent.add_resource(DeploymentResource::Network(NetworkIntent::new(network.clone())?));
    }

    let database_data = deployment_id_from_native(inventory, ResourceKind::Volume, "database-data")?;
    let observer_cache = deployment_id_from_native(inventory, ResourceKind::Volume, "observer-cache")?;
    let shared_assets = deployment_id_from_native(inventory, ResourceKind::Volume, "shared-assets")?;
    for volume in [&database_data, &observer_cache, &shared_assets] {
        intent.add_resource(DeploymentResource::Volume(VolumeIntent::new(volume.clone())?));
    }

    let api_image = DeploymentResourceId::new(ResourceKind::Image, "registry.example.invalid/complex/api:1")?;
    let database_image = DeploymentResourceId::new(ResourceKind::Image, "registry.example.invalid/complex/db:1")?;
    let proxy_image = DeploymentResourceId::new(ResourceKind::Image, "registry.example.invalid/complex/proxy:1")?;
    for image in [&api_image, &database_image, &proxy_image] {
        if matches!(case.version, "5.4.0" | "5.5.0") {
            intent.add_resource(DeploymentResource::ExternalPrecondition(ExternalPrecondition::new(
                image.clone(),
            )?));
        } else {
            intent.add_resource(DeploymentResource::Image(ImageIntent::new(
                image.clone(),
                ImageSource::new(image.name())?,
                ImagePullPolicy::Missing,
            )?));
        }
    }

    // Inspect cannot reconstruct secret delivery form or target. Preserve both
    // discovered secrets only as explicit external prerequisites.
    for native_secret in ["s-db", "s-tls"] {
        intent.add_resource(DeploymentResource::ExternalPrecondition(ExternalPrecondition::new(
            deployment_id_from_native(inventory, ResourceKind::Secret, native_secret)?,
        )?));
    }

    let application = deployment_id_from_native(inventory, ResourceKind::Pod, "p-app")?;
    let edge = deployment_id_from_native(inventory, ResourceKind::Pod, "p-edge")?;
    let api = deployment_id_from_native(inventory, ResourceKind::Container, "c-api")?;
    let worker = deployment_id_from_native(inventory, ResourceKind::Container, "c-worker")?;
    let proxy = deployment_id_from_native(inventory, ResourceKind::Container, "c-proxy")?;
    let metrics = deployment_id_from_native(inventory, ResourceKind::Container, "c-metrics")?;
    let database = deployment_id_from_native(inventory, ResourceKind::Container, "c-db")?;
    let backup = deployment_id_from_native(inventory, ResourceKind::Container, "c-backup")?;

    let mut application_pod = PodIntent::new(application.clone())?;
    application_pod.add_network(NetworkAttachment::new(app_network.clone())?)?;
    application_pod.add_network(NetworkAttachment::new(data_network.clone())?)?;
    application_pod.add_member(api.clone())?;
    application_pod.add_member(worker.clone())?;
    intent.add_resource(DeploymentResource::Pod(application_pod));

    let mut edge_pod = PodIntent::new(edge.clone())?;
    edge_pod.add_network(NetworkAttachment::new(app_network.clone())?)?;
    edge_pod.add_network(NetworkAttachment::new(edge_network.clone())?)?;
    edge_pod.add_member(proxy.clone())?;
    edge_pod.add_member(metrics.clone())?;
    intent.add_resource(DeploymentResource::Pod(edge_pod));

    for container in [
        configured_container_intent(
            inventory,
            "c-api",
            &api_image,
            Some(("p-app", &application)),
            &[],
            Some((&shared_assets, "/srv/assets", MountAccess::ReadWrite)),
        )?,
        configured_container_intent(
            inventory,
            "c-worker",
            &api_image,
            Some(("p-app", &application)),
            &[],
            Some((&shared_assets, "/srv/assets", MountAccess::ReadOnly)),
        )?,
        configured_container_intent(
            inventory,
            "c-proxy",
            &proxy_image,
            Some(("p-edge", &edge)),
            &[],
            Some((&shared_assets, "/srv/assets", MountAccess::ReadOnly)),
        )?,
        configured_container_intent(inventory, "c-metrics", &proxy_image, Some(("p-edge", &edge)), &[], None)?,
        configured_container_intent(
            inventory,
            "c-db",
            &database_image,
            None,
            &[&data_network],
            Some((&database_data, "/var/lib/database", MountAccess::ReadWrite)),
        )?,
        configured_container_intent(
            inventory,
            "c-backup",
            &database_image,
            None,
            &[&data_network],
            Some((&database_data, "/var/lib/database", MountAccess::ReadOnly)),
        )?,
        configured_container_intent(
            inventory,
            "c-observer",
            &proxy_image,
            None,
            &[&app_network],
            Some((&observer_cache, "/var/cache/observer", MountAccess::ReadWrite)),
        )?,
        configured_container_intent(inventory, "c-isolated", &proxy_image, None, &[&isolated_network], None)?,
    ] {
        intent.add_resource(DeploymentResource::Container(container));
    }

    assert_configured_dependencies(inventory, "c-api", &["c-db"])?;
    assert_configured_dependencies(inventory, "c-backup", &["c-db"])?;
    assert_configured_dependencies(inventory, "c-proxy", &["c-api"])?;
    assert_configured_dependencies(inventory, "c-worker", &["c-api"])?;
    for native_id in ["c-db", "c-isolated", "c-metrics", "c-observer"] {
        assert_configured_dependencies(inventory, native_id, &[])?;
    }

    intent.add_startup_dependency(StartupDependency::new(database.clone(), api.clone())?);
    intent.add_startup_dependency(StartupDependency::new(database, backup)?);
    intent.add_startup_dependency(StartupDependency::new(api, proxy)?);
    // c-worker depends on c-api in the source, but both are members of the
    // application pod. Their start anchors are identical, so no fabricated
    // same-pod order is added to the target intent.
    Ok(intent)
}

type OperationSpec<'a> = (SemanticOperationAction, ResourceKind, &'a str);

fn operation_label(spec: OperationSpec<'_>) -> String {
    format!("{:?}/{:?}/{}", spec.0, spec.1, spec.2)
}

fn insert_expected_operation(
    expected: &mut BTreeMap<String, Vec<String>>,
    operation: OperationSpec<'_>,
    prerequisites: &[OperationSpec<'_>],
) {
    let mut prerequisites = prerequisites.iter().copied().map(operation_label).collect::<Vec<_>>();
    prerequisites.sort_unstable();
    assert!(expected.insert(operation_label(operation), prerequisites).is_none());
}

#[allow(clippy::too_many_lines)] // The full expected graph is intentionally visible as one audited contract.
fn assert_exact_plan_dependencies(plan: &podman_lens::DeploymentPlan, managed_images: bool) {
    use ResourceKind::{Container, Image, Network, Pod, Volume};
    use SemanticOperationAction::{Create, EnsureImage, StartContainer, StartPod};

    let mut expected = BTreeMap::new();
    for operation in [
        (Create, Network, "app-net"),
        (Create, Network, "data-net"),
        (Create, Network, "edge-net"),
        (Create, Network, "isolated-net"),
        (Create, Volume, "database-data"),
        (Create, Volume, "observer-cache"),
        (Create, Volume, "shared-assets"),
    ] {
        insert_expected_operation(&mut expected, operation, &[]);
    }
    if managed_images {
        for operation in [
            (EnsureImage, Image, "registry.example.invalid/complex/api:1"),
            (EnsureImage, Image, "registry.example.invalid/complex/db:1"),
            (EnsureImage, Image, "registry.example.invalid/complex/proxy:1"),
        ] {
            insert_expected_operation(&mut expected, operation, &[]);
        }
    }

    insert_expected_operation(
        &mut expected,
        (Create, Pod, "application"),
        &[(Create, Network, "app-net"), (Create, Network, "data-net")],
    );
    insert_expected_operation(
        &mut expected,
        (Create, Pod, "edge"),
        &[(Create, Network, "app-net"), (Create, Network, "edge-net")],
    );

    let mut api_dependencies = vec![(Create, Pod, "application"), (Create, Volume, "shared-assets")];
    let mut worker_dependencies = api_dependencies.clone();
    let mut proxy_dependencies = vec![(Create, Pod, "edge"), (Create, Volume, "shared-assets")];
    let mut metrics_dependencies = vec![(Create, Pod, "edge")];
    let mut database_dependencies = vec![(Create, Network, "data-net"), (Create, Volume, "database-data")];
    let mut backup_dependencies = database_dependencies.clone();
    let mut observer_dependencies = vec![(Create, Network, "app-net"), (Create, Volume, "observer-cache")];
    let mut isolated_dependencies = vec![(Create, Network, "isolated-net")];
    if managed_images {
        api_dependencies.push((EnsureImage, Image, "registry.example.invalid/complex/api:1"));
        worker_dependencies.push((EnsureImage, Image, "registry.example.invalid/complex/api:1"));
        proxy_dependencies.push((EnsureImage, Image, "registry.example.invalid/complex/proxy:1"));
        metrics_dependencies.push((EnsureImage, Image, "registry.example.invalid/complex/proxy:1"));
        database_dependencies.push((EnsureImage, Image, "registry.example.invalid/complex/db:1"));
        backup_dependencies.push((EnsureImage, Image, "registry.example.invalid/complex/db:1"));
        observer_dependencies.push((EnsureImage, Image, "registry.example.invalid/complex/proxy:1"));
        isolated_dependencies.push((EnsureImage, Image, "registry.example.invalid/complex/proxy:1"));
    }
    for (name, prerequisites) in [
        ("api", api_dependencies),
        ("worker", worker_dependencies),
        ("proxy", proxy_dependencies),
        ("metrics", metrics_dependencies),
        ("database", database_dependencies),
        ("backup", backup_dependencies),
        ("observer", observer_dependencies),
        ("isolated-control", isolated_dependencies),
    ] {
        insert_expected_operation(&mut expected, (Create, Container, name), &prerequisites);
    }

    insert_expected_operation(
        &mut expected,
        (StartPod, Pod, "application"),
        &[
            (Create, Pod, "application"),
            (Create, Container, "api"),
            (Create, Container, "worker"),
            (StartContainer, Container, "database"),
        ],
    );
    insert_expected_operation(
        &mut expected,
        (StartPod, Pod, "edge"),
        &[
            (Create, Pod, "edge"),
            (Create, Container, "metrics"),
            (Create, Container, "proxy"),
            (StartPod, Pod, "application"),
        ],
    );
    for (name, prerequisites) in [
        ("database", vec![(Create, Container, "database")]),
        (
            "backup",
            vec![(Create, Container, "backup"), (StartContainer, Container, "database")],
        ),
        ("observer", vec![(Create, Container, "observer")]),
        ("isolated-control", vec![(Create, Container, "isolated-control")]),
    ] {
        insert_expected_operation(&mut expected, (StartContainer, Container, name), &prerequisites);
    }

    let actual = plan
        .operations()
        .iter()
        .map(|operation| {
            let id = operation.id();
            let mut prerequisites = operation
                .depends_on()
                .iter()
                .map(|dependency| {
                    operation_label((
                        dependency.action(),
                        dependency.resource().kind(),
                        dependency.resource().name(),
                    ))
                })
                .collect::<Vec<_>>();
            prerequisites.sort_unstable();
            (
                operation_label((id.action(), id.resource().kind(), id.resource().name())),
                prerequisites,
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual, expected,
        "the complete operation dependency graph must remain exact"
    );
}

fn rendered_body<'a>(
    rendering: &'a podman_lens::DeploymentRendering,
    action: SemanticOperationAction,
    kind: ResourceKind,
    name: &str,
) -> Result<&'a Value, io::Error> {
    let operation = rendering
        .operations()
        .iter()
        .find(|operation| {
            operation.operation().id().action() == action
                && operation.operation().id().resource().kind() == kind
                && operation.operation().id().resource().name() == name
        })
        .ok_or_else(|| io::Error::other(format!("missing rendered {action:?} {kind:?} operation {name}")))?;
    let RenderedHttpBody::Json(body) = operation.libpod().body() else {
        return Err(io::Error::other(format!(
            "rendered {action:?} {kind:?} operation {name} must have JSON body"
        )));
    };
    Ok(body)
}

fn assert_network_names(body: &Value, expected: &[&str]) -> Result<(), io::Error> {
    let mut actual = body
        .get("Networks")
        .and_then(Value::as_object)
        .ok_or_else(|| io::Error::other("native Networks object must be present"))?
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
    assert!(body.get("networks").is_none());
    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One table-driven test audits the same complete contract across all 14 contexts.
async fn redaction_and_reviewed_full_topology_plan_render_hold_for_every_version_and_mode()
-> Result<(), Box<dyn std::error::Error>> {
    for case in VERSIONS {
        for mode in MODES {
            let observed = inventory(case, mode).await?;
            let debug = format!("{observed:?}");
            let snapshot = serde_json::to_string(&v1::inventory(&observed))?;
            for protected in [
                "COMPLEX_PROTECTED_VALUE_NEVER_PRINT",
                "COMPLEX_DB_PROTECTED_VALUE",
                "COMPLEX_IMAGE_PROTECTED_VALUE",
                "COMPLEX_NORMAL_HEALTH_SENTINEL",
                "COMPLEX_STARTUP_HEALTH_SENTINEL",
            ] {
                assert!(!debug.contains(protected));
                assert!(!snapshot.contains(protected));
            }

            let graph = discover(&observed, &exact_root(ResourceKind::Container, "c-api")?)?;
            assert_eq!(graph.resolved_roots()[0].id(), "c-api");
            let graph_snapshot = serde_json::to_string(&v1::graph(&graph))?;
            for protected in [
                "COMPLEX_PROTECTED_VALUE_NEVER_PRINT",
                "COMPLEX_DB_PROTECTED_VALUE",
                "COMPLEX_IMAGE_PROTECTED_VALUE",
                "COMPLEX_NORMAL_HEALTH_SENTINEL",
                "COMPLEX_STARTUP_HEALTH_SENTINEL",
            ] {
                assert!(!graph_snapshot.contains(protected));
            }
            assert!(graph.dependencies().iter().any(|dependency| {
                dependency.dependent().id() == "c-api" && dependency.prerequisite().id() == "sha256:api"
            }));

            let intent = reviewed_complex_intent(&observed, case, mode)?;
            let planning = plan_deployment(&intent);
            assert_eq!(planning, plan_deployment(&intent));
            assert!(planning.findings().is_empty());
            let plan = planning.plan().ok_or("matching target must produce a plan")?;
            let managed_images = !matches!(case.version, "5.4.0" | "5.5.0");
            assert_eq!(plan.operations().len(), if managed_images { 26 } else { 23 });
            assert_eq!(plan.external_preconditions().len(), if managed_images { 2 } else { 5 });

            assert_exact_plan_dependencies(plan, managed_images);
            assert!(
                plan.operations().iter().all(|operation| {
                    !(operation.id().action() == SemanticOperationAction::StartContainer
                        && matches!(operation.id().resource().name(), "api" | "worker" | "proxy" | "metrics"))
                }),
                "pod members must start only through their pod anchors"
            );

            let rendering = render_deployment(plan);
            let repeated_rendering = render_deployment(plan);
            assert_eq!(rendering, repeated_rendering);
            assert!(rendering.findings().is_empty());
            let rendering = rendering.rendering().ok_or("matching target must render")?;
            assert_eq!(rendering.status(), RenderStatus::Exact);
            assert_eq!(rendering.operations().len(), plan.operations().len());
            for (operation, rendered) in plan.operations().iter().zip(rendering.operations()) {
                assert_eq!(rendered.operation().id(), operation.id());
            }
            assert!(
                rendering
                    .operations()
                    .iter()
                    .all(|operation| { operation.cli().program() == "podman" && !operation.cli().argv().is_empty() })
            );
            assert!(rendering.operations().iter().all(|operation| {
                operation.libpod().method() == RenderedHttpMethod::Post
                    && operation
                        .libpod()
                        .path_and_query()
                        .starts_with(&format!("/v{}/libpod/", case.version))
            }));

            let application_body = rendered_body(
                rendering,
                SemanticOperationAction::Create,
                ResourceKind::Pod,
                "application",
            )?;
            assert_network_names(application_body, &["app-net", "data-net"])?;
            let edge_body = rendered_body(rendering, SemanticOperationAction::Create, ResourceKind::Pod, "edge")?;
            assert_network_names(edge_body, &["app-net", "edge-net"])?;

            let api_body = rendered_body(
                rendering,
                SemanticOperationAction::Create,
                ResourceKind::Container,
                "api",
            )?;
            assert_eq!(api_body.get("pod").and_then(Value::as_str), Some("application"));
            assert!(api_body.get("Networks").is_none());
            let database_body = rendered_body(
                rendering,
                SemanticOperationAction::Create,
                ResourceKind::Container,
                "database",
            )?;
            assert_network_names(database_body, &["data-net"])?;
            let observer_body = rendered_body(
                rendering,
                SemanticOperationAction::Create,
                ResourceKind::Container,
                "observer",
            )?;
            assert_network_names(observer_body, &["app-net"])?;
            let isolated_body = rendered_body(
                rendering,
                SemanticOperationAction::Create,
                ResourceKind::Container,
                "isolated-control",
            )?;
            assert_network_names(isolated_body, &["isolated-net"])?;

            for container_name in [
                "api",
                "worker",
                "proxy",
                "metrics",
                "database",
                "backup",
                "observer",
                "isolated-control",
            ] {
                let body = rendered_body(
                    rendering,
                    SemanticOperationAction::Create,
                    ResourceKind::Container,
                    container_name,
                )?;
                let body = body
                    .as_object()
                    .ok_or_else(|| io::Error::other("container create body must be an object"))?;
                for non_authored in [
                    "healthconfig",
                    "startupHealthConfig",
                    "health_check_on_failure_action",
                    "log_configuration",
                    "privileged",
                    "cap_add",
                    "cap_drop",
                    "resource_limits",
                    "r_limits",
                    "restart_policy",
                    "secrets",
                    "secret_env",
                    "env",
                    "hostname",
                    "networks",
                ] {
                    assert!(
                        !body.contains_key(non_authored),
                        "{case_version} {mode_name} {container_name} unexpectedly authored {non_authored}",
                        case_version = case.version,
                        mode_name = mode_name(mode)
                    );
                }
            }
            for network_name in ["app-net", "data-net", "edge-net", "isolated-net"] {
                let body = rendered_body(
                    rendering,
                    SemanticOperationAction::Create,
                    ResourceKind::Network,
                    network_name,
                )?;
                for non_authored in ["subnets", "routes", "internal", "options"] {
                    assert!(body.get(non_authored).is_none());
                }
            }
            for volume_name in ["database-data", "observer-cache", "shared-assets"] {
                let body = rendered_body(
                    rendering,
                    SemanticOperationAction::Create,
                    ResourceKind::Volume,
                    volume_name,
                )?;
                assert!(body.get("UID").is_none());
                assert!(body.get("GID").is_none());
            }
            assert!(plan.operations().iter().all(|operation| {
                !(operation.id().action() == SemanticOperationAction::Create
                    && operation.id().resource().kind() == ResourceKind::Secret)
            }));
            assert!(
                rendering
                    .operations()
                    .iter()
                    .all(|operation| { !operation.cli().argv().iter().any(|argument| argument == "--secret") })
            );
            let rendered_debug = format!("{rendering:?}");
            for non_promoted in [
                "COMPLEX_PROTECTED_VALUE_NEVER_PRINT",
                "COMPLEX_DB_PROTECTED_VALUE",
                "COMPLEX_IMAGE_PROTECTED_VALUE",
                "COMPLEX_NORMAL_HEALTH_SENTINEL",
                "COMPLEX_STARTUP_HEALTH_SENTINEL",
                "/synthetic/config/proxy",
                "nodev",
                "nosuid",
            ] {
                assert!(!rendered_debug.contains(non_promoted));
            }
        }
    }
    Ok(())
}
