//! Typed, provenance-aware observations at the Podman native input boundary.
//!
//! This module deliberately models what the selected Podman service reported; it does not infer
//! desired deployment intent.  In particular, runtime-assigned addresses, local image IDs and
//! current lifecycle state never share a type with configured facts.  BoxFerry-facing adapters
//! must make an explicit mapping decision for every [`ObservationOrigin`].

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    net::IpAddr,
};

use crate::{
    Diagnostic, DiagnosticCode, InventoryFinding, JsonValueKind, ResourceEvidence, ResourceIdentity, ResourceKind,
    SensitiveEnvironmentValue,
};

/// The observation state of one native field.
///
/// `Absent` means that the reviewed wire field was absent or `null`; it never means malformed,
/// unavailable, inapplicable, or omitted from the native model.  Those states are represented
/// separately so an adapter cannot turn a decoder failure into an intentional empty value.
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum ObservationField<T> {
    /// The field was absent from an otherwise decoded response.
    Absent,
    /// The field was decoded and carries its source disposition.
    Observed(ObservedValue<T>),
    /// The containing resource or section could not be acquired.
    Unavailable,
    /// The field was present but could not be decoded according to its reviewed shape.
    Malformed,
    /// The reviewed native version does not give this field a usable meaning.
    VersionInapplicable,
    /// The field has no meaning for this resource kind.
    NotApplicable,
    /// The field was deliberately retained only as bounded unmodelled metadata.
    Unmodelled(UnmodelledFieldId),
}

impl<T> fmt::Debug for ObservationField<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ObservationField")
            .field(&match self {
                Self::Absent => "absent",
                Self::Observed(_) => "observed",
                Self::Unavailable => "unavailable",
                Self::Malformed => "malformed",
                Self::VersionInapplicable => "version_inapplicable",
                Self::NotApplicable => "not_applicable",
                Self::Unmodelled(id) => id.as_str(),
            })
            .finish()
    }
}

impl<T> ObservationField<T> {
    /// Returns the observed value only when the field decoded successfully.
    #[must_use]
    pub const fn observed(&self) -> Option<&ObservedValue<T>> {
        match self {
            Self::Observed(value) => Some(value),
            _ => None,
        }
    }

    /// Returns whether this field contains a usable observation.
    #[must_use]
    pub const fn is_observed(&self) -> bool {
        matches!(self, Self::Observed(_))
    }

    /// Returns whether the native field was present but did not match its reviewed shape.
    #[must_use]
    pub const fn is_malformed(&self) -> bool {
        matches!(self, Self::Malformed)
    }
}

/// Provenance that prevents observed runtime facts from becoming desired intent accidentally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ObservationOrigin {
    /// An explicit setting declared in the native resource configuration.
    Configured,
    /// A native effective value that may incorporate Podman defaults.
    Effective,
    /// A value allocated by the runtime, such as an address or a live state.
    RuntimeAssigned,
    /// A local resolver result, such as a resolved image ID.
    LocalResolution,
}

/// A successfully decoded value and its non-promotable provenance.
#[derive(Clone, Eq, PartialEq)]
pub struct ObservedValue<T> {
    value: T,
    origin: ObservationOrigin,
}

impl<T> fmt::Debug for ObservedValue<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObservedValue")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

impl<T> ObservedValue<T> {
    /// Creates an observed value with explicit provenance.
    #[must_use]
    pub const fn new(value: T, origin: ObservationOrigin) -> Self {
        Self { value, origin }
    }

    /// Returns the value exactly as observed.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Returns the source disposition of the value.
    #[must_use]
    pub const fn origin(&self) -> ObservationOrigin {
        self.origin
    }
}

/// Stable semantic identifier for bounded metadata not modelled by this release.
///
/// The identifier is intentionally independent from a JSON spelling.  The accompanying
/// [`UnmodelledField`] records its observed JSON path and kind, but never the raw value.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum UnmodelledFieldId {
    /// Container `HostConfig` data outside the bounded typed subset.
    ContainerHostConfig,
    /// Container secret-grant metadata outside the bounded typed subset.
    ContainerSecretGrant,
    /// Container configuration data outside the bounded typed subset.
    ContainerConfig,
    /// Container network-settings data outside the bounded typed subset.
    ContainerNetworkSettings,
    /// Container mount data outside the bounded typed subset.
    ContainerMount,
    /// Other container inspect data outside the bounded typed subset.
    ContainerTopLevel,
    /// Pod membership data outside the bounded typed subset.
    PodMember,
    /// Pod infra-configuration data outside the bounded typed subset.
    PodInfraConfig,
    /// Other pod inspect data outside the bounded typed subset.
    PodTopLevel,
    /// Network subnet data outside the bounded typed subset.
    NetworkSubnet,
    /// Network route data outside the bounded typed subset.
    NetworkRoute,
    /// Other network inspect data outside the bounded typed subset.
    NetworkTopLevel,
    /// Volume inspect data outside the bounded typed subset.
    VolumeTopLevel,
    /// Image configuration data outside the bounded typed subset.
    ImageConfig,
    /// Other image inspect data outside the bounded typed subset.
    ImageTopLevel,
    /// Secret specification metadata outside the bounded typed subset.
    SecretSpec,
    /// Other secret inspect data outside the bounded typed subset.
    SecretTopLevel,
}

impl UnmodelledFieldId {
    /// Returns the stable semantic identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContainerHostConfig => "podman.native.container.host-config",
            Self::ContainerSecretGrant => "podman.native.container.secret-grant",
            Self::ContainerConfig => "podman.native.container.config",
            Self::ContainerNetworkSettings => "podman.native.container.network-settings",
            Self::ContainerMount => "podman.native.container.mount",
            Self::ContainerTopLevel => "podman.native.container.top-level",
            Self::PodMember => "podman.native.pod.member",
            Self::PodInfraConfig => "podman.native.pod.infra-config",
            Self::PodTopLevel => "podman.native.pod.top-level",
            Self::NetworkSubnet => "podman.native.network.subnet",
            Self::NetworkRoute => "podman.native.network.route",
            Self::NetworkTopLevel => "podman.native.network.top-level",
            Self::VolumeTopLevel => "podman.native.volume.top-level",
            Self::ImageConfig => "podman.native.image.config",
            Self::ImageTopLevel => "podman.native.image.top-level",
            Self::SecretSpec => "podman.native.secret.spec",
            Self::SecretTopLevel => "podman.native.secret.top-level",
        }
    }
}

/// Bounded, redacted metadata for one native field that remains unmodelled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnmodelledField {
    id: UnmodelledFieldId,
    path: String,
    json_kind: JsonValueKind,
    resource: ResourceIdentity,
    evidence: ResourceEvidence,
}

impl UnmodelledField {
    #[allow(clippy::too_many_arguments)] // private typed decoder construction keeps every field explicit.
    pub(crate) fn new(
        path: String,
        json_kind: JsonValueKind,
        resource: ResourceIdentity,
        evidence: ResourceEvidence,
    ) -> Self {
        Self {
            id: semantic_unmodelled_id(resource.kind(), &path),
            path,
            json_kind,
            resource,
            evidence,
        }
    }

    /// Returns the stable semantic ID, never a raw native value.
    #[must_use]
    pub fn id(&self) -> &UnmodelledFieldId {
        &self.id
    }

    /// Returns the observed JSON path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the observed JSON value kind.
    #[must_use]
    pub const fn json_kind(&self) -> JsonValueKind {
        self.json_kind
    }

    /// Returns the carrying resource identity.
    #[must_use]
    pub fn resource(&self) -> &ResourceIdentity {
        &self.resource
    }

    /// Returns immutable version evidence for this observation.
    #[must_use]
    pub fn evidence(&self) -> &ResourceEvidence {
        &self.evidence
    }
}

fn semantic_unmodelled_id(kind: ResourceKind, path: &str) -> UnmodelledFieldId {
    match (kind, path) {
        (ResourceKind::Container, value) if value.starts_with("$.HostConfig") => UnmodelledFieldId::ContainerHostConfig,
        (ResourceKind::Container, value) if value.starts_with("$.Config.Secrets") => {
            UnmodelledFieldId::ContainerSecretGrant
        }
        (ResourceKind::Container, value) if value.starts_with("$.Config") => UnmodelledFieldId::ContainerConfig,
        (ResourceKind::Container, value) if value.starts_with("$.NetworkSettings") => {
            UnmodelledFieldId::ContainerNetworkSettings
        }
        (ResourceKind::Container, value) if value.starts_with("$.Mounts") => UnmodelledFieldId::ContainerMount,
        (ResourceKind::Pod, value) if value.starts_with("$.Containers") => UnmodelledFieldId::PodMember,
        (ResourceKind::Pod, value) if value.starts_with("$.InfraConfig") => UnmodelledFieldId::PodInfraConfig,
        (ResourceKind::Network, value) if value.starts_with("$.subnets") => UnmodelledFieldId::NetworkSubnet,
        (ResourceKind::Network, value) if value.starts_with("$.routes") => UnmodelledFieldId::NetworkRoute,
        (ResourceKind::Image, value) if value.starts_with("$.Config") => UnmodelledFieldId::ImageConfig,
        (ResourceKind::Secret, value) if value.starts_with("$.Spec") => UnmodelledFieldId::SecretSpec,
        (ResourceKind::Container, _) => UnmodelledFieldId::ContainerTopLevel,
        (ResourceKind::Pod, _) => UnmodelledFieldId::PodTopLevel,
        (ResourceKind::Network, _) => UnmodelledFieldId::NetworkTopLevel,
        (ResourceKind::Volume, _) => UnmodelledFieldId::VolumeTopLevel,
        (ResourceKind::Image, _) => UnmodelledFieldId::ImageTopLevel,
        (ResourceKind::Secret, _) => UnmodelledFieldId::SecretTopLevel,
    }
}

/// Completeness state for bounded unmodelled metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnmodelledCompleteness {
    /// All direct unmodelled fields were retained within configured bounds.
    Complete,
    /// The observation is partial or the retention budget overflowed.
    Incomplete,
}

/// Acquisition state of one resource observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ResourceObservationState {
    /// The inspected response decoded according to the current native contract.
    Complete,
    /// The resource could not be acquired during the non-atomic inventory read.
    Unavailable,
    /// The resource inspect response was malformed or contradicted the list identity.
    Malformed,
}

/// Resource-wide observation information shared by every detail variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationHeader {
    identity: ResourceIdentity,
    state: ResourceObservationState,
    evidence: ResourceEvidence,
    findings: Vec<InventoryFinding>,
    unmodelled: Vec<UnmodelledField>,
    unmodelled_completeness: UnmodelledCompleteness,
}

impl ObservationHeader {
    pub(crate) fn complete(
        identity: ResourceIdentity,
        evidence: ResourceEvidence,
        findings: Vec<InventoryFinding>,
        unmodelled: Vec<UnmodelledField>,
        unmodelled_completeness: UnmodelledCompleteness,
    ) -> Self {
        Self {
            identity,
            state: ResourceObservationState::Complete,
            evidence,
            findings,
            unmodelled,
            unmodelled_completeness,
        }
    }

    pub(crate) fn incomplete(
        identity: ResourceIdentity,
        evidence: ResourceEvidence,
        state: ResourceObservationState,
        findings: Vec<InventoryFinding>,
    ) -> Self {
        Self {
            identity,
            state,
            evidence,
            findings,
            unmodelled: Vec::new(),
            unmodelled_completeness: UnmodelledCompleteness::Incomplete,
        }
    }

    /// Returns the stable native identity.
    #[must_use]
    pub fn identity(&self) -> &ResourceIdentity {
        &self.identity
    }

    /// Returns the typed non-atomic acquisition state for this resource.
    #[must_use]
    pub const fn state(&self) -> ResourceObservationState {
        self.state
    }

    /// Returns immutable source/version evidence.
    #[must_use]
    pub fn evidence(&self) -> &ResourceEvidence {
        &self.evidence
    }

    /// Returns redacted structured findings for this resource.
    #[must_use]
    pub fn findings(&self) -> &[InventoryFinding] {
        &self.findings
    }

    pub(crate) fn findings_mut(&mut self) -> &mut Vec<InventoryFinding> {
        &mut self.findings
    }

    /// Returns bounded unmodelled metadata without raw values.
    #[must_use]
    pub fn unmodelled_fields(&self) -> &[UnmodelledField] {
        &self.unmodelled
    }

    /// Returns whether bounded metadata accounts for all unmodelled fields.
    #[must_use]
    pub const fn unmodelled_completeness(&self) -> UnmodelledCompleteness {
        self.unmodelled_completeness
    }
}

/// A relationship used internally by canonical discovery derivation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeRelationship {
    pub(crate) kind: ResourceKind,
    /// One relationship can retain several native references when the wire format carries an
    /// identifier and a name for the same grant.  Resolution requires every supplied reference
    /// to select the same target; discovery must never choose one spelling silently.
    pub(crate) references: Vec<String>,
    /// Every source location that asserted this one native relationship.
    pub(crate) field_paths: Vec<String>,
}

impl NativeRelationship {
    pub(crate) fn new(kind: ResourceKind, target_id: impl Into<String>, field_path: impl Into<String>) -> Self {
        Self {
            kind,
            references: vec![target_id.into()],
            field_paths: vec![field_path.into()],
        }
    }

    pub(crate) fn coalesced(
        kind: ResourceKind,
        references: impl IntoIterator<Item = (String, String)>,
    ) -> Option<Self> {
        let mut values = Vec::new();
        let mut paths = Vec::new();
        for (value, path) in references {
            if !values.contains(&value) {
                values.push(value);
            }
            paths.push(path);
        }
        (!values.is_empty()).then_some(Self {
            kind,
            references: values,
            field_paths: paths,
        })
    }
}

/// A protected runtime environment observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedEnvironment {
    entries: Vec<ProtectedEnvironmentEntry>,
}

impl ProtectedEnvironment {
    pub(crate) fn new(entries: Vec<ProtectedEnvironmentEntry>) -> Self {
        Self { entries }
    }

    /// Returns variable names and protected value states in source order.
    #[must_use]
    pub fn entries(&self) -> &[ProtectedEnvironmentEntry] {
        &self.entries
    }
}

/// One protected runtime environment name/value-state pair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedEnvironmentEntry {
    name: String,
    value: ProtectedEnvironmentValue,
}

impl ProtectedEnvironmentEntry {
    pub(crate) fn new(name: String, value: ProtectedEnvironmentValue) -> Self {
        Self { name, value }
    }

    /// Returns the variable name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the protected state, never a public deployment value.
    #[must_use]
    pub fn value(&self) -> &ProtectedEnvironmentValue {
        &self.value
    }
}

/// Protected runtime environment value state.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedEnvironmentValue {
    /// The source value is deliberately not retained.
    Redacted,
    /// An explicitly authorized opaque value; formatting and snapshots remain redacted.
    AuthorizedOpaque(SensitiveEnvironmentValue),
}

/// A bounded configured label collection.
pub type Labels = BTreeMap<String, String>;

/// A configured container command observed from `Config.Cmd`.
///
/// This is native input evidence, not a deployment argument type. Its constructor stays private
/// so callers cannot accidentally manufacture an observation with invented provenance.
#[derive(Clone, Eq, PartialEq)]
pub struct ConfiguredContainerCommand(Vec<String>);

impl ConfiguredContainerCommand {
    pub(crate) const fn new(arguments: Vec<String>) -> Self {
        Self(arguments)
    }

    /// Returns the declared command arguments in their native order.
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.0
    }
}

impl fmt::Debug for ConfiguredContainerCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfiguredContainerCommand")
            .field("argument_count", &self.0.len())
            .finish()
    }
}

/// A configured container entrypoint observed from `Config.Entrypoint`.
#[derive(Clone, Eq, PartialEq)]
pub struct ConfiguredContainerEntrypoint(Vec<String>);

impl ConfiguredContainerEntrypoint {
    pub(crate) const fn new(arguments: Vec<String>) -> Self {
        Self(arguments)
    }

    /// Returns the declared entrypoint arguments in their native order.
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.0
    }
}

impl fmt::Debug for ConfiguredContainerEntrypoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfiguredContainerEntrypoint")
            .field("argument_count", &self.0.len())
            .finish()
    }
}

macro_rules! configured_container_text {
    ($type:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Eq, PartialEq)]
        pub struct $type(String);

        impl $type {
            pub(crate) fn new(value: String) -> Self {
                Self(value)
            }

            /// Returns the native configured spelling.
            #[must_use]
            pub fn value(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $type {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($type), "([redacted])"))
            }
        }
    };
}

configured_container_text!(
    ConfiguredContainerUser,
    "A configured container user from `Config.User`."
);
configured_container_text!(
    ConfiguredContainerWorkdir,
    "A configured container working directory from `Config.WorkingDir`."
);
configured_container_text!(
    ConfiguredContainerHostname,
    "A configured container hostname from `Config.Hostname`."
);

/// One native relationship reference with its exact source location.
#[derive(Clone, Eq, PartialEq)]
pub struct NativeResourceReference {
    reference: String,
    field_path: String,
}

impl NativeResourceReference {
    pub(crate) fn new(reference: String, field_path: String) -> Self {
        Self { reference, field_path }
    }

    /// Returns the native identifier or name that requires explicit resolution.
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// Returns the reviewed native field that supplied this reference.
    #[must_use]
    pub fn field_path(&self) -> &str {
        &self.field_path
    }
}

impl fmt::Debug for NativeResourceReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeResourceReference")
            .field("field_path", &self.field_path)
            .finish_non_exhaustive()
    }
}

/// A typed declared container mount kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContainerMountKind {
    /// A named Podman volume.
    NamedVolume,
    /// A host bind mount. Its source path stays local-resolution evidence.
    Bind,
}

/// The source of a typed native mount.
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContainerMountSource {
    /// A configured named-volume reference.
    NamedVolume(String),
    /// A host-specific path observed from the local Podman service.
    LocalBindPath(String),
}

/// An explicitly configured `SELinux` relabel policy for one bind mount.
///
/// Podman reports this intent as the case-sensitive `z` or `Z` option. The
/// decoder retains only that closed semantic choice; it never exposes the
/// surrounding native bind specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContainerMountSelinuxRelabel {
    /// Relabel content so multiple containers may share it (`z`).
    Shared,
    /// Relabel content for private use by one container (`Z`).
    Private,
}

impl ContainerMountSource {
    /// Returns the native source spelling. A bind path is local-resolution evidence and must not
    /// be promoted automatically into portable intent.
    #[must_use]
    pub fn value(&self) -> &str {
        match self {
            Self::NamedVolume(value) | Self::LocalBindPath(value) => value,
        }
    }
}

impl fmt::Debug for ContainerMountSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::NamedVolume(_) => "named_volume",
            Self::LocalBindPath(_) => "local_bind_path",
        };
        formatter.debug_tuple("ContainerMountSource").field(&kind).finish()
    }
}

/// One typed native mount. Every nested field keeps its independent native observation state.
#[derive(Clone, Eq, PartialEq)]
pub struct ContainerMountObservation {
    kind: ContainerMountKind,
    source: ObservationField<ContainerMountSource>,
    local_backing_path: ObservationField<String>,
    destination: ObservationField<String>,
    writable: ObservationField<bool>,
    options: ObservationField<Vec<String>>,
    selinux_relabel: ObservationField<ContainerMountSelinuxRelabel>,
    propagation: ObservationField<String>,
    subpath: ObservationField<String>,
}

impl ContainerMountObservation {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        kind: ContainerMountKind,
        source: ObservationField<ContainerMountSource>,
        local_backing_path: ObservationField<String>,
        destination: ObservationField<String>,
        writable: ObservationField<bool>,
        options: ObservationField<Vec<String>>,
        selinux_relabel: ObservationField<ContainerMountSelinuxRelabel>,
        propagation: ObservationField<String>,
        subpath: ObservationField<String>,
    ) -> Self {
        Self {
            kind,
            source,
            local_backing_path,
            destination,
            writable,
            options,
            selinux_relabel,
            propagation,
            subpath,
        }
    }

    /// Returns the accepted native mount kind.
    #[must_use]
    pub const fn kind(&self) -> ContainerMountKind {
        self.kind
    }
    /// Returns source evidence; bind paths are always local-resolution evidence.
    #[must_use]
    pub fn source(&self) -> &ObservationField<ContainerMountSource> {
        &self.source
    }
    /// Returns the host-specific backing path when Podman supplied one. This is always local
    /// resolution evidence and cannot be promoted automatically.
    #[must_use]
    pub fn local_backing_path(&self) -> &ObservationField<String> {
        &self.local_backing_path
    }
    /// Returns the configured container destination.
    #[must_use]
    pub fn destination(&self) -> &ObservationField<String> {
        &self.destination
    }
    /// Returns the observed writable setting.
    #[must_use]
    pub fn writable(&self) -> &ObservationField<bool> {
        &self.writable
    }
    /// Returns mount options when the native response supplied them.
    #[must_use]
    pub fn options(&self) -> &ObservationField<Vec<String>> {
        &self.options
    }

    /// Returns the configured `SELinux` relabel choice recovered from bounded
    /// native mount evidence.
    #[must_use]
    pub fn selinux_relabel(&self) -> &ObservationField<ContainerMountSelinuxRelabel> {
        &self.selinux_relabel
    }
    /// Returns mount propagation when the native response supplied it.
    #[must_use]
    pub fn propagation(&self) -> &ObservationField<String> {
        &self.propagation
    }
    /// Returns named-volume subpath evidence when the native response supplied it.
    #[must_use]
    pub fn subpath(&self) -> &ObservationField<String> {
        &self.subpath
    }
}

impl fmt::Debug for ContainerMountObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContainerMountObservation")
            .field("kind", &self.kind)
            .field("source", &self.source)
            .field(
                "local_backing_path_state",
                &observation_field_state(&self.local_backing_path),
            )
            .field("destination_state", &observation_field_state(&self.destination))
            .field("writable", &self.writable)
            .field(
                "option_count",
                &self.options.observed().map_or(0, |options| options.value().len()),
            )
            .field("selinux_relabel", &self.selinux_relabel)
            .field("propagation_state", &observation_field_state(&self.propagation))
            .field("subpath_state", &observation_field_state(&self.subpath))
            .finish()
    }
}

fn observation_field_state<T>(field: &ObservationField<T>) -> &'static str {
    match field {
        ObservationField::Observed(_) => "observed",
        ObservationField::Absent => "absent",
        ObservationField::Unavailable => "unavailable",
        ObservationField::Malformed => "malformed",
        ObservationField::VersionInapplicable => "version-inapplicable",
        ObservationField::NotApplicable => "not-applicable",
        ObservationField::Unmodelled(_) => "unmodelled",
    }
}

/// Coalesced secret ID/name evidence. Both spellings must resolve to one secret before traversal.
#[derive(Clone, Eq, PartialEq)]
pub struct ContainerSecretReference {
    id: Option<NativeResourceReference>,
    name: Option<NativeResourceReference>,
}

impl ContainerSecretReference {
    pub(crate) const fn new(id: Option<NativeResourceReference>, name: Option<NativeResourceReference>) -> Self {
        Self { id, name }
    }
    /// Returns the optional native secret-ID source evidence.
    #[must_use]
    pub fn id(&self) -> Option<&NativeResourceReference> {
        self.id.as_ref()
    }
    /// Returns the optional native secret-name source evidence.
    #[must_use]
    pub fn name(&self) -> Option<&NativeResourceReference> {
        self.name.as_ref()
    }
}

impl fmt::Debug for ContainerSecretReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContainerSecretReference")
            .field("has_id", &self.id.is_some())
            .field("has_name", &self.name.is_some())
            .finish()
    }
}

/// One typed native secret grant. It never carries secret payload bytes.
#[derive(Clone, Eq, PartialEq)]
pub struct ContainerSecretGrantObservation {
    reference: ObservationField<ContainerSecretReference>,
    uid: ObservationField<u32>,
    gid: ObservationField<u32>,
    mode: ObservationField<u32>,
}

impl ContainerSecretGrantObservation {
    pub(crate) const fn new(
        reference: ObservationField<ContainerSecretReference>,
        uid: ObservationField<u32>,
        gid: ObservationField<u32>,
        mode: ObservationField<u32>,
    ) -> Self {
        Self {
            reference,
            uid,
            gid,
            mode,
        }
    }
    /// Returns coalesced ID/name source evidence.
    #[must_use]
    pub fn reference(&self) -> &ObservationField<ContainerSecretReference> {
        &self.reference
    }
    /// Returns effective UID metadata. Podman inspect does not preserve whether zero was explicit.
    #[must_use]
    pub fn uid(&self) -> &ObservationField<u32> {
        &self.uid
    }
    /// Returns effective GID metadata. Podman inspect does not preserve whether zero was explicit.
    #[must_use]
    pub fn gid(&self) -> &ObservationField<u32> {
        &self.gid
    }
    /// Returns effective file-mode metadata. Podman inspect does not preserve whether zero was explicit.
    #[must_use]
    pub fn mode(&self) -> &ObservationField<u32> {
        &self.mode
    }
}

impl fmt::Debug for ContainerSecretGrantObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContainerSecretGrantObservation")
            .field("reference", &self.reference)
            .field("uid", &self.uid)
            .field("gid", &self.gid)
            .field("mode", &self.mode)
            .finish()
    }
}

/// A bounded native restart policy name observed from HostConfig.RestartPolicy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NativeRestartPolicyName {
    /// Never restart automatically.
    No,
    /// Always restart automatically.
    Always,
    /// Restart after a failure.
    OnFailure,
    /// Restart unless explicitly stopped.
    UnlessStopped,
}

/// Typed, effective native restart-policy evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRestartPolicyObservation {
    name: ObservationField<NativeRestartPolicyName>,
    maximum_retry_count: ObservationField<u64>,
}

impl NativeRestartPolicyObservation {
    pub(crate) const fn new(
        name: ObservationField<NativeRestartPolicyName>,
        maximum_retry_count: ObservationField<u64>,
    ) -> Self {
        Self {
            name,
            maximum_retry_count,
        }
    }

    /// Returns the effective policy name or its field state.
    #[must_use]
    pub fn name(&self) -> &ObservationField<NativeRestartPolicyName> {
        &self.name
    }

    /// Returns the effective native retry count, including valid zero.
    #[must_use]
    pub fn maximum_retry_count(&self) -> &ObservationField<u64> {
        &self.maximum_retry_count
    }
}

/// A protected health-check command. Its argument values are never formatted or snapshotted.
#[derive(Clone, Eq, PartialEq)]
pub struct ProtectedHealthCommand {
    arguments: Vec<String>,
}

impl ProtectedHealthCommand {
    pub(crate) const fn new(arguments: Vec<String>) -> Self {
        Self { arguments }
    }

    /// Returns the number of protected command arguments without disclosing their values.
    #[must_use]
    pub fn argument_count(&self) -> usize {
        self.arguments.len()
    }
    /// Lets an explicitly authorized caller use arguments without formatting or serializing them.
    pub fn expose<R>(&self, use_arguments: impl FnOnce(&[String]) -> R) -> R {
        use_arguments(&self.arguments)
    }
}

impl fmt::Debug for ProtectedHealthCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedHealthCommand")
            .field("argument_count", &self.arguments.len())
            .finish()
    }
}

impl fmt::Display for ProtectedHealthCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

/// Native health command syntax with values retained as protected evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NativeHealthCommand {
    /// Podman's explicit NONE health-check form.
    Disabled,
    /// A shell command whose arguments are protected.
    Shell(ProtectedHealthCommand),
    /// A direct executable command whose arguments are protected.
    Exec(ProtectedHealthCommand),
}

/// Typed normal-health observation from Config.Healthcheck.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeHealthCheckObservation {
    command: ObservationField<NativeHealthCommand>,
    interval: ObservationField<i64>,
    timeout: ObservationField<i64>,
    retries: ObservationField<u64>,
    start_period: ObservationField<i64>,
}

impl NativeHealthCheckObservation {
    pub(crate) const fn new(
        command: ObservationField<NativeHealthCommand>,
        interval: ObservationField<i64>,
        timeout: ObservationField<i64>,
        retries: ObservationField<u64>,
        start_period: ObservationField<i64>,
    ) -> Self {
        Self {
            command,
            interval,
            timeout,
            retries,
            start_period,
        }
    }

    /// Returns protected health-command evidence or its field state.
    #[must_use]
    pub fn command(&self) -> &ObservationField<NativeHealthCommand> {
        &self.command
    }
    /// Returns the native interval, including effective zero.
    #[must_use]
    pub fn interval(&self) -> &ObservationField<i64> {
        &self.interval
    }
    /// Returns the native timeout, including effective zero.
    #[must_use]
    pub fn timeout(&self) -> &ObservationField<i64> {
        &self.timeout
    }
    /// Returns the effective native retry count, including zero.
    #[must_use]
    pub fn retries(&self) -> &ObservationField<u64> {
        &self.retries
    }
    /// Returns the native start period, including effective zero.
    #[must_use]
    pub fn start_period(&self) -> &ObservationField<i64> {
        &self.start_period
    }
}

/// The bounded normal-health failure action reported by Podman.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NativeHealthFailureAction {
    /// Retains the unhealthy state without stopping the container.
    None,
    /// Kills the container.
    Kill,
    /// Restarts the container.
    Restart,
    /// Stops the container.
    Stop,
}

/// Typed startup-health observation from Config.StartupHealthCheck.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeStartupHealthCheckObservation {
    command: ObservationField<NativeHealthCommand>,
    interval: ObservationField<i64>,
    timeout: ObservationField<i64>,
    retries: ObservationField<u64>,
    start_period: ObservationField<i64>,
    successes: ObservationField<u64>,
}

impl NativeStartupHealthCheckObservation {
    pub(crate) const fn new(
        command: ObservationField<NativeHealthCommand>,
        interval: ObservationField<i64>,
        timeout: ObservationField<i64>,
        retries: ObservationField<u64>,
        start_period: ObservationField<i64>,
        successes: ObservationField<u64>,
    ) -> Self {
        Self {
            command,
            interval,
            timeout,
            retries,
            start_period,
            successes,
        }
    }

    /// Returns protected startup-command evidence or its field state.
    #[must_use]
    pub fn command(&self) -> &ObservationField<NativeHealthCommand> {
        &self.command
    }
    /// Returns the native interval, including effective zero.
    #[must_use]
    pub fn interval(&self) -> &ObservationField<i64> {
        &self.interval
    }
    /// Returns the native timeout, including effective zero.
    #[must_use]
    pub fn timeout(&self) -> &ObservationField<i64> {
        &self.timeout
    }
    /// Returns the effective native retry count, including zero.
    #[must_use]
    pub fn retries(&self) -> &ObservationField<u64> {
        &self.retries
    }
    /// Returns the effective native start period, including zero.
    #[must_use]
    pub fn start_period(&self) -> &ObservationField<i64> {
        &self.start_period
    }
    /// Returns the effective native startup success threshold, including zero.
    #[must_use]
    pub fn successes(&self) -> &ObservationField<u64> {
        &self.successes
    }
}

/// The bounded logging drivers represented by this native observation batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NativeLogDriver {
    /// Podman's journald driver.
    Journald,
    /// Podman's k8s-file driver.
    K8sFile,
}

/// Native logging observation from HostConfig.LogConfig.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeLoggingObservation {
    driver: ObservationField<NativeLogDriver>,
    size: ObservationField<String>,
}

impl NativeLoggingObservation {
    pub(crate) const fn new(driver: ObservationField<NativeLogDriver>, size: ObservationField<String>) -> Self {
        Self { driver, size }
    }

    /// Returns the effective logging driver or its field state.
    #[must_use]
    pub fn driver(&self) -> &ObservationField<NativeLogDriver> {
        &self.driver
    }

    /// Returns the effective native log-size spelling or its field state.
    #[must_use]
    pub fn size(&self) -> &ObservationField<String> {
        &self.size
    }
}

/// One reviewed Linux capability spelling from native container inspection.
///
/// This input-only type is deliberately separate from the deployment capability type.
/// Native order and duplicates are evidence and therefore remain intact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeCapability(String);

impl NativeCapability {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    /// Returns the exact reviewed native capability spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque native security options. Values are deliberately never retained or exposed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOpaqueSecurityOptions {
    count: usize,
}

impl NativeOpaqueSecurityOptions {
    pub(crate) const fn new(count: usize) -> Self {
        Self { count }
    }

    /// Returns the number of opaque security options.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Returns whether no opaque security options were observed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Effective native security evidence from `HostConfig`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeSecurityObservation {
    privileged: ObservationField<bool>,
    cap_add: ObservationField<Vec<NativeCapability>>,
    cap_drop: ObservationField<Vec<NativeCapability>>,
    security_options: ObservationField<NativeOpaqueSecurityOptions>,
    read_only_root_filesystem: ObservationField<bool>,
}

impl NativeSecurityObservation {
    pub(crate) const fn new(
        privileged: ObservationField<bool>,
        cap_add: ObservationField<Vec<NativeCapability>>,
        cap_drop: ObservationField<Vec<NativeCapability>>,
        security_options: ObservationField<NativeOpaqueSecurityOptions>,
        read_only_root_filesystem: ObservationField<bool>,
    ) -> Self {
        Self {
            privileged,
            cap_add,
            cap_drop,
            security_options,
            read_only_root_filesystem,
        }
    }

    /// Returns effective privileged state.
    #[must_use]
    pub fn privileged(&self) -> &ObservationField<bool> {
        &self.privileged
    }

    /// Returns added capabilities in native order, including duplicates.
    #[must_use]
    pub fn cap_add(&self) -> &ObservationField<Vec<NativeCapability>> {
        &self.cap_add
    }

    /// Returns dropped capabilities in native order, including duplicates.
    #[must_use]
    pub fn cap_drop(&self) -> &ObservationField<Vec<NativeCapability>> {
        &self.cap_drop
    }

    /// Returns only the count and state of opaque security options.
    #[must_use]
    pub fn security_options(&self) -> &ObservationField<NativeOpaqueSecurityOptions> {
        &self.security_options
    }

    /// Returns effective read-only-root-filesystem state.
    #[must_use]
    pub fn read_only_root_filesystem(&self) -> &ObservationField<bool> {
        &self.read_only_root_filesystem
    }
}

/// A bounded native private or host namespace mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NativeNamespaceMode {
    /// A private namespace.
    Private,
    /// The host namespace.
    Host,
}

/// A bounded native IPC namespace mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NativeIpcNamespaceMode {
    /// A private IPC namespace.
    Private,
    /// The host IPC namespace.
    Host,
    /// A shareable private IPC namespace.
    Shareable,
    /// No IPC namespace.
    None,
}

/// Effective native namespace evidence from `HostConfig`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeNamespaceObservation {
    pid: ObservationField<NativeNamespaceMode>,
    ipc: ObservationField<NativeIpcNamespaceMode>,
    uts: ObservationField<NativeNamespaceMode>,
    cgroup: ObservationField<NativeNamespaceMode>,
}

impl NativeNamespaceObservation {
    pub(crate) const fn new(
        pid: ObservationField<NativeNamespaceMode>,
        ipc: ObservationField<NativeIpcNamespaceMode>,
        uts: ObservationField<NativeNamespaceMode>,
        cgroup: ObservationField<NativeNamespaceMode>,
    ) -> Self {
        Self { pid, ipc, uts, cgroup }
    }

    /// Returns effective PID namespace evidence.
    #[must_use]
    pub fn pid(&self) -> &ObservationField<NativeNamespaceMode> {
        &self.pid
    }

    /// Returns effective IPC namespace evidence.
    #[must_use]
    pub fn ipc(&self) -> &ObservationField<NativeIpcNamespaceMode> {
        &self.ipc
    }

    /// Returns effective UTS namespace evidence.
    #[must_use]
    pub fn uts(&self) -> &ObservationField<NativeNamespaceMode> {
        &self.uts
    }

    /// Returns effective cgroup namespace evidence.
    #[must_use]
    pub fn cgroup(&self) -> &ObservationField<NativeNamespaceMode> {
        &self.cgroup
    }
}

/// One native ulimit observation. Values are retained exactly without output-intent validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeUlimitObservation {
    name: ObservationField<String>,
    soft: ObservationField<i64>,
    hard: ObservationField<i64>,
}

impl NativeUlimitObservation {
    pub(crate) const fn new(
        name: ObservationField<String>,
        soft: ObservationField<i64>,
        hard: ObservationField<i64>,
    ) -> Self {
        Self { name, soft, hard }
    }

    /// Returns the exact native limit name.
    #[must_use]
    pub fn name(&self) -> &ObservationField<String> {
        &self.name
    }

    /// Returns the native soft limit, including zero and -1.
    #[must_use]
    pub fn soft(&self) -> &ObservationField<i64> {
        &self.soft
    }

    /// Returns the native hard limit, including zero and -1.
    #[must_use]
    pub fn hard(&self) -> &ObservationField<i64> {
        &self.hard
    }
}

/// Effective native CPU, memory, PID, and ulimit evidence from `HostConfig`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeResourceControlObservation {
    cpu_shares: ObservationField<u64>,
    cpu_period: ObservationField<u64>,
    cpu_quota: ObservationField<i64>,
    memory: ObservationField<i64>,
    pids_limit: ObservationField<i64>,
    ulimits: ObservationField<Vec<NativeUlimitObservation>>,
}

impl NativeResourceControlObservation {
    pub(crate) const fn new(
        cpu_shares: ObservationField<u64>,
        cpu_period: ObservationField<u64>,
        cpu_quota: ObservationField<i64>,
        memory: ObservationField<i64>,
        pids_limit: ObservationField<i64>,
        ulimits: ObservationField<Vec<NativeUlimitObservation>>,
    ) -> Self {
        Self {
            cpu_shares,
            cpu_period,
            cpu_quota,
            memory,
            pids_limit,
            ulimits,
        }
    }

    /// Returns native CPU shares, preserving zero.
    #[must_use]
    pub fn cpu_shares(&self) -> &ObservationField<u64> {
        &self.cpu_shares
    }

    /// Returns native CPU period, preserving zero.
    #[must_use]
    pub fn cpu_period(&self) -> &ObservationField<u64> {
        &self.cpu_period
    }

    /// Returns native CPU quota, preserving zero and negative native sentinels.
    #[must_use]
    pub fn cpu_quota(&self) -> &ObservationField<i64> {
        &self.cpu_quota
    }

    /// Returns native memory bytes, preserving zero and negative native sentinels.
    #[must_use]
    pub fn memory(&self) -> &ObservationField<i64> {
        &self.memory
    }

    /// Returns native PID limit, preserving zero and -1.
    #[must_use]
    pub fn pids_limit(&self) -> &ObservationField<i64> {
        &self.pids_limit
    }

    /// Returns native ulimits in source order.
    #[must_use]
    pub fn ulimits(&self) -> &ObservationField<Vec<NativeUlimitObservation>> {
        &self.ulimits
    }
}

/// Container-specific native observations.
#[derive(Clone, Eq, PartialEq)]
pub struct ContainerObservation {
    configured_image: ObservationField<String>,
    labels: ObservationField<Labels>,
    local_image_id: ObservationField<String>,
    relationships: ObservationField<Vec<NativeRelationship>>,
    environment: ObservationField<ProtectedEnvironment>,
    command: ObservationField<ConfiguredContainerCommand>,
    entrypoint: ObservationField<ConfiguredContainerEntrypoint>,
    user: ObservationField<ConfiguredContainerUser>,
    working_directory: ObservationField<ConfiguredContainerWorkdir>,
    hostname: ObservationField<ConfiguredContainerHostname>,
    pod_membership: ObservationField<NativeResourceReference>,
    native_dependencies: ObservationField<Vec<NativeResourceReference>>,
    mounts: ObservationField<Vec<ContainerMountObservation>>,
    secret_grants: ObservationField<Vec<ContainerSecretGrantObservation>>,
    memory_swappiness: ObservationField<u64>,
    infra: ObservationField<bool>,
    restart_policy: ObservationField<NativeRestartPolicyObservation>,
    health_check: ObservationField<NativeHealthCheckObservation>,
    health_failure_action: ObservationField<NativeHealthFailureAction>,
    startup_health_check: ObservationField<NativeStartupHealthCheckObservation>,
    logging: ObservationField<NativeLoggingObservation>,
    security: ObservationField<NativeSecurityObservation>,
    namespaces: ObservationField<NativeNamespaceObservation>,
    resource_controls: ObservationField<NativeResourceControlObservation>,
    networking: ObservationField<NativeNetworkingObservation>,
}

macro_rules! observation_debug {
    ($type:ty, $($field:ident),+ $(,)?) => {
        impl fmt::Debug for $type {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut debug = formatter.debug_struct(stringify!($type));
                $(debug.field(stringify!($field), &self.$field);)+
                debug.finish()
            }
        }
    };
}

observation_debug!(
    ContainerObservation,
    labels,
    configured_image,
    local_image_id,
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
    memory_swappiness,
    infra,
    networking,
    restart_policy,
    health_check,
    health_failure_action,
    startup_health_check,
    logging,
    security,
    namespaces,
    resource_controls,
);

impl ContainerObservation {
    #[allow(clippy::too_many_arguments)] // private typed decoder construction keeps every field explicit.
    pub(crate) fn new(
        labels: ObservationField<Labels>,
        configured_image: ObservationField<String>,
        local_image_id: ObservationField<String>,
        relationships: ObservationField<Vec<NativeRelationship>>,
        environment: ObservationField<ProtectedEnvironment>,
        command: ObservationField<ConfiguredContainerCommand>,
        entrypoint: ObservationField<ConfiguredContainerEntrypoint>,
        user: ObservationField<ConfiguredContainerUser>,
        working_directory: ObservationField<ConfiguredContainerWorkdir>,
        hostname: ObservationField<ConfiguredContainerHostname>,
        pod_membership: ObservationField<NativeResourceReference>,
        native_dependencies: ObservationField<Vec<NativeResourceReference>>,
        mounts: ObservationField<Vec<ContainerMountObservation>>,
        secret_grants: ObservationField<Vec<ContainerSecretGrantObservation>>,
        memory_swappiness: ObservationField<u64>,
        infra: ObservationField<bool>,
        restart_policy: ObservationField<NativeRestartPolicyObservation>,
        health_check: ObservationField<NativeHealthCheckObservation>,
        health_failure_action: ObservationField<NativeHealthFailureAction>,
        startup_health_check: ObservationField<NativeStartupHealthCheckObservation>,
        logging: ObservationField<NativeLoggingObservation>,
        security: ObservationField<NativeSecurityObservation>,
        namespaces: ObservationField<NativeNamespaceObservation>,
        resource_controls: ObservationField<NativeResourceControlObservation>,
        networking: ObservationField<NativeNetworkingObservation>,
    ) -> Self {
        Self {
            configured_image,
            labels,
            local_image_id,
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
            memory_swappiness,
            infra,
            restart_policy,
            health_check,
            health_failure_action,
            startup_health_check,
            logging,
            security,
            namespaces,
            resource_controls,
            networking,
        }
    }

    /// Returns the configured container labels or their observation state.
    #[must_use]
    pub fn labels(&self) -> &ObservationField<Labels> {
        &self.labels
    }
    /// Returns the configured image spelling or its observation state.
    ///
    /// This is the only container image observation that discovery may use as a dependency edge.
    #[must_use]
    pub fn configured_image(&self) -> &ObservationField<String> {
        &self.configured_image
    }
    /// Returns the locally resolved image identity or its observation state.
    ///
    /// A local image ID proves what this Podman service used; it is not deployment intent.
    #[must_use]
    pub fn local_image_id(&self) -> &ObservationField<String> {
        &self.local_image_id
    }
    /// Returns protected runtime environment observations or their observation state.
    #[must_use]
    pub fn environment(&self) -> &ObservationField<ProtectedEnvironment> {
        &self.environment
    }
    /// Returns the configured command or its observation state.
    #[must_use]
    pub fn command(&self) -> &ObservationField<ConfiguredContainerCommand> {
        &self.command
    }
    /// Returns the configured entrypoint or its observation state.
    #[must_use]
    pub fn entrypoint(&self) -> &ObservationField<ConfiguredContainerEntrypoint> {
        &self.entrypoint
    }
    /// Returns the configured user or its observation state.
    #[must_use]
    pub fn user(&self) -> &ObservationField<ConfiguredContainerUser> {
        &self.user
    }
    /// Returns the configured working directory or its observation state.
    #[must_use]
    pub fn working_directory(&self) -> &ObservationField<ConfiguredContainerWorkdir> {
        &self.working_directory
    }
    /// Returns the configured hostname or its observation state.
    #[must_use]
    pub fn hostname(&self) -> &ObservationField<ConfiguredContainerHostname> {
        &self.hostname
    }
    /// Returns the container's configured pod-membership evidence or its state.
    #[must_use]
    pub fn pod_membership(&self) -> &ObservationField<NativeResourceReference> {
        &self.pod_membership
    }
    /// Returns declared native container dependencies or their observation state.
    #[must_use]
    pub fn native_dependencies(&self) -> &ObservationField<Vec<NativeResourceReference>> {
        &self.native_dependencies
    }
    /// Returns accepted named-volume and bind mount observations or their state.
    #[must_use]
    pub fn mounts(&self) -> &ObservationField<Vec<ContainerMountObservation>> {
        &self.mounts
    }
    /// Returns typed secret grants without secret payload material or their state.
    #[must_use]
    pub fn secret_grants(&self) -> &ObservationField<Vec<ContainerSecretGrantObservation>> {
        &self.secret_grants
    }
    /// Returns the configured memory-swappiness value or its observation state.
    #[must_use]
    pub fn memory_swappiness(&self) -> &ObservationField<u64> {
        &self.memory_swappiness
    }
    /// Returns effective restart-policy evidence or its observation state.
    #[must_use]
    pub fn restart_policy(&self) -> &ObservationField<NativeRestartPolicyObservation> {
        &self.restart_policy
    }
    /// Returns effective normal-health inspect evidence or its observation state.
    ///
    /// This may include an image default and is not authored deployment intent.
    #[must_use]
    pub fn health_check(&self) -> &ObservationField<NativeHealthCheckObservation> {
        &self.health_check
    }
    /// Returns effective normal-health failure action or its observation state.
    ///
    /// This may include an image default and is not authored deployment intent.
    #[must_use]
    pub fn health_failure_action(&self) -> &ObservationField<NativeHealthFailureAction> {
        &self.health_failure_action
    }
    /// Returns effective startup-health inspect evidence or its observation state.
    ///
    /// This may include an image default and is not authored deployment intent.
    #[must_use]
    pub fn startup_health_check(&self) -> &ObservationField<NativeStartupHealthCheckObservation> {
        &self.startup_health_check
    }
    /// Returns effective logging evidence or its observation state.
    #[must_use]
    pub fn logging(&self) -> &ObservationField<NativeLoggingObservation> {
        &self.logging
    }
    /// Returns effective security evidence or its observation state.
    #[must_use]
    pub fn security(&self) -> &ObservationField<NativeSecurityObservation> {
        &self.security
    }
    /// Returns effective namespace evidence for any inspected container, including pod members.
    #[must_use]
    pub fn namespaces(&self) -> &ObservationField<NativeNamespaceObservation> {
        &self.namespaces
    }
    /// Returns effective resource-control evidence or its observation state.
    #[must_use]
    pub fn resource_controls(&self) -> &ObservationField<NativeResourceControlObservation> {
        &self.resource_controls
    }
    /// Returns the infra-container marker or its observation state.
    #[must_use]
    pub fn infra(&self) -> &ObservationField<bool> {
        &self.infra
    }
    /// Returns bounded configured networking evidence for an unpodded container.
    ///
    /// Pod-member networking is topology-owned by its pod and is never promoted here.
    #[must_use]
    pub fn networking(&self) -> &ObservationField<NativeNetworkingObservation> {
        &self.networking
    }
    pub(crate) fn relationships(&self) -> &ObservationField<Vec<NativeRelationship>> {
        &self.relationships
    }
}

/// Pod-specific native observations.
#[derive(Clone, Eq, PartialEq)]
pub struct PodObservation {
    labels: ObservationField<Labels>,
    relationships: ObservationField<Vec<NativeRelationship>>,
    create_infra: ObservationField<bool>,
    networking: ObservationField<NativeNetworkingObservation>,
}
observation_debug!(PodObservation, labels, relationships, create_infra, networking);

impl PodObservation {
    pub(crate) fn new(
        labels: ObservationField<Labels>,
        relationships: ObservationField<Vec<NativeRelationship>>,
        create_infra: ObservationField<bool>,
        networking: ObservationField<NativeNetworkingObservation>,
    ) -> Self {
        Self {
            labels,
            relationships,
            create_infra,
            networking,
        }
    }
    /// Returns the configured pod labels or their observation state.
    #[must_use]
    pub fn labels(&self) -> &ObservationField<Labels> {
        &self.labels
    }
    /// Returns whether the pod was created with an infra container.
    #[must_use]
    pub fn create_infra(&self) -> &ObservationField<bool> {
        &self.create_infra
    }
    /// Returns networking observed only from `Pod.InspectPodData.InfraConfig`.
    #[must_use]
    pub fn networking(&self) -> &ObservationField<NativeNetworkingObservation> {
        &self.networking
    }
    pub(crate) fn relationships(&self) -> &ObservationField<Vec<NativeRelationship>> {
        &self.relationships
    }
}

/// A protocol carried by a native inspected port binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NativePortProtocol {
    /// TCP.
    Tcp,
    /// UDP.
    Udp,
    /// SCTP.
    Sctp,
}

/// One bounded native port-binding observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativePortBindingObservation {
    container_port: u16,
    protocol: NativePortProtocol,
    host_ip: ObservationField<IpAddr>,
    host_port: ObservationField<u16>,
}

impl NativePortBindingObservation {
    pub(crate) const fn new(
        container_port: u16,
        protocol: NativePortProtocol,
        host_ip: ObservationField<IpAddr>,
        host_port: ObservationField<u16>,
    ) -> Self {
        Self {
            container_port,
            protocol,
            host_ip,
            host_port,
        }
    }
    /// Returns the container port from the binding key.
    #[must_use]
    pub const fn container_port(&self) -> u16 {
        self.container_port
    }
    /// Returns the native transport protocol.
    #[must_use]
    pub const fn protocol(&self) -> NativePortProtocol {
        self.protocol
    }
    /// Returns the optional host IP or its observation state.
    #[must_use]
    pub fn host_ip(&self) -> &ObservationField<IpAddr> {
        &self.host_ip
    }
    /// Returns the optional host port or its observation state.
    #[must_use]
    pub fn host_port(&self) -> &ObservationField<u16> {
        &self.host_port
    }
}

/// Bounded, opaque native network options. Option values are deliberately never exposed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeOpaqueNetworkOptions {
    count: usize,
}

impl NativeOpaqueNetworkOptions {
    pub(crate) const fn new(count: usize) -> Self {
        Self { count }
    }
    /// Returns the number of opaque options without exposing keys or values.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }
    /// Returns whether no opaque options were observed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Native networking evidence observed from authoritative pod infra or unpodded host config.
///
/// This is deliberately separate from declared deployment networking intent. Host entries and
/// free-form network options remain non-promotable bounded metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeNetworkingObservation {
    port_bindings: ObservationField<Vec<NativePortBindingObservation>>,
    create_net_ns: ObservationField<bool>,
    host_network: ObservationField<bool>,
    dns_servers: ObservationField<Vec<IpAddr>>,
    dns_search: ObservationField<Vec<String>>,
    dns_options: ObservationField<Vec<String>>,
    host_entries: ObservationField<NativeOpaqueNetworkOptions>,
    networks: ObservationField<Vec<NativeResourceReference>>,
    network_options: ObservationField<NativeOpaqueNetworkOptions>,
    no_manage_resolv_conf: ObservationField<bool>,
    no_manage_hosts: ObservationField<bool>,
    static_ip: ObservationField<IpAddr>,
    static_mac: ObservationField<String>,
}

impl NativeNetworkingObservation {
    #[allow(clippy::too_many_arguments)] // private decoder construction keeps every source field explicit.
    pub(crate) fn new(
        port_bindings: ObservationField<Vec<NativePortBindingObservation>>,
        create_net_ns: ObservationField<bool>,
        host_network: ObservationField<bool>,
        dns_servers: ObservationField<Vec<IpAddr>>,
        dns_search: ObservationField<Vec<String>>,
        dns_options: ObservationField<Vec<String>>,
        host_entries: ObservationField<NativeOpaqueNetworkOptions>,
        networks: ObservationField<Vec<NativeResourceReference>>,
        network_options: ObservationField<NativeOpaqueNetworkOptions>,
        no_manage_resolv_conf: ObservationField<bool>,
        no_manage_hosts: ObservationField<bool>,
        static_ip: ObservationField<IpAddr>,
        static_mac: ObservationField<String>,
    ) -> Self {
        Self {
            port_bindings,
            create_net_ns,
            host_network,
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
        }
    }
    /// Returns bounded port-binding evidence.
    #[must_use]
    pub fn port_bindings(&self) -> &ObservationField<Vec<NativePortBindingObservation>> {
        &self.port_bindings
    }
    /// Returns the configured container network-namespace creation gate.
    #[must_use]
    pub fn create_net_ns(&self) -> &ObservationField<bool> {
        &self.create_net_ns
    }
    /// Returns the effective host-network gate.
    #[must_use]
    pub fn host_network(&self) -> &ObservationField<bool> {
        &self.host_network
    }
    /// Returns copied/configured DNS server evidence.
    #[must_use]
    pub fn dns_servers(&self) -> &ObservationField<Vec<IpAddr>> {
        &self.dns_servers
    }
    /// Returns copied/configured DNS search evidence.
    #[must_use]
    pub fn dns_search(&self) -> &ObservationField<Vec<String>> {
        &self.dns_search
    }
    /// Returns copied/configured DNS option evidence.
    #[must_use]
    pub fn dns_options(&self) -> &ObservationField<Vec<String>> {
        &self.dns_options
    }
    /// Returns the state of `/etc/hosts` entry data.
    ///
    /// `PodmanLens` intentionally does not parse this free-form hosts-file syntax as aliases or
    /// expose its values. A present entry list is therefore `Unmodelled`.
    #[must_use]
    pub fn host_entries(&self) -> &ObservationField<NativeOpaqueNetworkOptions> {
        &self.host_entries
    }
    /// Returns effective native network names in native order; that order is not a contract.
    #[must_use]
    pub fn networks(&self) -> &ObservationField<Vec<NativeResourceReference>> {
        &self.networks
    }
    /// Returns opaque network-option evidence without key/value semantics.
    #[must_use]
    pub fn network_options(&self) -> &ObservationField<NativeOpaqueNetworkOptions> {
        &self.network_options
    }
    /// Returns the effective resolver-management gate.
    #[must_use]
    pub fn no_manage_resolv_conf(&self) -> &ObservationField<bool> {
        &self.no_manage_resolv_conf
    }
    /// Returns the effective hosts-file-management gate.
    #[must_use]
    pub fn no_manage_hosts(&self) -> &ObservationField<bool> {
        &self.no_manage_hosts
    }
    /// Returns static IP evidence only where the inspected field has reviewed meaning.
    #[must_use]
    pub fn static_ip(&self) -> &ObservationField<IpAddr> {
        &self.static_ip
    }
    /// Returns static MAC evidence only where the inspected field has reviewed meaning.
    #[must_use]
    pub fn static_mac(&self) -> &ObservationField<String> {
        &self.static_mac
    }
}

/// Network-specific native observations.
#[derive(Clone, Eq, PartialEq)]
pub struct NetworkObservation {
    labels: ObservationField<Labels>,
    internal: ObservationField<bool>,
    options: ObservationField<NetworkOptionKeys>,
    subnets: ObservationField<Vec<NativeNetworkSubnetObservation>>,
    routes: ObservationField<Vec<NativeNetworkRouteObservation>>,
}

impl NetworkObservation {
    pub(crate) fn new(
        labels: ObservationField<Labels>,
        internal: ObservationField<bool>,
        options: ObservationField<NetworkOptionKeys>,
        subnets: ObservationField<Vec<NativeNetworkSubnetObservation>>,
        routes: ObservationField<Vec<NativeNetworkRouteObservation>>,
    ) -> Self {
        Self {
            labels,
            internal,
            options,
            subnets,
            routes,
        }
    }
    /// Returns the configured network labels or their observation state.
    #[must_use]
    pub fn labels(&self) -> &ObservationField<Labels> {
        &self.labels
    }
    /// Returns the network-internal flag or its observation state.
    #[must_use]
    pub fn internal(&self) -> &ObservationField<bool> {
        &self.internal
    }
    /// Returns only network option keys; native option values may contain credentials and are
    /// never exposed through the public observation contract.
    #[must_use]
    pub fn options(&self) -> &ObservationField<NetworkOptionKeys> {
        &self.options
    }
    /// Returns typed effective native IPAM subnet observations or their observation state.
    #[must_use]
    pub fn subnets(&self) -> &ObservationField<Vec<NativeNetworkSubnetObservation>> {
        &self.subnets
    }
    /// Returns typed effective native static-route observations or their observation state.
    #[must_use]
    pub fn routes(&self) -> &ObservationField<Vec<NativeNetworkRouteObservation>> {
        &self.routes
    }
}
observation_debug!(NetworkObservation, labels, internal, options, subnets, routes);

/// A syntax-validated native CIDR wire spelling observed from network inspection.
///
/// This is defensive raw-wire preservation, not [`crate::NetworkCidr`] deployment intent or a
/// claim that every accepted spelling is valid for every native field and Podman version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeNetworkCidr {
    spelling: String,
    network: IpAddr,
    prefix: u8,
}

impl NativeNetworkCidr {
    pub(crate) fn parse(spelling: String) -> Option<Self> {
        let (network, prefix) = spelling.split_once('/')?;
        let network = network.parse::<IpAddr>().ok()?;
        let prefix = prefix.parse::<u8>().ok()?;
        (prefix <= if network.is_ipv4() { 32 } else { 128 }).then_some(Self {
            spelling,
            network,
            prefix,
        })
    }

    /// Returns the exact syntax-validated native CIDR wire spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    /// Returns whether an address of the same family lies within this CIDR.
    #[must_use]
    pub(crate) fn contains(&self, address: IpAddr) -> bool {
        self.network.is_ipv4() == address.is_ipv4()
            && native_masked_address(self.network, self.prefix) == native_masked_address(address, self.prefix)
    }

    /// Returns whether an address has the same family as this CIDR.
    #[must_use]
    pub(crate) const fn has_address_family(&self, address: IpAddr) -> bool {
        self.network.is_ipv4() == address.is_ipv4()
    }
}

/// An effective native network lease range with independently optional endpoint evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeNetworkLeaseRange {
    start_ip: ObservationField<IpAddr>,
    end_ip: ObservationField<IpAddr>,
}

impl NativeNetworkLeaseRange {
    pub(crate) const fn new(start_ip: ObservationField<IpAddr>, end_ip: ObservationField<IpAddr>) -> Self {
        Self { start_ip, end_ip }
    }
    /// Returns the optional inclusive lease-range start address or its observation state.
    #[must_use]
    pub const fn start_ip(&self) -> &ObservationField<IpAddr> {
        &self.start_ip
    }
    /// Returns the optional inclusive lease-range end address or its observation state.
    #[must_use]
    pub const fn end_ip(&self) -> &ObservationField<IpAddr> {
        &self.end_ip
    }
}

/// One typed native IPAM subnet observation. Every nested member keeps its own observation state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeNetworkSubnetObservation {
    cidr: ObservationField<NativeNetworkCidr>,
    gateway: ObservationField<IpAddr>,
    lease_range: ObservationField<NativeNetworkLeaseRange>,
}

impl NativeNetworkSubnetObservation {
    pub(crate) const fn new(
        cidr: ObservationField<NativeNetworkCidr>,
        gateway: ObservationField<IpAddr>,
        lease_range: ObservationField<NativeNetworkLeaseRange>,
    ) -> Self {
        Self {
            cidr,
            gateway,
            lease_range,
        }
    }
    /// Returns the native subnet CIDR evidence.
    #[must_use]
    pub fn cidr(&self) -> &ObservationField<NativeNetworkCidr> {
        &self.cidr
    }
    /// Returns the optional effective native gateway.
    #[must_use]
    pub fn gateway(&self) -> &ObservationField<IpAddr> {
        &self.gateway
    }
    /// Returns the optional effective native lease range.
    #[must_use]
    pub fn lease_range(&self) -> &ObservationField<NativeNetworkLeaseRange> {
        &self.lease_range
    }
}

/// A route kind observed from the native network inspect response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NativeNetworkRouteType {
    /// A forwarding route that requires a gateway.
    Unicast,
    /// A route that drops matching traffic.
    Blackhole,
    /// A route that reports the destination as unreachable.
    Unreachable,
    /// A route that reports the destination as administratively prohibited.
    Prohibit,
}

/// One typed native static-route observation. Every nested member keeps its own observation state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeNetworkRouteObservation {
    destination: ObservationField<NativeNetworkCidr>,
    gateway: ObservationField<IpAddr>,
    metric: ObservationField<u32>,
    route_type: ObservationField<NativeNetworkRouteType>,
}

impl NativeNetworkRouteObservation {
    pub(crate) const fn new(
        destination: ObservationField<NativeNetworkCidr>,
        gateway: ObservationField<IpAddr>,
        metric: ObservationField<u32>,
        route_type: ObservationField<NativeNetworkRouteType>,
    ) -> Self {
        Self {
            destination,
            gateway,
            metric,
            route_type,
        }
    }
    /// Returns the native destination CIDR evidence.
    #[must_use]
    pub fn destination(&self) -> &ObservationField<NativeNetworkCidr> {
        &self.destination
    }
    /// Returns the optional effective native route gateway.
    #[must_use]
    pub fn gateway(&self) -> &ObservationField<IpAddr> {
        &self.gateway
    }
    /// Returns the optional effective native route metric, preserving an explicit zero.
    #[must_use]
    pub fn metric(&self) -> &ObservationField<u32> {
        &self.metric
    }
    /// Returns the native route type. This is version-inapplicable before Podman 6.0.
    #[must_use]
    pub fn route_type(&self) -> &ObservationField<NativeNetworkRouteType> {
        &self.route_type
    }
}

fn native_masked_address(address: IpAddr, prefix: u8) -> IpAddr {
    match address {
        IpAddr::V4(address) => {
            let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
            IpAddr::V4(std::net::Ipv4Addr::from(u32::from(address) & mask))
        }
        IpAddr::V6(address) => {
            let mask = if prefix == 0 { 0 } else { u128::MAX << (128 - prefix) };
            IpAddr::V6(std::net::Ipv6Addr::from(u128::from(address) & mask))
        }
    }
}

/// Public, value-free network option observation.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct NetworkOptionKeys(BTreeSet<String>);

impl NetworkOptionKeys {
    pub(crate) fn new(keys: impl IntoIterator<Item = String>) -> Self {
        Self(keys.into_iter().collect())
    }

    /// Returns observed option keys in deterministic order, without their values.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }

    /// Returns the number of observed option keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether no option keys were observed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for NetworkOptionKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NetworkOptionKeys")
            .field("count", &self.len())
            .finish()
    }
}

/// The native wire representation of a volume owner ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VolumeOwnerIdWireValue {
    /// Podman's reviewed `omitempty` wire shape omitted the property, which canonically may mean
    /// the Podman default of zero.
    WireAbsentMayMeanZero,
    /// A concrete numeric value was present, including literal zero.
    Explicit(UnixId),
}

/// Bounded Unix user or group identifier from a native volume response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnixId(u32);

impl UnixId {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }
    /// Returns the literal value reported by Podman.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// An exact, validated RFC 3339 timestamp from a native Podman response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeTimestamp(String);

impl NativeTimestamp {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    /// Returns the exact timestamp spelling reported by Podman.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Count-only evidence for secret-driver options. Option names and values are never retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeSecretDriverOptions {
    count: usize,
}

impl NativeSecretDriverOptions {
    pub(crate) const fn new(count: usize) -> Self {
        Self { count }
    }

    /// Returns the number of opaque driver options.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Returns whether no driver options were observed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Native secret-driver metadata without option names or values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeSecretDriverObservation {
    name: ObservationField<String>,
    options: ObservationField<NativeSecretDriverOptions>,
}

impl NativeSecretDriverObservation {
    pub(crate) const fn new(
        name: ObservationField<String>,
        options: ObservationField<NativeSecretDriverOptions>,
    ) -> Self {
        Self { name, options }
    }

    /// Returns the effective driver name.
    #[must_use]
    pub fn name(&self) -> &ObservationField<String> {
        &self.name
    }

    /// Returns only the state and count of opaque driver options.
    #[must_use]
    pub fn options(&self) -> &ObservationField<NativeSecretDriverOptions> {
        &self.options
    }
}

/// Volume-specific native observations.
#[derive(Clone, Eq, PartialEq)]
pub struct VolumeObservation {
    labels: ObservationField<Labels>,
    uid: ObservationField<VolumeOwnerIdWireValue>,
    gid: ObservationField<VolumeOwnerIdWireValue>,
    driver: ObservationField<String>,
    created_at: ObservationField<NativeTimestamp>,
    anonymous: ObservationField<bool>,
}
observation_debug!(VolumeObservation, labels, uid, gid, driver, created_at, anonymous);

impl VolumeObservation {
    pub(crate) fn new(
        labels: ObservationField<Labels>,
        uid: ObservationField<VolumeOwnerIdWireValue>,
        gid: ObservationField<VolumeOwnerIdWireValue>,
        driver: ObservationField<String>,
        created_at: ObservationField<NativeTimestamp>,
        anonymous: ObservationField<bool>,
    ) -> Self {
        Self {
            labels,
            uid,
            gid,
            driver,
            created_at,
            anonymous,
        }
    }
    /// Returns the configured volume labels or their observation state.
    #[must_use]
    pub fn labels(&self) -> &ObservationField<Labels> {
        &self.labels
    }
    /// Returns the wire-level volume UID observation or its observation state.
    #[must_use]
    pub fn uid(&self) -> &ObservationField<VolumeOwnerIdWireValue> {
        &self.uid
    }
    /// Returns the wire-level volume GID observation or its observation state.
    #[must_use]
    pub fn gid(&self) -> &ObservationField<VolumeOwnerIdWireValue> {
        &self.gid
    }
    /// Returns the effective volume driver name.
    #[must_use]
    pub fn driver(&self) -> &ObservationField<String> {
        &self.driver
    }
    /// Returns the effective creation timestamp.
    #[must_use]
    pub fn created_at(&self) -> &ObservationField<NativeTimestamp> {
        &self.created_at
    }
    /// Returns whether Podman reported this as an anonymous volume.
    #[must_use]
    pub fn anonymous(&self) -> &ObservationField<bool> {
        &self.anonymous
    }
}

/// Image-specific native observations.
#[derive(Clone, Eq, PartialEq)]
pub struct ImageObservation {
    labels: ObservationField<Labels>,
    repo_tags: ObservationField<Vec<String>>,
    repo_digests: ObservationField<Vec<String>>,
    environment: ObservationField<ProtectedEnvironment>,
    digest: ObservationField<String>,
    created: ObservationField<NativeTimestamp>,
    author: ObservationField<String>,
    architecture: ObservationField<String>,
    operating_system: ObservationField<String>,
    manifest_type: ObservationField<String>,
}

pub(crate) struct ImageObservationFields {
    pub(crate) labels: ObservationField<Labels>,
    pub(crate) repo_tags: ObservationField<Vec<String>>,
    pub(crate) repo_digests: ObservationField<Vec<String>>,
    pub(crate) environment: ObservationField<ProtectedEnvironment>,
    pub(crate) digest: ObservationField<String>,
    pub(crate) created: ObservationField<NativeTimestamp>,
    pub(crate) author: ObservationField<String>,
    pub(crate) architecture: ObservationField<String>,
    pub(crate) operating_system: ObservationField<String>,
    pub(crate) manifest_type: ObservationField<String>,
}
observation_debug!(
    ImageObservation,
    labels,
    repo_tags,
    repo_digests,
    environment,
    digest,
    created,
    author,
    architecture,
    operating_system,
    manifest_type
);

impl ImageObservation {
    pub(crate) fn new(fields: ImageObservationFields) -> Self {
        let ImageObservationFields {
            labels,
            repo_tags,
            repo_digests,
            environment,
            digest,
            created,
            author,
            architecture,
            operating_system,
            manifest_type,
        } = fields;
        Self {
            labels,
            repo_tags,
            repo_digests,
            environment,
            digest,
            created,
            author,
            architecture,
            operating_system,
            manifest_type,
        }
    }
    /// Returns the configured image labels or their observation state.
    #[must_use]
    pub fn labels(&self) -> &ObservationField<Labels> {
        &self.labels
    }
    /// Returns locally resolved repository tags or their observation state.
    #[must_use]
    pub fn repo_tags(&self) -> &ObservationField<Vec<String>> {
        &self.repo_tags
    }
    /// Returns locally resolved repository digests or their observation state.
    #[must_use]
    pub fn repo_digests(&self) -> &ObservationField<Vec<String>> {
        &self.repo_digests
    }
    /// Returns protected image-environment observations or their observation state.
    #[must_use]
    pub fn environment(&self) -> &ObservationField<ProtectedEnvironment> {
        &self.environment
    }
    /// Returns the effective image digest.
    #[must_use]
    pub fn digest(&self) -> &ObservationField<String> {
        &self.digest
    }
    /// Returns the effective image creation timestamp.
    #[must_use]
    pub fn created(&self) -> &ObservationField<NativeTimestamp> {
        &self.created
    }
    /// Returns the configured image author metadata.
    #[must_use]
    pub fn author(&self) -> &ObservationField<String> {
        &self.author
    }
    /// Returns the effective image architecture.
    #[must_use]
    pub fn architecture(&self) -> &ObservationField<String> {
        &self.architecture
    }
    /// Returns the effective image operating-system name.
    #[must_use]
    pub fn operating_system(&self) -> &ObservationField<String> {
        &self.operating_system
    }
    /// Returns the effective image manifest media type.
    #[must_use]
    pub fn manifest_type(&self) -> &ObservationField<String> {
        &self.manifest_type
    }
}

/// Secret metadata observations.  Secret payload bytes are never represented.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretObservation {
    labels: ObservationField<Labels>,
    driver: ObservationField<NativeSecretDriverObservation>,
    created_at: ObservationField<NativeTimestamp>,
    updated_at: ObservationField<NativeTimestamp>,
}
observation_debug!(SecretObservation, labels, driver, created_at, updated_at);

impl SecretObservation {
    pub(crate) fn new(
        labels: ObservationField<Labels>,
        driver: ObservationField<NativeSecretDriverObservation>,
        created_at: ObservationField<NativeTimestamp>,
        updated_at: ObservationField<NativeTimestamp>,
    ) -> Self {
        Self {
            labels,
            driver,
            created_at,
            updated_at,
        }
    }
    /// Returns the configured secret labels or their observation state.
    #[must_use]
    pub fn labels(&self) -> &ObservationField<Labels> {
        &self.labels
    }
    /// Returns the secret-driver metadata or its observation state.
    #[must_use]
    pub fn driver(&self) -> &ObservationField<NativeSecretDriverObservation> {
        &self.driver
    }
    /// Returns the effective creation timestamp.
    #[must_use]
    pub fn created_at(&self) -> &ObservationField<NativeTimestamp> {
        &self.created_at
    }
    /// Returns the effective last-update timestamp.
    #[must_use]
    pub fn updated_at(&self) -> &ObservationField<NativeTimestamp> {
        &self.updated_at
    }
}

/// Resource-kind-specific observation payload.
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)] // kind-safe public enum avoids heap allocation at every observation access.
pub enum ResourceDetails {
    /// Container-only fields.
    Container(ContainerObservation),
    /// Pod-only fields.
    Pod(PodObservation),
    /// Network-only fields.
    Network(NetworkObservation),
    /// Volume-only fields.
    Volume(VolumeObservation),
    /// Image-only fields.
    Image(ImageObservation),
    /// Secret metadata-only fields.
    Secret(SecretObservation),
}

impl fmt::Debug for ResourceDetails {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(value) => formatter
                .debug_tuple("ResourceDetails::Container")
                .field(value)
                .finish(),
            Self::Pod(value) => formatter.debug_tuple("ResourceDetails::Pod").field(value).finish(),
            Self::Network(value) => formatter.debug_tuple("ResourceDetails::Network").field(value).finish(),
            Self::Volume(value) => formatter.debug_tuple("ResourceDetails::Volume").field(value).finish(),
            Self::Image(value) => formatter.debug_tuple("ResourceDetails::Image").field(value).finish(),
            Self::Secret(value) => formatter.debug_tuple("ResourceDetails::Secret").field(value).finish(),
        }
    }
}

impl ResourceDetails {
    /// Returns the exact resource kind carried by this variant.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        match self {
            Self::Container(_) => ResourceKind::Container,
            Self::Pod(_) => ResourceKind::Pod,
            Self::Network(_) => ResourceKind::Network,
            Self::Volume(_) => ResourceKind::Volume,
            Self::Image(_) => ResourceKind::Image,
            Self::Secret(_) => ResourceKind::Secret,
        }
    }
}

/// One complete or partial typed native resource observation.
#[derive(Clone, Eq, PartialEq)]
pub struct ResourceObservation {
    header: ObservationHeader,
    details: ResourceDetails,
}

impl ResourceObservation {
    pub(crate) fn try_new(header: ObservationHeader, details: ResourceDetails) -> Result<Self, Diagnostic> {
        if header.identity().kind() != details.kind() {
            return Err(Diagnostic::new(DiagnosticCode::ResourceMalformed));
        }
        Ok(Self { header, details })
    }

    pub(crate) fn incomplete(header: ObservationHeader) -> Self {
        let details = incomplete_details(header.identity().kind(), header.state());
        Self { header, details }
    }

    /// Returns resource-wide identity, evidence, findings, and completeness information.
    #[must_use]
    pub fn header(&self) -> &ObservationHeader {
        &self.header
    }
    /// Returns a kind-safe resource-specific payload.
    #[must_use]
    pub fn details(&self) -> &ResourceDetails {
        &self.details
    }

    pub(crate) fn header_mut(&mut self) -> &mut ObservationHeader {
        &mut self.header
    }

    pub(crate) fn relationships(&self) -> Option<&ObservationField<Vec<NativeRelationship>>> {
        match &self.details {
            ResourceDetails::Container(value) => Some(value.relationships()),
            ResourceDetails::Pod(value) => Some(value.relationships()),
            _ => None,
        }
    }

    pub(crate) fn labels(&self) -> &ObservationField<Labels> {
        match &self.details {
            ResourceDetails::Container(value) => value.labels(),
            ResourceDetails::Pod(value) => value.labels(),
            ResourceDetails::Network(value) => value.labels(),
            ResourceDetails::Volume(value) => value.labels(),
            ResourceDetails::Image(value) => value.labels(),
            ResourceDetails::Secret(value) => value.labels(),
        }
    }

    pub(crate) fn image_repo_tags(&self) -> Option<&ObservationField<Vec<String>>> {
        match &self.details {
            ResourceDetails::Image(value) => Some(value.repo_tags()),
            _ => None,
        }
    }

    pub(crate) fn image_repo_digests(&self) -> Option<&ObservationField<Vec<String>>> {
        match &self.details {
            ResourceDetails::Image(value) => Some(value.repo_digests()),
            _ => None,
        }
    }
}

fn incomplete_field<T>(state: ResourceObservationState) -> ObservationField<T> {
    if state == ResourceObservationState::Malformed {
        ObservationField::Malformed
    } else {
        ObservationField::Unavailable
    }
}

fn incomplete_details(kind: ResourceKind, state: ResourceObservationState) -> ResourceDetails {
    match kind {
        ResourceKind::Container => ResourceDetails::Container(ContainerObservation::new(
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
        )),
        ResourceKind::Pod => ResourceDetails::Pod(PodObservation::new(
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
        )),
        ResourceKind::Network => ResourceDetails::Network(NetworkObservation::new(
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
        )),
        ResourceKind::Volume => ResourceDetails::Volume(VolumeObservation::new(
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
        )),
        ResourceKind::Image => ResourceDetails::Image(ImageObservation::new(ImageObservationFields {
            labels: incomplete_field(state),
            repo_tags: incomplete_field(state),
            repo_digests: incomplete_field(state),
            environment: incomplete_field(state),
            digest: incomplete_field(state),
            created: incomplete_field(state),
            author: incomplete_field(state),
            architecture: incomplete_field(state),
            operating_system: incomplete_field(state),
            manifest_type: incomplete_field(state),
        })),
        ResourceKind::Secret => ResourceDetails::Secret(SecretObservation::new(
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
            incomplete_field(state),
        )),
    }
}

impl fmt::Debug for ResourceObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceObservation")
            .field("identity", self.header.identity())
            .field("state", &self.header.state())
            .field("finding_count", &self.header.findings().len())
            .field("unmodelled_field_count", &self.header.unmodelled_fields().len())
            .field("detail_kind", &self.details.kind())
            .finish()
    }
}
