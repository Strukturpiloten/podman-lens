//! Read-only, versioned acquisition of a redacted native Podman inventory.
//!
//! Wire JSON stays private to this module. The public inventory deliberately retains only typed
//! identity, relationship, labels, bounded unknown-field metadata, evidence, and findings.

use std::{collections::BTreeMap, fmt, net::IpAddr};

use serde_json::{Map, Value};

use crate::observation::{
    ConfiguredContainerCommand, ConfiguredContainerEntrypoint, ConfiguredContainerHostname, ConfiguredContainerUser,
    ConfiguredContainerWorkdir, ContainerMountKind, ContainerMountObservation, ContainerMountSource,
    ContainerObservation, ContainerSecretGrantObservation, ContainerSecretReference, ImageObservation, Labels,
    NativeCapability, NativeHealthCheckObservation, NativeHealthCommand, NativeHealthFailureAction,
    NativeIpcNamespaceMode, NativeLogDriver, NativeLoggingObservation, NativeNamespaceMode, NativeNamespaceObservation,
    NativeNetworkCidr, NativeNetworkLeaseRange, NativeNetworkRouteObservation, NativeNetworkRouteType,
    NativeNetworkSubnetObservation, NativeNetworkingObservation, NativeOpaqueNetworkOptions,
    NativeOpaqueSecurityOptions, NativePortBindingObservation, NativePortProtocol, NativeRelationship,
    NativeResourceControlObservation, NativeResourceReference, NativeRestartPolicyName, NativeRestartPolicyObservation,
    NativeSecurityObservation, NativeStartupHealthCheckObservation, NativeUlimitObservation, NetworkObservation,
    NetworkOptionKeys, ObservationField, ObservationHeader, ObservationOrigin, ObservedValue, PodObservation,
    ProtectedEnvironment, ProtectedEnvironmentEntry, ProtectedEnvironmentValue, ProtectedHealthCommand,
    ResourceDetails, ResourceObservation, ResourceObservationState, SecretObservation, UnixId as VolumeOwnerUnixId,
    UnmodelledCompleteness, UnmodelledField, VolumeObservation, VolumeOwnerIdWireValue,
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
        command,
        entrypoint,
        user,
        working_directory,
        hostname,
        pod_membership,
        native_dependencies,
        mounts,
        secret_grants,
        image_aliases,
        network,
        memory_swappiness,
        configured_image,
        local_image_id,
        is_infra,
        container_networking,
        pod_create_infra,
        pod_networking,
        secret_driver,
        volume_owner_user,
        volume_owner_group,
        container_b3,
        mut findings,
        known,
    ) = match listed_identity.kind {
        ResourceKind::Container => decode_container(listed_identity, object, options, &evidence)?,
        ResourceKind::Pod => decode_pod(listed_identity, object, &evidence)?,
        ResourceKind::Network => decode_network(listed_identity, object, &evidence)?,
        ResourceKind::Volume => decode_volume(listed_identity, object)?,
        ResourceKind::Image => decode_image(listed_identity, object, options)?,
        ResourceKind::Secret => decode_secret(listed_identity, object)?,
    };
    let mut unknown_fields = UnknownFieldCollector::new(&identity, &evidence, unknown_field_limit);
    unknown_top_level(object, known, &mut unknown_fields);
    unknown_nested_fields(listed_identity.kind, object, &evidence, &mut unknown_fields);
    append_container_unmodelled(&mut unknown_fields, container_b3.as_ref());
    let (unknown_fields, unmodelled_completeness) = finish_unknown_fields(unknown_fields, &identity, &mut findings);
    let header = ObservationHeader::complete(
        identity.clone(),
        evidence,
        findings,
        unknown_fields,
        unmodelled_completeness,
    );
    ResourceObservation::try_new(
        header,
        details_from_decoded(
            identity.kind(),
            DecodedDetails {
                labels,
                relationships,
                environment,
                command,
                entrypoint,
                user,
                working_directory,
                hostname,
                pod_membership,
                native_dependencies,
                mounts,
                secret_grants,
                image_aliases,
                network,
                memory_swappiness,
                configured_image,
                container_b3,
                local_image_id,
                is_infra,
                container_networking,
                pod_create_infra,
                pod_networking,
                secret_driver,
                volume_owner_user,
                volume_owner_group,
            },
        ),
    )
}

fn append_container_unmodelled(
    unknown_fields: &mut UnknownFieldCollector<'_>,
    container_b3: Option<&ContainerB3Decoded>,
) {
    if let Some(container_b3) = container_b3 {
        for (path, kind) in &container_b3.unmodelled {
            if !unknown_fields.push_kind(path.clone(), *kind) {
                break;
            }
        }
    }
}

fn finish_unknown_fields(
    unknown_fields: UnknownFieldCollector<'_>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> (Vec<UnmodelledField>, UnmodelledCompleteness) {
    let (unknown_fields, overflowed) = unknown_fields.finish();
    if overflowed {
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
    let completeness = if overflowed {
        UnmodelledCompleteness::Incomplete
    } else {
        UnmodelledCompleteness::Complete
    };
    (unknown_fields, completeness)
}

type Decoded = (
    ResourceIdentity,
    ObservationField<Labels>,
    ObservationField<Vec<NativeRelationship>>,
    ObservationField<ProtectedEnvironment>,
    Option<ObservationField<ConfiguredContainerCommand>>,
    Option<ObservationField<ConfiguredContainerEntrypoint>>,
    Option<ObservationField<ConfiguredContainerUser>>,
    Option<ObservationField<ConfiguredContainerWorkdir>>,
    Option<ObservationField<ConfiguredContainerHostname>>,
    Option<ObservationField<NativeResourceReference>>,
    Option<ObservationField<Vec<NativeResourceReference>>>,
    Option<ObservationField<Vec<ContainerMountObservation>>>,
    Option<ObservationField<Vec<ContainerSecretGrantObservation>>>,
    ObservationField<Vec<String>>,
    Option<NetworkDecoded>,
    Option<ObservationField<u64>>,
    Option<ObservationField<String>>,
    Option<ObservationField<String>>,
    Option<ObservationField<bool>>,
    Option<ObservationField<NativeNetworkingObservation>>,
    Option<ObservationField<bool>>,
    Option<ObservationField<NativeNetworkingObservation>>,
    Option<ObservationField<String>>,
    Option<ObservationField<VolumeOwnerIdWireValue>>,
    Option<ObservationField<VolumeOwnerIdWireValue>>,
    Option<ContainerB3Decoded>,
    Vec<InventoryFinding>,
    &'static [&'static str],
);

type NetworkDecoded = (
    ObservationField<bool>,
    ObservationField<NetworkOptionKeys>,
    ObservationField<Vec<NativeNetworkSubnetObservation>>,
    ObservationField<Vec<NativeNetworkRouteObservation>>,
);
struct ContainerB3Decoded {
    restart_policy: ObservationField<NativeRestartPolicyObservation>,
    health_check: ObservationField<NativeHealthCheckObservation>,
    health_failure_action: ObservationField<NativeHealthFailureAction>,
    startup_health_check: ObservationField<NativeStartupHealthCheckObservation>,
    logging: ObservationField<NativeLoggingObservation>,
    security: ObservationField<NativeSecurityObservation>,
    namespaces: ObservationField<NativeNamespaceObservation>,
    resource_controls: ObservationField<NativeResourceControlObservation>,
    unmodelled: Vec<(String, JsonValueKind)>,
}

impl Default for ContainerB3Decoded {
    fn default() -> Self {
        Self {
            restart_policy: ObservationField::NotApplicable,
            health_check: ObservationField::NotApplicable,
            health_failure_action: ObservationField::NotApplicable,
            startup_health_check: ObservationField::NotApplicable,
            logging: ObservationField::NotApplicable,
            security: ObservationField::NotApplicable,
            namespaces: ObservationField::NotApplicable,
            resource_controls: ObservationField::NotApplicable,
            unmodelled: Vec::new(),
        }
    }
}

struct DecodedDetails {
    labels: ObservationField<Labels>,
    relationships: ObservationField<Vec<NativeRelationship>>,
    environment: ObservationField<ProtectedEnvironment>,
    command: Option<ObservationField<ConfiguredContainerCommand>>,
    entrypoint: Option<ObservationField<ConfiguredContainerEntrypoint>>,
    user: Option<ObservationField<ConfiguredContainerUser>>,
    working_directory: Option<ObservationField<ConfiguredContainerWorkdir>>,
    hostname: Option<ObservationField<ConfiguredContainerHostname>>,
    pod_membership: Option<ObservationField<NativeResourceReference>>,
    native_dependencies: Option<ObservationField<Vec<NativeResourceReference>>>,
    mounts: Option<ObservationField<Vec<ContainerMountObservation>>>,
    secret_grants: Option<ObservationField<Vec<ContainerSecretGrantObservation>>>,
    image_aliases: ObservationField<Vec<String>>,
    network: Option<NetworkDecoded>,
    memory_swappiness: Option<ObservationField<u64>>,
    configured_image: Option<ObservationField<String>>,
    container_b3: Option<ContainerB3Decoded>,
    local_image_id: Option<ObservationField<String>>,
    is_infra: Option<ObservationField<bool>>,
    container_networking: Option<ObservationField<NativeNetworkingObservation>>,
    pod_create_infra: Option<ObservationField<bool>>,
    pod_networking: Option<ObservationField<NativeNetworkingObservation>>,
    secret_driver: Option<ObservationField<String>>,
    volume_owner_user: Option<ObservationField<VolumeOwnerIdWireValue>>,
    volume_owner_group: Option<ObservationField<VolumeOwnerIdWireValue>>,
}

fn details_from_decoded(kind: ResourceKind, details: DecodedDetails) -> ResourceDetails {
    let DecodedDetails {
        labels,
        relationships,
        environment,
        command,
        entrypoint,
        user,
        working_directory,
        hostname,
        pod_membership,
        native_dependencies,
        mounts,
        secret_grants,
        image_aliases,
        network,
        memory_swappiness,
        configured_image,
        local_image_id,
        is_infra,
        container_networking,
        pod_create_infra,
        pod_networking,
        secret_driver,
        volume_owner_user,
        container_b3,
        volume_owner_group,
    } = details;
    let ContainerB3Decoded {
        restart_policy,
        health_check,
        health_failure_action,
        startup_health_check,
        logging,
        security,
        namespaces,
        resource_controls,
        unmodelled: _,
    } = container_b3.unwrap_or_default();
    match kind {
        ResourceKind::Container => ResourceDetails::Container(ContainerObservation::new(
            labels,
            configured_image.unwrap_or(ObservationField::NotApplicable),
            local_image_id.unwrap_or(ObservationField::NotApplicable),
            relationships,
            environment,
            command.unwrap_or(ObservationField::NotApplicable),
            entrypoint.unwrap_or(ObservationField::NotApplicable),
            user.unwrap_or(ObservationField::NotApplicable),
            working_directory.unwrap_or(ObservationField::NotApplicable),
            hostname.unwrap_or(ObservationField::NotApplicable),
            pod_membership.unwrap_or(ObservationField::Absent),
            native_dependencies.unwrap_or(ObservationField::Absent),
            mounts.unwrap_or(ObservationField::Absent),
            secret_grants.unwrap_or(ObservationField::Absent),
            memory_swappiness.unwrap_or(ObservationField::NotApplicable),
            is_infra.unwrap_or(ObservationField::Absent),
            restart_policy,
            health_check,
            health_failure_action,
            startup_health_check,
            logging,
            security,
            namespaces,
            resource_controls,
            container_networking.unwrap_or(ObservationField::Absent),
        )),
        ResourceKind::Pod => ResourceDetails::Pod(PodObservation::new(
            labels,
            relationships,
            pod_create_infra.unwrap_or(ObservationField::Absent),
            pod_networking.unwrap_or(ObservationField::Absent),
        )),
        ResourceKind::Network => {
            let (internal, options, subnets, routes) = network.unwrap_or((
                ObservationField::NotApplicable,
                ObservationField::NotApplicable,
                ObservationField::NotApplicable,
                ObservationField::NotApplicable,
            ));
            ResourceDetails::Network(NetworkObservation::new(labels, internal, options, subnets, routes))
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
    let configuration = decode_container_configuration(object, options, &identity, &mut findings);
    let labels = configuration.labels;
    let environment = configuration.environment;
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
    let pod_membership = decode_native_reference(object.get("Pod"), "$.Pod", &identity, &mut findings);
    relationship_decoding.merge(append_native_reference_relationship(
        &pod_membership,
        ResourceKind::Pod,
        &mut relationships,
    ));
    let container_networking = decode_container_networking(object, &pod_membership, &identity, &mut findings);
    if matches!(pod_membership, ObservationField::Absent) {
        relationship_decoding.merge(decode_container_networks(
            object,
            &identity,
            &mut relationships,
            &mut findings,
        ));
    }
    let mounts = decode_container_mounts(object, &identity, &mut relationships, &mut findings);
    relationship_decoding.merge(mounts.relationships);
    let native_dependencies = decode_native_dependencies(object.get("Dependencies"), &identity, &mut findings);
    relationship_decoding.merge(append_native_dependency_relationships(
        &native_dependencies,
        &mut relationships,
    ));
    let secret_grants =
        decode_container_secret_grants(object.get("Config"), &identity, &mut relationships, &mut findings);
    relationship_decoding.merge(secret_grants.relationships);
    let memory_swappiness = decode_memory_swappiness(object, evidence, &identity, &mut findings);
    let container_b3 = decode_container_b3(object, &identity, &mut findings);
    let is_infra = decode_is_infra(object, &identity, &mut findings);
    Ok((
        identity,
        labels,
        relationship_field(relationships, relationship_decoding),
        environment,
        Some(configuration.command),
        Some(configuration.entrypoint),
        Some(configuration.user),
        Some(configuration.working_directory),
        Some(configuration.hostname),
        Some(pod_membership),
        Some(native_dependencies),
        Some(mounts.field),
        Some(secret_grants.field),
        ObservationField::NotApplicable,
        None,
        Some(memory_swappiness),
        Some(configured_image),
        Some(local_image_id),
        Some(is_infra),
        Some(container_networking),
        None,
        None,
        None,
        None,
        None,
        Some(container_b3),
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

fn decode_pod(
    listed: &ResourceIdentity,
    object: &Map<String, Value>,
    evidence: &ResourceEvidence,
) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["Id"], &["Name"])?;
    let mut relationships = Vec::new();
    let mut findings = Vec::new();
    let labels = labels(object.get("Labels"), "$.Labels", &identity, &mut findings);
    let mut relationship_decoding = decode_pod_containers(object, &identity, &mut relationships, &mut findings);
    let (create_infra, networking, networking_relationships) =
        decode_pod_networking(object, &identity, evidence, &mut relationships, &mut findings);
    relationship_decoding.merge(networking_relationships);
    Ok((
        identity,
        labels,
        relationship_field(relationships, relationship_decoding),
        ObservationField::NotApplicable,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        ObservationField::NotApplicable,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(create_infra),
        Some(networking),
        None,
        None,
        None,
        None,
        findings,
        &["Id", "Name", "Labels", "Containers", "CreateInfra", "InfraConfig"],
    ))
}

struct ContainerConfigurationDecoded {
    labels: ObservationField<Labels>,
    environment: ObservationField<ProtectedEnvironment>,
    command: ObservationField<ConfiguredContainerCommand>,
    entrypoint: ObservationField<ConfiguredContainerEntrypoint>,
    user: ObservationField<ConfiguredContainerUser>,
    working_directory: ObservationField<ConfiguredContainerWorkdir>,
    hostname: ObservationField<ConfiguredContainerHostname>,
}

fn decode_container_b3(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ContainerB3Decoded {
    let mut unmodelled = Vec::new();
    let (health_check, health_failure_action, startup_health_check) = match object.get("Config") {
        None | Some(Value::Null) => (
            ObservationField::Absent,
            ObservationField::Absent,
            ObservationField::Absent,
        ),
        Some(Value::Object(config)) => (
            decode_native_health_check(
                config.get("Healthcheck"),
                "$.Config.Healthcheck",
                identity,
                findings,
                &mut unmodelled,
            ),
            decode_native_health_failure_action(
                config.get("HealthcheckOnFailureAction"),
                identity,
                findings,
                &mut unmodelled,
            ),
            decode_native_startup_health_check(config.get("StartupHealthCheck"), identity, findings, &mut unmodelled),
        ),
        Some(_) => (
            native_malformed_field("$.Config", identity, findings),
            native_malformed_field("$.Config", identity, findings),
            native_malformed_field("$.Config", identity, findings),
        ),
    };
    let (restart_policy, logging, security, namespaces, resource_controls) = match object.get("HostConfig") {
        None | Some(Value::Null) => (
            ObservationField::Absent,
            ObservationField::Absent,
            ObservationField::Absent,
            ObservationField::Absent,
            ObservationField::Absent,
        ),
        Some(Value::Object(host_config)) => (
            decode_native_restart_policy(host_config.get("RestartPolicy"), identity, findings, &mut unmodelled),
            decode_native_logging(host_config.get("LogConfig"), identity, findings, &mut unmodelled),
            decode_native_security(host_config, identity, findings, &mut unmodelled),
            decode_native_namespaces(host_config, identity, findings, &mut unmodelled),
            decode_native_resource_controls(host_config, identity, findings),
        ),
        Some(_) => (
            native_malformed_field("$.HostConfig", identity, findings),
            native_malformed_field("$.HostConfig", identity, findings),
            native_malformed_field("$.HostConfig", identity, findings),
            native_malformed_field("$.HostConfig", identity, findings),
            native_malformed_field("$.HostConfig", identity, findings),
        ),
    };
    ContainerB3Decoded {
        restart_policy,
        health_check,
        health_failure_action,
        startup_health_check,
        logging,
        security,
        namespaces,
        resource_controls,
        unmodelled,
    }
}

fn decode_native_restart_policy(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<NativeRestartPolicyObservation> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(value) = value.as_object() else {
        return native_malformed_field("$.HostConfig.RestartPolicy", identity, findings);
    };
    let name = match value.get("Name") {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(json_value @ Value::String(name)) => match name.as_str() {
            "no" => ObservationField::Observed(ObservedValue::new(
                NativeRestartPolicyName::No,
                ObservationOrigin::Effective,
            )),
            "always" => ObservationField::Observed(ObservedValue::new(
                NativeRestartPolicyName::Always,
                ObservationOrigin::Effective,
            )),
            "on-failure" => ObservationField::Observed(ObservedValue::new(
                NativeRestartPolicyName::OnFailure,
                ObservationOrigin::Effective,
            )),
            "unless-stopped" => ObservationField::Observed(ObservedValue::new(
                NativeRestartPolicyName::UnlessStopped,
                ObservationOrigin::Effective,
            )),
            _ => native_unmodelled_field("$.HostConfig.RestartPolicy.Name", json_value, unmodelled),
        },
        Some(_) => native_malformed_field("$.HostConfig.RestartPolicy.Name", identity, findings),
    };
    let maximum_retry_count = native_u64_field(
        value.get("MaximumRetryCount"),
        "$.HostConfig.RestartPolicy.MaximumRetryCount",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    ObservationField::Observed(ObservedValue::new(
        NativeRestartPolicyObservation::new(name, maximum_retry_count),
        ObservationOrigin::Effective,
    ))
}

fn decode_native_health_check(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<NativeHealthCheckObservation> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(value) = value.as_object() else {
        return native_malformed_field(path, identity, findings);
    };
    let command = native_health_command(
        value.get("Test"),
        &format!("{path}.Test"),
        identity,
        findings,
        unmodelled,
    );
    let interval = native_i64_field(
        value.get("Interval"),
        &format!("{path}.Interval"),
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let timeout = native_i64_field(
        value.get("Timeout"),
        &format!("{path}.Timeout"),
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let retries = native_u64_field(
        value.get("Retries"),
        &format!("{path}.Retries"),
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let start_period = native_i64_field(
        value.get("StartPeriod"),
        &format!("{path}.StartPeriod"),
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    ObservationField::Observed(ObservedValue::new(
        NativeHealthCheckObservation::new(command, interval, timeout, retries, start_period),
        ObservationOrigin::Effective,
    ))
}

fn decode_native_startup_health_check(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<NativeStartupHealthCheckObservation> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(value) = value.as_object() else {
        return native_malformed_field("$.Config.StartupHealthCheck", identity, findings);
    };
    let command = native_health_command(
        value.get("Test"),
        "$.Config.StartupHealthCheck.Test",
        identity,
        findings,
        unmodelled,
    );
    let interval = native_i64_field(
        value.get("Interval"),
        "$.Config.StartupHealthCheck.Interval",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let timeout = native_i64_field(
        value.get("Timeout"),
        "$.Config.StartupHealthCheck.Timeout",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let retries = native_u64_field(
        value.get("Retries"),
        "$.Config.StartupHealthCheck.Retries",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let start_period = native_i64_field(
        value.get("StartPeriod"),
        "$.Config.StartupHealthCheck.StartPeriod",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let successes = native_u64_field(
        value.get("Successes"),
        "$.Config.StartupHealthCheck.Successes",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    ObservationField::Observed(ObservedValue::new(
        NativeStartupHealthCheckObservation::new(command, interval, timeout, retries, start_period, successes),
        ObservationOrigin::Effective,
    ))
}

fn native_health_command(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<NativeHealthCommand> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(values) = value.as_array() else {
        return native_malformed_field(path, identity, findings);
    };
    let Some(values) = values.iter().map(Value::as_str).collect::<Option<Vec<_>>>() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            path,
        ));
        return ObservationField::Malformed;
    };
    let Some((kind, arguments)) = values.split_first() else {
        return native_malformed_field(path, identity, findings);
    };
    let command = match *kind {
        "NONE" if arguments.is_empty() => NativeHealthCommand::Disabled,
        "CMD" if !arguments.is_empty() => NativeHealthCommand::Exec(ProtectedHealthCommand::new(
            arguments.iter().map(ToString::to_string).collect(),
        )),
        "CMD-SHELL" if !arguments.is_empty() => NativeHealthCommand::Shell(ProtectedHealthCommand::new(
            arguments.iter().map(ToString::to_string).collect(),
        )),
        "NONE" | "CMD" | "CMD-SHELL" => {
            return native_malformed_field(path, identity, findings);
        }
        _ => return native_unmodelled_field(path, value, unmodelled),
    };
    ObservationField::Observed(ObservedValue::new(command, ObservationOrigin::Effective))
}

fn decode_native_health_failure_action(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<NativeHealthFailureAction> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(action_name) = value.as_str() else {
        return native_malformed_field("$.Config.HealthcheckOnFailureAction", identity, findings);
    };
    let action = match action_name {
        "none" => NativeHealthFailureAction::None,
        "kill" => NativeHealthFailureAction::Kill,
        "restart" => NativeHealthFailureAction::Restart,
        "stop" => NativeHealthFailureAction::Stop,
        _ => {
            return native_unmodelled_field("$.Config.HealthcheckOnFailureAction", value, unmodelled);
        }
    };
    ObservationField::Observed(ObservedValue::new(action, ObservationOrigin::Effective))
}

fn decode_native_logging(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<NativeLoggingObservation> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(value) = value.as_object() else {
        return native_malformed_field("$.HostConfig.LogConfig", identity, findings);
    };
    let driver = match value.get("Type") {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(json_value @ Value::String(driver)) => match driver.as_str() {
            "journald" => ObservationField::Observed(ObservedValue::new(
                NativeLogDriver::Journald,
                ObservationOrigin::Effective,
            )),
            "k8s-file" => ObservationField::Observed(ObservedValue::new(
                NativeLogDriver::K8sFile,
                ObservationOrigin::Effective,
            )),
            _ => native_unmodelled_field("$.HostConfig.LogConfig.Type", json_value, unmodelled),
        },
        Some(_) => native_malformed_field("$.HostConfig.LogConfig.Type", identity, findings),
    };
    let size = native_string_field(
        value.get("Size"),
        "$.HostConfig.LogConfig.Size",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    ObservationField::Observed(ObservedValue::new(
        NativeLoggingObservation::new(driver, size),
        ObservationOrigin::Effective,
    ))
}

fn decode_native_security(
    host_config: &Map<String, Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<NativeSecurityObservation> {
    let privileged = native_bool_field(
        host_config.get("Privileged"),
        "$.HostConfig.Privileged",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let cap_add = native_capabilities(
        host_config.get("CapAdd"),
        "$.HostConfig.CapAdd",
        identity,
        findings,
        unmodelled,
    );
    let cap_drop = native_capabilities(
        host_config.get("CapDrop"),
        "$.HostConfig.CapDrop",
        identity,
        findings,
        unmodelled,
    );
    let security_options = native_security_options(host_config.get("SecurityOpt"), identity, findings);
    let read_only_root_filesystem = native_bool_field(
        host_config.get("ReadonlyRootfs"),
        "$.HostConfig.ReadonlyRootfs",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    ObservationField::Observed(ObservedValue::new(
        NativeSecurityObservation::new(
            privileged,
            cap_add,
            cap_drop,
            security_options,
            read_only_root_filesystem,
        ),
        ObservationOrigin::Effective,
    ))
}

fn native_capabilities(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<Vec<NativeCapability>> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(values) = value.as_array() else {
        return native_malformed_field(path, identity, findings);
    };
    let mut decoded = Vec::with_capacity(values.len());
    let mut unknown = false;
    for (index, value) in values.iter().enumerate() {
        let Some(capability) = value.as_str() else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
                index,
            ));
            return ObservationField::Malformed;
        };
        let Some(semantic) = capability.strip_prefix("CAP_") else {
            unmodelled.push((format!("{path}[{index}]"), JsonValueKind::String));
            unknown = true;
            continue;
        };
        if NATIVE_CAPABILITIES.contains(&semantic) {
            decoded.push(NativeCapability::new(capability.to_owned()));
        } else {
            unmodelled.push((format!("{path}[{index}]"), JsonValueKind::String));
            unknown = true;
        }
    }
    if unknown {
        ObservationField::Unmodelled(crate::UnmodelledFieldId::ContainerHostConfig)
    } else {
        ObservationField::Observed(ObservedValue::new(decoded, ObservationOrigin::Effective))
    }
}

const NATIVE_CAPABILITIES: &[&str] = &[
    "AUDIT_CONTROL",
    "AUDIT_READ",
    "AUDIT_WRITE",
    "BLOCK_SUSPEND",
    "BPF",
    "CHECKPOINT_RESTORE",
    "CHOWN",
    "DAC_OVERRIDE",
    "DAC_READ_SEARCH",
    "FOWNER",
    "FSETID",
    "IPC_LOCK",
    "IPC_OWNER",
    "KILL",
    "LEASE",
    "LINUX_IMMUTABLE",
    "MAC_ADMIN",
    "MAC_OVERRIDE",
    "MKNOD",
    "NET_ADMIN",
    "NET_BIND_SERVICE",
    "NET_BROADCAST",
    "NET_RAW",
    "PERFMON",
    "SETFCAP",
    "SETGID",
    "SETPCAP",
    "SETUID",
    "SYS_ADMIN",
    "SYS_BOOT",
    "SYS_CHROOT",
    "SYS_MODULE",
    "SYS_NICE",
    "SYS_PACCT",
    "SYS_PTRACE",
    "SYS_RAWIO",
    "SYS_RESOURCE",
    "SYS_TIME",
    "SYS_TTY_CONFIG",
    "SYSLOG",
    "WAKE_ALARM",
];

fn native_security_options(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<NativeOpaqueSecurityOptions> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(values) = value.as_array() else {
        return native_malformed_field("$.HostConfig.SecurityOpt", identity, findings);
    };
    if let Some(index) = values.iter().position(|value| !value.is_string()) {
        findings.push(InventoryFinding::at_occurrence(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.HostConfig.SecurityOpt",
            index,
        ));
        return ObservationField::Malformed;
    }
    ObservationField::Observed(ObservedValue::new(
        NativeOpaqueSecurityOptions::new(values.len()),
        ObservationOrigin::Effective,
    ))
}

fn decode_native_namespaces(
    host_config: &Map<String, Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<NativeNamespaceObservation> {
    let pid = native_namespace_mode(
        host_config.get("PidMode"),
        "$.HostConfig.PidMode",
        identity,
        findings,
        unmodelled,
        true,
    );
    let ipc = native_ipc_namespace_mode(host_config.get("IpcMode"), identity, findings, unmodelled);
    let uts = native_namespace_mode(
        host_config.get("UTSMode"),
        "$.HostConfig.UTSMode",
        identity,
        findings,
        unmodelled,
        true,
    );
    let cgroup = native_namespace_mode(
        host_config.get("CgroupMode"),
        "$.HostConfig.CgroupMode",
        identity,
        findings,
        unmodelled,
        false,
    );
    ObservationField::Observed(ObservedValue::new(
        NativeNamespaceObservation::new(pid, ipc, uts, cgroup),
        ObservationOrigin::Effective,
    ))
}

fn native_namespace_mode(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
    empty_is_private: bool,
) -> ObservationField<NativeNamespaceMode> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(mode) = value.as_str() else {
        return native_malformed_field(path, identity, findings);
    };
    let mode = match mode {
        "" if empty_is_private => NativeNamespaceMode::Private,
        "private" => NativeNamespaceMode::Private,
        "host" => NativeNamespaceMode::Host,
        _ if native_mode_is_syntactically_valid(mode) => {
            return native_unmodelled_field(path, value, unmodelled);
        }
        _ => return native_malformed_field(path, identity, findings),
    };
    ObservationField::Observed(ObservedValue::new(mode, ObservationOrigin::Effective))
}

fn native_ipc_namespace_mode(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<NativeIpcNamespaceMode> {
    let path = "$.HostConfig.IpcMode";
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(mode) = value.as_str() else {
        return native_malformed_field(path, identity, findings);
    };
    let mode = match mode {
        "" | "private" => NativeIpcNamespaceMode::Private,
        "host" => NativeIpcNamespaceMode::Host,
        "shareable" => NativeIpcNamespaceMode::Shareable,
        "none" => NativeIpcNamespaceMode::None,
        _ if native_mode_is_syntactically_valid(mode) => {
            return native_unmodelled_field(path, value, unmodelled);
        }
        _ => return native_malformed_field(path, identity, findings),
    };
    ObservationField::Observed(ObservedValue::new(mode, ObservationOrigin::Effective))
}

fn native_mode_is_syntactically_valid(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn decode_native_resource_controls(
    host_config: &Map<String, Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<NativeResourceControlObservation> {
    let cpu_shares = native_u64_field(
        host_config.get("CpuShares"),
        "$.HostConfig.CpuShares",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let cpu_period = native_u64_field(
        host_config.get("CpuPeriod"),
        "$.HostConfig.CpuPeriod",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let cpu_quota = native_i64_field(
        host_config.get("CpuQuota"),
        "$.HostConfig.CpuQuota",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let memory = native_i64_field(
        host_config.get("Memory"),
        "$.HostConfig.Memory",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let pids_limit = native_i64_field(
        host_config.get("PidsLimit"),
        "$.HostConfig.PidsLimit",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let ulimits = decode_native_ulimits(host_config.get("Ulimits"), identity, findings);
    ObservationField::Observed(ObservedValue::new(
        NativeResourceControlObservation::new(cpu_shares, cpu_period, cpu_quota, memory, pids_limit, ulimits),
        ObservationOrigin::Effective,
    ))
}

fn decode_native_ulimits(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Vec<NativeUlimitObservation>> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(values) = value.as_array() else {
        return native_malformed_field("$.HostConfig.Ulimits", identity, findings);
    };
    let mut decoded = Vec::with_capacity(values.len());
    let mut malformed = false;
    for (index, value) in values.iter().enumerate() {
        let Some(value) = value.as_object() else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.HostConfig.Ulimits",
                index,
            ));
            malformed = true;
            continue;
        };
        let path = format!("$.HostConfig.Ulimits[{index}]");
        let name = native_string_field(
            value.get("Name"),
            &format!("{path}.Name"),
            identity,
            ObservationOrigin::Effective,
            findings,
        );
        let soft = native_i64_field(
            value.get("Soft"),
            &format!("{path}.Soft"),
            identity,
            ObservationOrigin::Effective,
            findings,
        );
        let hard = native_i64_field(
            value.get("Hard"),
            &format!("{path}.Hard"),
            identity,
            ObservationOrigin::Effective,
            findings,
        );
        if name.is_malformed() || soft.is_malformed() || hard.is_malformed() {
            malformed = true;
            continue;
        }
        decoded.push(NativeUlimitObservation::new(name, soft, hard));
    }
    if malformed {
        ObservationField::Malformed
    } else {
        ObservationField::Observed(ObservedValue::new(decoded, ObservationOrigin::Effective))
    }
}

fn native_i64_field(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<i64> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(value) => value.as_i64().map_or_else(
            || native_malformed_field(path, identity, findings),
            |value| ObservationField::Observed(ObservedValue::new(value, origin)),
        ),
    }
}

fn native_u64_field(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<u64> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(value) => value.as_u64().map_or_else(
            || native_malformed_field(path, identity, findings),
            |value| ObservationField::Observed(ObservedValue::new(value, origin)),
        ),
    }
}

fn native_string_field(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<String> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::String(value)) => ObservationField::Observed(ObservedValue::new(value.clone(), origin)),
        Some(_) => native_malformed_field(path, identity, findings),
    }
}

fn native_unmodelled_field<T>(
    path: &str,
    value: &Value,
    unmodelled: &mut Vec<(String, JsonValueKind)>,
) -> ObservationField<T> {
    unmodelled.push((path.to_owned(), json_value_kind(value)));
    ObservationField::Unmodelled(if path.starts_with("$.Config") {
        crate::UnmodelledFieldId::ContainerConfig
    } else {
        crate::UnmodelledFieldId::ContainerHostConfig
    })
}

fn decode_container_configuration(
    object: &Map<String, Value>,
    options: AcquisitionOptions,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ContainerConfigurationDecoded {
    let config = match object.get("Config") {
        None | Some(Value::Null) => {
            return ContainerConfigurationDecoded {
                labels: ObservationField::Absent,
                environment: ObservationField::Absent,
                command: ObservationField::Absent,
                entrypoint: ObservationField::Absent,
                user: ObservationField::Absent,
                working_directory: ObservationField::Absent,
                hostname: ObservationField::Absent,
            };
        }
        Some(Value::Object(config)) => config,
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config",
            ));
            return ContainerConfigurationDecoded {
                labels: ObservationField::Malformed,
                environment: ObservationField::Malformed,
                command: ObservationField::Malformed,
                entrypoint: ObservationField::Malformed,
                user: ObservationField::Malformed,
                working_directory: ObservationField::Malformed,
                hostname: ObservationField::Malformed,
            };
        }
    };
    let labels = labels(config.get("Labels"), "$.Config.Labels", identity, findings);
    let (environment, environment_findings) =
        decode_environment(config.get("Env"), options.environment_values, identity, "$.Config.Env");
    findings.extend(environment_findings);
    ContainerConfigurationDecoded {
        labels,
        environment,
        command: decode_configured_arguments(
            config.get("Cmd"),
            "$.Config.Cmd",
            identity,
            findings,
            ConfiguredContainerCommand::new,
        ),
        entrypoint: decode_configured_arguments(
            config.get("Entrypoint"),
            "$.Config.Entrypoint",
            identity,
            findings,
            ConfiguredContainerEntrypoint::new,
        ),
        user: decode_configured_text(
            config.get("User"),
            "$.Config.User",
            identity,
            findings,
            ConfiguredContainerUser::new,
        ),
        working_directory: decode_configured_text(
            config.get("WorkingDir"),
            "$.Config.WorkingDir",
            identity,
            findings,
            ConfiguredContainerWorkdir::new,
        ),
        hostname: decode_configured_text(
            config.get("Hostname"),
            "$.Config.Hostname",
            identity,
            findings,
            ConfiguredContainerHostname::new,
        ),
    }
}

#[allow(clippy::single_match_else)] // keeps malformed-array handling adjacent to the reviewed wire shape.
fn decode_configured_arguments<T>(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    constructor: impl FnOnce(Vec<String>) -> T,
) -> ObservationField<T> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Array(values)) => {
            let arguments = values.iter().map(Value::as_str).collect::<Option<Vec<_>>>();
            match arguments {
                Some(arguments) => ObservationField::Observed(ObservedValue::new(
                    constructor(arguments.into_iter().map(ToOwned::to_owned).collect()),
                    ObservationOrigin::Configured,
                )),
                None => {
                    findings.push(InventoryFinding::field(
                        DiagnosticCode::ResourceMalformed,
                        identity.clone(),
                        path,
                    ));
                    ObservationField::Malformed
                }
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

fn decode_configured_text<T>(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    constructor: impl FnOnce(String) -> T,
) -> ObservationField<T> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::String(value)) => ObservationField::Observed(ObservedValue::new(
            constructor(value.clone()),
            ObservationOrigin::Configured,
        )),
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

fn decode_network(
    listed: &ResourceIdentity,
    object: &Map<String, Value>,
    evidence: &ResourceEvidence,
) -> PodmanLensResult<Decoded> {
    let identity = identity_from_inspect(listed, object, &["id"], &["name"])?;
    let (network, mut findings) = decode_network_details(object, &identity, evidence);
    let labels = labels(object.get("labels"), "$.labels", &identity, &mut findings);
    Ok((
        identity,
        labels,
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
        None,
        ObservationField::NotApplicable,
        Some(network),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        findings,
        &["id", "name", "labels", "internal", "options", "subnets", "routes"],
    ))
}

fn decode_network_details(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    evidence: &ResourceEvidence,
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
    let subnets = decode_native_network_subnets(object.get("subnets"), identity, &mut findings);
    let routes = decode_native_network_routes(object.get("routes"), identity, evidence, &mut findings);
    ((internal, options, subnets, routes), findings)
}

fn decode_native_network_subnets(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Vec<NativeNetworkSubnetObservation>> {
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
            "$.subnets",
        ));
        return ObservationField::Malformed;
    };
    let mut decoded = Vec::with_capacity(values.len());
    let mut malformed = false;
    for (index, value) in values.iter().enumerate() {
        let path = format!("$.subnets[{index}]");
        let Some(object) = value.as_object() else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.subnets",
                index,
            ));
            continue;
        };
        let cidr = native_cidr_field(
            object.get("subnet"),
            &format!("{path}.subnet"),
            identity,
            findings,
            true,
        );
        let gateway = native_ip_field(
            object.get("gateway"),
            &format!("{path}.gateway"),
            identity,
            ObservationOrigin::Effective,
            findings,
        );
        let lease_range = native_lease_range_field(
            object.get("lease_range"),
            cidr.observed().map(ObservedValue::value),
            &format!("{path}.lease_range"),
            identity,
            findings,
        );
        let member_malformed = cidr.is_malformed() || gateway.is_malformed() || lease_range.is_malformed();
        let gateway_outside = matches!(
            (&cidr, &gateway),
            (ObservationField::Observed(cidr), ObservationField::Observed(gateway))
                if !cidr.value().contains(*gateway.value())
        );
        if gateway_outside {
            malformed = true;
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                format!("{path}.gateway"),
            ));
        }
        if member_malformed || gateway_outside {
            malformed = true;
            continue;
        }
        decoded.push(NativeNetworkSubnetObservation::new(cidr, gateway, lease_range));
    }
    if malformed {
        ObservationField::Malformed
    } else {
        ObservationField::Observed(ObservedValue::new(decoded, ObservationOrigin::Effective))
    }
}

fn decode_native_network_routes(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    evidence: &ResourceEvidence,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Vec<NativeNetworkRouteObservation>> {
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
            "$.routes",
        ));
        return ObservationField::Malformed;
    };
    let mut decoded = Vec::with_capacity(values.len());
    let mut malformed = false;
    for (index, value) in values.iter().enumerate() {
        let path = format!("$.routes[{index}]");
        let Some(object) = value.as_object() else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.routes",
                index,
            ));
            continue;
        };
        let destination = native_cidr_field(
            object.get("destination"),
            &format!("{path}.destination"),
            identity,
            findings,
            true,
        );
        let gateway = native_ip_field(
            object.get("gateway"),
            &format!("{path}.gateway"),
            identity,
            ObservationOrigin::Effective,
            findings,
        );
        let metric = native_u32_field(object.get("metric"), &format!("{path}.metric"), identity, findings);
        let route_type = native_route_type_field(
            object.get("route_type"),
            &format!("{path}.route_type"),
            identity,
            evidence,
            findings,
        );
        let member_malformed =
            destination.is_malformed() || gateway.is_malformed() || metric.is_malformed() || route_type.is_malformed();
        let gateway_wrong_family = matches!(
            (&destination, &gateway),
            (ObservationField::Observed(destination), ObservationField::Observed(gateway))
                if !destination.value().has_address_family(*gateway.value())
        );
        let invalid_gateway_semantics = match (&route_type, &gateway) {
            (ObservationField::Observed(route_type), gateway) => match route_type.value() {
                NativeNetworkRouteType::Unicast => !gateway.is_observed(),
                NativeNetworkRouteType::Blackhole
                | NativeNetworkRouteType::Unreachable
                | NativeNetworkRouteType::Prohibit => gateway.is_observed(),
            },
            // Podman 5.x does not expose route type. Its static routes still have the native
            // unicast gateway requirement, so an inapplicable nested member must not disable
            // validation of the independently observed gateway.
            (ObservationField::VersionInapplicable, gateway) => !gateway.is_observed(),
            _ => false,
        };
        if gateway_wrong_family || invalid_gateway_semantics {
            malformed = true;
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                format!("{path}.gateway"),
            ));
        }
        if member_malformed || gateway_wrong_family || invalid_gateway_semantics {
            malformed = true;
            continue;
        }
        decoded.push(NativeNetworkRouteObservation::new(
            destination,
            gateway,
            metric,
            route_type,
        ));
    }
    if malformed {
        ObservationField::Malformed
    } else {
        ObservationField::Observed(ObservedValue::new(decoded, ObservationOrigin::Effective))
    }
}

fn native_cidr_field(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    required: bool,
) -> ObservationField<NativeNetworkCidr> {
    match value {
        None | Some(Value::Null) if !required => ObservationField::Absent,
        Some(Value::String(value)) => match NativeNetworkCidr::parse(value.clone()) {
            Some(value) => ObservationField::Observed(ObservedValue::new(value, ObservationOrigin::Effective)),
            None => native_malformed_field(path, identity, findings),
        },
        _ => native_malformed_field(path, identity, findings),
    }
}

fn native_ip_field(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<IpAddr> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::String(value)) => match value.parse() {
            Ok(value) => ObservationField::Observed(ObservedValue::new(value, origin)),
            Err(_) => native_malformed_field(path, identity, findings),
        },
        _ => native_malformed_field(path, identity, findings),
    }
}

fn native_u32_field(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<u32> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Number(value)) => match value.as_u64().and_then(|value| value.try_into().ok()) {
            Some(value) => ObservationField::Observed(ObservedValue::new(value, ObservationOrigin::Effective)),
            None => native_malformed_field(path, identity, findings),
        },
        _ => native_malformed_field(path, identity, findings),
    }
}

fn native_lease_range_field(
    value: Option<&Value>,
    cidr: Option<&NativeNetworkCidr>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<NativeNetworkLeaseRange> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Object(value)) => {
            let start = native_ip_field(
                value.get("start_ip"),
                &format!("{path}.start_ip"),
                identity,
                ObservationOrigin::Effective,
                findings,
            );
            let end = native_ip_field(
                value.get("end_ip"),
                &format!("{path}.end_ip"),
                identity,
                ObservationOrigin::Effective,
                findings,
            );
            if start.is_malformed() || end.is_malformed() {
                return ObservationField::Malformed;
            }
            let outside_cidr = cidr.is_some_and(|cidr| {
                [start.observed(), end.observed()]
                    .into_iter()
                    .flatten()
                    .any(|endpoint| !cidr.contains(*endpoint.value()))
            });
            let reversed = matches!(
                (&start, &end),
                (ObservationField::Observed(start), ObservationField::Observed(end))
                    if !native_address_precedes_or_equals(*start.value(), *end.value())
            );
            if outside_cidr || reversed {
                findings.push(InventoryFinding::field(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    path,
                ));
                return ObservationField::Malformed;
            }
            ObservationField::Observed(ObservedValue::new(
                NativeNetworkLeaseRange::new(start, end),
                ObservationOrigin::Effective,
            ))
        }
        _ => native_malformed_field(path, identity, findings),
    }
}

fn native_route_type_field(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    evidence: &ResourceEvidence,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<NativeNetworkRouteType> {
    if !native_network_route_types_are_available(evidence) {
        findings.push(InventoryFinding::field(
            DiagnosticCode::VersionInapplicableField,
            identity.clone(),
            path,
        ));
        return ObservationField::VersionInapplicable;
    }
    match value {
        None | Some(Value::Null) => ObservationField::Observed(ObservedValue::new(
            NativeNetworkRouteType::Unicast,
            ObservationOrigin::Effective,
        )),
        Some(Value::String(value)) => {
            let value = match value.as_str() {
                "unicast" => NativeNetworkRouteType::Unicast,
                "blackhole" => NativeNetworkRouteType::Blackhole,
                "unreachable" => NativeNetworkRouteType::Unreachable,
                "prohibit" => NativeNetworkRouteType::Prohibit,
                _ => return ObservationField::Unmodelled(crate::UnmodelledFieldId::NetworkRoute),
            };
            ObservationField::Observed(ObservedValue::new(value, ObservationOrigin::Effective))
        }
        _ => native_malformed_field(path, identity, findings),
    }
}

fn native_network_route_types_are_available(evidence: &ResourceEvidence) -> bool {
    semver::Version::parse(evidence.engine_version()).is_ok_and(|version| version >= semver::Version::new(6, 0, 0))
}

fn native_malformed_field<T>(
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<T> {
    findings.push(InventoryFinding::field(
        DiagnosticCode::ResourceMalformed,
        identity.clone(),
        path,
    ));
    ObservationField::Malformed
}

fn native_address_precedes_or_equals(start: IpAddr, end: IpAddr) -> bool {
    match (start, end) {
        (IpAddr::V4(start), IpAddr::V4(end)) => u32::from(start) <= u32::from(end),
        (IpAddr::V6(start), IpAddr::V6(end)) => u128::from(start) <= u128::from(end),
        _ => false,
    }
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
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        ObservationField::NotApplicable,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(uid),
        Some(gid),
        None,
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
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        aliases,
        None,
        None,
        None,
        None,
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
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        ObservationField::NotApplicable,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(driver),
        None,
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

fn append_native_reference_relationship(
    field: &ObservationField<NativeResourceReference>,
    kind: ResourceKind,
    relationships: &mut Vec<NativeRelationship>,
) -> RelationshipDecoding {
    match field {
        ObservationField::Observed(reference) => {
            relationships.push(NativeRelationship::new(
                kind,
                reference.value().reference(),
                reference.value().field_path(),
            ));
            RelationshipDecoding {
                supplied: true,
                malformed: false,
            }
        }
        ObservationField::Malformed => RelationshipDecoding {
            supplied: true,
            malformed: true,
        },
        ObservationField::Absent | ObservationField::NotApplicable => RelationshipDecoding::default(),
        ObservationField::Unavailable | ObservationField::VersionInapplicable | ObservationField::Unmodelled(_) => {
            RelationshipDecoding {
                supplied: true,
                malformed: true,
            }
        }
    }
}

fn append_native_dependency_relationships(
    field: &ObservationField<Vec<NativeResourceReference>>,
    relationships: &mut Vec<NativeRelationship>,
) -> RelationshipDecoding {
    match field {
        ObservationField::Observed(references) => {
            relationships.extend(references.value().iter().map(|reference| {
                NativeRelationship::new(ResourceKind::Container, reference.reference(), reference.field_path())
            }));
            RelationshipDecoding {
                supplied: true,
                malformed: false,
            }
        }
        ObservationField::Malformed => RelationshipDecoding {
            supplied: true,
            malformed: true,
        },
        ObservationField::Absent | ObservationField::NotApplicable => RelationshipDecoding::default(),
        ObservationField::Unavailable | ObservationField::VersionInapplicable | ObservationField::Unmodelled(_) => {
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

struct ContainerMountDecoded {
    field: ObservationField<Vec<ContainerMountObservation>>,
    relationships: RelationshipDecoding,
}

#[allow(clippy::too_many_lines, clippy::single_match_else)] // one bounded native object is decoded atomically.
fn decode_container_mounts(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> ContainerMountDecoded {
    let Some(value) = object.get("Mounts") else {
        return ContainerMountDecoded {
            field: ObservationField::Absent,
            relationships: RelationshipDecoding::default(),
        };
    };
    if value.is_null() {
        return ContainerMountDecoded {
            field: ObservationField::Absent,
            relationships: RelationshipDecoding::default(),
        };
    }
    let Some(mounts) = value.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Mounts",
        ));
        return ContainerMountDecoded {
            field: ObservationField::Malformed,
            relationships: RelationshipDecoding {
                supplied: true,
                malformed: true,
            },
        };
    };

    let mut decoded = Vec::with_capacity(mounts.len());
    let mut malformed = false;
    for (index, mount) in mounts.iter().enumerate() {
        let path = format!("$.Mounts[{index}]");
        let Some(mount) = mount.as_object() else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Mounts",
                index,
            ));
            continue;
        };
        let Some(Value::String(kind)) = mount.get("Type") else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Mounts",
                index,
            ));
            continue;
        };
        let kind = match kind.as_str() {
            "volume" => ContainerMountKind::NamedVolume,
            "bind" => ContainerMountKind::Bind,
            _ => {
                malformed = true;
                findings.push(InventoryFinding::field(
                    DiagnosticCode::NativeFieldUnsupported,
                    identity.clone(),
                    format!("{path}.Type"),
                ));
                continue;
            }
        };
        let destination = optional_observed_string(
            mount.get("Destination"),
            &format!("{path}.Destination"),
            identity,
            findings,
            ObservationOrigin::Configured,
        );
        let (source, local_backing_path) = match kind {
            ContainerMountKind::NamedVolume => {
                let source =
                    match required_observed_string(mount.get("Name"), &format!("{path}.Name"), identity, findings) {
                        Ok(value) => ContainerMountSource::NamedVolume(value),
                        Err(()) => {
                            malformed = true;
                            continue;
                        }
                    };
                let local = optional_observed_string(
                    mount.get("Source"),
                    &format!("{path}.Source"),
                    identity,
                    findings,
                    ObservationOrigin::LocalResolution,
                );
                (source, local)
            }
            ContainerMountKind::Bind => {
                let source = match required_observed_string(
                    mount.get("Source"),
                    &format!("{path}.Source"),
                    identity,
                    findings,
                ) {
                    Ok(value) => ContainerMountSource::LocalBindPath(value),
                    Err(()) => {
                        malformed = true;
                        continue;
                    }
                };
                (source, ObservationField::Absent)
            }
        };
        let writable = optional_observed_bool(mount.get("RW"), &format!("{path}.RW"), identity, findings);
        let options =
            optional_observed_string_array(mount.get("Options"), &format!("{path}.Options"), identity, findings);
        let propagation = optional_observed_string(
            mount.get("Propagation"),
            &format!("{path}.Propagation"),
            identity,
            findings,
            ObservationOrigin::Effective,
        );
        let subpath = optional_observed_string(
            mount.get("SubPath"),
            &format!("{path}.SubPath"),
            identity,
            findings,
            ObservationOrigin::Configured,
        );
        let member_malformed = destination.is_malformed()
            || local_backing_path.is_malformed()
            || writable.is_malformed()
            || options.is_malformed()
            || propagation.is_malformed()
            || subpath.is_malformed();
        if member_malformed {
            malformed = true;
            continue;
        }
        if let (ContainerMountKind::NamedVolume, ContainerMountSource::NamedVolume(name)) = (kind, &source) {
            relationships.push(NativeRelationship::new(
                ResourceKind::Volume,
                name,
                format!("{path}.Name"),
            ));
        }
        decoded.push(ContainerMountObservation::new(
            kind,
            ObservationField::Observed(ObservedValue::new(
                source,
                match kind {
                    ContainerMountKind::NamedVolume => ObservationOrigin::Configured,
                    ContainerMountKind::Bind => ObservationOrigin::LocalResolution,
                },
            )),
            local_backing_path,
            destination,
            writable,
            options,
            propagation,
            subpath,
        ));
    }
    ContainerMountDecoded {
        field: if malformed {
            ObservationField::Malformed
        } else {
            ObservationField::Observed(ObservedValue::new(decoded, ObservationOrigin::Effective))
        },
        relationships: RelationshipDecoding {
            supplied: true,
            malformed,
        },
    }
}

struct ContainerSecretGrantsDecoded {
    field: ObservationField<Vec<ContainerSecretGrantObservation>>,
    relationships: RelationshipDecoding,
}

#[allow(clippy::too_many_lines)] // relationship safety requires all grant members be reviewed together.
fn decode_container_secret_grants(
    config: Option<&Value>,
    identity: &ResourceIdentity,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> ContainerSecretGrantsDecoded {
    let Some(config) = config else {
        return ContainerSecretGrantsDecoded {
            field: ObservationField::Absent,
            relationships: RelationshipDecoding::default(),
        };
    };
    if config.is_null() {
        return ContainerSecretGrantsDecoded {
            field: ObservationField::Absent,
            relationships: RelationshipDecoding::default(),
        };
    }
    let Some(config) = config.as_object() else {
        return ContainerSecretGrantsDecoded {
            field: ObservationField::Malformed,
            relationships: RelationshipDecoding {
                supplied: true,
                malformed: true,
            },
        };
    };
    let Some(secrets) = config.get("Secrets") else {
        return ContainerSecretGrantsDecoded {
            field: ObservationField::Absent,
            relationships: RelationshipDecoding::default(),
        };
    };
    if secrets.is_null() {
        return ContainerSecretGrantsDecoded {
            field: ObservationField::Absent,
            relationships: RelationshipDecoding::default(),
        };
    }
    let Some(secrets) = secrets.as_array() else {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            "$.Config.Secrets",
        ));
        return ContainerSecretGrantsDecoded {
            field: ObservationField::Malformed,
            relationships: RelationshipDecoding {
                supplied: true,
                malformed: true,
            },
        };
    };
    let mut decoded = Vec::with_capacity(secrets.len());
    let mut malformed = false;
    for (index, secret) in secrets.iter().enumerate() {
        let path = format!("$.Config.Secrets[{index}]");
        let Some(secret) = secret.as_object() else {
            malformed = true;
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Config.Secrets",
                index,
            ));
            continue;
        };
        let id = optional_native_reference(secret.get("ID"), &format!("{path}.ID"), identity, findings);
        let name = optional_native_reference(secret.get("Name"), &format!("{path}.Name"), identity, findings);
        let reference = match (id, name) {
            (Ok(Some(id)), Ok(Some(name))) => ObservationField::Observed(ObservedValue::new(
                ContainerSecretReference::new(Some(id), Some(name)),
                ObservationOrigin::Configured,
            )),
            (Ok(Some(id)), Ok(None)) => ObservationField::Observed(ObservedValue::new(
                ContainerSecretReference::new(Some(id), None),
                ObservationOrigin::Configured,
            )),
            (Ok(None), Ok(Some(name))) => ObservationField::Observed(ObservedValue::new(
                ContainerSecretReference::new(None, Some(name)),
                ObservationOrigin::Configured,
            )),
            (Ok(None), Ok(None)) | (Err(()), _) | (_, Err(())) => {
                malformed = true;
                findings.push(InventoryFinding::at_occurrence(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    "$.Config.Secrets",
                    index,
                ));
                ObservationField::Malformed
            }
        };
        let uid = optional_observed_u32(secret.get("UID"), &format!("{path}.UID"), identity, findings);
        let gid = optional_observed_u32(secret.get("GID"), &format!("{path}.GID"), identity, findings);
        let mode = optional_observed_u32(secret.get("Mode"), &format!("{path}.Mode"), identity, findings);
        if reference.is_malformed() || uid.is_malformed() || gid.is_malformed() || mode.is_malformed() {
            malformed = true;
            continue;
        }
        if let ObservationField::Observed(reference) = &reference {
            let references = reference
                .value()
                .id()
                .into_iter()
                .chain(reference.value().name())
                .map(|item| (item.reference().to_owned(), item.field_path().to_owned()));
            if let Some(relationship) = NativeRelationship::coalesced(ResourceKind::Secret, references) {
                relationships.push(relationship);
            }
        }
        decoded.push(ContainerSecretGrantObservation::new(reference, uid, gid, mode));
    }
    ContainerSecretGrantsDecoded {
        field: if malformed {
            ObservationField::Malformed
        } else {
            ObservationField::Observed(ObservedValue::new(decoded, ObservationOrigin::Effective))
        },
        relationships: RelationshipDecoding {
            supplied: true,
            malformed,
        },
    }
}

fn decode_native_reference(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<NativeResourceReference> {
    match optional_native_reference(value, path, identity, findings) {
        Ok(Some(value)) => ObservationField::Observed(ObservedValue::new(value, ObservationOrigin::Configured)),
        Ok(None) => ObservationField::Absent,
        Err(()) => ObservationField::Malformed,
    }
}

fn optional_native_reference(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> Result<Option<NativeResourceReference>, ()> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => {
            Ok(Some(NativeResourceReference::new(value.clone(), path.to_owned())))
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
            ));
            Err(())
        }
    }
}

fn decode_native_dependencies(
    value: Option<&Value>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Vec<NativeResourceReference>> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Array(values)) => {
            let mut references = Vec::with_capacity(values.len());
            for (index, value) in values.iter().enumerate() {
                let path = format!("$.Dependencies[{index}]");
                let Ok(Some(reference)) = optional_native_reference(Some(value), &path, identity, findings) else {
                    findings.push(InventoryFinding::field(
                        DiagnosticCode::ResourceMalformed,
                        identity.clone(),
                        "$.Dependencies",
                    ));
                    return ObservationField::Malformed;
                };
                references.push(reference);
            }
            ObservationField::Observed(ObservedValue::new(references, ObservationOrigin::Configured))
        }
        Some(_) => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.Dependencies",
            ));
            ObservationField::Malformed
        }
    }
}

fn required_observed_string(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> Result<String, ()> {
    match value {
        Some(Value::String(value)) if !value.is_empty() => Ok(value.clone()),
        _ => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
            ));
            Err(())
        }
    }
}

fn optional_observed_string(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
    origin: ObservationOrigin,
) -> ObservationField<String> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::String(value)) => ObservationField::Observed(ObservedValue::new(value.clone(), origin)),
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

fn optional_observed_bool(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<bool> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Bool(value)) => {
            ObservationField::Observed(ObservedValue::new(*value, ObservationOrigin::Effective))
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

#[allow(clippy::single_match_else)] // preserves the malformed array boundary beside its decoder.
fn optional_observed_string_array(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Vec<String>> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Array(values)) => match values.iter().map(Value::as_str).collect::<Option<Vec<_>>>() {
            Some(values) => ObservationField::Observed(ObservedValue::new(
                values.into_iter().map(ToOwned::to_owned).collect(),
                ObservationOrigin::Effective,
            )),
            None => {
                findings.push(InventoryFinding::field(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    path,
                ));
                ObservationField::Malformed
            }
        },
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

#[allow(clippy::single_match_else)] // preserves numeric-range validation beside the decoder.
fn optional_observed_u32(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<u32> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Number(value)) => match value.as_u64().and_then(|value| u32::try_from(value).ok()) {
            Some(value) => ObservationField::Observed(ObservedValue::new(value, ObservationOrigin::Effective)),
            None => {
                findings.push(InventoryFinding::field(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    path,
                ));
                ObservationField::Malformed
            }
        },
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

#[allow(clippy::too_many_lines)] // gate/object consistency stays adjacent to the field family.
fn decode_pod_networking(
    object: &Map<String, Value>,
    identity: &ResourceIdentity,
    evidence: &ResourceEvidence,
    relationships: &mut Vec<NativeRelationship>,
    findings: &mut Vec<InventoryFinding>,
) -> (
    ObservationField<bool>,
    ObservationField<NativeNetworkingObservation>,
    RelationshipDecoding,
) {
    let create_infra = native_bool_field(
        object.get("CreateInfra"),
        "$.CreateInfra",
        identity,
        ObservationOrigin::Effective,
        findings,
    );
    let infra_config = object.get("InfraConfig");
    match (&create_infra, infra_config) {
        (ObservationField::Observed(value), None | Some(Value::Null)) if *value.value() => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.InfraConfig",
            ));
            (
                create_infra,
                ObservationField::Malformed,
                RelationshipDecoding {
                    supplied: true,
                    malformed: true,
                },
            )
        }
        (ObservationField::Observed(value), Some(Value::Object(config))) if *value.value() => {
            let (networking, relationship_decoding) = decode_native_networking(
                config,
                "$.InfraConfig",
                identity,
                evidence,
                ObservationOrigin::Effective,
                findings,
            );
            if let ObservationField::Observed(networking) = &networking {
                if let ObservationField::Observed(networks) = networking.value().networks() {
                    relationships.extend(networks.value().iter().map(|network| {
                        NativeRelationship::new(ResourceKind::Network, network.reference(), network.field_path())
                    }));
                }
            }
            (create_infra, networking, relationship_decoding)
        }
        (ObservationField::Observed(value), Some(_)) if *value.value() => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.InfraConfig",
            ));
            (
                create_infra,
                ObservationField::Malformed,
                RelationshipDecoding {
                    supplied: true,
                    malformed: true,
                },
            )
        }
        (ObservationField::Observed(value), Some(config)) if !*value.value() && !config.is_null() => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.InfraConfig",
            ));
            (
                create_infra,
                ObservationField::Malformed,
                RelationshipDecoding {
                    supplied: true,
                    malformed: true,
                },
            )
        }
        (ObservationField::Observed(value), _) if !*value.value() => {
            (create_infra, ObservationField::Absent, RelationshipDecoding::default())
        }
        (ObservationField::Absent, None | Some(Value::Null)) => {
            (create_infra, ObservationField::Absent, RelationshipDecoding::default())
        }
        (ObservationField::Absent, Some(config)) if !config.is_null() => {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.InfraConfig",
            ));
            (
                create_infra,
                ObservationField::Malformed,
                RelationshipDecoding {
                    supplied: true,
                    malformed: true,
                },
            )
        }
        _ => (
            create_infra,
            ObservationField::Malformed,
            RelationshipDecoding {
                supplied: true,
                malformed: true,
            },
        ),
    }
}

#[allow(clippy::too_many_lines)] // source-local field family is intentionally explicit.
fn decode_container_networking(
    object: &Map<String, Value>,
    pod_membership: &ObservationField<NativeResourceReference>,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<NativeNetworkingObservation> {
    if pod_membership.observed().is_some() {
        if object
            .get("HostConfig")
            .and_then(Value::as_object)
            .is_some_and(|host_config| {
                [
                    "CreateNetNS",
                    "PortBindings",
                    "Dns",
                    "DnsSearch",
                    "DnsOptions",
                    "ExtraHosts",
                    "NoManageResolvConf",
                    "NoManageHosts",
                ]
                .iter()
                .any(|key| host_config.contains_key(*key))
            })
        {
            findings.push(InventoryFinding::field(
                DiagnosticCode::NativeFieldUnsupported,
                identity.clone(),
                "$.HostConfig",
            ));
        }
        return ObservationField::NotApplicable;
    }
    let Some(host_config) = object.get("HostConfig") else {
        return ObservationField::Absent;
    };
    let Some(host_config) = host_config.as_object() else {
        if !host_config.is_null() {
            findings.push(InventoryFinding::field(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                "$.HostConfig",
            ));
            return ObservationField::Malformed;
        }
        return ObservationField::Absent;
    };
    let create_net_ns = native_bool_field(
        host_config.get("CreateNetNS"),
        "$.HostConfig.CreateNetNS",
        identity,
        ObservationOrigin::Configured,
        findings,
    );
    let no_manage_resolv_conf = native_bool_field(
        host_config.get("NoManageResolvConf"),
        "$.HostConfig.NoManageResolvConf",
        identity,
        ObservationOrigin::Configured,
        findings,
    );
    let no_manage_hosts = native_bool_field(
        host_config.get("NoManageHosts"),
        "$.HostConfig.NoManageHosts",
        identity,
        ObservationOrigin::Configured,
        findings,
    );
    let mut local_findings = Vec::new();
    let ports = native_port_bindings(
        host_config.get("PortBindings"),
        "$.HostConfig.PortBindings",
        identity,
        ObservationOrigin::Configured,
        &mut local_findings,
    );
    let resolver_managed = !matches!(no_manage_resolv_conf.observed().map(ObservedValue::value), Some(true));
    let hosts_managed = !matches!(no_manage_hosts.observed().map(ObservedValue::value), Some(true));
    let dns_servers = if resolver_managed {
        native_ip_list(
            host_config.get("Dns"),
            "$.HostConfig.Dns",
            identity,
            ObservationOrigin::Configured,
            &mut local_findings,
        )
    } else {
        ObservationField::NotApplicable
    };
    let dns_search = if resolver_managed {
        native_string_list(
            host_config.get("DnsSearch"),
            "$.HostConfig.DnsSearch",
            identity,
            ObservationOrigin::Configured,
            &mut local_findings,
        )
    } else {
        ObservationField::NotApplicable
    };
    let dns_options = if resolver_managed {
        native_string_list(
            host_config.get("DnsOptions"),
            "$.HostConfig.DnsOptions",
            identity,
            ObservationOrigin::Configured,
            &mut local_findings,
        )
    } else {
        ObservationField::NotApplicable
    };
    let host_entries = if hosts_managed {
        native_unmodelled_string_list(
            host_config.get("ExtraHosts"),
            "$.HostConfig.ExtraHosts",
            identity,
            crate::UnmodelledFieldId::ContainerHostConfig,
            &mut local_findings,
        )
    } else {
        ObservationField::NotApplicable
    };
    findings.extend(local_findings);
    if ports.is_malformed()
        || dns_servers.is_malformed()
        || dns_search.is_malformed()
        || dns_options.is_malformed()
        || host_entries.is_malformed()
        || create_net_ns.is_malformed()
    {
        return ObservationField::Malformed;
    }
    ObservationField::Observed(ObservedValue::new(
        NativeNetworkingObservation::new(
            ports,
            create_net_ns,
            ObservationField::NotApplicable,
            dns_servers,
            dns_search,
            dns_options,
            host_entries,
            ObservationField::NotApplicable,
            ObservationField::NotApplicable,
            no_manage_resolv_conf,
            no_manage_hosts,
            ObservationField::NotApplicable,
            ObservationField::NotApplicable,
        ),
        ObservationOrigin::Configured,
    ))
}

#[allow(clippy::too_many_lines)] // native field provenance and applicability remain explicit.
fn decode_native_networking(
    object: &Map<String, Value>,
    prefix: &str,
    identity: &ResourceIdentity,
    evidence: &ResourceEvidence,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> (ObservationField<NativeNetworkingObservation>, RelationshipDecoding) {
    let port_bindings = native_port_bindings(
        object.get("PortBindings"),
        &format!("{prefix}.PortBindings"),
        identity,
        origin,
        findings,
    );
    let host_network = native_bool_field(
        object.get("HostNetwork"),
        &format!("{prefix}.HostNetwork"),
        identity,
        origin,
        findings,
    );
    let no_manage_resolv_conf = native_bool_field(
        object.get("NoManageResolvConf"),
        &format!("{prefix}.NoManageResolvConf"),
        identity,
        origin,
        findings,
    );
    let no_manage_hosts = native_bool_field(
        object.get("NoManageHosts"),
        &format!("{prefix}.NoManageHosts"),
        identity,
        origin,
        findings,
    );
    let resolver_managed = !matches!(no_manage_resolv_conf.observed().map(ObservedValue::value), Some(true));
    let hosts_managed = !matches!(no_manage_hosts.observed().map(ObservedValue::value), Some(true));
    let dns_servers = if resolver_managed {
        native_ip_list(
            object.get("DNSServer"),
            &format!("{prefix}.DNSServer"),
            identity,
            origin,
            findings,
        )
    } else {
        ObservationField::NotApplicable
    };
    let dns_search = if resolver_managed {
        native_string_list(
            object.get("DNSSearch"),
            &format!("{prefix}.DNSSearch"),
            identity,
            origin,
            findings,
        )
    } else {
        ObservationField::NotApplicable
    };
    let dns_options = if resolver_managed {
        native_string_list(
            object.get("DNSOption"),
            &format!("{prefix}.DNSOption"),
            identity,
            origin,
            findings,
        )
    } else {
        ObservationField::NotApplicable
    };
    let host_entries = if hosts_managed {
        native_unmodelled_string_list(
            object.get("HostAdd"),
            &format!("{prefix}.HostAdd"),
            identity,
            crate::UnmodelledFieldId::PodInfraConfig,
            findings,
        )
    } else {
        ObservationField::NotApplicable
    };
    let (networks, relationships) = native_network_references(
        object.get("Networks"),
        &format!("{prefix}.Networks"),
        identity,
        findings,
    );
    let network_options = native_opaque_options(
        object.get("NetworkOptions"),
        &format!("{prefix}.NetworkOptions"),
        identity,
        origin,
        findings,
    );
    let static_ip = if semver::Version::parse(evidence.engine_version())
        .is_ok_and(|version| version <= semver::Version::new(5, 8, 6))
    {
        native_ip_field(
            object.get("StaticIP"),
            &format!("{prefix}.StaticIP"),
            identity,
            origin,
            findings,
        )
    } else {
        if object.get("StaticIP").is_some_and(|value| !value.is_null()) {
            findings.push(InventoryFinding::field(
                DiagnosticCode::VersionInapplicableField,
                identity.clone(),
                format!("{prefix}.StaticIP"),
            ));
        }
        ObservationField::VersionInapplicable
    };
    if object.get("StaticMAC").is_some_and(|value| !value.is_null()) {
        findings.push(InventoryFinding::field(
            DiagnosticCode::VersionInapplicableField,
            identity.clone(),
            format!("{prefix}.StaticMAC"),
        ));
    }
    let static_mac = ObservationField::VersionInapplicable;
    let malformed = [
        port_bindings.is_malformed(),
        host_network.is_malformed(),
        dns_servers.is_malformed(),
        dns_search.is_malformed(),
        dns_options.is_malformed(),
        host_entries.is_malformed(),
        networks.is_malformed(),
        network_options.is_malformed(),
        no_manage_resolv_conf.is_malformed(),
        no_manage_hosts.is_malformed(),
        static_ip.is_malformed(),
    ]
    .into_iter()
    .any(|value| value);
    let networking = if malformed {
        ObservationField::Malformed
    } else {
        ObservationField::Observed(ObservedValue::new(
            NativeNetworkingObservation::new(
                port_bindings,
                host_network,
                ObservationField::NotApplicable,
                dns_servers,
                dns_search,
                dns_options,
                host_entries,
                networks,
                network_options,
                no_manage_resolv_conf,
                no_manage_hosts,
                static_ip,
                static_mac,
            ),
            origin,
        ))
    };
    (networking, relationships)
}

fn native_bool_field(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<bool> {
    match value {
        None | Some(Value::Null) => ObservationField::Absent,
        Some(Value::Bool(value)) => ObservationField::Observed(ObservedValue::new(*value, origin)),
        Some(_) => native_malformed_field(path, identity, findings),
    }
}

fn native_ip_list(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Vec<IpAddr>> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(values) = value.as_array() else {
        return native_malformed_field(path, identity, findings);
    };
    let mut decoded = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let Some(value) = value.as_str().and_then(|value| value.parse::<IpAddr>().ok()) else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
                index,
            ));
            return ObservationField::Malformed;
        };
        decoded.push(value);
    }
    ObservationField::Observed(ObservedValue::new(decoded, origin))
}

fn native_string_list(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Vec<String>> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(values) = value.as_array() else {
        return native_malformed_field(path, identity, findings);
    };
    let mut decoded = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let Some(value) = value
            .as_str()
            .filter(|value| !value.is_empty() && !value.chars().any(char::is_control))
        else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
                index,
            ));
            return ObservationField::Malformed;
        };
        decoded.push(value.to_owned());
    }
    ObservationField::Observed(ObservedValue::new(decoded, origin))
}

fn native_unmodelled_string_list(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    field: crate::UnmodelledFieldId,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<NativeOpaqueNetworkOptions> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(values) = value.as_array() else {
        return native_malformed_field(path, identity, findings);
    };
    if values.iter().any(|value| !value.is_string()) {
        findings.push(InventoryFinding::field(
            DiagnosticCode::ResourceMalformed,
            identity.clone(),
            path,
        ));
        return ObservationField::Malformed;
    }
    ObservationField::Unmodelled(field)
}

fn native_opaque_options(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<NativeOpaqueNetworkOptions> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(options) = value.as_object() else {
        return native_malformed_field(path, identity, findings);
    };
    ObservationField::Observed(ObservedValue::new(
        NativeOpaqueNetworkOptions::new(options.len()),
        origin,
    ))
}

fn native_network_references(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    findings: &mut Vec<InventoryFinding>,
) -> (ObservationField<Vec<NativeResourceReference>>, RelationshipDecoding) {
    let Some(value) = value else {
        return (ObservationField::Absent, RelationshipDecoding::default());
    };
    if value.is_null() {
        return (ObservationField::Absent, RelationshipDecoding::default());
    }
    let Some(values) = value.as_array() else {
        let field = native_malformed_field(path, identity, findings);
        return (
            field,
            RelationshipDecoding {
                supplied: true,
                malformed: true,
            },
        );
    };
    let mut references = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let Some(value) = value.as_str().filter(|value| !value.is_empty()) else {
            findings.push(InventoryFinding::at_occurrence(
                DiagnosticCode::ResourceMalformed,
                identity.clone(),
                path,
                index,
            ));
            return (
                ObservationField::Malformed,
                RelationshipDecoding {
                    supplied: true,
                    malformed: true,
                },
            );
        };
        references.push(NativeResourceReference::new(
            value.to_owned(),
            format!("{path}[{index}]"),
        ));
    }
    (
        ObservationField::Observed(ObservedValue::new(references, ObservationOrigin::Effective)),
        RelationshipDecoding {
            supplied: true,
            malformed: false,
        },
    )
}

fn native_port_bindings(
    value: Option<&Value>,
    path: &str,
    identity: &ResourceIdentity,
    origin: ObservationOrigin,
    findings: &mut Vec<InventoryFinding>,
) -> ObservationField<Vec<NativePortBindingObservation>> {
    let Some(value) = value else {
        return ObservationField::Absent;
    };
    if value.is_null() {
        return ObservationField::Absent;
    }
    let Some(bindings) = value.as_object() else {
        return native_malformed_field(path, identity, findings);
    };
    let mut decoded = Vec::new();
    for (key, values) in bindings {
        let Some((port, protocol)) = native_port_binding_key(key) else {
            return native_malformed_field(&format!("{path}.{key}"), identity, findings);
        };
        let Some(values) = values.as_array() else {
            return native_malformed_field(&format!("{path}.{key}"), identity, findings);
        };
        for (index, value) in values.iter().enumerate() {
            let Some(value) = value.as_object() else {
                findings.push(InventoryFinding::at_occurrence(
                    DiagnosticCode::ResourceMalformed,
                    identity.clone(),
                    format!("{path}.{key}"),
                    index,
                ));
                return ObservationField::Malformed;
            };
            let host_ip = match value.get("HostIp") {
                None | Some(Value::Null) => ObservationField::Absent,
                Some(Value::String(value)) if value.is_empty() => ObservationField::Absent,
                Some(Value::String(value)) => match value.parse() {
                    Ok(value) => ObservationField::Observed(ObservedValue::new(value, origin)),
                    Err(_) => {
                        return native_malformed_field(&format!("{path}.{key}[{index}].HostIp"), identity, findings);
                    }
                },
                Some(_) => return native_malformed_field(&format!("{path}.{key}[{index}].HostIp"), identity, findings),
            };
            let host_port = match value.get("HostPort") {
                None | Some(Value::Null) => ObservationField::Absent,
                Some(Value::String(value)) if value.is_empty() => ObservationField::Absent,
                Some(Value::String(value)) => match value.parse() {
                    Ok(value) if value != 0 => ObservationField::Observed(ObservedValue::new(value, origin)),
                    _ => return native_malformed_field(&format!("{path}.{key}[{index}].HostPort"), identity, findings),
                },
                Some(_) => {
                    return native_malformed_field(&format!("{path}.{key}[{index}].HostPort"), identity, findings);
                }
            };
            decoded.push(NativePortBindingObservation::new(port, protocol, host_ip, host_port));
        }
    }
    ObservationField::Observed(ObservedValue::new(decoded, origin))
}

fn native_port_binding_key(value: &str) -> Option<(u16, NativePortProtocol)> {
    let (port, protocol) = value.split_once('/')?;
    let port = port.parse().ok().filter(|port: &u16| *port != 0)?;
    let protocol = match protocol {
        "tcp" => NativePortProtocol::Tcp,
        "udp" => NativePortProtocol::Udp,
        "sctp" => NativePortProtocol::Sctp,
        _ => return None,
    };
    Some((port, protocol))
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

    fn push_kind(&mut self, path: String, kind: JsonValueKind) -> bool {
        if self.fields.len() >= self.limit {
            self.overflowed = true;
            return false;
        }
        self.fields.push(UnmodelledField::new(
            path,
            kind,
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

#[allow(clippy::too_many_lines)] // explicit known-member sets are the audit boundary.
fn unknown_nested_fields(
    kind: ResourceKind,
    object: &Map<String, Value>,
    evidence: &ResourceEvidence,
    fields: &mut UnknownFieldCollector<'_>,
) {
    match kind {
        ResourceKind::Container => {
            unknown_object_members(
                object.get("Config"),
                "$.Config",
                &[
                    "Labels",
                    "Env",
                    "Secrets",
                    "Cmd",
                    "Entrypoint",
                    "User",
                    "WorkingDir",
                    "Hostname",
                    "Healthcheck",
                    "HealthcheckOnFailureAction",
                    "StartupHealthCheck",
                ],
                fields,
            );
            unknown_object_members(
                object
                    .get("Config")
                    .and_then(Value::as_object)
                    .and_then(|config| config.get("Healthcheck")),
                "$.Config.Healthcheck",
                &["Test", "Interval", "Timeout", "Retries", "StartPeriod"],
                fields,
            );
            unknown_object_members(
                object
                    .get("Config")
                    .and_then(Value::as_object)
                    .and_then(|config| config.get("StartupHealthCheck")),
                "$.Config.StartupHealthCheck",
                &["Test", "Interval", "Timeout", "Retries", "StartPeriod", "Successes"],
                fields,
            );
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
            unknown_array_object_members(
                object.get("Mounts"),
                "$.Mounts",
                &[
                    "Type",
                    "Name",
                    "Source",
                    "Destination",
                    "RW",
                    "Options",
                    "Propagation",
                    "SubPath",
                ],
                fields,
            );
            unknown_unsupported_mounts(object.get("Mounts"), fields);
            unknown_array_object_members(
                object
                    .get("Config")
                    .and_then(Value::as_object)
                    .and_then(|config| config.get("Secrets")),
                "$.Config.Secrets",
                &["ID", "Name", "UID", "GID", "Mode"],
                fields,
            );
            unknown_object_members(
                object
                    .get("HostConfig")
                    .and_then(Value::as_object)
                    .and_then(|host_config| host_config.get("RestartPolicy")),
                "$.HostConfig.RestartPolicy",
                &["Name", "MaximumRetryCount"],
                fields,
            );
            unknown_object_members(
                object
                    .get("HostConfig")
                    .and_then(Value::as_object)
                    .and_then(|host_config| host_config.get("LogConfig")),
                "$.HostConfig.LogConfig",
                &["Type", "Size"],
                fields,
            );
            unknown_array_object_members(
                object
                    .get("HostConfig")
                    .and_then(Value::as_object)
                    .and_then(|host_config| host_config.get("Ulimits")),
                "$.HostConfig.Ulimits",
                &["Name", "Soft", "Hard"],
                fields,
            );
            // HostAdd intentionally remains value-free unmodelled hosts-file data. All other
            // listed members have a bounded typed observation; unknown siblings remain explicit
            // metadata rather than being hidden by the accepted HostConfig object.
            unknown_object_members(
                object.get("HostConfig"),
                "$.HostConfig",
                &[
                    "MemorySwappiness",
                    "CreateNetNS",
                    "PortBindings",
                    "Dns",
                    "DnsSearch",
                    "DnsOptions",
                    "NoManageResolvConf",
                    "NoManageHosts",
                    "RestartPolicy",
                    "LogConfig",
                    "Privileged",
                    "CapAdd",
                    "CapDrop",
                    "SecurityOpt",
                    "ReadonlyRootfs",
                    "PidMode",
                    "IpcMode",
                    "UTSMode",
                    "CgroupMode",
                    "CpuShares",
                    "CpuPeriod",
                    "CpuQuota",
                    "Memory",
                    "PidsLimit",
                    "Ulimits",
                ],
                fields,
            );
        }
        ResourceKind::Pod => {
            unknown_array_object_members(object.get("Containers"), "$.Containers", &["Id"], fields);
            unknown_object_members(
                object.get("InfraConfig"),
                "$.InfraConfig",
                &[
                    "PortBindings",
                    "HostNetwork",
                    "DNSServer",
                    "DNSSearch",
                    "DNSOption",
                    "Networks",
                    "NetworkOptions",
                    "NoManageResolvConf",
                    "NoManageHosts",
                    "StaticMAC",
                    "StaticIP",
                ],
                fields,
            );
        }
        ResourceKind::Network => unknown_network_nested_fields(object, evidence, fields),
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

fn unknown_network_nested_fields(
    object: &Map<String, Value>,
    evidence: &ResourceEvidence,
    fields: &mut UnknownFieldCollector<'_>,
) {
    unknown_array_object_members(
        object.get("subnets"),
        "$.subnets",
        &["subnet", "gateway", "lease_range"],
        fields,
    );
    if let Some(subnets) = object.get("subnets").and_then(Value::as_array) {
        for (index, subnet) in subnets.iter().enumerate() {
            unknown_object_members(
                subnet.as_object().and_then(|subnet| subnet.get("lease_range")),
                &format!("$.subnets[{index}].lease_range"),
                &["start_ip", "end_ip"],
                fields,
            );
        }
    }
    unknown_array_object_members(
        object.get("routes"),
        "$.routes",
        &["destination", "gateway", "metric", "route_type"],
        fields,
    );
    unknown_network_route_type_members(object.get("routes"), evidence, fields);
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

fn unknown_unsupported_mounts(value: Option<&Value>, fields: &mut UnknownFieldCollector<'_>) {
    let Some(mounts) = value.and_then(Value::as_array) else {
        return;
    };
    for (index, mount) in mounts.iter().enumerate() {
        let unsupported = mount
            .as_object()
            .and_then(|mount| mount.get("Type"))
            .and_then(Value::as_str)
            .is_some_and(|kind| !matches!(kind, "volume" | "bind"));
        if unsupported && !fields.push(|| format!("$.Mounts[{index}]"), mount) {
            break;
        }
    }
}

fn unknown_network_route_type_members(
    value: Option<&Value>,
    evidence: &ResourceEvidence,
    fields: &mut UnknownFieldCollector<'_>,
) {
    let Some(routes) = value.and_then(Value::as_array) else {
        return;
    };
    for (index, route) in routes.iter().enumerate() {
        let Some(route) = route.as_object() else {
            continue;
        };
        let Some(route_type) = route.get("route_type") else {
            continue;
        };
        let unsupported = !native_network_route_types_are_available(evidence)
            || !route_type
                .as_str()
                .is_some_and(|value| matches!(value, "unicast" | "blackhole" | "unreachable" | "prohibit"));
        if unsupported && !fields.push(|| format!("$.routes[{index}].route_type"), route_type) {
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
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
            )),
            ResourceDetails::Pod(PodObservation::new(
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
            )),
            ResourceDetails::Network(NetworkObservation::new(
                ObservationField::Absent,
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
            ResourceDetails::Pod(PodObservation::new(
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
                ObservationField::Absent,
            )),
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
