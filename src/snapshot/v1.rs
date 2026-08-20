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
    memory_swappiness: Option<FieldStateSnapshot>,
    infra: Option<FieldStateSnapshot>,
    secret_driver: Option<FieldStateSnapshot>,
    volume_uid: Option<VolumeOwnerFieldSnapshot>,
    volume_gid: Option<VolumeOwnerFieldSnapshot>,
}

#[derive(Debug, Serialize)]
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

fn details(source: &ResourceDetails) -> ResourceDetailsSnapshot {
    match source {
        ResourceDetails::Container(value) => ResourceDetailsSnapshot {
            kind: "container",
            labels: label_summary(value.labels()),
            configured_image: Some(field_summary(value.configured_image())),
            local_image_id: Some(field_summary(value.local_image_id())),
            relationships: Some(collection_summary(value.relationships(), Vec::len)),
            environment: Some(environment(value.environment())),
            image_aliases: None,
            network: None,
            memory_swappiness: Some(field_summary(value.memory_swappiness())),
            infra: Some(field_summary(value.infra())),
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
            image_aliases: None,
            network: None,
            memory_swappiness: None,
            infra: None,
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
            image_aliases: None,
            network: Some(NetworkSnapshot {
                internal: field_summary(value.internal()),
                options: collection_summary(value.options(), crate::NetworkOptionKeys::len),
                subnets: collection_summary(value.subnets(), Vec::len),
            }),
            memory_swappiness: None,
            infra: None,
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
            image_aliases: None,
            network: None,
            memory_swappiness: None,
            infra: None,
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
            image_aliases: Some(collection_summary(value.aliases(), Vec::len)),
            network: None,
            memory_swappiness: None,
            infra: None,
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
            image_aliases: None,
            network: None,
            memory_swappiness: None,
            infra: None,
            secret_driver: Some(field_summary(value.driver())),
            volume_uid: None,
            volume_gid: None,
        },
    }
}

fn label_summary(value: &ObservationField<crate::Labels>) -> FieldCountSnapshot {
    collection_summary(value, BTreeMap::len)
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
