//! Version 1 redacted snapshots for acquired Podman inventory and discovery graphs.
//!
//! These DTOs intentionally implement only [`serde::Serialize`]. They are stable export views,
//! not an input format. Constructors copy safe identity and evidence metadata while omitting
//! environment values, secret material, connection details, raw unknown JSON, label values,
//! driver-option values, and Compose ownership values.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    DependencyEvidence, DiscoveryExplanationKind, DiscoveryRootOrigin, GroupingEvidence, JsonValueKind,
    ObservationField, ResourceDetails, ResourceGraph, ResourceIdentity, ResourceInventory, ResourceKind,
    ResourceObservation, UnmodelledCompleteness,
};

/// The schema version emitted by every version 1 snapshot.
pub const SCHEMA_VERSION: u8 = 1;

/// Returns an always-redacted, serialization-only snapshot of an acquired inventory.
#[must_use]
pub fn inventory(source: &ResourceInventory) -> InventorySnapshot {
    InventorySnapshot::from_inventory(source)
}

/// Returns an always-redacted, serialization-only snapshot of a discovery graph.
#[must_use]
pub fn graph(source: &ResourceGraph) -> GraphSnapshot {
    GraphSnapshot::from_graph(source)
}

/// A versioned, always-redacted inventory export.
#[derive(Debug, Serialize)]
pub struct InventorySnapshot {
    schema_version: u8,
    service: ServiceSnapshot,
    sections: Vec<InventorySectionSnapshot>,
}

impl InventorySnapshot {
    /// Builds an always-redacted snapshot from an acquired inventory.
    #[must_use]
    pub fn from_inventory(source: &ResourceInventory) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            service: ServiceSnapshot {
                engine: source.service().engine_version().original().to_owned(),
                api: source.service().api_version().original().to_owned(),
                target_podman: source.service().target_profile().podman_version().original().to_owned(),
                target_api: source.service().target_profile().api_version().original().to_owned(),
            },
            sections: source
                .sections()
                .iter()
                .map(|section| InventorySectionSnapshot {
                    kind: resource_kind(section.kind()),
                    availability: match section.availability() {
                        crate::InventorySectionAvailability::Available => "available",
                        crate::InventorySectionAvailability::Unavailable => "unavailable",
                        crate::InventorySectionAvailability::Malformed => "malformed",
                    },
                    findings: section.findings().iter().map(inventory_finding).collect(),
                    observations: section.observations().iter().map(observation).collect(),
                })
                .collect(),
        }
    }

    /// Returns the emitted snapshot schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u8 {
        self.schema_version
    }
}

/// A versioned, always-redacted discovery-graph export.
#[derive(Debug, Serialize)]
pub struct GraphSnapshot {
    schema_version: u8,
    requested_roots: Vec<ResourceSelectorSnapshot>,
    requested_label_roots: Vec<LabelSelectorSnapshot>,
    all_requested: bool,
    resolved_roots: Vec<ResourceIdentitySnapshot>,
    groups: Vec<ResourceGroupSnapshot>,
    shared_prerequisites: Vec<ResourceIdentitySnapshot>,
    dependencies: Vec<ResourceDependencySnapshot>,
    grouping_edges: Vec<GroupingEdgeSnapshot>,
    findings: Vec<DiscoveryFindingSnapshot>,
    explanations: Vec<DiscoveryExplanationSnapshot>,
}

impl GraphSnapshot {
    /// Builds an always-redacted snapshot from a discovery graph.
    #[must_use]
    pub fn from_graph(source: &ResourceGraph) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            requested_roots: source
                .requested_roots()
                .iter()
                .map(|selector| ResourceSelectorSnapshot {
                    kind: resource_kind(selector.kind()),
                    reference: selector.reference().to_owned(),
                })
                .collect(),
            requested_label_roots: source.requested_label_roots().iter().map(label_selector).collect(),
            all_requested: source.all_requested(),
            resolved_roots: source.resolved_roots().iter().map(identity).collect(),
            groups: source
                .groups()
                .iter()
                .map(|group| ResourceGroupSnapshot {
                    id: identity(group.id()),
                    members: group.members().iter().map(identity).collect(),
                    prerequisites: group.prerequisites().iter().map(identity).collect(),
                })
                .collect(),
            shared_prerequisites: source.shared_prerequisites().iter().map(identity).collect(),
            dependencies: source
                .dependencies()
                .iter()
                .map(|dependency| ResourceDependencySnapshot {
                    dependent: identity(dependency.dependent()),
                    prerequisite: identity(dependency.prerequisite()),
                    evidence: match dependency.evidence() {
                        DependencyEvidence::NativeRelationship { field_paths } => DependencyEvidenceSnapshot {
                            kind: "native_relationship",
                            field_paths: field_paths.clone(),
                        },
                    },
                })
                .collect(),
            grouping_edges: source
                .grouping_edges()
                .iter()
                .map(|edge| GroupingEdgeSnapshot {
                    left: identity(edge.left()),
                    right: identity(edge.right()),
                    evidence: match edge.evidence() {
                        GroupingEvidence::PodMembership => "pod_membership",
                        GroupingEvidence::ContainerDependency => "container_dependency",
                        GroupingEvidence::ComposeOwnership { .. } => "compose_ownership",
                    },
                    field_paths: edge.field_paths().to_vec(),
                })
                .collect(),
            findings: source
                .findings()
                .iter()
                .map(|finding| DiscoveryFindingSnapshot {
                    code: finding.code().as_str(),
                    resource: finding.resource_identity().map(identity),
                    selector: finding.selector().map(|selector| ResourceSelectorSnapshot {
                        kind: resource_kind(selector.kind()),
                        reference: selector.reference().to_owned(),
                    }),
                    label_selector: finding.label_selector().map(label_selector),
                    field_path: finding.field_path().map(str::to_owned),
                })
                .collect(),
            explanations: source
                .explanations()
                .iter()
                .map(|explanation| DiscoveryExplanationSnapshot {
                    kind: explanation_kind(explanation.kind()),
                    resource: identity(explanation.resource()),
                    related: explanation.related().map(identity),
                    position: explanation.position(),
                    root_origin: explanation.root_origin().map(root_origin),
                })
                .collect(),
        }
    }

    /// Returns the emitted snapshot schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u8 {
        self.schema_version
    }
}

#[derive(Debug, Serialize)]
struct ServiceSnapshot {
    engine: String,
    api: String,
    target_podman: String,
    target_api: String,
}

#[derive(Debug, Serialize)]
struct InventorySectionSnapshot {
    kind: &'static str,
    availability: &'static str,
    findings: Vec<InventoryFindingSnapshot>,
    observations: Vec<ResourceObservationSnapshot>,
}

#[derive(Debug, Serialize)]
struct ResourceObservationSnapshot {
    header: ObservationHeaderSnapshot,
    details: ResourceDetailsSnapshot,
}

#[derive(Debug, Serialize)]
struct ObservationHeaderSnapshot {
    identity: ResourceIdentitySnapshot,
    state: &'static str,
    unmodelled_completeness: &'static str,
    unmodelled_fields: Vec<UnmodelledFieldSnapshot>,
    findings: Vec<InventoryFindingSnapshot>,
    evidence: ResourceEvidenceSnapshot,
}

#[derive(Debug, Serialize)]
struct ResourceIdentitySnapshot {
    kind: &'static str,
    id: String,
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct UnmodelledFieldSnapshot {
    id: String,
    path: String,
    json_kind: &'static str,
    resource: ResourceIdentitySnapshot,
    evidence: ResourceEvidenceSnapshot,
}

#[derive(Debug, Serialize)]
struct ResourceDetailsSnapshot {
    kind: &'static str,
    labels: FieldCountSnapshot,
    configured_image: Option<FieldStateSnapshot>,
    local_image_id: Option<FieldStateSnapshot>,
    relationships: Option<FieldCountSnapshot>,
    environment: Option<ProtectedEnvironmentSnapshot>,
    image_aliases: Option<FieldCountSnapshot>,
    network: Option<NetworkSnapshot>,
    networking: Option<NativeNetworkingSnapshot>,
    memory_swappiness: Option<FieldStateSnapshot>,
    infra: Option<FieldStateSnapshot>,
    restart_policy: Option<NativeRestartPolicySnapshot>,
    health_check: Option<NativeHealthCheckSnapshot>,
    health_failure_action: Option<FieldStateSnapshot>,
    startup_health_check: Option<NativeStartupHealthCheckSnapshot>,
    logging: Option<NativeLoggingSnapshot>,
    create_infra: Option<FieldStateSnapshot>,
    secret_driver: Option<FieldStateSnapshot>,
    volume_uid: Option<VolumeOwnerFieldSnapshot>,
    volume_gid: Option<VolumeOwnerFieldSnapshot>,
    command: Option<FieldCountSnapshot>,
    entrypoint: Option<FieldCountSnapshot>,
    user: Option<FieldStateSnapshot>,
    working_directory: Option<FieldStateSnapshot>,
    hostname: Option<FieldStateSnapshot>,
    pod_membership: Option<FieldStateSnapshot>,
    native_dependencies: Option<FieldCountSnapshot>,
    mounts: Option<FieldCountSnapshot>,
    secret_grants: Option<FieldCountSnapshot>,
}

#[derive(Clone, Debug, Serialize)]
struct FieldStateSnapshot {
    state: &'static str,
    origin: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct FieldCountSnapshot {
    #[serde(flatten)]
    field: FieldStateSnapshot,
    count: usize,
}

#[derive(Debug, Serialize)]
struct ProtectedEnvironmentSnapshot {
    #[serde(flatten)]
    field: FieldStateSnapshot,
    entries: Vec<EnvironmentEntrySnapshot>,
}

#[derive(Debug, Serialize)]
struct EnvironmentEntrySnapshot {
    name: String,
    value_state: &'static str,
}

#[derive(Debug, Serialize)]
struct NetworkSnapshot {
    internal: FieldStateSnapshot,
    options: FieldCountSnapshot,
    subnets: FieldCountSnapshot,
    routes: FieldCountSnapshot,
}
/// Redacted state-only restart-policy snapshot.
#[derive(Debug, Serialize)]
struct NativeRestartPolicySnapshot {
    name: FieldStateSnapshot,
    maximum_retry_count: FieldStateSnapshot,
}

/// Redacted state/count-only native health snapshot. Command values are never serialized.
#[derive(Debug, Serialize)]
struct NativeHealthCheckSnapshot {
    command: FieldCountSnapshot,
    interval: FieldStateSnapshot,
    timeout: FieldStateSnapshot,
    retries: FieldStateSnapshot,
    start_period: FieldStateSnapshot,
}

/// Redacted state/count-only native startup-health snapshot. Command values are never serialized.
#[derive(Debug, Serialize)]
struct NativeStartupHealthCheckSnapshot {
    command: FieldCountSnapshot,
    interval: FieldStateSnapshot,
    timeout: FieldStateSnapshot,
    retries: FieldStateSnapshot,
    start_period: FieldStateSnapshot,
    successes: FieldStateSnapshot,
}

/// Redacted state-only native logging snapshot.
#[derive(Debug, Serialize)]
struct NativeLoggingSnapshot {
    driver: FieldStateSnapshot,
    size: FieldStateSnapshot,
}

/// Redacted state/count-only native networking snapshot. It deliberately omits every address,
/// host entry, network name, DNS value, port value, option key, and option value.
#[derive(Debug, Serialize)]
struct NativeNetworkingSnapshot {
    port_bindings: FieldCountSnapshot,
    create_net_ns: FieldStateSnapshot,
    host_network: FieldStateSnapshot,
    dns_servers: FieldCountSnapshot,
    dns_search: FieldCountSnapshot,
    dns_options: FieldCountSnapshot,
    host_entries: FieldStateSnapshot,
    networks: FieldCountSnapshot,
    network_options: FieldCountSnapshot,
    no_manage_resolv_conf: FieldStateSnapshot,
    no_manage_hosts: FieldStateSnapshot,
    static_ip: FieldStateSnapshot,
    static_mac: FieldStateSnapshot,
}

#[derive(Debug, Serialize)]
struct VolumeOwnerFieldSnapshot {
    #[serde(flatten)]
    field: FieldStateSnapshot,
    wire_value: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct ResourceEvidenceSnapshot {
    engine_version: String,
    api_version: String,
    podman_minor_line: String,
    minimum_podman_version: String,
    maximum_exclusive_podman_version: String,
    minimum_libpod_api_version: String,
    observed_podman_version: String,
    observed_libpod_api_version: String,
    evidence_source: String,
    evidence_revision: String,
    evidence_release_tag: String,
}

#[derive(Debug, Serialize)]
struct InventoryFindingSnapshot {
    code: &'static str,
    resource: Option<ResourceIdentitySnapshot>,
    field_path: Option<String>,
    occurrence: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ResourceSelectorSnapshot {
    kind: &'static str,
    reference: String,
}

#[derive(Debug, Serialize)]
struct LabelSelectorSnapshot {
    name: String,
    exact_value_requested: bool,
}

#[derive(Debug, Serialize)]
struct ResourceGroupSnapshot {
    id: ResourceIdentitySnapshot,
    members: Vec<ResourceIdentitySnapshot>,
    prerequisites: Vec<ResourceIdentitySnapshot>,
}

#[derive(Debug, Serialize)]
struct ResourceDependencySnapshot {
    dependent: ResourceIdentitySnapshot,
    prerequisite: ResourceIdentitySnapshot,
    evidence: DependencyEvidenceSnapshot,
}

#[derive(Debug, Serialize)]
struct DependencyEvidenceSnapshot {
    kind: &'static str,
    field_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GroupingEdgeSnapshot {
    left: ResourceIdentitySnapshot,
    right: ResourceIdentitySnapshot,
    evidence: &'static str,
    field_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DiscoveryFindingSnapshot {
    code: &'static str,
    resource: Option<ResourceIdentitySnapshot>,
    selector: Option<ResourceSelectorSnapshot>,
    label_selector: Option<LabelSelectorSnapshot>,
    field_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct DiscoveryExplanationSnapshot {
    kind: &'static str,
    resource: ResourceIdentitySnapshot,
    related: Option<ResourceIdentitySnapshot>,
    position: Option<usize>,
    root_origin: Option<RootOriginSnapshot>,
}

#[derive(Debug, Serialize)]
struct RootOriginSnapshot {
    kind: &'static str,
    position: Option<usize>,
}

fn observation(source: &ResourceObservation) -> ResourceObservationSnapshot {
    let header = source.header();
    ResourceObservationSnapshot {
        header: ObservationHeaderSnapshot {
            identity: identity(header.identity()),
            state: match header.state() {
                crate::ResourceObservationState::Complete => "complete",
                crate::ResourceObservationState::Unavailable => "unavailable",
                crate::ResourceObservationState::Malformed => "malformed",
            },
            unmodelled_completeness: match header.unmodelled_completeness() {
                UnmodelledCompleteness::Complete => "complete",
                UnmodelledCompleteness::Incomplete => "incomplete",
            },
            unmodelled_fields: header
                .unmodelled_fields()
                .iter()
                .map(|field| UnmodelledFieldSnapshot {
                    id: field.id().as_str().to_owned(),
                    path: field.path().to_owned(),
                    json_kind: json_value_kind(field.json_kind()),
                    resource: identity(field.resource()),
                    evidence: evidence(field.evidence()),
                })
                .collect(),
            findings: header.findings().iter().map(inventory_finding).collect(),
            evidence: evidence(header.evidence()),
        },
        details: details(source.details()),
    }
}

#[allow(clippy::too_many_lines)] // one exhaustive kind-safe snapshot projection keeps redaction audit local.
fn details(source: &ResourceDetails) -> ResourceDetailsSnapshot {
    match source {
        ResourceDetails::Container(value) => ResourceDetailsSnapshot {
            kind: "container",
            labels: label_summary(value.labels()),
            configured_image: Some(field_summary(value.configured_image())),
            local_image_id: Some(field_summary(value.local_image_id())),
            relationships: Some(collection_summary(value.relationships(), Vec::len)),
            environment: Some(environment(value.environment())),
            command: Some(collection_summary(value.command(), |value| value.arguments().len())),
            entrypoint: Some(collection_summary(value.entrypoint(), |value| value.arguments().len())),
            user: Some(field_summary(value.user())),
            working_directory: Some(field_summary(value.working_directory())),
            hostname: Some(field_summary(value.hostname())),
            pod_membership: Some(field_summary(value.pod_membership())),
            native_dependencies: Some(collection_summary(value.native_dependencies(), Vec::len)),
            mounts: Some(collection_summary(value.mounts(), Vec::len)),
            secret_grants: Some(collection_summary(value.secret_grants(), Vec::len)),
            image_aliases: None,
            network: None,
            networking: Some(native_networking(value.networking())),
            memory_swappiness: Some(field_summary(value.memory_swappiness())),
            infra: Some(field_summary(value.infra())),
            restart_policy: Some(native_restart_policy(value.restart_policy())),
            health_check: Some(native_health_check(value.health_check())),
            health_failure_action: Some(field_summary(value.health_failure_action())),
            startup_health_check: Some(native_startup_health_check(value.startup_health_check())),
            logging: Some(native_logging(value.logging())),
            create_infra: None,
            secret_driver: None,
            volume_uid: None,
            volume_gid: None,
        },
        ResourceDetails::Pod(value) => ResourceDetailsSnapshot {
            kind: "pod",
            labels: label_summary(value.labels()),
            configured_image: None,
            local_image_id: None,
            relationships: Some(collection_summary(value.relationships(), Vec::len)),
            environment: None,
            command: None,
            entrypoint: None,
            user: None,
            working_directory: None,
            hostname: None,
            pod_membership: None,
            native_dependencies: None,
            mounts: None,
            secret_grants: None,
            image_aliases: None,
            network: None,
            networking: Some(native_networking(value.networking())),
            memory_swappiness: None,
            infra: None,
            restart_policy: None,
            health_check: None,
            health_failure_action: None,
            startup_health_check: None,
            logging: None,
            create_infra: Some(field_summary(value.create_infra())),
            secret_driver: None,
            volume_uid: None,
            volume_gid: None,
        },
        ResourceDetails::Network(value) => ResourceDetailsSnapshot {
            kind: "network",
            labels: label_summary(value.labels()),
            configured_image: None,
            local_image_id: None,
            relationships: None,
            environment: None,
            command: None,
            entrypoint: None,
            user: None,
            working_directory: None,
            hostname: None,
            pod_membership: None,
            native_dependencies: None,
            mounts: None,
            secret_grants: None,
            image_aliases: None,
            network: Some(NetworkSnapshot {
                internal: field_summary(value.internal()),
                options: collection_summary(value.options(), crate::NetworkOptionKeys::len),
                subnets: collection_summary(value.subnets(), Vec::len),
                routes: collection_summary(value.routes(), Vec::len),
            }),
            networking: None,
            memory_swappiness: None,
            infra: None,
            restart_policy: None,
            health_check: None,
            health_failure_action: None,
            startup_health_check: None,
            logging: None,
            create_infra: None,
            secret_driver: None,
            volume_uid: None,
            volume_gid: None,
        },
        ResourceDetails::Volume(value) => ResourceDetailsSnapshot {
            kind: "volume",
            labels: label_summary(value.labels()),
            configured_image: None,
            local_image_id: None,
            relationships: None,
            environment: None,
            command: None,
            entrypoint: None,
            user: None,
            working_directory: None,
            hostname: None,
            pod_membership: None,
            native_dependencies: None,
            mounts: None,
            secret_grants: None,
            image_aliases: None,
            network: None,
            networking: None,
            memory_swappiness: None,
            infra: None,
            restart_policy: None,
            health_check: None,
            health_failure_action: None,
            startup_health_check: None,
            logging: None,
            create_infra: None,
            secret_driver: None,
            volume_uid: Some(volume_owner(value.uid())),
            volume_gid: Some(volume_owner(value.gid())),
        },
        ResourceDetails::Image(value) => ResourceDetailsSnapshot {
            kind: "image",
            labels: label_summary(value.labels()),
            configured_image: None,
            local_image_id: None,
            relationships: None,
            environment: Some(environment(value.environment())),
            command: None,
            entrypoint: None,
            user: None,
            working_directory: None,
            hostname: None,
            pod_membership: None,
            native_dependencies: None,
            mounts: None,
            secret_grants: None,
            image_aliases: Some(collection_summary(value.aliases(), Vec::len)),
            network: None,
            networking: None,
            memory_swappiness: None,
            infra: None,
            restart_policy: None,
            health_check: None,
            health_failure_action: None,
            startup_health_check: None,
            logging: None,
            create_infra: None,
            secret_driver: None,
            volume_uid: None,
            volume_gid: None,
        },
        ResourceDetails::Secret(value) => ResourceDetailsSnapshot {
            kind: "secret",
            labels: label_summary(value.labels()),
            configured_image: None,
            local_image_id: None,
            relationships: None,
            environment: None,
            command: None,
            entrypoint: None,
            user: None,
            working_directory: None,
            hostname: None,
            pod_membership: None,
            native_dependencies: None,
            mounts: None,
            secret_grants: None,
            image_aliases: None,
            network: None,
            networking: None,
            memory_swappiness: None,
            infra: None,
            restart_policy: None,
            health_check: None,
            health_failure_action: None,
            startup_health_check: None,
            logging: None,
            create_infra: None,
            secret_driver: Some(field_summary(value.driver())),
            volume_uid: None,
            volume_gid: None,
        },
    }
}

fn label_summary(value: &ObservationField<crate::Labels>) -> FieldCountSnapshot {
    collection_summary(value, BTreeMap::len)
}

fn native_restart_policy(
    value: &ObservationField<crate::NativeRestartPolicyObservation>,
) -> NativeRestartPolicySnapshot {
    let Some(value) = value.observed().map(crate::ObservedValue::value) else {
        let state = field_summary(value);
        return NativeRestartPolicySnapshot {
            name: state.clone(),
            maximum_retry_count: state,
        };
    };
    NativeRestartPolicySnapshot {
        name: field_summary(value.name()),
        maximum_retry_count: field_summary(value.maximum_retry_count()),
    }
}

fn health_command_count(value: &crate::NativeHealthCommand) -> usize {
    match value {
        crate::NativeHealthCommand::Disabled => 0,
        crate::NativeHealthCommand::Shell(command) | crate::NativeHealthCommand::Exec(command) => {
            command.argument_count()
        }
    }
}

fn native_health_check(value: &ObservationField<crate::NativeHealthCheckObservation>) -> NativeHealthCheckSnapshot {
    let Some(value) = value.observed().map(crate::ObservedValue::value) else {
        let state = field_summary(value);
        return NativeHealthCheckSnapshot {
            command: empty_collection_summary(&state),
            interval: state.clone(),
            timeout: state.clone(),
            retries: state.clone(),
            start_period: state,
        };
    };
    NativeHealthCheckSnapshot {
        command: collection_summary(value.command(), health_command_count),
        interval: field_summary(value.interval()),
        timeout: field_summary(value.timeout()),
        retries: field_summary(value.retries()),
        start_period: field_summary(value.start_period()),
    }
}

fn native_startup_health_check(
    value: &ObservationField<crate::NativeStartupHealthCheckObservation>,
) -> NativeStartupHealthCheckSnapshot {
    let Some(value) = value.observed().map(crate::ObservedValue::value) else {
        let state = field_summary(value);
        return NativeStartupHealthCheckSnapshot {
            command: empty_collection_summary(&state),
            interval: state.clone(),
            timeout: state.clone(),
            retries: state.clone(),
            start_period: state.clone(),
            successes: state,
        };
    };
    NativeStartupHealthCheckSnapshot {
        command: collection_summary(value.command(), health_command_count),
        interval: field_summary(value.interval()),
        timeout: field_summary(value.timeout()),
        retries: field_summary(value.retries()),
        start_period: field_summary(value.start_period()),
        successes: field_summary(value.successes()),
    }
}

fn native_logging(value: &ObservationField<crate::NativeLoggingObservation>) -> NativeLoggingSnapshot {
    let Some(value) = value.observed().map(crate::ObservedValue::value) else {
        let state = field_summary(value);
        return NativeLoggingSnapshot {
            driver: state.clone(),
            size: state,
        };
    };
    NativeLoggingSnapshot {
        driver: field_summary(value.driver()),
        size: field_summary(value.size()),
    }
}

fn native_networking(value: &ObservationField<crate::NativeNetworkingObservation>) -> NativeNetworkingSnapshot {
    let Some(value) = value.observed().map(crate::ObservedValue::value) else {
        let state = field_summary(value);
        return NativeNetworkingSnapshot {
            port_bindings: empty_collection_summary(&state),
            create_net_ns: state.clone(),
            host_network: state.clone(),
            dns_servers: empty_collection_summary(&state),
            dns_search: empty_collection_summary(&state),
            dns_options: empty_collection_summary(&state),
            host_entries: state.clone(),
            networks: empty_collection_summary(&state),
            network_options: empty_collection_summary(&state),
            no_manage_resolv_conf: state.clone(),
            no_manage_hosts: state.clone(),
            static_ip: state.clone(),
            static_mac: state,
        };
    };
    NativeNetworkingSnapshot {
        port_bindings: collection_summary(value.port_bindings(), Vec::len),
        create_net_ns: field_summary(value.create_net_ns()),
        host_network: field_summary(value.host_network()),
        dns_servers: collection_summary(value.dns_servers(), Vec::len),
        dns_search: collection_summary(value.dns_search(), Vec::len),
        dns_options: collection_summary(value.dns_options(), Vec::len),
        host_entries: field_summary(value.host_entries()),
        networks: collection_summary(value.networks(), Vec::len),
        network_options: collection_summary(value.network_options(), crate::NativeOpaqueNetworkOptions::len),
        no_manage_resolv_conf: field_summary(value.no_manage_resolv_conf()),
        no_manage_hosts: field_summary(value.no_manage_hosts()),
        static_ip: field_summary(value.static_ip()),
        static_mac: field_summary(value.static_mac()),
    }
}

fn empty_collection_summary(field: &FieldStateSnapshot) -> FieldCountSnapshot {
    FieldCountSnapshot {
        field: field.clone(),
        count: 0,
    }
}

fn collection_summary<T>(value: &ObservationField<T>, count: impl FnOnce(&T) -> usize) -> FieldCountSnapshot {
    let count = value.observed().map_or(0, |observed| count(observed.value()));
    FieldCountSnapshot {
        field: field_summary(value),
        count,
    }
}

fn field_summary<T>(value: &ObservationField<T>) -> FieldStateSnapshot {
    FieldStateSnapshot {
        state: field_state(value),
        origin: value.observed().map(|value| observation_origin(value.origin())),
    }
}

fn field_state<T>(value: &ObservationField<T>) -> &'static str {
    match value {
        ObservationField::Absent => "absent",
        ObservationField::Observed(_) => "observed",
        ObservationField::Unavailable => "unavailable",
        ObservationField::Malformed => "malformed",
        ObservationField::VersionInapplicable => "version_inapplicable",
        ObservationField::NotApplicable => "not_applicable",
        ObservationField::Unmodelled(_) => "unmodelled",
    }
}
fn observation_origin(value: crate::ObservationOrigin) -> &'static str {
    match value {
        crate::ObservationOrigin::Configured => "configured",
        crate::ObservationOrigin::Effective => "effective",
        crate::ObservationOrigin::RuntimeAssigned => "runtime_assigned",
        crate::ObservationOrigin::LocalResolution => "local_resolution",
    }
}

fn environment(value: &ObservationField<crate::ProtectedEnvironment>) -> ProtectedEnvironmentSnapshot {
    let entries = value.observed().map_or_else(Vec::new, |environment| {
        environment
            .value()
            .entries()
            .iter()
            .map(|entry| EnvironmentEntrySnapshot {
                name: entry.name().to_owned(),
                value_state: match entry.value() {
                    crate::ProtectedEnvironmentValue::Redacted => "redacted",
                    crate::ProtectedEnvironmentValue::AuthorizedOpaque(_) => "authorized_opaque_redacted",
                },
            })
            .collect()
    });
    ProtectedEnvironmentSnapshot {
        field: field_summary(value),
        entries,
    }
}

fn volume_owner(value: &ObservationField<crate::VolumeOwnerIdWireValue>) -> VolumeOwnerFieldSnapshot {
    let wire_value = value.observed().map(|value| match value.value() {
        crate::VolumeOwnerIdWireValue::WireAbsentMayMeanZero => "wire_absent_may_mean_zero",
        crate::VolumeOwnerIdWireValue::Explicit(_) => "explicit",
    });
    VolumeOwnerFieldSnapshot {
        field: field_summary(value),
        wire_value,
    }
}

fn identity(source: &ResourceIdentity) -> ResourceIdentitySnapshot {
    ResourceIdentitySnapshot {
        kind: resource_kind(source.kind()),
        id: source.id().to_owned(),
        name: source.name().map(str::to_owned),
    }
}

fn evidence(source: &crate::ResourceEvidence) -> ResourceEvidenceSnapshot {
    let capability = source.capability();
    let reference = capability.evidence();
    ResourceEvidenceSnapshot {
        engine_version: source.engine_version().to_owned(),
        api_version: source.api_version().to_owned(),
        podman_minor_line: capability.podman_minor_line().to_owned(),
        minimum_podman_version: capability.minimum_podman_version().to_owned(),
        maximum_exclusive_podman_version: capability.maximum_exclusive_podman_version().to_owned(),
        minimum_libpod_api_version: capability.minimum_libpod_api_version().to_owned(),
        observed_podman_version: capability.observed_podman_version().to_owned(),
        observed_libpod_api_version: capability.observed_libpod_api_version().to_owned(),
        evidence_source: reference.source().to_owned(),
        evidence_revision: reference.revision().to_owned(),
        evidence_release_tag: reference.release_tag().to_owned(),
    }
}

fn inventory_finding(source: &crate::InventoryFinding) -> InventoryFindingSnapshot {
    InventoryFindingSnapshot {
        code: source.code().as_str(),
        resource: source.resource().map(identity),
        field_path: source.field_path().map(str::to_owned),
        occurrence: source.occurrence(),
    }
}

fn label_selector(source: &crate::LabelSelector) -> LabelSelectorSnapshot {
    LabelSelectorSnapshot {
        name: source.name().to_owned(),
        exact_value_requested: source.value().is_some(),
    }
}

fn resource_kind(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Container => "container",
        ResourceKind::Pod => "pod",
        ResourceKind::Network => "network",
        ResourceKind::Volume => "volume",
        ResourceKind::Image => "image",
        ResourceKind::Secret => "secret",
    }
}

fn json_value_kind(kind: JsonValueKind) -> &'static str {
    match kind {
        JsonValueKind::Null => "null",
        JsonValueKind::Boolean => "boolean",
        JsonValueKind::Number => "number",
        JsonValueKind::String => "string",
        JsonValueKind::Array => "array",
        JsonValueKind::Object => "object",
    }
}

fn explanation_kind(kind: &DiscoveryExplanationKind) -> &'static str {
    match kind {
        DiscoveryExplanationKind::Root => "root",
        DiscoveryExplanationKind::IncludedMember => "included_member",
        DiscoveryExplanationKind::Prerequisite => "prerequisite",
        DiscoveryExplanationKind::StoppedSharedBoundary => "stopped_shared_boundary",
        DiscoveryExplanationKind::AuthorizedNetworkCrossing => "authorized_network_crossing",
        DiscoveryExplanationKind::AuthorizedSharedCrossing => "authorized_shared_crossing",
        DiscoveryExplanationKind::StrongEvidenceMerge => "strong_evidence_merge",
        DiscoveryExplanationKind::GroupOrdering => "group_ordering",
    }
}

fn root_origin(origin: &DiscoveryRootOrigin) -> RootOriginSnapshot {
    match origin {
        DiscoveryRootOrigin::ResourceSelector { position } => RootOriginSnapshot {
            kind: "resource_selector",
            position: Some(*position),
        },
        DiscoveryRootOrigin::LabelSelector { position } => RootOriginSnapshot {
            kind: "label_selector",
            position: Some(*position),
        },
        DiscoveryRootOrigin::All => RootOriginSnapshot {
            kind: "all",
            position: None,
        },
    }
}
