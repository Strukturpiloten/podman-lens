//! Read-only, versioned acquisition of a redacted native Podman inventory.
//!
//! Wire JSON stays private to this module. The public inventory deliberately retains only typed
//! identity, relationship, labels, bounded unknown-field metadata, evidence, and findings.

use std::{collections::BTreeMap, fmt};

use serde_json::{Map, Value};

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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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

/// Stable native identity for one observed resource.
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

/// The observed completeness of one record in a non-atomic inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ObservationState {
    /// The inspect response was decoded.
    Complete,
    /// A listed resource disappeared or could not be decoded during inspection.
    Partial,
}

/// A typed relationship asserted by a native inspect response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceRelationship {
    kind: ResourceKind,
    target_id: String,
    field_path: String,
}

impl ResourceRelationship {
    fn new(kind: ResourceKind, target_id: impl Into<String>, field_path: impl Into<String>) -> Self {
        Self {
            kind,
            target_id: target_id.into(),
            field_path: field_path.into(),
        }
    }

    /// Returns the kind of the referenced prerequisite or member resource.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Returns the native reference spelling exactly as reported by Podman.
    #[must_use]
    pub fn target_id(&self) -> &str {
        &self.target_id
    }

    /// Returns the exact source field that asserted this relationship.
    #[must_use]
    pub fn field_path(&self) -> &str {
        &self.field_path
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

/// Metadata for native data that M2 does not decode into a typed field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownNativeField {
    path: String,
    json_kind: JsonValueKind,
    resource: ResourceIdentity,
    evidence: ResourceEvidence,
}

impl UnknownNativeField {
    fn new(path: String, value: &Value, resource: ResourceIdentity, evidence: ResourceEvidence) -> Self {
        Self {
            path,
            json_kind: json_value_kind(value),
            resource,
            evidence,
        }
    }

    /// Returns the JSON path relative to the resource root.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns only the unknown field's JSON kind, never its raw value.
    #[must_use]
    pub const fn json_kind(&self) -> JsonValueKind {
        self.json_kind
    }

    /// Returns the resource that carried the unknown field.
    #[must_use]
    pub fn resource(&self) -> &ResourceIdentity {
        &self.resource
    }

    /// Returns the source/version evidence that gives the unknown field meaning.
    #[must_use]
    pub fn evidence(&self) -> &ResourceEvidence {
        &self.evidence
    }
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

/// One environment entry in its original source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentEntry {
    name: String,
    value: EnvironmentValue,
}

impl EnvironmentEntry {
    /// Returns the environment variable name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether the runtime value was redacted or explicitly included.
    #[must_use]
    pub fn value(&self) -> &EnvironmentValue {
        &self.value
    }
}

/// The safely retained representation of an environment value.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EnvironmentValue {
    /// The value was not requested and is not retained.
    Redacted,
    /// The value was explicitly requested and remains opaque to formatting and serialization.
    Included(SensitiveEnvironmentValue),
}

/// Typed network properties that affect connectivity but do not imply application ownership.
#[derive(Clone, Eq, PartialEq)]
pub struct NetworkDetails {
    internal: Option<bool>,
    options: BTreeMap<String, String>,
    subnets: Vec<String>,
}

impl fmt::Debug for NetworkDetails {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NetworkDetails")
            .field("internal", &self.internal)
            .field("option_count", &self.options.len())
            .field("subnet_count", &self.subnets.len())
            .finish()
    }
}

impl NetworkDetails {
    /// Returns the explicit internal-network property, when the service reported it.
    #[must_use]
    pub const fn internal(&self) -> Option<bool> {
        self.internal
    }

    /// Returns native driver options without interpreting them as ownership evidence.
    #[must_use]
    pub fn options(&self) -> &BTreeMap<String, String> {
        &self.options
    }

    /// Returns reported subnet spellings in source order.
    #[must_use]
    pub fn subnets(&self) -> &[String] {
        &self.subnets
    }
}

/// A typed resource observation independent of the private Libpod wire response.
#[derive(Clone, Eq, PartialEq)]
pub struct ResourceRecord {
    identity: ResourceIdentity,
    state: ObservationState,
    labels: BTreeMap<String, String>,
    relationships: Vec<ResourceRelationship>,
    environment: Vec<EnvironmentEntry>,
    image_aliases: Vec<String>,
    network: Option<NetworkDetails>,
    memory_swappiness: Option<u64>,
    secret_driver: Option<String>,
    unknown_fields: Vec<UnknownNativeField>,
    findings: Vec<InventoryFinding>,
    evidence: ResourceEvidence,
}

impl ResourceRecord {
    fn partial(identity: ResourceIdentity, evidence: ResourceEvidence, finding: DiagnosticCode) -> Self {
        Self {
            identity: identity.clone(),
            state: ObservationState::Partial,
            labels: BTreeMap::new(),
            relationships: Vec::new(),
            environment: Vec::new(),
            image_aliases: Vec::new(),
            network: None,
            memory_swappiness: None,
            secret_driver: None,
            unknown_fields: Vec::new(),
            findings: vec![InventoryFinding::for_resource(finding, identity)],
            evidence,
        }
    }

    /// Returns the stable resource identity.
    #[must_use]
    pub fn identity(&self) -> &ResourceIdentity {
        &self.identity
    }

    /// Returns whether this record was fully inspected.
    #[must_use]
    pub const fn state(&self) -> ObservationState {
        self.state
    }

    /// Returns labels exactly as represented by the JSON object.
    #[must_use]
    pub fn labels(&self) -> &BTreeMap<String, String> {
        &self.labels
    }

    /// Returns native relationships in source order.
    #[must_use]
    pub fn relationships(&self) -> &[ResourceRelationship] {
        &self.relationships
    }

    /// Returns duplicate-preserving runtime environment entries in source order.
    #[must_use]
    pub fn environment(&self) -> &[EnvironmentEntry] {
        &self.environment
    }

    /// Returns an image's native aliases in their original source order; other records return an empty slice.
    #[must_use]
    pub fn image_aliases(&self) -> &[String] {
        &self.image_aliases
    }

    /// Returns network-specific connectivity fields for a network record only.
    #[must_use]
    pub fn network(&self) -> Option<&NetworkDetails> {
        self.network.as_ref()
    }

    /// Returns the modeled `HostConfig.MemorySwappiness` value when present and version-applicable.
    #[must_use]
    pub const fn memory_swappiness(&self) -> Option<u64> {
        self.memory_swappiness
    }

    /// Returns the native secret driver metadata for a secret record when reported.
    #[must_use]
    pub fn secret_driver(&self) -> Option<&str> {
        self.secret_driver.as_deref()
    }

    /// Returns metadata for native fields not yet modeled by M2.
    #[must_use]
    pub fn unknown_fields(&self) -> &[UnknownNativeField] {
        &self.unknown_fields
    }

    /// Returns non-fatal findings attached to this record.
    #[must_use]
    pub fn findings(&self) -> &[InventoryFinding] {
        &self.findings
    }

    /// Returns source and version evidence for this record.
    #[must_use]
    pub fn evidence(&self) -> &ResourceEvidence {
        &self.evidence
    }
}

impl fmt::Debug for ResourceRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceRecord")
            .field("identity", &self.identity)
            .field("state", &self.state)
            .field("label_count", &self.labels.len())
            .field("relationships", &self.relationships)
            .field("environment", &self.environment)
            .field("image_alias_count", &self.image_aliases.len())
            .field("network", &self.network)
            .field("memory_swappiness", &self.memory_swappiness)
            .field("has_secret_driver", &self.secret_driver.is_some())
            .field("unknown_fields", &self.unknown_fields)
            .field("findings", &self.findings)
            .field("evidence", &self.evidence)
            .finish()
    }
}

/// Availability and records for one independently listed resource kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventorySection {
    kind: ResourceKind,
    available: bool,
    records: Vec<ResourceRecord>,
    findings: Vec<InventoryFinding>,
}

impl InventorySection {
    fn unavailable(kind: ResourceKind, code: DiagnosticCode) -> Self {
        Self {
            kind,
            available: false,
            records: Vec::new(),
            findings: vec![InventoryFinding::section(code)],
        }
    }

    /// Returns the resource kind represented by this section.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Returns whether the kind's list response could be decoded.
    #[must_use]
    pub const fn available(&self) -> bool {
        self.available
    }

    /// Returns records in the deterministic list response order.
    #[must_use]
    pub fn records(&self) -> &[ResourceRecord] {
        &self.records
    }

    /// Returns kind-wide findings that have no stable individual resource.
    #[must_use]
    pub fn findings(&self) -> &[InventoryFinding] {
        &self.findings
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
                let mut records = Vec::with_capacity(listed.identities.len());
                for identity in listed.identities {
                    let record = inspect_record(
                        transport,
                        service.api_version(),
                        evidence.clone(),
                        identity,
                        options,
                        remaining_unknown_fields.min(MAX_UNKNOWN_FIELDS_PER_RECORD),
                    )
                    .await;
                    remaining_unknown_fields = remaining_unknown_fields.saturating_sub(record.unknown_fields.len());
                    records.push(record);
                }
                sections.push(InventorySection {
                    kind,
                    available: true,
                    records,
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
        .map(|section| (section.kind, section.available))
        .collect::<BTreeMap<_, _>>();
    let mut targets = BTreeMap::<(ResourceKind, String), Vec<String>>::new();
    for section in sections.iter() {
        for record in &section.records {
            let identity = record.identity();
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
            for alias in record.image_aliases() {
                targets
                    .entry((identity.kind(), alias.clone()))
                    .or_default()
                    .push(identity.id().to_owned());
            }
        }
    }
    for section in sections.iter_mut() {
        for record in &mut section.records {
            let unresolved = record
                .relationships
                .iter()
                .filter(|relationship| available.get(&relationship.kind()).copied().unwrap_or(false))
                .filter(|relationship| {
                    !targets.contains_key(&(relationship.kind(), relationship.target_id().to_owned()))
                })
                .map(|relationship| relationship.field_path().to_owned())
                .collect::<Vec<_>>();
            record.findings.extend(unresolved.into_iter().map(|path| {
                InventoryFinding::field(DiagnosticCode::UnresolvedRelationship, record.identity.clone(), path)
            }));
        }
    }

    let pod_members = sections
        .iter()
        .filter(|section| section.kind == ResourceKind::Pod && section.available)
        .flat_map(|section| section.records.iter())
        .map(|pod| {
            (
                pod.identity.id.clone(),
                pod.relationships
                    .iter()
                    .filter(|relationship| relationship.kind == ResourceKind::Container)
                    .filter_map(|relationship| {
                        canonical_target(&targets, ResourceKind::Container, relationship.target_id())
                            .map(ToOwned::to_owned)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let container_pods = sections
        .iter()
        .filter(|section| section.kind == ResourceKind::Container && section.available)
        .flat_map(|section| section.records.iter())
        .map(|container| {
            (
                container.identity.id.clone(),
                container
                    .relationships
                    .iter()
                    .filter(|relationship| relationship.kind == ResourceKind::Pod)
                    .filter_map(|relationship| {
                        canonical_target(&targets, ResourceKind::Pod, relationship.target_id()).map(ToOwned::to_owned)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for section in sections {
        for record in &mut section.records {
            let disagrees = match record.identity.kind {
                ResourceKind::Pod => pod_members.get(&record.identity.id).is_some_and(|members| {
                    members.iter().any(|member| {
                        container_pods
                            .get(member)
                            .is_some_and(|pods| !pods.contains(&record.identity.id))
                    })
                }),
                ResourceKind::Container => container_pods.get(&record.identity.id).is_some_and(|pods| {
                    pods.iter().any(|pod| {
                        pod_members
                            .get(pod)
                            .is_some_and(|members| !members.contains(&record.identity.id))
                    })
                }),
                _ => false,
            };
            if disagrees {
                record.findings.push(InventoryFinding::field(
                    DiagnosticCode::PodMembershipConflict,
                    record.identity.clone(),
                    "$.PodMembership",
                ));
            }
        }
    }
}

fn canonical_target<'a>(
    targets: &'a BTreeMap<(ResourceKind, String), Vec<String>>,
    kind: ResourceKind,
    reference: &str,
) -> Option<&'a str> {
    let candidates = targets.get(&(kind, reference.to_owned()))?;
    let [identity] = candidates.as_slice() else {
        return None;
    };
    Some(identity)
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

async fn inspect_record(
    transport: &dyn LibpodTransport,
    api_version: &ObservedApiVersion,
    evidence: ResourceEvidence,
    identity: ResourceIdentity,
    options: AcquisitionOptions,
    unknown_field_limit: usize,
) -> ResourceRecord {
    let Ok(path) = LibpodPath::resource(api_version, identity.kind.collection(), identity.id(), "json") else {
        return ResourceRecord::partial(identity, evidence, DiagnosticCode::ResourceMalformed);
    };
    let response = send_get(transport, path).await;
    match response {
        Ok(response) if response.status() == 404 => {
            ResourceRecord::partial(identity, evidence, DiagnosticCode::ResourceUnavailable)
        }
        Ok(response) => match decode_record(&identity, &response, evidence.clone(), options, unknown_field_limit) {
            Ok(record) => record,
            Err(error) => ResourceRecord::partial(identity, evidence, error.code()),
        },
        Err(_) => ResourceRecord::partial(identity, evidence, DiagnosticCode::ResourceUnavailable),
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

fn decode_record(
    listed_identity: &ResourceIdentity,
    response: &LibpodResponse,
    evidence: ResourceEvidence,
    options: AcquisitionOptions,
    unknown_field_limit: usize,
) -> PodmanLensResult<ResourceRecord> {
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
        secret_driver,
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
    Ok(ResourceRecord {
        identity,
        state: ObservationState::Complete,
        labels,
        relationships,
        environment,
        image_aliases,
        network,
        memory_swappiness,
        secret_driver,
        unknown_fields,
        findings,
        evidence,
    })
}

type Decoded = (
    ResourceIdentity,
    BTreeMap<String, String>,
    Vec<ResourceRelationship>,
    Vec<EnvironmentEntry>,
    Vec<String>,
    Option<NetworkDetails>,
    Option<u64>,
    Option<String>,
    Vec<InventoryFinding>,
    &'static [&'static str],
);

fn decode_container(
    listed: &ResourceIdentity,
    object: &Map<String, Value>,
    options: AcquisitionOptions,
    evidence: &ResourceEvidence,
) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["Id"], &["Name"])?;
    let labels = labels(
        object
            .get("Config")
            .and_then(Value::as_object)
            .and_then(|config| config.get("Labels")),
    )?;
    let mut relationships = Vec::new();
    let mut findings = Vec::new();
    append_optional_relationship(
        object,
        "Image",
        ResourceKind::Image,
        "$.Image",
        &identity,
        &mut relationships,
        &mut findings,
    );
    append_optional_relationship(
        object,
        "ImageName",
        ResourceKind::Image,
        "$.ImageName",
        &identity,
        &mut relationships,
        &mut findings,
    );
    append_optional_relationship(
        object,
        "Pod",
        ResourceKind::Pod,
        "$.Pod",
        &identity,
        &mut relationships,
        &mut findings,
    );
    decode_container_networks(object, &identity, &mut relationships, &mut findings);
    decode_mounts(object, &identity, &mut relationships, &mut findings);
    decode_dependencies(object, &identity, &mut relationships, &mut findings);
    decode_container_secrets(object, &identity, &mut relationships, &mut findings);
    let (environment, environment_findings) = decode_environment(
        object
            .get("Config")
            .and_then(Value::as_object)
            .and_then(|config| config.get("Env")),
        options.environment_values,
        &identity,
    );
    findings.extend(environment_findings);
    let memory_swappiness = decode_memory_swappiness(object, evidence, &identity, &mut findings);
    Ok((
        identity,
        labels,
        relationships,
        environment,
        Vec::new(),
        None,
        memory_swappiness,
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
        ],
    ))
}

fn decode_pod(listed: &ResourceIdentity, object: &Map<String, Value>) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["Id"], &["Name"])?;
    let labels = labels(object.get("Labels"))?;
    let mut relationships = Vec::new();
    let mut findings = Vec::new();
    decode_pod_containers(object, &identity, &mut relationships, &mut findings);
    decode_pod_networks(object, &identity, &mut relationships, &mut findings);
    Ok((
        identity,
        labels,
        relationships,
        Vec::new(),
        Vec::new(),
        None,
        None,
        None,
        findings,
        &["Id", "Name", "Labels", "Containers", "Networks"],
    ))
}

fn decode_network(listed: &ResourceIdentity, object: &Map<String, Value>) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["id"], &["name"])?;
    let labels = labels(object.get("labels"))?;
    let (network, findings) = decode_network_details(object, &identity);
    Ok((
        identity,
        labels,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Some(network),
        None,
        None,
        findings,
        &["id", "name", "labels", "internal", "options", "subnets"],
    ))
}

fn decode_network_details(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
) -> (NetworkDetails, Vec<InventoryFinding>) {
    let mut findings = Vec::new();
    let internal = match object.get("internal") {
        Some(value) if value.is_null() => None,
        Some(value) => {
            if let Some(value) = value.as_bool() {
                Some(value)
            } else {
                findings.push(InventoryFinding::field(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    "$.internal",
                ));
                None
            }
        }
        None => None,
    };
    let options = if let Ok(options) = string_map(object.get("options")) {
        options
    } else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.options",
        ));
        BTreeMap::new()
    };
    let subnets = match object.get("subnets") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(subnets)) => {
            let mut decoded = Vec::new();
            for (index, subnet) in subnets.iter().enumerate() {
                let value = subnet
                    .as_object()
                    .and_then(|subnet| required_string(subnet, "subnet").ok());
                match value {
                    Some(value) => decoded.push(value.to_owned()),
                    None => findings.push(InventoryFinding::at_occurrence(
                        DiagnosticCode::ResourceMalformed,
                        identity.clone(),
                        "$.subnets",
                        index,
                    )),
                }
            }
            decoded
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.subnets",
            ));
            Vec::new()
        }
    };
    (
        NetworkDetails {
            internal,
            options,
            subnets,
        },
        findings,
    )
}

fn decode_volume(listed: &ResourceIdentity, object: &Map<String, Value>) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["Name"], &["Name"])?;
    let labels = labels(object.get("Labels"))?;
    Ok((
        identity,
        labels,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        None,
        None,
        Vec::new(),
        &["Name", "Labels"],
    ))
}

fn decode_image(
    listed: &ResourceIdentity,
    object: &Map<String, Value>,
    options: AcquisitionOptions,
) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["Id"], &["Names"])?;
    let labels = labels(object.get("Labels"))?;
    let mut findings = Vec::new();
    let config = match object.get("Config") {
        None | Some(Value::Null) => None,
        Some(Value::Object(config)) => Some(config),
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config",
            ));
            None
        }
    };
    let (environment, mut environment_findings) = decode_environment(
        config.and_then(|config| config.get("Env")),
        options.environment_values,
        &identity,
    );
    findings.append(&mut environment_findings);
    let aliases = image_aliases(object.get("Names"), &identity, &mut findings);
    Ok((
        identity,
        labels,
        Vec::new(),
        environment,
        aliases,
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
    let labels = labels(spec.get("Labels"))?;
    let mut findings = Vec::new();
    if object.contains_key("SecretData") || spec.contains_key("SecretData") {
        findings.push(InventoryFinding::for_resource(
            DiagnosticCode::SecretPayloadDiscarded,
            identity.clone(),
        ));
    }
    let driver = match spec.get("Driver") {
        None | Some(Value::Null) => None,
        Some(Value::String(driver)) if !driver.is_empty() => Some(driver.to_owned()),
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Spec.Driver",
            ));
            None
        }
    };
    Ok((
        identity,
        labels,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        None,
        driver,
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

fn append_optional_relationship(
    object: &Map<String, Value>,
    key: &str,
    kind: ResourceKind,
    path: &str,
    identity: &ResourceIdentity,
    relationships: &mut Vec<ResourceRelationship>,
    findings: &mut Vec<InventoryFinding>,
) {
    match object.get(key) {
        None | Some(Value::Null) => {}
        Some(Value::String(value)) if !value.is_empty() => {
            relationships.push(ResourceRelationship::new(kind, value, path));
        }
        Some(_) => findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            path,
        )),
    }
}

fn decode_container_networks(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<ResourceRelationship>,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(settings) = object.get("NetworkSettings") else {
        return;
    };
    if settings.is_null() {
        return;
    }
    let Some(settings) = settings.as_object() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.NetworkSettings",
        ));
        return;
    };
    let Some(networks) = settings.get("Networks") else {
        return;
    };
    if networks.is_null() {
        return;
    }
    let Some(networks) = networks.as_object() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.NetworkSettings.Networks",
        ));
        return;
    };
    for (name, details) in networks {
        if name.is_empty() || !details.is_object() && !details.is_null() {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                format!("$.NetworkSettings.Networks.{name}"),
            ));
            continue;
        }
        relationships.push(ResourceRelationship::new(
            ResourceKind::Network,
            name,
            format!("$.NetworkSettings.Networks.{name}"),
        ));
    }
}

fn decode_mounts(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<ResourceRelationship>,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(mounts) = object.get("Mounts") else { return };
    if mounts.is_null() {
        return;
    }
    let Some(mounts) = mounts.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Mounts",
        ));
        return;
    };
    for (index, mount) in mounts.iter().enumerate() {
        let Some(mount) = mount.as_object() else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Mounts",
                index,
            ));
            continue;
        };
        match mount.get("Type") {
            Some(Value::String(kind)) if kind == "volume" => match required_string(mount, "Name") {
                Ok(name) => relationships.push(ResourceRelationship::new(
                    ResourceKind::Volume,
                    name,
                    format!("$.Mounts[{index}].Name"),
                )),
                Err(_) => findings.push(InventoryFinding::at_occurrence(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    "$.Mounts",
                    index,
                )),
            },
            Some(Value::String(_)) => {}
            _ => findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Mounts",
                index,
            )),
        }
    }
}

fn decode_dependencies(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<ResourceRelationship>,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(dependencies) = object.get("Dependencies") else {
        return;
    };
    if dependencies.is_null() {
        return;
    }
    let Some(dependencies) = dependencies.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Dependencies",
        ));
        return;
    };
    for (index, dependency) in dependencies.iter().enumerate() {
        match dependency.as_str().filter(|value| !value.is_empty()) {
            Some(value) => relationships.push(ResourceRelationship::new(
                ResourceKind::Container,
                value,
                format!("$.Dependencies[{index}]"),
            )),
            None => findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Dependencies",
                index,
            )),
        }
    }
}

fn decode_container_secrets(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<ResourceRelationship>,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(config) = object.get("Config") else { return };
    if config.is_null() {
        return;
    }
    let Some(config) = config.as_object() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Config",
        ));
        return;
    };
    let Some(secrets) = config.get("Secrets") else { return };
    if secrets.is_null() {
        return;
    }
    let Some(secrets) = secrets.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Config.Secrets",
        ));
        return;
    };
    for (index, secret) in secrets.iter().enumerate() {
        let Some(secret) = secret.as_object() else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config.Secrets",
                index,
            ));
            continue;
        };
        let mut valid = false;
        for key in ["ID", "Name"] {
            match secret.get(key) {
                None | Some(Value::Null) => {}
                Some(Value::String(value)) if !value.is_empty() => {
                    relationships.push(ResourceRelationship::new(
                        ResourceKind::Secret,
                        value,
                        format!("$.Config.Secrets[{index}].{key}"),
                    ));
                    valid = true;
                }
                Some(_) => findings.push(InventoryFinding::at_occurrence(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    "$.Config.Secrets",
                    index,
                )),
            }
        }
        if !valid {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config.Secrets",
                index,
            ));
        }
    }
}

fn decode_pod_containers(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<ResourceRelationship>,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(containers) = object.get("Containers") else {
        return;
    };
    if containers.is_null() {
        return;
    }
    let Some(containers) = containers.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Containers",
        ));
        return;
    };
    for (index, container) in containers.iter().enumerate() {
        match container
            .as_object()
            .and_then(|container| required_string(container, "Id").ok())
        {
            Some(id) => relationships.push(ResourceRelationship::new(
                ResourceKind::Container,
                id,
                format!("$.Containers[{index}].Id"),
            )),
            None => findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Containers",
                index,
            )),
        }
    }
}

fn decode_pod_networks(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<ResourceRelationship>,
    findings: &mut Vec<InventoryFinding>,
) {
    let Some(networks) = object.get("Networks") else { return };
    if networks.is_null() {
        return;
    }
    let Some(networks) = networks.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Networks",
        ));
        return;
    };
    for (index, network) in networks.iter().enumerate() {
        match network.as_str().filter(|name| !name.is_empty()) {
            Some(name) => relationships.push(ResourceRelationship::new(
                ResourceKind::Network,
                name,
                format!("$.Networks[{index}]"),
            )),
            None => findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Networks",
                index,
            )),
        }
    }
}

fn decode_memory_swappiness(
    object: &Map<String, Value>,
    evidence: &ResourceEvidence,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> Option<u64> {
    let host_config = object.get("HostConfig")?;
    let Some(host_config) = host_config.as_object() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.HostConfig",
        ));
        return None;
    };
    let value = host_config.get("MemorySwappiness")?;
    if value.is_null() {
        if evidence.api_version().starts_with("5.4.") {
            findings.push(InventoryFinding::field(
                DiagnosticCode::VersionInapplicableField,
                identity.clone(),
                "$.HostConfig.MemorySwappiness",
            ));
        }
        return None;
    }
    if let Some(value) = value.as_u64() {
        Some(value)
    } else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.HostConfig.MemorySwappiness",
        ));
        None
    }
}

fn image_aliases(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> Vec<String> {
    let Some(value) = value else { return Vec::new() };
    if value.is_null() {
        return Vec::new();
    }
    let Some(values) = value.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Names",
        ));
        return Vec::new();
    };
    let mut aliases = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        match value.as_str().filter(|value| !value.is_empty()) {
            Some(value) => aliases.push(value.to_owned()),
            None => findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Names",
                index,
            )),
        }
    }
    aliases
}

fn decode_environment(
    value: Option<&Value>,
    policy: EnvironmentValuePolicy,
    identity: &ResourceIdentity,
) -> (Vec<EnvironmentEntry>, Vec<InventoryFinding>) {
    let Some(value) = value else {
        return (Vec::new(), Vec::new());
    };
    if value.is_null() {
        return (Vec::new(), Vec::new());
    }
    let Some(entries) = value.as_array() else {
        return (
            Vec::new(),
            vec![InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config.Env",
            )],
        );
    };
    let mut decoded = Vec::with_capacity(entries.len());
    let mut findings = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let Some(entry) = entry.as_str() else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::EnvironmentMalformed,
                identity.clone(),
                "$.Config.Env",
                index,
            ));
            continue;
        };
        let Some((name, value)) = entry.split_once('=') else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::EnvironmentMalformed,
                identity.clone(),
                "$.Config.Env",
                index,
            ));
            continue;
        };
        if name.is_empty() {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::EnvironmentMalformed,
                identity.clone(),
                "$.Config.Env",
                index,
            ));
            continue;
        }
        decoded.push(EnvironmentEntry {
            name: name.to_owned(),
            value: match policy {
                EnvironmentValuePolicy::Redact => EnvironmentValue::Redacted,
                EnvironmentValuePolicy::Include => {
                    EnvironmentValue::Included(SensitiveEnvironmentValue::new(value.to_owned()))
                }
            },
        });
    }
    (decoded, findings)
}

fn labels(value: Option<&Value>) -> PodmanLensResult<BTreeMap<String, String>> {
    string_map(value)
}

fn string_map(value: Option<&Value>) -> PodmanLensResult<BTreeMap<String, String>> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    if value.is_null() {
        return Ok(BTreeMap::new());
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
        .collect()
}

struct UnknownFieldCollector<'a> {
    resource: &'a ResourceIdentity,
    evidence: &'a ResourceEvidence,
    limit: usize,
    fields: Vec<UnknownNativeField>,
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
        self.fields.push(UnknownNativeField::new(
            path(),
            value,
            self.resource.clone(),
            self.evidence.clone(),
        ));
        true
    }

    fn finish(self) -> (Vec<UnknownNativeField>, bool) {
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
        }
        ResourceKind::Pod => unknown_array_object_members(object.get("Containers"), "$.Containers", &["Id"], fields),
        ResourceKind::Network => unknown_array_object_members(object.get("subnets"), "$.subnets", &["subnet"], fields),
        ResourceKind::Image => unknown_object_members(object.get("Config"), "$.Config", &["Env"], fields),
        ResourceKind::Secret => {
            unknown_object_members(object.get("Spec"), "$.Spec", &["Name", "Labels", "SecretData"], fields);
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
