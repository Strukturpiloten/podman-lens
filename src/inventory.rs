//! Read-only, versioned acquisition of a redacted native Podman inventory.
//!
//! Wire JSON stays private to this module. The public inventory deliberately retains only typed
//! identity, relationship, labels, bounded unknown-field metadata, evidence, and findings.

use std::{collections::BTreeMap, fmt};

use serde_json::{Map, Value};

use crate::observation::{
    ContainerObservation, ImageObservation, Labels, NativeRelationship, NetworkObservation, NetworkOptionKeys,
    ObservationField, ObservationHeader, ObservationOrigin, ObservedValue, PodObservation, ProtectedEnvironment,
    ProtectedEnvironmentEntry, ProtectedEnvironmentValue, ResourceDetails, ResourceObservation,
    ResourceObservationState, SecretObservation, UnixId as VolumeOwnerUnixId, UnmodelledCompleteness, UnmodelledField,
    VolumeObservation, VolumeOwnerIdWireValue,
};
use crate::{
    CapabilityCatalogueEntry, Diagnostic, DiagnosticCode, LibpodMethod, LibpodPath, LibpodRequest, LibpodResponse,
    LibpodTransport, ObservedApiVersion, PodmanLensResult, ServiceObservation, capability_catalogue,
    probe_libpod_service,
};

/// Maximum JSON body accepted by the inventory decoder after transport framing limits apply.
pub const MAX_INVENTORY_JSON_BYTES: usize = 16 * 1024 * 1024;
/// Maximum unknown native fields retained for one resource record.
pub const MAX_UNKNOWN_FIELDS_PER_RECORD: usize = 128;
/// Maximum unknown native fields retained across one inventory response.
pub const MAX_UNKNOWN_FIELDS_PER_INVENTORY: usize = 2_048;

/// Controls how runtime environment values are retained after an explicit read-only inspection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum EnvironmentValuePolicy {
    /// Retain environment names and source order while replacing every value with a marker.
    #[default]
    Redact,
    /// Retain values behind an opaque non-serializing wrapper.
    Include,
}

/// Explicit controls for one native inventory acquisition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AcquisitionOptions {
    environment_values: EnvironmentValuePolicy,
}

impl AcquisitionOptions {
    /// Uses the default policy that redacts runtime environment values.
    #[must_use]
    pub const fn redacted() -> Self {
        Self {
            environment_values: EnvironmentValuePolicy::Redact,
        }
    }

    /// Explicitly authorizes retaining runtime environment values in opaque sensitive wrappers.
    ///
    /// The resulting values do not implement `Serialize`, `Debug`, or `Display` as plaintext.
    #[must_use]
    pub const fn include_environment_values() -> Self {
        Self {
            environment_values: EnvironmentValuePolicy::Include,
        }
    }

    /// Returns the selected environment-value policy.
    #[must_use]
    pub const fn environment_value_policy(self) -> EnvironmentValuePolicy {
        self.environment_values
    }
}

/// A Podman resource kind available from the Libpod inventory API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ResourceKind {
    /// A container.
    Container,
    /// A pod.
    Pod,
    /// A network.
    Network,
    /// A named volume.
    Volume,
    /// An image.
    Image,
    /// Secret metadata.
    Secret,
}

impl ResourceKind {
    const ALL: [Self; 6] = [
        Self::Container,
        Self::Pod,
        Self::Network,
        Self::Volume,
        Self::Image,
        Self::Secret,
    ];

    /// Returns this kind's fixed canonical order in inventories, identities, and resource groups.
    ///
    /// The order is container, pod, network, volume, image, then secret. It is an explicit
    /// public determinism contract rather than the incidental declaration order of this enum.
    #[must_use]
    pub const fn canonical_rank(self) -> u8 {
        match self {
            Self::Container => 0,
            Self::Pod => 1,
            Self::Network => 2,
            Self::Volume => 3,
            Self::Image => 4,
            Self::Secret => 5,
        }
    }

    const fn collection(self) -> &'static str {
        match self {
            Self::Container => "containers",
            Self::Pod => "pods",
            Self::Network => "networks",
            Self::Volume => "volumes",
            Self::Image => "images",
            Self::Secret => "secrets",
        }
    }
}

impl Ord for ResourceKind {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.canonical_rank().cmp(&other.canonical_rank())
    }
}

impl PartialOrd for ResourceKind {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Stable native identity for one observed resource.
///
/// Its derived ordering uses [`ResourceKind::canonical_rank`], then the stable ID and observed
/// name, so resource and group ordering never depends on enum declaration order.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceIdentity {
    kind: ResourceKind,
    id: String,
    name: Option<String>,
}

impl ResourceIdentity {
    fn new(kind: ResourceKind, id: String, name: Option<String>) -> Self {
        Self { kind, id, name }
    }

    /// Returns the Podman resource kind.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Returns the stable identifier used for deterministic inspect requests.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the observed human-facing name when the resource exposes one.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Source and version evidence attached to every inventory record and unknown field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceEvidence {
    engine_version: String,
    api_version: String,
    capability: CapabilityCatalogueEntry,
}

impl ResourceEvidence {
    fn from_service(service: &ServiceObservation) -> PodmanLensResult<Self> {
        let engine = service.engine_version().as_semver();
        let capability = capability_catalogue()?
            .into_iter()
            .find(|entry| {
                let minimum = semver::Version::parse(entry.minimum_podman_version()).ok();
                let maximum = semver::Version::parse(entry.maximum_exclusive_podman_version()).ok();
                minimum.is_some_and(|minimum| engine >= &minimum) && maximum.is_some_and(|maximum| engine < &maximum)
            })
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::InventoryEvidenceUnavailable))?;
        Ok(Self {
            engine_version: service.engine_version().original().to_owned(),
            api_version: service.api_version().original().to_owned(),
            capability,
        })
    }

    /// Returns the engine version observed before acquisition began.
    #[must_use]
    pub fn engine_version(&self) -> &str {
        &self.engine_version
    }

    /// Returns the Libpod API version used for this record's requests.
    #[must_use]
    pub fn api_version(&self) -> &str {
        &self.api_version
    }

    /// Returns immutable catalogue evidence for the matching reviewed Podman line.
    #[must_use]
    pub fn capability(&self) -> &CapabilityCatalogueEntry {
        &self.capability
    }
}

/// The JSON kind of an unknown field whose value was intentionally not retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum JsonValueKind {
    /// `null`.
    Null,
    /// A JSON boolean.
    Boolean,
    /// A JSON number.
    Number,
    /// A JSON string.
    String,
    /// A JSON array.
    Array,
    /// A JSON object.
    Object,
}

/// One structured non-fatal acquisition finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventoryFinding {
    code: DiagnosticCode,
    resource: Option<ResourceIdentity>,
    field_path: Option<String>,
    occurrence: Option<usize>,
}

impl InventoryFinding {
    fn section(code: DiagnosticCode) -> Self {
        Self {
            code,
            resource: None,
            field_path: None,
            occurrence: None,
        }
    }

    fn for_resource(code: DiagnosticCode, resource: ResourceIdentity) -> Self {
        Self {
            code,
            resource: Some(resource),
            field_path: None,
            occurrence: None,
        }
    }

    fn field(code: DiagnosticCode, resource: ResourceIdentity, field_path: impl Into<String>) -> Self {
        Self {
            code,
            resource: Some(resource),
            field_path: Some(field_path.into()),
            occurrence: None,
        }
    }

    fn at_occurrence(
        code: DiagnosticCode,
        resource: ResourceIdentity,
        field_path: impl Into<String>,
        occurrence: usize,
    ) -> Self {
        Self {
            code,
            resource: Some(resource),
            field_path: Some(field_path.into()),
            occurrence: Some(occurrence),
        }
    }

    /// Returns the stable diagnostic rule code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the affected resource, when the finding has a stable resource identity.
    #[must_use]
    pub fn resource(&self) -> Option<&ResourceIdentity> {
        self.resource.as_ref()
    }

    /// Returns the source JSON path for a field-specific finding.
    #[must_use]
    pub fn field_path(&self) -> Option<&str> {
        self.field_path.as_deref()
    }

    /// Returns the zero-based source-array occurrence for an item-specific finding.
    #[must_use]
    pub const fn occurrence(&self) -> Option<usize> {
        self.occurrence
    }
}

/// A non-serializing opaque runtime environment value.
#[derive(Clone, Eq, PartialEq)]
pub struct SensitiveEnvironmentValue(String);

impl SensitiveEnvironmentValue {
    fn new(value: String) -> Self {
        Self(value)
    }

    /// Lets an explicitly authorized caller use this value without formatting or serializing it.
    pub fn expose<R>(&self, use_value: impl FnOnce(&str) -> R) -> R {
        use_value(&self.0)
    }
}

impl fmt::Debug for SensitiveEnvironmentValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SensitiveEnvironmentValue([redacted])")
    }
}

impl fmt::Display for SensitiveEnvironmentValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

/// Explicit acquisition state for one independently listed resource kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InventorySectionAvailability {
    /// The list response decoded; individual observations retain their own state.
    Available,
    /// The list endpoint could not be acquired.
    Unavailable,
    /// The list endpoint returned malformed or unsupported data.
    Malformed,
}

/// Typed observations for one independently listed resource kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventorySection {
    kind: ResourceKind,
    availability: InventorySectionAvailability,
    observations: Vec<ResourceObservation>,
    findings: Vec<InventoryFinding>,
}

impl InventorySection {
    fn unavailable(kind: ResourceKind, code: DiagnosticCode) -> Self {
        Self {
            kind,
            availability: if code == DiagnosticCode::InventoryJson || code == DiagnosticCode::InventoryShape {
                InventorySectionAvailability::Malformed
            } else {
                InventorySectionAvailability::Unavailable
            },
            observations: Vec::new(),
            findings: vec![InventoryFinding::section(code)],
        }
    }

    /// Returns the resource kind represented by this section.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Returns the typed list-acquisition state for this resource kind.
    #[must_use]
    pub const fn availability(&self) -> InventorySectionAvailability {
        self.availability
    }

    /// Returns typed observations in deterministic list response order.
    #[must_use]
    pub fn observations(&self) -> &[ResourceObservation] {
        &self.observations
    }

    /// Returns kind-wide findings that have no stable individual resource.
    #[must_use]
    pub fn findings(&self) -> &[InventoryFinding] {
        &self.findings
    }

    pub(crate) fn is_available(&self) -> bool {
        self.availability == InventorySectionAvailability::Available
    }
}

/// Complete result of one non-atomic read-only Libpod inventory acquisition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceInventory {
    service: ServiceObservation,
    sections: Vec<InventorySection>,
}

impl ResourceInventory {
    /// Returns the version observation performed before every list and inspect request.
    #[must_use]
    pub fn service(&self) -> &ServiceObservation {
        &self.service
    }

    /// Returns all six sections in fixed Podman resource-kind order.
    #[must_use]
    pub fn sections(&self) -> &[InventorySection] {
        &self.sections
    }

    /// Finds one section by resource kind.
    #[must_use]
    pub fn section(&self, kind: ResourceKind) -> Option<&InventorySection> {
        self.sections.iter().find(|section| section.kind == kind)
    }

    /// Returns every acquired observation in deterministic resource-kind and list order.
    pub fn observations(&self) -> impl Iterator<Item = &ResourceObservation> {
        self.sections.iter().flat_map(InventorySection::observations)
    }

    /// Finds one observation by its complete stable native identity.
    #[must_use]
    pub fn observation(&self, identity: &ResourceIdentity) -> Option<&ResourceObservation> {
        self.observations()
            .find(|observation| observation.header().identity() == identity)
    }
}

/// Probes and then acquires all supported native Podman resource kinds.
///
/// The request sequence is deterministic: the fixed M1 probe precedes container, pod, network,
/// volume, image, and secret lists; every successfully listed stable ID is then inspected in list
/// order. Only bodyless `GET` requests are generated. Secret metadata inspection never requests a
/// payload endpoint, and unexpected `SecretData` fields are discarded with `PLN0018`.
///
/// A failed list makes only its section unavailable. A disappeared or malformed inspect response
/// produces a partial record and does not hide unrelated observations.
///
/// # Errors
///
/// Returns the fixed probe diagnostic when no reviewed Libpod service can be observed. Individual
/// list and inspect failures are retained in the returned inventory instead.
pub async fn acquire_inventory(
    transport: &dyn LibpodTransport,
    options: AcquisitionOptions,
) -> PodmanLensResult<ResourceInventory> {
    let service = probe_libpod_service(transport).await?;
    let evidence = ResourceEvidence::from_service(&service)?;
    // Listing is its own deterministic phase. It makes a partial list visible as a section-wide
    // race instead of letting early inspect requests influence which later kinds are listed.
    let mut listed = Vec::with_capacity(ResourceKind::ALL.len());
    for kind in ResourceKind::ALL {
        listed.push((kind, list_section(transport, &service, kind).await));
    }
    let mut sections = Vec::with_capacity(ResourceKind::ALL.len());
    let mut remaining_unknown_fields = MAX_UNKNOWN_FIELDS_PER_INVENTORY;
    for (kind, identities) in listed {
        match identities {
            Ok(listed) => {
                let mut observations = Vec::with_capacity(listed.identities.len());
                for identity in listed.identities {
                    let observation = inspect_observation(
                        transport,
                        service.api_version(),
                        evidence.clone(),
                        identity,
                        options,
                        remaining_unknown_fields.min(MAX_UNKNOWN_FIELDS_PER_RECORD),
                    )
                    .await;
                    remaining_unknown_fields =
                        remaining_unknown_fields.saturating_sub(observation.header().unmodelled_fields().len());
                    observations.push(observation);
                }
                sections.push(InventorySection {
                    kind,
                    availability: InventorySectionAvailability::Available,
                    observations,
                    findings: listed.findings,
                });
            }
            Err(error) => sections.push(InventorySection::unavailable(kind, error.code())),
        }
    }
    reconcile_relationships(&mut sections);
    Ok(ResourceInventory { service, sections })
}

#[allow(clippy::too_many_lines)] // Reconciliation keeps both directional membership checks adjacent.
fn reconcile_relationships(sections: &mut [InventorySection]) {
    let available = sections
        .iter()
        .map(|section| (section.kind, section.is_available()))
        .collect::<BTreeMap<_, _>>();
    let mut targets = BTreeMap::<(ResourceKind, String), Vec<String>>::new();
    for section in sections.iter() {
        for observation in &section.observations {
            let identity = observation.header().identity();
            targets
                .entry((identity.kind(), identity.id().to_owned()))
                .or_default()
                .push(identity.id().to_owned());
            if let Some(name) = identity.name() {
                targets
                    .entry((identity.kind(), name.to_owned()))
                    .or_default()
                    .push(identity.id().to_owned());
            }
            if let Some(ObservationField::Observed(aliases)) = observation.image_aliases() {
                for alias in aliases.value() {
                    targets
                        .entry((identity.kind(), alias.clone()))
                        .or_default()
                        .push(identity.id().to_owned());
                }
            }
        }
    }
    for candidates in targets.values_mut() {
        candidates.sort();
        candidates.dedup();
    }
    for section in sections.iter_mut() {
        for observation in &mut section.observations {
            let Some(ObservationField::Observed(relationships)) = observation.relationships() else {
                continue;
            };
            let relationship_findings = relationships
                .value()
                .iter()
                .filter(|relationship| available.get(&relationship.kind).copied().unwrap_or(false))
                .flat_map(
                    |relationship| match resolve_relationship_target(&targets, relationship) {
                        RelationshipTarget::One(_) => Vec::new(),
                        RelationshipTarget::Unresolved => relationship
                            .field_paths
                            .iter()
                            .cloned()
                            .map(|path| (DiagnosticCode::UnresolvedRelationship, path))
                            .collect(),
                        RelationshipTarget::Ambiguous => relationship
                            .field_paths
                            .iter()
                            .cloned()
                            .map(|path| (DiagnosticCode::RelationshipAmbiguous, path))
                            .collect(),
                        RelationshipTarget::Conflict => relationship
                            .field_paths
                            .iter()
                            .cloned()
                            .map(|path| (DiagnosticCode::RelationshipConflict, path))
                            .collect(),
                    },
                )
                .collect::<Vec<_>>();
            let pod_membership_unresolved = observation.header().identity().kind() == ResourceKind::Pod
                && relationships.value().iter().any(|relationship| {
                    relationship.kind == ResourceKind::Container
                        && !matches!(
                            resolve_relationship_target(&targets, relationship),
                            RelationshipTarget::One(_)
                        )
                });
            let identity = observation.header().identity().clone();
            observation.header_mut().findings_mut().extend(
                relationship_findings
                    .into_iter()
                    .map(|(code, path)| InventoryFinding::field(code, identity.clone(), path)),
            );
            if pod_membership_unresolved {
                observation.header_mut().findings_mut().push(InventoryFinding::field(
                    DiagnosticCode::PodMembershipConflict,
                    identity,
                    "$.Containers",
                ));
            }
        }
    }

    let mut pod_members = BTreeMap::<String, Vec<String>>::new();
    let mut container_pods = BTreeMap::<String, Vec<String>>::new();
    for section in sections.iter() {
        if !section.is_available() {
            continue;
        }
        for observation in &section.observations {
            let identity = observation.header().identity();
            let Some(ObservationField::Observed(relationships)) = observation.relationships() else {
                continue;
            };
            for relationship in relationships.value() {
                match (identity.kind(), relationship.kind) {
                    (ResourceKind::Pod, ResourceKind::Container) => {
                        if let RelationshipTarget::One(target) = resolve_relationship_target(&targets, relationship) {
                            pod_members
                                .entry(identity.id().to_owned())
                                .or_default()
                                .push(target.to_owned());
                        }
                    }
                    (ResourceKind::Container, ResourceKind::Pod) => {
                        if let RelationshipTarget::One(target) = resolve_relationship_target(&targets, relationship) {
                            container_pods
                                .entry(identity.id().to_owned())
                                .or_default()
                                .push(target.to_owned());
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    for section in sections.iter_mut() {
        for observation in &mut section.observations {
            let identity = observation.header().identity().clone();
            let conflict = match identity.kind() {
                ResourceKind::Pod => pod_members.get(identity.id()).is_some_and(|members| {
                    members.iter().any(|member| {
                        !container_pods
                            .get(member)
                            .is_some_and(|pods| pods.contains(&identity.id().to_owned()))
                    })
                }),
                ResourceKind::Container => container_pods.get(identity.id()).is_some_and(|pods| {
                    pods.iter().any(|pod| {
                        !pod_members
                            .get(pod)
                            .is_some_and(|members| members.contains(&identity.id().to_owned()))
                    })
                }),
                _ => false,
            };
            if conflict {
                observation.header_mut().findings_mut().push(InventoryFinding::field(
                    DiagnosticCode::PodMembershipConflict,
                    identity,
                    "$.PodMembership",
                ));
            }
        }
    }

    for section in sections.iter_mut() {
        for observation in &mut section.observations {
            let ResourceDetails::Container(container) = observation.details() else {
                continue;
            };
            let (ObservationField::Observed(configured), ObservationField::Observed(local)) =
                (container.configured_image(), container.local_image_id())
            else {
                continue;
            };
            let configured = NativeRelationship::new(ResourceKind::Image, configured.value(), "$.ImageName");
            let local = NativeRelationship::new(ResourceKind::Image, local.value(), "$.Image");
            if let (RelationshipTarget::One(configured), RelationshipTarget::One(local)) = (
                resolve_relationship_target(&targets, &configured),
                resolve_relationship_target(&targets, &local),
            ) {
                if configured != local {
                    let identity = observation.header().identity().clone();
                    observation.header_mut().findings_mut().push(InventoryFinding::field(
                        DiagnosticCode::RelationshipConflict,
                        identity,
                        "$.ImageName",
                    ));
                }
            }
        }
    }
}

enum RelationshipTarget<'a> {
    One(&'a str),
    Unresolved,
    Ambiguous,
    Conflict,
}

fn resolve_relationship_target<'a>(
    targets: &'a BTreeMap<(ResourceKind, String), Vec<String>>,
    relationship: &NativeRelationship,
) -> RelationshipTarget<'a> {
    let mut resolved = std::collections::BTreeSet::new();
    for reference in &relationship.references {
        let Some(candidates) = targets.get(&(relationship.kind, reference.clone())) else {
            return RelationshipTarget::Unresolved;
        };
        let [candidate] = candidates.as_slice() else {
            return RelationshipTarget::Ambiguous;
        };
        resolved.insert(candidate.as_str());
    }
    match resolved.into_iter().collect::<Vec<_>>().as_slice() {
        [target] => RelationshipTarget::One(target),
        [] => RelationshipTarget::Unresolved,
        _ => RelationshipTarget::Conflict,
    }
}

struct ListedSection {
    identities: Vec<ResourceIdentity>,
    findings: Vec<InventoryFinding>,
}

async fn list_section(
    transport: &dyn LibpodTransport,
    service: &ServiceObservation,
    kind: ResourceKind,
) -> PodmanLensResult<ListedSection> {
    let list_path = list_path(service.api_version(), kind);
    let path = list_path?;
    let response = send_get(transport, path).await;
    response.and_then(|response| decode_list(kind, &response))
}

async fn inspect_observation(
    transport: &dyn LibpodTransport,
    api_version: &ObservedApiVersion,
    evidence: ResourceEvidence,
    identity: ResourceIdentity,
    options: AcquisitionOptions,
    unknown_field_limit: usize,
) -> ResourceObservation {
    let Ok(path) = LibpodPath::resource(api_version, identity.kind.collection(), identity.id(), "json") else {
        return partial_observation(identity, evidence, DiagnosticCode::ResourceMalformed);
    };
    let response = send_get(transport, path).await;
    match response {
        Ok(response) if response.status() == 404 => {
            partial_observation(identity, evidence, DiagnosticCode::ResourceUnavailable)
        }
        Ok(response) => {
            match decode_observation(&identity, &response, evidence.clone(), options, unknown_field_limit) {
                Ok(record) => record,
                Err(error) => partial_observation(identity, evidence, error.code()),
            }
        }
        Err(_) => partial_observation(identity, evidence, DiagnosticCode::ResourceUnavailable),
    }
}

async fn send_get(transport: &dyn LibpodTransport, path: LibpodPath) -> PodmanLensResult<LibpodResponse> {
    let request = LibpodRequest::new(LibpodMethod::Get, path, Vec::new())?;
    transport
        .send(&request)
        .await
        .map_err(|error| error.diagnostic().clone())
}

fn list_path(api_version: &ObservedApiVersion, kind: ResourceKind) -> PodmanLensResult<LibpodPath> {
    let query = match kind {
        ResourceKind::Container => "?all=true&sync=true",
        // The Libpod default omits intermediate images. A complete inventory cannot inherit that
        // presentation default, even though M3 will treat images as shared prerequisites.
        ResourceKind::Image => "?all=true",
        _ => "",
    };
    LibpodPath::parse(format!(
        "/v{}/libpod/{}/json{query}",
        api_version.original(),
        kind.collection()
    ))
}

fn decode_list(kind: ResourceKind, response: &LibpodResponse) -> PodmanLensResult<ListedSection> {
    require_ok_json(response)?;
    let value = decode_json(response.body())?;
    let entries = match kind {
        ResourceKind::Volume => match value.as_object().and_then(|object| object.get("Volumes")) {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(entries)) => entries.iter().collect(),
            Some(_) => return Err(Diagnostic::new(DiagnosticCode::InventoryShape)),
        },
        _ => value
            .as_array()
            .map(|entries| entries.iter().collect())
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::InventoryShape))?,
    };
    let mut identities = Vec::with_capacity(entries.len());
    let mut findings = Vec::new();
    for entry in entries {
        match list_identity(kind, entry) {
            Ok(identity)
                if identities
                    .iter()
                    .any(|previous: &ResourceIdentity| previous.id == identity.id) =>
            {
                findings.push(InventoryFinding::for_resource(
                    DiagnosticCode::ResourceMalformed,
                    identity,
                ));
            }
            Ok(identity) => identities.push(identity),
            Err(_) => findings.push(InventoryFinding::section(DiagnosticCode::ResourceMalformed)),
        }
    }
    identities.sort_by(|left, right| left.id.cmp(&right.id).then_with(|| left.name.cmp(&right.name)));
    Ok(ListedSection { identities, findings })
}

fn list_identity(kind: ResourceKind, value: &Value) -> PodmanLensResult<ResourceIdentity> {
    let object = value
        .as_object()
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::InventoryShape))?;
    let id = match kind {
        ResourceKind::Volume => required_string(object, "Name")?,
        ResourceKind::Secret => required_string(object, "ID")?,
        ResourceKind::Network => required_string(object, "id")?,
        _ => required_string(object, "Id")?,
    };
    let name = match kind {
        ResourceKind::Container | ResourceKind::Image => first_string(object.get("Names")),
        ResourceKind::Secret => object
            .get("Spec")
            .and_then(Value::as_object)
            .and_then(|spec| optional_string(spec, "Name")),
        ResourceKind::Network => optional_string(object, "name"),
        _ => optional_string(object, "Name"),
    };
    Ok(ResourceIdentity::new(kind, id.to_owned(), name.map(ToOwned::to_owned)))
}

fn decode_observation(
    listed_identity: &ResourceIdentity,
    response: &LibpodResponse,
    evidence: ResourceEvidence,
    options: AcquisitionOptions,
    unknown_field_limit: usize,
) -> PodmanLensResult<ResourceObservation> {
    require_ok_json(response)?;
    let value = decode_json(response.body())?;
    let object = value
        .as_object()
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::InventoryShape))?;
    let (
        identity,
        labels,
        relationships,
        environment,
        image_aliases,
        network,
        memory_swappiness,
        configured_image,
        local_image_id,
        is_infra,
        secret_driver,
        volume_owner_user,
        volume_owner_group,
        mut findings,
        known,
    ) = match listed_identity.kind {
        ResourceKind::Container => decode_container(listed_identity, object, options, &evidence)?,
        ResourceKind::Pod => decode_pod(listed_identity, object)?,
        ResourceKind::Network => decode_network(listed_identity, object)?,
        ResourceKind::Volume => decode_volume(listed_identity, object)?,
        ResourceKind::Image => decode_image(listed_identity, object, options)?,
        ResourceKind::Secret => decode_secret(listed_identity, object)?,
    };
    let mut unknown_fields = UnknownFieldCollector::new(&identity, &evidence, unknown_field_limit);
    unknown_top_level(object, known, &mut unknown_fields);
    unknown_nested_fields(listed_identity.kind, object, &mut unknown_fields);
    let (unknown_fields, unknown_field_overflow) = unknown_fields.finish();
    if unknown_field_overflow {
        findings.push(InventoryFinding::field(
            DiagnosticCode::UnknownFieldOverflow,
            identity.clone(),
            "$",
        ));
    }
    findings.extend(
        unknown_fields.iter().map(|field| {
            InventoryFinding::field(DiagnosticCode::NativeFieldUnsupported, identity.clone(), field.path())
        }),
    );
    let header = ObservationHeader::complete(
        identity.clone(),
        evidence,
        findings,
        unknown_fields,
        if unknown_field_overflow {
            UnmodelledCompleteness::Incomplete
        } else {
            UnmodelledCompleteness::Complete
        },
    );
    ResourceObservation::try_new(
        header,
        details_from_decoded(
            identity.kind(),
            DecodedDetails {
                labels,
                relationships,
                environment,
                image_aliases,
                network,
                memory_swappiness,
                configured_image,
                local_image_id,
                is_infra,
                secret_driver,
                volume_owner_user,
                volume_owner_group,
            },
        ),
    )
}

type Decoded = (
    ResourceIdentity,
    ObservationField<Labels>,
    ObservationField<Vec<NativeRelationship>>,
    ObservationField<ProtectedEnvironment>,
    ObservationField<Vec<String>>,
    Option<NetworkDecoded>,
    Option<ObservationField<u64>>,
    Option<ObservationField<String>>,
    Option<ObservationField<String>>,
    Option<ObservationField<bool>>,
    Option<ObservationField<String>>,
    Option<ObservationField<VolumeOwnerIdWireValue>>,
    Option<ObservationField<VolumeOwnerIdWireValue>>,
    Vec<InventoryFinding>,
    &'static [&'static str],
);

type NetworkDecoded = (
    ObservationField<bool>,
    ObservationField<NetworkOptionKeys>,
    ObservationField<Vec<String>>,
);

struct DecodedDetails {
    labels: ObservationField<Labels>,
    relationships: ObservationField<Vec<NativeRelationship>>,
    environment: ObservationField<ProtectedEnvironment>,
    image_aliases: ObservationField<Vec<String>>,
    network: Option<NetworkDecoded>,
    memory_swappiness: Option<ObservationField<u64>>,
    configured_image: Option<ObservationField<String>>,
    local_image_id: Option<ObservationField<String>>,
    is_infra: Option<ObservationField<bool>>,
    secret_driver: Option<ObservationField<String>>,
    volume_owner_user: Option<ObservationField<VolumeOwnerIdWireValue>>,
    volume_owner_group: Option<ObservationField<VolumeOwnerIdWireValue>>,
}

fn details_from_decoded(kind: ResourceKind, details: DecodedDetails) -> ResourceDetails {
    let DecodedDetails {
        labels,
        relationships,
        environment,
        image_aliases,
        network,
        memory_swappiness,
        configured_image,
        local_image_id,
        is_infra,
        secret_driver,
        volume_owner_user,
        volume_owner_group,
    } = details;
    match kind {
        ResourceKind::Container => ResourceDetails::Container(ContainerObservation::new(
            labels,
            configured_image.unwrap_or(ObservationField::NotApplicable),
            local_image_id.unwrap_or(ObservationField::NotApplicable),
            relationships,
            environment,
            memory_swappiness.unwrap_or(ObservationField::NotApplicable),
            is_infra.unwrap_or(ObservationField::Absent),
        )),
        ResourceKind::Pod => ResourceDetails::Pod(PodObservation::new(labels, relationships)),
        ResourceKind::Network => {
            let (internal, options, subnets) = network.unwrap_or((
                ObservationField::NotApplicable,
                ObservationField::NotApplicable,
                ObservationField::NotApplicable,
            ));
            ResourceDetails::Network(NetworkObservation::new(labels, internal, options, subnets))
        }
        ResourceKind::Volume => ResourceDetails::Volume(VolumeObservation::new(
            labels,
            volume_owner_user.unwrap_or(ObservationField::Malformed),
            volume_owner_group.unwrap_or(ObservationField::Malformed),
        )),
        ResourceKind::Image => ResourceDetails::Image(ImageObservation::new(labels, image_aliases, environment)),
        ResourceKind::Secret => ResourceDetails::Secret(SecretObservation::new(
            labels,
            secret_driver.unwrap_or(ObservationField::Absent),
        )),
    }
}

fn partial_observation(
    identity: ResourceIdentity,
    evidence: ResourceEvidence,
    finding: DiagnosticCode,
) -> ResourceObservation {
    ResourceObservation::incomplete(ObservationHeader::incomplete(
        identity.clone(),
        evidence,
        if finding == DiagnosticCode::ResourceUnavailable {
            ResourceObservationState::Unavailable
        } else {
            ResourceObservationState::Malformed
        },
        vec![InventoryFinding::for_resource(finding, identity)],
    ))
}

fn decode_container(
    listed: &ResourceIdentity,
    object: &Map<String, Value>,
    options: AcquisitionOptions,
    evidence: &ResourceEvidence,
) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["Id"], &["Name"])?;
    let mut findings = Vec::new();
    let (labels, environment) = decode_container_configuration(object, options, &identity, &mut findings);
    let mut relationships = Vec::new();
    let local_image_id = optional_string_field(
        object.get("Image"),
        "$.Image",
        &identity,
        ObservationOrigin::LocalResolution,
        &mut findings,
    );
    let configured_image = optional_string_field(
        object.get("ImageName"),
        "$.ImageName",
        &identity,
        ObservationOrigin::Configured,
        &mut findings,
    );
    append_configured_image_relationship(&configured_image, &mut relationships);
    let mut relationship_decoding = RelationshipDecoding::from_field(&configured_image);
    relationship_decoding.merge(append_optional_relationship(
        object,
        "Pod",
        ResourceKind::Pod,
        "$.Pod",
        &identity,
        &mut relationships,
        &mut findings,
    ));
    relationship_decoding.merge(decode_container_networks(
        object,
        &identity,
        &mut relationships,
        &mut findings,
    ));
    relationship_decoding.merge(decode_mounts(object, &identity, &mut relationships, &mut findings));
    relationship_decoding.merge(decode_dependencies(
        object,
        &identity,
        &mut relationships,
        &mut findings,
    ));
    relationship_decoding.merge(decode_container_secrets(
        object,
        &identity,
        &mut relationships,
        &mut findings,
    ));
    let memory_swappiness = decode_memory_swappiness(object, evidence, &identity, &mut findings);
    let is_infra = decode_is_infra(object, &identity, &mut findings);
    Ok((
        identity,
        labels,
        relationship_field(relationships, relationship_decoding),
        environment,
        ObservationField::NotApplicable,
        None,
        Some(memory_swappiness),
        Some(configured_image),
        Some(local_image_id),
        Some(is_infra),
        None,
        None,
        None,
        findings,
        &[
            "Id",
            "Name",
            "Image",
            "ImageName",
            "Pod",
            "NetworkSettings",
            "Mounts",
            "Dependencies",
            "Config",
            "HostConfig",
            "IsInfra",
        ],
    ))
}

fn decode_pod(listed: &ResourceIdentity, object: &Map<String, Value>) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["Id"], &["Name"])?;
    let mut relationships = Vec::new();
    let mut findings = Vec::new();
    let labels = labels(object.get("Labels"), "$.Labels", &identity, &mut findings);
    let mut relationship_decoding = decode_pod_containers(object, &identity, &mut relationships, &mut findings);
    relationship_decoding.merge(decode_pod_networks(
        object,
        &identity,
        &mut relationships,
        &mut findings,
    ));
    Ok((
        identity,
        labels,
        relationship_field(relationships, relationship_decoding),
        ObservationField::NotApplicable,
        ObservationField::NotApplicable,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        findings,
        &["Id", "Name", "Labels", "Containers", "Networks"],
    ))
}

fn decode_container_configuration(
    object: &Map<String, Value>,
    options: AcquisitionOptions,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> (ObservationField<Labels>, ObservationField<ProtectedEnvironment>) {
    let config = match object.get("Config") {
        None | Some(Value::Null) => return (ObservationField::Absent, ObservationField::Absent),
        Some(Value::Object(config)) => config,
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config",
            ));
            return (ObservationField::Malformed, ObservationField::Malformed);
        }
    };
    let labels = labels(config.get("Labels"), "$.Config.Labels", identity, findings);
    let (environment, environment_findings) =
        decode_environment(config.get("Env"), options.environment_values, identity, "$.Config.Env");
    findings.extend(environment_findings);
    (labels, environment)
}

fn decode_network(listed: &ResourceIdentity, object: &Map<String, Value>) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["id"], &["name"])?;
    let (network, mut findings) = decode_network_details(object, &identity);
    let labels = labels(object.get("labels"), "$.labels", &identity, &mut findings);
    Ok((
        identity,
        labels,
        ObservationField::NotApplicable,
        ObservationField::NotApplicable,
        ObservationField::NotApplicable,
        Some(network),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        findings,
        &["id", "name", "labels", "internal", "options", "subnets"],
    ))
}

fn decode_network_details(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
) -> (NetworkDecoded, Vec<InventoryFinding>) {
    let mut findings = Vec::new();
    let internal = match object.get("internal") {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Bool(value)) => {
            ObservationField::Observed(ObservedValue::new(*value, ObservationOrigin::Effective))
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.internal",
            ));
            ObservationField::Malformed
        }
    };
    let options = match string_map(object.get("options")) {
        Ok(Some(options)) => ObservationField::Observed(ObservedValue::new(
            NetworkOptionKeys::new(options.into_keys()),
            ObservationOrigin::Effective,
        )),
        Ok(None) => ObservationField::Absent,
        Err(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.options",
            ));
            ObservationField::Malformed
        }
    };
    let subnets = match object.get("subnets") {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Array(subnets)) => {
            let mut decoded = Vec::new();
            let mut malformed = false;
            for (index, subnet) in subnets.iter().enumerate() {
                let value = subnet
                    .as_object()
                    .and_then(|subnet| required_string(subnet, "subnet").ok());
                if let Some(value) = value {
                    decoded.push(value.to_owned());
                } else {
                    malformed = true;
                    findings.push(InventoryFinding::at_occurrence(
                        DiagnosticCode::ResourceMalformed,
                        identity.clone(),
                        "$.subnets",
                        index,
                    ));
                }
            }
            if malformed {
                ObservationField::Malformed
            } else {
                ObservationField::Observed(ObservedValue::new(decoded, ObservationOrigin::Effective))
            }
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.subnets",
            ));
            ObservationField::Malformed
        }
    };
    ((internal, options, subnets), findings)
}

fn decode_volume(listed: &ResourceIdentity, object: &Map<String, Value>) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["Name"], &["Name"])?;
    let mut findings = Vec::new();
    let labels = labels(object.get("Labels"), "$.Labels", &identity, &mut findings);
    let uid = decode_volume_owner(object.get("UID"), "$.UID", &identity, &mut findings);
    let gid = decode_volume_owner(object.get("GID"), "$.GID", &identity, &mut findings);
    Ok((
        identity,
        labels,
        ObservationField::NotApplicable,
        ObservationField::NotApplicable,
        ObservationField::NotApplicable,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(uid),
        Some(gid),
        findings,
        &["Name", "Labels", "UID", "GID"],
    ))
}

fn decode_volume_owner(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<VolumeOwnerIdWireValue> {
    match value {
        None => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::VolumeOwnerDefaultAmbiguous,
                identity.clone(),
                path,
            ));
            ObservationField::Observed(ObservedValue::new(
                VolumeOwnerIdWireValue::WireAbsentMayMeanZero,
                ObservationOrigin::Effective,
            ))
        }
        Some(Value::Number(value)) => {
            if let Some(value) = value.as_u64().and_then(|value| u32::try_from(value).ok()) {
                ObservationField::Observed(ObservedValue::new(
                    VolumeOwnerIdWireValue::Explicit(VolumeOwnerUnixId::new(value)),
                    ObservationOrigin::Effective,
                ))
            } else {
                findings.push(InventoryFinding::field(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    path,
                ));
                ObservationField::Malformed
            }
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
            ));
            ObservationField::Malformed
        }
    }
}

fn decode_image(
    listed: &ResourceIdentity,
    object: &Map<String, Value>,
    options: AcquisitionOptions,
) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["Id"], &["Names"])?;
    let mut findings = Vec::new();
    let labels = labels(object.get("Labels"), "$.Labels", &identity, &mut findings);
    let (config, config_malformed) = match object.get("Config") {
        None | Some(Value::Null) => (None, false),
        Some(Value::Object(config)) => (Some(config), false),
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config",
            ));
            (None, true)
        }
    };
    let environment = if config_malformed {
        ObservationField::Malformed
    } else {
        let (environment, mut environment_findings) = decode_environment(
            config.and_then(|config| config.get("Env")),
            options.environment_values,
            &identity,
            "$.Config.Env",
        );
        findings.append(&mut environment_findings);
        environment
    };
    let aliases = image_aliases(object.get("Names"), &identity, &mut findings);
    Ok((
        identity,
        labels,
        ObservationField::NotApplicable,
        environment,
        aliases,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        findings,
        &["Id", "Names", "Labels", "Config"],
    ))
}

fn decode_secret(listed: &ResourceIdentity, object: &Map<String, Value>) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["ID"], &["Spec"])?;
    let spec = object
        .get("Spec")
        .and_then(Value::as_object)
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::ResourceMalformed))?;
    let mut findings = Vec::new();
    let labels = labels(spec.get("Labels"), "$.Spec.Labels", &identity, &mut findings);
    if object.contains_key("SecretData") || spec.contains_key("SecretData") {
        findings.push(InventoryFinding::for_resource(
            DiagnosticCode::SecretPayloadDiscarded,
            identity.clone(),
        ));
    }
    let driver = match spec.get("Driver") {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::String(driver)) => {
            ObservationField::Observed(ObservedValue::new(driver.to_owned(), ObservationOrigin::Configured))
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Spec.Driver",
            ));
            ObservationField::Malformed
        }
    };
    Ok((
        identity,
        labels,
        ObservationField::NotApplicable,
        ObservationField::NotApplicable,
        ObservationField::NotApplicable,
        None,
        None,
        None,
        None,
        None,
        Some(driver),
        None,
        None,
        findings,
        &["ID", "Spec", "SecretData"],
    ))
}

fn identity_from_inspect(
    listed: &ResourceIdentity,
    object: &Map<String, Value>,
    id_keys: &[&str],
    name_keys: &[&str],
) -> PodmanLensResult<ResourceIdentity> {
    let id = required_string_any(object, id_keys)?;
    if id != listed.id() {
        return Err(Diagnostic::new(DiagnosticCode::ResourceMalformed));
    }
    let name = if listed.kind == ResourceKind::Secret {
        object
            .get("Spec")
            .and_then(Value::as_object)
            .and_then(|spec| optional_string(spec, "Name"))
    } else {
        optional_string_any(object, name_keys).or_else(|| listed.name())
    };
    Ok(ResourceIdentity::new(
        listed.kind,
        id.to_owned(),
        name.map(ToOwned::to_owned),
    ))
}

#[derive(Clone, Copy, Default)]
struct RelationshipDecoding {
    supplied: bool,
    malformed: bool,
}

impl RelationshipDecoding {
    fn from_field<T>(field: &ObservationField<T>) -> Self {
        Self {
            supplied: !matches!(field, ObservationField::Absent | ObservationField::NotApplicable),
            malformed: matches!(field, ObservationField::Malformed),
        }
    }

    fn merge(&mut self, other: Self) {
        self.supplied |= other.supplied;
        self.malformed |= other.malformed;
    }
}

fn append_configured_image_relationship(
    configured_image: &ObservationField<String>,
    relationships: &mut Vec<NativeRelationship>,
) {
    if let ObservationField::Observed(value) = configured_image {
        relationships.push(NativeRelationship::new(
            ResourceKind::Image,
            value.value(),
            "$.ImageName",
        ));
    }
}

fn append_optional_relationship(
    object: &Map<String, Value>,
    key: &str,
    kind: ResourceKind,
    path: &str,
    identity: &ResourceIdentity,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> RelationshipDecoding {
    match object.get(key) {
        None | Some(Value::Null) => RelationshipDecoding::default(),
        Some(Value::String(value)) if !value.is_empty() => {
            relationships.push(NativeRelationship::new(kind, value, path));
            RelationshipDecoding {
                supplied: true,
                malformed: false,
            }
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
            ));
            RelationshipDecoding {
                supplied: true,
                malformed: true,
            }
        }
    }
}

fn optional_string_field(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<String> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::String(value)) if !value.is_empty() => {
            ObservationField::Observed(ObservedValue::new(value.clone(), origin))
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
            ));
            ObservationField::Malformed
        }
    }
}

fn decode_container_networks(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> RelationshipDecoding {
    let Some(settings) = object.get("NetworkSettings") else {
        return RelationshipDecoding::default();
    };
    if settings.is_null() {
        return RelationshipDecoding::default();
    }
    let Some(settings) = settings.as_object() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.NetworkSettings",
        ));
        return RelationshipDecoding {
            supplied: true,
            malformed: true,
        };
    };
    let Some(networks) = settings.get("Networks") else {
        return RelationshipDecoding::default();
    };
    if networks.is_null() {
        return RelationshipDecoding::default();
    }
    let Some(networks) = networks.as_object() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.NetworkSettings.Networks",
        ));
        return RelationshipDecoding {
            supplied: true,
            malformed: true,
        };
    };
    let mut malformed = false;
    for (name, details) in networks {
        if name.is_empty() || !details.is_object() && !details.is_null() {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                format!("$.NetworkSettings.Networks.{name}"),
            ));
            malformed = true;
            continue;
        }
        relationships.push(NativeRelationship::new(
            ResourceKind::Network,
            name,
            format!("$.NetworkSettings.Networks.{name}"),
        ));
    }
    RelationshipDecoding {
        supplied: true,
        malformed,
    }
}

fn decode_mounts(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> RelationshipDecoding {
    let Some(mounts) = object.get("Mounts") else {
        return RelationshipDecoding::default();
    };
    if mounts.is_null() {
        return RelationshipDecoding::default();
    }
    let Some(mounts) = mounts.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Mounts",
        ));
        return RelationshipDecoding {
            supplied: true,
            malformed: true,
        };
    };
    let mut malformed = false;
    for (index, mount) in mounts.iter().enumerate() {
        let Some(mount) = mount.as_object() else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Mounts",
                index,
            ));
            malformed = true;
            continue;
        };
        match mount.get("Type") {
            Some(Value::String(kind)) if kind == "volume" => {
                if let Ok(name) = required_string(mount, "Name") {
                    relationships.push(NativeRelationship::new(
                        ResourceKind::Volume,
                        name,
                        format!("$.Mounts[{index}].Name"),
                    ));
                } else {
                    malformed = true;
                    findings.push(InventoryFinding::at_occurrence(
                        DiagnosticCode::ResourceMalformed,
                        identity.clone(),
                        "$.Mounts",
                        index,
                    ));
                }
            }
            Some(Value::String(_)) => {}
            _ => {
                malformed = true;
                findings.push(InventoryFinding::at_occurrence(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    "$.Mounts",
                    index,
                ));
            }
        }
    }
    RelationshipDecoding {
        supplied: true,
        malformed,
    }
}

fn decode_dependencies(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> RelationshipDecoding {
    let Some(dependencies) = object.get("Dependencies") else {
        return RelationshipDecoding::default();
    };
    if dependencies.is_null() {
        return RelationshipDecoding::default();
    }
    let Some(dependencies) = dependencies.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Dependencies",
        ));
        return RelationshipDecoding {
            supplied: true,
            malformed: true,
        };
    };
    let mut malformed = false;
    for (index, dependency) in dependencies.iter().enumerate() {
        if let Some(value) = dependency.as_str().filter(|value| !value.is_empty()) {
            relationships.push(NativeRelationship::new(
                ResourceKind::Container,
                value,
                format!("$.Dependencies[{index}]"),
            ));
        } else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Dependencies",
                index,
            ));
        }
    }
    RelationshipDecoding {
        supplied: true,
        malformed,
    }
}

fn decode_container_secrets(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> RelationshipDecoding {
    let Some(config) = object.get("Config") else {
        return RelationshipDecoding::default();
    };
    if config.is_null() {
        return RelationshipDecoding::default();
    }
    let Some(config) = config.as_object() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Config",
        ));
        return RelationshipDecoding {
            supplied: true,
            malformed: true,
        };
    };
    let Some(secrets) = config.get("Secrets") else {
        return RelationshipDecoding::default();
    };
    if secrets.is_null() {
        return RelationshipDecoding::default();
    }
    let Some(secrets) = secrets.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Config.Secrets",
        ));
        return RelationshipDecoding {
            supplied: true,
            malformed: true,
        };
    };
    let mut malformed = false;
    for (index, secret) in secrets.iter().enumerate() {
        let Some(secret) = secret.as_object() else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config.Secrets",
                index,
            ));
            malformed = true;
            continue;
        };
        let mut references = Vec::new();
        let mut member_malformed = false;
        for key in ["ID", "Name"] {
            match secret.get(key) {
                None | Some(Value::Null) => {}
                Some(Value::String(value)) if !value.is_empty() => {
                    references.push((value.to_owned(), format!("$.Config.Secrets[{index}].{key}")));
                }
                Some(_) => {
                    member_malformed = true;
                    findings.push(InventoryFinding::at_occurrence(
                        DiagnosticCode::ResourceMalformed,
                        identity.clone(),
                        "$.Config.Secrets",
                        index,
                    ));
                }
            }
        }
        if let Some(relationship) = (!member_malformed)
            .then(|| NativeRelationship::coalesced(ResourceKind::Secret, references))
            .flatten()
        {
            relationships.push(relationship);
        } else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config.Secrets",
                index,
            ));
        }
    }
    RelationshipDecoding {
        supplied: true,
        malformed,
    }
}

fn decode_pod_containers(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> RelationshipDecoding {
    let Some(containers) = object.get("Containers") else {
        return RelationshipDecoding::default();
    };
    if containers.is_null() {
        return RelationshipDecoding::default();
    }
    let Some(containers) = containers.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Containers",
        ));
        return RelationshipDecoding {
            supplied: true,
            malformed: true,
        };
    };
    let mut malformed = false;
    for (index, container) in containers.iter().enumerate() {
        if let Some(id) = container
            .as_object()
            .and_then(|container| required_string(container, "Id").ok())
        {
            relationships.push(NativeRelationship::new(
                ResourceKind::Container,
                id,
                format!("$.Containers[{index}].Id"),
            ));
        } else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Containers",
                index,
            ));
        }
    }
    RelationshipDecoding {
        supplied: true,
        malformed,
    }
}

fn decode_pod_networks(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> RelationshipDecoding {
    let Some(networks) = object.get("Networks") else {
        return RelationshipDecoding::default();
    };
    if networks.is_null() {
        return RelationshipDecoding::default();
    }
    let Some(networks) = networks.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Networks",
        ));
        return RelationshipDecoding {
            supplied: true,
            malformed: true,
        };
    };
    let mut malformed = false;
    for (index, network) in networks.iter().enumerate() {
        if let Some(name) = network.as_str().filter(|name| !name.is_empty()) {
            relationships.push(NativeRelationship::new(
                ResourceKind::Network,
                name,
                format!("$.Networks[{index}]"),
            ));
        } else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Networks",
                index,
            ));
        }
    }
    RelationshipDecoding {
        supplied: true,
        malformed,
    }
}

fn decode_memory_swappiness(
    object: &Map<String, Value>,
    evidence: &ResourceEvidence,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<u64> {
    let Some(host_config) = object.get("HostConfig") else {
        return ObservationField::Absent;
    };
    if host_config.is_null() {
        return ObservationField::Absent;
    }
    let Some(host_config) = host_config.as_object() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.HostConfig",
        ));
        return ObservationField::Malformed;
    };
    let Some(value) = host_config.get("MemorySwappiness") else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        if evidence.api_version().starts_with("5.4.") {
            findings.push(InventoryFinding::field(
                DiagnosticCode::VersionInapplicableField,
                identity.clone(),
                "$.HostConfig.MemorySwappiness",
            ));
            return ObservationField::VersionInapplicable;
        }
        return ObservationField::Absent;
    }
    if let Some(value) = value.as_u64() {
        ObservationField::Observed(ObservedValue::new(value, ObservationOrigin::Effective))
    } else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.HostConfig.MemorySwappiness",
        ));
        ObservationField::Malformed
    }
}

fn decode_is_infra(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<bool> {
    match object.get("IsInfra") {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Bool(value)) => {
            ObservationField::Observed(ObservedValue::new(*value, ObservationOrigin::Effective))
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.IsInfra",
            ));
            ObservationField::Malformed
        }
    }
}

fn image_aliases(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Vec<String>> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(values) = value.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Names",
        ));
        return ObservationField::Malformed;
    };
    let mut aliases = Vec::with_capacity(values.len());
    let mut malformed = false;
    for (index, value) in values.iter().enumerate() {
        if let Some(value) = value.as_str().filter(|value| !value.is_empty()) {
            aliases.push(value.to_owned());
        } else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Names",
                index,
            ));
        }
    }
    if malformed {
        ObservationField::Malformed
    } else {
        ObservationField::Observed(ObservedValue::new(aliases, ObservationOrigin::LocalResolution))
    }
}

fn relationship_field(
    relationships: Vec<NativeRelationship>,
    decoding: RelationshipDecoding,
) -> ObservationField<Vec<NativeRelationship>> {
    if decoding.malformed {
        ObservationField::Malformed
    } else if !decoding.supplied {
        ObservationField::Absent
    } else {
        ObservationField::Observed(ObservedValue::new(relationships, ObservationOrigin::Effective))
    }
}

fn decode_environment(
    value: Option<&Value>,
    policy: EnvironmentValuePolicy,
    identity: &ResourceIdentity,
    path: &str,
) -> (ObservationField<ProtectedEnvironment>, Vec<InventoryFinding>) {
    let Some(value) = value else {
        return (ObservationField::Absent, Vec::new());
    };
    if value.is_null() {
        return (ObservationField::Absent, Vec::new());
    }
    let Some(entries) = value.as_array() else {
        return (
            ObservationField::Malformed,
            vec![InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
            )],
        );
    };
    let mut decoded = Vec::with_capacity(entries.len());
    let mut findings = Vec::new();
    let mut malformed = false;
    for (index, entry) in entries.iter().enumerate() {
        let Some(entry) = entry.as_str() else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::EnvironmentMalformed,
                identity.clone(),
                path,
                index,
            ));
            continue;
        };
        let Some((name, value)) = entry.split_once('=') else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::EnvironmentMalformed,
                identity.clone(),
                path,
                index,
            ));
            continue;
        };
        if name.is_empty() {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::EnvironmentMalformed,
                identity.clone(),
                path,
                index,
            ));
            continue;
        }
        decoded.push(ProtectedEnvironmentEntry::new(
            name.to_owned(),
            match policy {
                EnvironmentValuePolicy::Redact => ProtectedEnvironmentValue::Redacted,
                EnvironmentValuePolicy::Include => {
                    ProtectedEnvironmentValue::AuthorizedOpaque(SensitiveEnvironmentValue::new(value.to_owned()))
                }
            },
        ));
    }
    let field = if malformed {
        ObservationField::Malformed
    } else {
        ObservationField::Observed(ObservedValue::new(
            ProtectedEnvironment::new(decoded),
            ObservationOrigin::Effective,
        ))
    };
    (field, findings)
}

fn labels(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Labels> {
    match string_map(value) {
        Ok(Some(labels)) => ObservationField::Observed(ObservedValue::new(labels, ObservationOrigin::Configured)),
        Ok(None) => ObservationField::Absent,
        Err(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
            ));
            ObservationField::Malformed
        }
    }
}

fn string_map(value: Option<&Value>) -> PodmanLensResult<Option<BTreeMap<String, String>>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let object = value
        .as_object()
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::ResourceMalformed))?;
    object
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|value| (key.clone(), value.to_owned()))
                .ok_or_else(|| Diagnostic::new(DiagnosticCode::ResourceMalformed))
        })
        .collect::<PodmanLensResult<BTreeMap<_, _>>>()
        .map(Some)
}

struct UnknownFieldCollector<'a> {
    resource: &'a ResourceIdentity,
    evidence: &'a ResourceEvidence,
    limit: usize,
    fields: Vec<UnmodelledField>,
    overflowed: bool,
}

impl<'a> UnknownFieldCollector<'a> {
    fn new(resource: &'a ResourceIdentity, evidence: &'a ResourceEvidence, limit: usize) -> Self {
        Self {
            resource,
            evidence,
            limit,
            fields: Vec::new(),
            overflowed: false,
        }
    }

    fn push(&mut self, path: impl FnOnce() -> String, value: &Value) -> bool {
        if self.fields.len() >= self.limit {
            self.overflowed = true;
            return false;
        }
        self.fields.push(UnmodelledField::new(
            path(),
            json_value_kind(value),
            self.resource.clone(),
            self.evidence.clone(),
        ));
        true
    }

    fn finish(self) -> (Vec<UnmodelledField>, bool) {
        (self.fields, self.overflowed)
    }
}

fn unknown_top_level(object: &Map<String, Value>, known: &[&str], fields: &mut UnknownFieldCollector<'_>) {
    for (key, value) in object.iter().filter(|(key, _)| !known.contains(&key.as_str())) {
        if !fields.push(|| format!("$.{key}"), value) {
            break;
        }
    }
}

fn unknown_nested_fields(kind: ResourceKind, object: &Map<String, Value>, fields: &mut UnknownFieldCollector<'_>) {
    match kind {
        ResourceKind::Container => {
            unknown_object_members(object.get("Config"), "$.Config", &["Labels", "Env", "Secrets"], fields);
            unknown_object_members(
                object.get("NetworkSettings"),
                "$.NetworkSettings",
                &["Networks"],
                fields,
            );
            if let Some(networks) = object
                .get("NetworkSettings")
                .and_then(Value::as_object)
                .and_then(|settings| settings.get("Networks"))
                .and_then(Value::as_object)
            {
                for (name, details) in networks {
                    unknown_object_members(
                        Some(details),
                        &format!("$.NetworkSettings.Networks.{name}"),
                        &[],
                        fields,
                    );
                    if fields.overflowed {
                        break;
                    }
                }
            }
            unknown_array_object_members(object.get("Mounts"), "$.Mounts", &["Type", "Name"], fields);
            unknown_array_object_members(
                object
                    .get("Config")
                    .and_then(Value::as_object)
                    .and_then(|config| config.get("Secrets")),
                "$.Config.Secrets",
                &["ID", "Name"],
                fields,
            );
            // `MemorySwappiness` is the only currently typed HostConfig member. Every other
            // direct member is deliberately retained as unsupported metadata instead of being
            // hidden by the accepted HostConfig object.
            unknown_object_members(object.get("HostConfig"), "$.HostConfig", &["MemorySwappiness"], fields);
        }
        ResourceKind::Pod => unknown_array_object_members(object.get("Containers"), "$.Containers", &["Id"], fields),
        ResourceKind::Network => unknown_array_object_members(object.get("subnets"), "$.subnets", &["subnet"], fields),
        ResourceKind::Image => unknown_object_members(object.get("Config"), "$.Config", &["Env"], fields),
        ResourceKind::Secret => {
            unknown_object_members(
                object.get("Spec"),
                "$.Spec",
                &["Name", "Labels", "Driver", "SecretData"],
                fields,
            );
        }
        ResourceKind::Volume => {}
    }
}

fn unknown_object_members(value: Option<&Value>, path: &str, known: &[&str], fields: &mut UnknownFieldCollector<'_>) {
    let Some(object) = value.and_then(Value::as_object) else {
        return;
    };
    for (key, value) in object.iter().filter(|(key, _)| !known.contains(&key.as_str())) {
        if !fields.push(|| format!("{path}.{key}"), value) {
            break;
        }
    }
}

fn unknown_array_object_members(
    value: Option<&Value>,
    path: &str,
    known: &[&str],
    fields: &mut UnknownFieldCollector<'_>,
) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    for (index, value) in values.iter().enumerate() {
        unknown_object_members(Some(value), &format!("{path}[{index}]"), known, fields);
        if fields.overflowed {
            break;
        }
    }
}

fn require_ok_json(response: &LibpodResponse) -> PodmanLensResult<()> {
    if response.status() != 200 {
        return Err(Diagnostic::new(DiagnosticCode::InventoryHttpStatus));
    }
    let values = response.headers().values("content-type").collect::<Vec<_>>();
    let [value] = values.as_slice() else {
        return Err(Diagnostic::new(DiagnosticCode::InventoryShape));
    };
    if value
        .split(';')
        .next()
        .is_some_and(|media_type| media_type.trim().eq_ignore_ascii_case("application/json"))
    {
        Ok(())
    } else {
        Err(Diagnostic::new(DiagnosticCode::InventoryShape))
    }
}

fn decode_json(body: &[u8]) -> PodmanLensResult<Value> {
    if body.len() > MAX_INVENTORY_JSON_BYTES {
        return Err(Diagnostic::new(DiagnosticCode::InventoryJson));
    }
    serde_json::from_slice(body).map_err(|_| Diagnostic::new(DiagnosticCode::InventoryJson))
}

fn required_string<'a>(object: &'a Map<String, Value>, key: &str) -> PodmanLensResult<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::InventoryShape))
}

fn required_string_any<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> PodmanLensResult<&'a str> {
    keys.iter()
        .find_map(|key| {
            object
                .get(*key)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
        })
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::InventoryShape))
}

fn optional_string<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn optional_string_any<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| optional_string(object, key))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::items_after_test_module)]
mod typed_observation_constructor_tests {
    use super::*;

    fn header(kind: ResourceKind) -> ObservationHeader {
        let capability = capability_catalogue().expect("embedded capability catalogue").remove(0);
        ObservationHeader::complete(
            ResourceIdentity::new(kind, format!("{kind:?}-id"), None),
            ResourceEvidence {
                engine_version: "6.1.0".to_owned(),
                api_version: "6.1.0".to_owned(),
                capability,
            },
            Vec::new(),
            Vec::new(),
            UnmodelledCompleteness::Complete,
        )
    }

    #[test]
    fn kind_safe_resource_observation_constructor_accepts_every_matching_detail_and_rejects_mismatches() {
        let details = [
            ResourceDetails::Container(ContainerObservation::new(
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
            )),
            ResourceDetails::Pod(PodObservation::new(ObservationField::Absent, ObservationField::Absent)),
            ResourceDetails::Network(NetworkObservation::new(
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
            )),
            ResourceDetails::Volume(VolumeObservation::new(
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
            )),
            ResourceDetails::Image(ImageObservation::new(
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
            )),
            ResourceDetails::Secret(SecretObservation::new(
                ObservationField::Absent,
                ObservationField::Absent,
            )),
        ];
        for detail in details {
            assert!(ResourceObservation::try_new(header(detail.kind()), detail).is_ok());
        }

        let error = ResourceObservation::try_new(
            header(ResourceKind::Container),
            ResourceDetails::Pod(PodObservation::new(ObservationField::Absent, ObservationField::Absent)),
        )
        .expect_err("kind mismatch must be a structured construction failure");
        assert_eq!(error.code(), DiagnosticCode::ResourceMalformed);
    }
}

fn first_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_array)
        .and_then(|values| values.iter().find_map(Value::as_str))
        .filter(|value| !value.is_empty())
}

const fn json_value_kind(value: &Value) -> JsonValueKind {
    match value {
        Value::Null => JsonValueKind::Null,
        Value::Bool(_) => JsonValueKind::Boolean,
        Value::Number(_) => JsonValueKind::Number,
        Value::String(_) => JsonValueKind::String,
        Value::Array(_) => JsonValueKind::Array,
        Value::Object(_) => JsonValueKind::Object,
    }
}
