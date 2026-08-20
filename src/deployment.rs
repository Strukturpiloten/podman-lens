//! Typed, transport-neutral Podman deployment intent and deterministic semantic planning.
//!
//! This module deliberately stops before command-line and Libpod HTTP rendering.  A plan says
//! *what* must be prepared, created, and started, and why one operation precedes another. M6
//! renders those semantics into version-specific transport representations.

use std::collections::{BTreeMap, BTreeSet};

use crate::networking::{
    DnsConfiguration, HostAlias, NetworkAttachment, NetworkRoute, NetworkSubnet, PortMapping, add_attachment, add_host,
    add_port, add_route, add_subnet,
};
use crate::settings::{ContainerSettings, MountIntent, SecretGrant, UnixId};
use crate::{
    CgroupController, ContainerRuntimeSettings, Diagnostic, DiagnosticCode, PodmanLensResult, ResourceKind,
    TargetExecutionContext, TargetProfile,
};

const MAX_REFERENCE_BYTES: usize = 256;
const MAX_CONNECTION_NAME_BYTES: usize = 64;

/// Stable target-side identity for one explicitly declared Podman resource.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DeploymentResourceId {
    kind: ResourceKind,
    name: String,
}

impl DeploymentResourceId {
    /// Creates a resource identity from an exact, non-empty target-side name.
    ///
    /// Names are retained as native spelling. Control characters and oversized values are
    /// rejected so they cannot become ambiguous operation identifiers or rendered arguments.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when `name` is empty, contains a control character, or is too large.
    pub fn new(kind: ResourceKind, name: impl Into<String>) -> PodmanLensResult<Self> {
        let name = name.into();
        validate_identifier(&name)?;
        Ok(Self { kind, name })
    }

    /// Returns the native resource kind.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Returns the exact target-side resource name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A non-sensitive, caller-owned Podman connection name for a deployment plan.
///
/// It is metadata only. Planning does not discover, open, or validate a connection. A connection
/// name is deliberately narrower than a connection URI: it is 1–64 ASCII bytes, starts with an
/// ASCII alphanumeric character, and otherwise contains only ASCII alphanumeric characters,
/// dots, underscores, or hyphens. Endpoint, path, credential, and token-like spellings therefore
/// cannot enter a rendered artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentConnectionReference(String);

impl DeploymentConnectionReference {
    /// Creates a non-sensitive Podman connection name.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when the name is empty, oversized, or outside the safe connection-name
    /// grammar.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        validate_connection_name(&value)?;
        Ok(Self(value))
    }

    /// Returns the caller-selected connection reference.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An external reference to secret material that is intentionally absent from a deployment plan.
#[derive(Clone, Eq, PartialEq)]
pub struct SensitiveInputReference(String);

impl SensitiveInputReference {
    /// Creates a reference to externally managed sensitive material.
    ///
    /// Literal and encoded payload prefixes are rejected. Encoding is not protection, and a
    /// deployment plan must never embed secret bytes.
    ///
    /// # Errors
    ///
    /// Returns `PLN0040` for a likely embedded payload and `PLN0034` for an invalid reference.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if ["literal:", "plaintext:", "base64:"].iter().any(|prefix| {
            value
                .get(..prefix.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
        }) {
            return Err(Diagnostic::new(DiagnosticCode::SensitivePayloadEmbedded));
        }
        validate_identifier(&value)?;
        Ok(Self(value))
    }

    /// Returns the external reference, never secret material itself.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SensitiveInputReference {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SensitiveInputReference([redacted])")
    }
}

/// One fully resolved image acquisition intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageIntent {
    identity: DeploymentResourceId,
    source: ImageSource,
    pull_policy: ImagePullPolicy,
}

/// The explicit image-acquisition policy for a managed image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImagePullPolicy {
    /// Always request a fresh image from the declared source.
    Always,
    /// Acquire the image only when it is not already available at the target.
    Missing,
    /// Never pull; require the exact image to have been made available separately.
    Never,
    /// Request a newer image when the target can compare one.
    Newer,
}

impl ImagePullPolicy {
    /// Returns Podman's exact lower-case policy spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Missing => "missing",
            Self::Never => "never",
            Self::Newer => "newer",
        }
    }
}

/// The portable classification of an image source without changing its spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImageSourceClassification {
    /// A registry-qualified source with an explicit tag or sha256 digest.
    Portable,
    /// An explicitly local source such as `localhost/example:1` or a content ID.
    Local,
    /// A repository without an explicit registry hostname.
    Unqualified,
    /// A registry-qualified repository without an explicit tag or digest.
    Tagless,
}

/// A validated source image spelling retained exactly as provided by the caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageSource {
    spelling: String,
    classification: ImageSourceClassification,
}

impl ImageSource {
    /// Validates and classifies an image source without rewriting it.
    ///
    /// # Errors
    ///
    /// Returns `PLN0041` for empty, control-containing, credential-bearing, or malformed
    /// sources.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let spelling = value.into();
        let classification =
            classify_image_source(&spelling).ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidImageReference))?;
        Ok(Self {
            spelling,
            classification,
        })
    }

    /// Returns the exact unmodified source spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    /// Returns the portability classification without altering this source.
    #[must_use]
    pub const fn classification(&self) -> ImageSourceClassification {
        self.classification
    }
}

impl ImageIntent {
    /// Declares an image that must be available before dependent containers are created.
    ///
    /// `source` is an image reference, not a shell fragment or secret-bearing credential.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an image identity mismatch and `PLN0041` for an invalid source spelling.
    ///
    /// The pull policy is required explicitly. There is no hidden default because image
    /// acquisition changes target state.
    pub fn new(
        identity: DeploymentResourceId,
        source: ImageSource,
        pull_policy: ImagePullPolicy,
    ) -> PodmanLensResult<Self> {
        require_kind(&identity, ResourceKind::Image)?;
        Ok(Self {
            identity,
            source,
            pull_policy,
        })
    }

    /// Returns the image identity.
    #[must_use]
    pub fn identity(&self) -> &DeploymentResourceId {
        &self.identity
    }

    /// Returns the exact image source reference.
    #[must_use]
    pub fn source(&self) -> &ImageSource {
        &self.source
    }

    /// Returns the explicit image-acquisition policy.
    #[must_use]
    pub const fn pull_policy(&self) -> ImagePullPolicy {
        self.pull_policy
    }
}

/// One typed network creation intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkIntent {
    identity: DeploymentResourceId,
    subnets: Vec<NetworkSubnet>,
    routes: Vec<NetworkRoute>,
}

impl NetworkIntent {
    /// Creates one network creation intent with no caller-declared IPAM configuration.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when the identity is not a network identity.
    pub fn new(identity: DeploymentResourceId) -> PodmanLensResult<Self> {
        require_kind(&identity, ResourceKind::Network)?;
        Ok(Self {
            identity,
            subnets: Vec::new(),
            routes: Vec::new(),
        })
    }

    /// Adds one subnet/IPAM declaration in declared order.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for duplicate subnets or capacity exhaustion.
    pub fn add_subnet(&mut self, subnet: NetworkSubnet) -> PodmanLensResult<()> {
        add_subnet(&mut self.subnets, subnet)
    }

    /// Adds one static route in declared order.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for duplicate routes or capacity exhaustion.
    pub fn add_route(&mut self, route: NetworkRoute) -> PodmanLensResult<()> {
        add_route(&mut self.routes, route)
    }

    /// Returns the target-side identity.
    #[must_use]
    pub fn identity(&self) -> &DeploymentResourceId {
        &self.identity
    }

    /// Returns caller-declared IPAM subnets in input order.
    #[must_use]
    pub fn subnets(&self) -> &[NetworkSubnet] {
        &self.subnets
    }

    /// Returns caller-declared static routes in input order.
    #[must_use]
    pub fn routes(&self) -> &[NetworkRoute] {
        &self.routes
    }
}
/// One typed named-volume creation intent with independently optional ownership fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeIntent {
    identity: DeploymentResourceId,
    uid: Option<UnixId>,
    gid: Option<UnixId>,
}

impl VolumeIntent {
    /// Creates one named-volume creation intent without ownership overrides.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when the identity is not a volume identity.
    pub fn new(identity: DeploymentResourceId) -> PodmanLensResult<Self> {
        require_kind(&identity, ResourceKind::Volume)?;
        Ok(Self {
            identity,
            uid: None,
            gid: None,
        })
    }

    /// Sets an optional volume-creation UID, preserving a present zero value.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` when ownership was already declared.
    pub fn set_uid(&mut self, uid: UnixId) -> PodmanLensResult<()> {
        set_volume_owner(&mut self.uid, uid)
    }

    /// Sets an optional volume-creation GID, preserving a present zero value.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` when ownership was already declared.
    pub fn set_gid(&mut self, gid: UnixId) -> PodmanLensResult<()> {
        set_volume_owner(&mut self.gid, gid)
    }

    /// Returns the named-volume identity.
    #[must_use]
    pub fn identity(&self) -> &DeploymentResourceId {
        &self.identity
    }

    /// Returns a caller-declared UID, distinct from omitted ownership.
    #[must_use]
    pub const fn uid(&self) -> Option<UnixId> {
        self.uid
    }

    /// Returns a caller-declared GID, distinct from omitted ownership.
    #[must_use]
    pub const fn gid(&self) -> Option<UnixId> {
        self.gid
    }
}

fn set_volume_owner(slot: &mut Option<UnixId>, value: UnixId) -> PodmanLensResult<()> {
    if slot.is_some() {
        return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
    }
    *slot = Some(value);
    Ok(())
}

/// A visible declaration that one exact prerequisite must already exist outside this plan.
///
/// External prerequisites are never inferred from an omitted managed resource. They make an
/// intentional deployment boundary explicit and never emit a semantic operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalPrecondition {
    identity: DeploymentResourceId,
}

impl ExternalPrecondition {
    /// Declares one exact image, network, volume, or secret that this plan deliberately does not manage.
    ///
    /// # Errors
    ///
    /// Returns `PLN0042` for container or pod identities, which need managed lifecycle and
    /// membership validation.
    pub fn new(identity: DeploymentResourceId) -> PodmanLensResult<Self> {
        if matches!(identity.kind(), ResourceKind::Container | ResourceKind::Pod) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidExternalPrecondition));
        }
        Ok(Self { identity })
    }

    /// Returns the exact target-side resource identity.
    #[must_use]
    pub fn identity(&self) -> &DeploymentResourceId {
        &self.identity
    }
}

/// One typed secret creation intent with external, non-serializable material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretIntent {
    identity: DeploymentResourceId,
    material: SensitiveInputReference,
}

impl SecretIntent {
    /// Declares a secret whose bytes are supplied by an external trusted input at deployment time.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when the identity is not a secret identity.
    pub fn new(identity: DeploymentResourceId, material: SensitiveInputReference) -> PodmanLensResult<Self> {
        require_kind(&identity, ResourceKind::Secret)?;
        Ok(Self { identity, material })
    }

    /// Returns the target-side secret identity.
    #[must_use]
    pub fn identity(&self) -> &DeploymentResourceId {
        &self.identity
    }

    /// Returns the external material reference.
    #[must_use]
    pub fn material(&self) -> &SensitiveInputReference {
        &self.material
    }
}

/// One typed Podman pod creation intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PodIntent {
    identity: DeploymentResourceId,
    networks: Vec<NetworkAttachment>,
    ports: Vec<PortMapping>,
    dns: DnsConfiguration,
    hosts: Vec<HostAlias>,
    infra_mounts: Vec<MountIntent>,
    members: Vec<DeploymentResourceId>,
}

impl PodIntent {
    /// Creates an empty Podman pod intent.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when the identity is not a pod.
    pub fn new(identity: DeploymentResourceId) -> PodmanLensResult<Self> {
        require_kind(&identity, ResourceKind::Pod)?;
        Ok(Self {
            identity,
            networks: Vec::new(),
            ports: Vec::new(),
            dns: DnsConfiguration::default(),
            hosts: Vec::new(),
            infra_mounts: Vec::new(),
            members: Vec::new(),
        })
    }

    /// Adds one network attachment owned by this pod's infra container.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when the attachment does not name a network identity.
    pub fn add_network(&mut self, network: NetworkAttachment) -> PodmanLensResult<()> {
        add_attachment(&mut self.networks, network)
    }

    /// Adds one port mapping owned by this pod's infra container.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for a duplicate mapping or capacity exhaustion.
    pub fn add_port(&mut self, port: PortMapping) -> PodmanLensResult<()> {
        add_port(&mut self.ports, port)
    }

    /// Returns mutable DNS configuration owned by this pod's infra container.
    #[must_use]
    pub fn dns_mut(&mut self) -> &mut DnsConfiguration {
        &mut self.dns
    }

    /// Adds one `/etc/hosts` entry owned by this pod's infra container.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for a duplicate alias or capacity exhaustion.
    pub fn add_host_alias(&mut self, host: HostAlias) -> PodmanLensResult<()> {
        add_host(&mut self.hosts, host)
    }

    /// Adds one mount for the Podman pod's infra container.
    ///
    /// The mount is not a declaration for every pod member. Member containers use
    /// [`ContainerIntent::add_mount`] instead.
    pub fn add_infra_mount(&mut self, mount: impl Into<MountIntent>) {
        self.infra_mounts.push(mount.into());
    }

    /// Adds one container that this pod explicitly owns.
    ///
    /// The planner verifies the matching container also names this pod.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when `container` is not a container identity.
    pub fn add_member(&mut self, container: DeploymentResourceId) -> PodmanLensResult<()> {
        require_kind(&container, ResourceKind::Container)?;
        self.members.push(container);
        Ok(())
    }

    /// Returns the pod identity.
    #[must_use]
    pub fn identity(&self) -> &DeploymentResourceId {
        &self.identity
    }

    /// Returns declared network prerequisites in input order.
    #[must_use]
    pub fn networks(&self) -> &[NetworkAttachment] {
        &self.networks
    }

    /// Returns declared pod-owned port mappings in input order.
    #[must_use]
    pub fn ports(&self) -> &[PortMapping] {
        &self.ports
    }

    /// Returns declared pod-owned DNS configuration.
    #[must_use]
    pub fn dns(&self) -> &DnsConfiguration {
        &self.dns
    }

    /// Returns declared pod-owned `/etc/hosts` aliases in input order.
    #[must_use]
    pub fn host_aliases(&self) -> &[HostAlias] {
        &self.hosts
    }

    /// Returns declared infra-container mounts in input order.
    #[must_use]
    pub fn infra_mounts(&self) -> &[MountIntent] {
        &self.infra_mounts
    }

    /// Returns declared pod members in input order.
    #[must_use]
    pub fn members(&self) -> &[DeploymentResourceId] {
        &self.members
    }
}

/// One typed Podman container creation and start intent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerIntent {
    identity: DeploymentResourceId,
    image: DeploymentResourceId,
    pod: Option<DeploymentResourceId>,
    networks: Vec<NetworkAttachment>,
    network_order: Option<Vec<DeploymentResourceId>>,
    ports: Vec<PortMapping>,
    dns: DnsConfiguration,
    hosts: Vec<HostAlias>,
    mounts: Vec<MountIntent>,
    secret_grants: Vec<SecretGrant>,
    settings: Box<ContainerSettings>,
    runtime: Box<ContainerRuntimeSettings>,
}

impl ContainerIntent {
    /// Creates a container intent with one required fully resolved image prerequisite.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when identities have the wrong resource kinds.
    pub fn new(identity: DeploymentResourceId, image: DeploymentResourceId) -> PodmanLensResult<Self> {
        require_kind(&identity, ResourceKind::Container)?;
        require_kind(&image, ResourceKind::Image)?;
        Ok(Self {
            identity,
            image,
            pod: None,
            networks: Vec::new(),
            network_order: None,
            ports: Vec::new(),
            dns: DnsConfiguration::default(),
            hosts: Vec::new(),
            mounts: Vec::new(),
            secret_grants: Vec::new(),
            settings: Box::default(),
            runtime: Box::default(),
        })
    }

    /// Assigns this container to one explicit pod.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when `pod` is not a pod identity, `PLN0035` when it repeats the
    /// existing pod, and `PLN0038` when it conflicts with an existing pod assignment.
    pub fn set_pod(&mut self, pod: DeploymentResourceId) -> PodmanLensResult<()> {
        require_kind(&pod, ResourceKind::Pod)?;
        match &self.pod {
            None => {
                self.pod = Some(pod);
                Ok(())
            }
            Some(existing) if existing == &pod => Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource)),
            Some(_) => Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination)),
        }
    }

    /// Adds a direct network attachment for an unpodded container.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when the attachment does not name a network identity.
    pub fn add_network(&mut self, network: NetworkAttachment) -> PodmanLensResult<()> {
        add_attachment(&mut self.networks, network)
    }

    /// Declares the exact network-connection order for this unpodded container.
    ///
    /// An omitted order lets the target choose its native default. An explicit order must contain
    /// every declared attachment exactly once and is only available on reviewed Podman 6.0 and
    /// newer targets.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an empty or non-network order and `PLN0038` when already set.
    pub fn set_network_order(&mut self, order: Vec<DeploymentResourceId>) -> PodmanLensResult<()> {
        if order.is_empty() || order.iter().any(|network| network.kind() != ResourceKind::Network) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        if self.network_order.is_some() {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
        }
        self.network_order = Some(order);
        Ok(())
    }

    /// Adds one host-to-container port mapping for an unpodded container.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for a duplicate mapping or capacity exhaustion.
    pub fn add_port(&mut self, port: PortMapping) -> PodmanLensResult<()> {
        add_port(&mut self.ports, port)
    }

    /// Returns mutable DNS configuration for an unpodded container.
    #[must_use]
    pub fn dns_mut(&mut self) -> &mut DnsConfiguration {
        &mut self.dns
    }

    /// Adds one `/etc/hosts` alias for an unpodded container.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for a duplicate alias or capacity exhaustion.
    pub fn add_host_alias(&mut self, host: HostAlias) -> PodmanLensResult<()> {
        add_host(&mut self.hosts, host)
    }

    /// Adds one typed named-volume, bind, or tmpfs mount.
    pub fn add_mount(&mut self, mount: impl Into<MountIntent>) {
        self.mounts.push(mount.into());
    }

    /// Adds one typed mounted or environment secret grant.
    pub fn add_secret_grant(&mut self, grant: SecretGrant) {
        self.secret_grants.push(grant);
    }

    /// Returns the container identity.
    #[must_use]
    pub fn identity(&self) -> &DeploymentResourceId {
        &self.identity
    }

    /// Returns the required image prerequisite.
    #[must_use]
    pub fn image(&self) -> &DeploymentResourceId {
        &self.image
    }

    /// Returns explicit pod membership, when requested.
    #[must_use]
    pub fn pod(&self) -> Option<&DeploymentResourceId> {
        self.pod.as_ref()
    }

    /// Returns direct network prerequisites.
    #[must_use]
    pub fn networks(&self) -> &[NetworkAttachment] {
        &self.networks
    }

    /// Returns the explicit declared network order, when one is required.
    #[must_use]
    pub fn network_order(&self) -> Option<&[DeploymentResourceId]> {
        self.network_order.as_deref()
    }

    /// Returns declared direct port mappings in input order.
    #[must_use]
    pub fn ports(&self) -> &[PortMapping] {
        &self.ports
    }

    /// Returns declared direct DNS configuration.
    #[must_use]
    pub fn dns(&self) -> &DnsConfiguration {
        &self.dns
    }

    /// Returns declared direct `/etc/hosts` aliases in input order.
    #[must_use]
    pub fn host_aliases(&self) -> &[HostAlias] {
        &self.hosts
    }

    /// Returns named-volume mounts.
    #[must_use]
    pub fn mounts(&self) -> &[MountIntent] {
        &self.mounts
    }

    /// Returns typed secret grants in declared order.
    #[must_use]
    pub fn secret_grants(&self) -> &[SecretGrant] {
        &self.secret_grants
    }

    /// Returns the typed optional container settings.
    #[must_use]
    pub fn settings(&self) -> &ContainerSettings {
        &self.settings
    }

    /// Returns mutable typed optional container settings.
    #[must_use]
    pub fn settings_mut(&mut self) -> &mut ContainerSettings {
        &mut self.settings
    }

    /// Returns typed M6-B3a runtime settings retained for semantic planning only.
    #[must_use]
    pub fn runtime(&self) -> &ContainerRuntimeSettings {
        &self.runtime
    }

    /// Returns mutable M6-B3a runtime settings retained for semantic planning only.
    #[must_use]
    pub fn runtime_mut(&mut self) -> &mut ContainerRuntimeSettings {
        &mut self.runtime
    }
}

/// One fully resolved Podman-native resource declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DeploymentResource {
    /// An exact resource intentionally managed outside this plan.
    ExternalPrecondition(ExternalPrecondition),
    /// An image that must be available before its consumers are created.
    Image(ImageIntent),
    /// A network.
    Network(NetworkIntent),
    /// A named volume.
    Volume(VolumeIntent),
    /// A secret whose material stays outside the plan.
    Secret(SecretIntent),
    /// A Podman pod.
    Pod(PodIntent),
    /// A Podman container.
    Container(ContainerIntent),
}

impl DeploymentResource {
    /// Returns the typed resource identity.
    #[must_use]
    pub fn identity(&self) -> &DeploymentResourceId {
        match self {
            Self::ExternalPrecondition(resource) => resource.identity(),
            Self::Image(resource) => resource.identity(),
            Self::Network(resource) => resource.identity(),
            Self::Volume(resource) => resource.identity(),
            Self::Secret(resource) => resource.identity(),
            Self::Pod(resource) => resource.identity(),
            Self::Container(resource) => resource.identity(),
        }
    }
}

/// A semantic ordering edge between two container start intents.
///
/// Pod members are lifted to their pod-start operation. Ordering two containers in the same pod
/// is rejected because Podman starts that pod as one unit.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StartupDependency {
    predecessor: DeploymentResourceId,
    dependent: DeploymentResourceId,
}

impl StartupDependency {
    /// Creates a strict start-order edge from `predecessor` to `dependent`.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` unless both identities are containers.
    pub fn new(predecessor: DeploymentResourceId, dependent: DeploymentResourceId) -> PodmanLensResult<Self> {
        require_kind(&predecessor, ResourceKind::Container)?;
        require_kind(&dependent, ResourceKind::Container)?;
        Ok(Self { predecessor, dependent })
    }

    /// Returns the container that must start first.
    #[must_use]
    pub fn predecessor(&self) -> &DeploymentResourceId {
        &self.predecessor
    }

    /// Returns the container that starts later.
    #[must_use]
    pub fn dependent(&self) -> &DeploymentResourceId {
        &self.dependent
    }
}

/// Explicit input to semantic Podman deployment planning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentIntent {
    target: TargetProfile,
    connection: Option<DeploymentConnectionReference>,
    resources: Vec<DeploymentResource>,
    startup_dependencies: Vec<StartupDependency>,
}

impl DeploymentIntent {
    /// Creates an empty intent for an explicit reviewed target profile.
    #[must_use]
    pub fn new(target: TargetProfile) -> Self {
        Self {
            target,
            connection: None,
            resources: Vec::new(),
            startup_dependencies: Vec::new(),
        }
    }

    /// Records a non-sensitive output-connection reference in this plan.
    pub fn set_connection(&mut self, connection: DeploymentConnectionReference) {
        self.connection = Some(connection);
    }

    /// Adds one fully resolved resource declaration.
    ///
    /// Duplicate identities and unresolved prerequisites are reported by [`plan_deployment`], so
    /// callers may collect all typed declarations before asking the planner to validate them.
    pub fn add_resource(&mut self, resource: DeploymentResource) {
        self.resources.push(resource);
    }

    /// Adds one explicit semantic container-start ordering edge.
    pub fn add_startup_dependency(&mut self, dependency: StartupDependency) {
        self.startup_dependencies.push(dependency);
    }

    /// Returns the explicit reviewed target profile.
    #[must_use]
    pub fn target(&self) -> &TargetProfile {
        &self.target
    }

    /// Returns the optional non-sensitive output connection reference.
    #[must_use]
    pub fn connection(&self) -> Option<&DeploymentConnectionReference> {
        self.connection.as_ref()
    }

    /// Returns typed resources in caller-supplied order.
    #[must_use]
    pub fn resources(&self) -> &[DeploymentResource] {
        &self.resources
    }

    /// Returns explicit semantic start-order edges in caller-supplied order.
    #[must_use]
    pub fn startup_dependencies(&self) -> &[StartupDependency] {
        &self.startup_dependencies
    }
}

/// The semantic phase of one deployment operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum SemanticOperationAction {
    /// Ensure an explicitly named image is available.
    EnsureImage,
    /// Create one native resource.
    Create,
    /// Start one managed Podman pod after all its member containers are created.
    StartPod,
    /// Start one managed container that does not belong to a pod.
    StartContainer,
}

/// Stable typed identity for one semantic operation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DeploymentOperationId {
    action: SemanticOperationAction,
    resource: DeploymentResourceId,
}

impl DeploymentOperationId {
    fn new(action: SemanticOperationAction, resource: DeploymentResourceId) -> Self {
        Self { action, resource }
    }

    /// Returns the semantic operation phase.
    #[must_use]
    pub const fn action(&self) -> SemanticOperationAction {
        self.action
    }

    /// Returns the resource operated on.
    #[must_use]
    pub fn resource(&self) -> &DeploymentResourceId {
        &self.resource
    }
}

/// One ordered, transport-neutral deployment operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentOperation {
    id: DeploymentOperationId,
    resource_intent: DeploymentResource,
    depends_on: Vec<DeploymentOperationId>,
    image_pull_policy: Option<ImagePullPolicy>,
}

impl DeploymentOperation {
    /// Returns the stable semantic operation identity.
    #[must_use]
    pub fn id(&self) -> &DeploymentOperationId {
        &self.id
    }

    /// Returns the complete typed intent rendered by this operation.
    ///
    /// This retains semantic source details such as a managed image source or the external
    /// sensitive-material reference of a secret. The reference is redacted by its own `Debug`
    /// implementation, and M5 does not serialize operations.
    #[must_use]
    pub fn resource_intent(&self) -> &DeploymentResource {
        &self.resource_intent
    }

    /// Returns prerequisite operations in deterministic order.
    #[must_use]
    pub fn depends_on(&self) -> &[DeploymentOperationId] {
        &self.depends_on
    }

    /// Returns the explicit policy for image-acquisition operations.
    #[must_use]
    pub const fn image_pull_policy(&self) -> Option<ImagePullPolicy> {
        self.image_pull_policy
    }
}

/// A complete, validated, sequentially executable semantic deployment plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentPlan {
    target: TargetProfile,
    connection: Option<DeploymentConnectionReference>,
    external_preconditions: Vec<ExternalPrecondition>,
    operations: Vec<DeploymentOperation>,
}

/// One redacted, structured reason why deployment planning did not produce a plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanningFinding {
    code: DiagnosticCode,
    subject: Option<DeploymentResourceId>,
    related: Vec<DeploymentResourceId>,
    field: Option<&'static str>,
    occurrence: Option<usize>,
    count: Option<usize>,
}

impl Ord for PlanningFinding {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            self.code.as_str(),
            &self.subject,
            &self.related,
            self.field,
            self.occurrence,
            self.count,
        )
            .cmp(&(
                other.code.as_str(),
                &other.subject,
                &other.related,
                other.field,
                other.occurrence,
                other.count,
            ))
    }
}

impl PartialOrd for PlanningFinding {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PlanningFinding {
    fn new(code: DiagnosticCode, subject: Option<DeploymentResourceId>, field: Option<&'static str>) -> Self {
        Self::detailed(code, subject, Vec::new(), field, None)
    }

    fn detailed(
        code: DiagnosticCode,
        subject: Option<DeploymentResourceId>,
        related: Vec<DeploymentResourceId>,
        field: Option<&'static str>,
        occurrence: Option<usize>,
    ) -> Self {
        Self {
            code,
            subject,
            related,
            field,
            occurrence,
            count: None,
        }
    }

    fn with_count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Returns the stable M5 diagnostic code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the redacted human-facing explanation associated with [`Self::code`].
    #[must_use]
    pub const fn message(&self) -> &'static str {
        Diagnostic::new(self.code).message()
    }

    /// Returns the affected target resource, when one exact resource caused the finding.
    #[must_use]
    pub fn subject(&self) -> Option<&DeploymentResourceId> {
        self.subject.as_ref()
    }

    /// Returns exact safe resource identities related to this finding.
    #[must_use]
    pub fn related(&self) -> &[DeploymentResourceId] {
        &self.related
    }

    /// Returns the stable intent field that caused the finding, when applicable.
    #[must_use]
    pub const fn field(&self) -> Option<&'static str> {
        self.field
    }

    /// Returns the one-based position of a duplicate prerequisite or startup edge.
    ///
    /// Aggregate duplicate resource declarations have no single position; use [`Self::count`]
    /// for their cardinality.
    #[must_use]
    pub const fn occurrence(&self) -> Option<usize> {
        self.occurrence
    }

    /// Returns the number of equivalent conflicting or duplicate declarations, when grouped.
    #[must_use]
    pub const fn count(&self) -> Option<usize> {
        self.count
    }
}

/// The complete structured outcome of semantic deployment planning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanningOutcome {
    plan: Option<DeploymentPlan>,
    findings: Vec<PlanningFinding>,
}

impl PlanningOutcome {
    /// Returns the semantic plan when no error findings were produced.
    #[must_use]
    pub fn plan(&self) -> Option<&DeploymentPlan> {
        self.plan.as_ref()
    }

    /// Returns all findings sorted by code, subject, and field.
    #[must_use]
    pub fn findings(&self) -> &[PlanningFinding] {
        &self.findings
    }

    /// Returns whether planning produced an executable semantic plan.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.plan.is_some()
    }
}

impl DeploymentPlan {
    /// Returns the explicit reviewed target profile.
    #[must_use]
    pub fn target(&self) -> &TargetProfile {
        &self.target
    }

    /// Returns the optional non-sensitive output connection reference.
    #[must_use]
    pub fn connection(&self) -> Option<&DeploymentConnectionReference> {
        self.connection.as_ref()
    }

    /// Returns explicit resource prerequisites intentionally managed outside this plan.
    ///
    /// Values are deterministically ordered by their typed target-side identity. M5 keeps them
    /// as semantic declarations; it does not render or execute them.
    #[must_use]
    pub fn external_preconditions(&self) -> &[ExternalPrecondition] {
        &self.external_preconditions
    }

    /// Returns operations in authoritative sequential execution order.
    #[must_use]
    pub fn operations(&self) -> &[DeploymentOperation] {
        &self.operations
    }
}

/// Validates explicit typed intent and returns its deterministic semantic deployment plan.
///
/// The plan never contains a shell fragment, HTTP request, or secret payload. It may retain typed
/// caller-declared public and sensitive environment intent so a later renderer can make an explicit
/// output decision. Sensitive values and references remain redacted from diagnostics, `Debug`,
/// observational snapshots, and serialized deployment artifacts; renderers must block them until a
/// safe output representation is available. Every operation is ordered after its declared
/// prerequisites. M6 renders supported operations as exact CLI arguments and Libpod API requests.
///
/// # Findings
///
/// The outcome uses `PLN0034`–`PLN0044` for invalid intent, duplicate or conflicting declarations,
/// missing prerequisites, unsupported combinations, cycles, sensitive payloads, bad image or
/// external-precondition declarations, pod membership, and same-pod start ordering.
#[must_use]
pub fn plan_deployment(intent: &DeploymentIntent) -> PlanningOutcome {
    let (resources, mut findings) = index_resources(intent.resources());
    validate_resources(&resources, intent.target(), &mut findings);
    validate_startup_dependencies(intent, &resources, &mut findings);
    if findings.is_empty() {
        let mut nodes = BTreeMap::<DeploymentOperationId, BTreeSet<DeploymentOperationId>>::new();
        for (identity, resource) in &resources {
            if matches!(resource, DeploymentResource::ExternalPrecondition(_)) {
                continue;
            }
            let action = if identity.kind() == ResourceKind::Image {
                SemanticOperationAction::EnsureImage
            } else {
                SemanticOperationAction::Create
            };
            let id = DeploymentOperationId::new(action, identity.clone());
            nodes.insert(id, create_dependencies(resource, &resources));
        }
        add_start_operations(intent, &resources, &mut nodes);
        match topological_operations(nodes, &resources) {
            Ok(operations) => {
                return PlanningOutcome {
                    plan: Some(DeploymentPlan {
                        target: intent.target.clone(),
                        connection: intent.connection.clone(),
                        external_preconditions: external_preconditions(&resources),
                        operations,
                    }),
                    findings,
                };
            }
            Err(operations) => findings.push(PlanningFinding::detailed(
                DiagnosticCode::DeploymentCycle,
                operations.first().map(|operation| operation.resource().clone()),
                operations
                    .into_iter()
                    .map(|operation| operation.resource().clone())
                    .collect(),
                Some("startup_dependencies"),
                None,
            )),
        }
    }
    sort_findings(&mut findings);
    PlanningOutcome { plan: None, findings }
}

fn index_resources(
    resources: &[DeploymentResource],
) -> (
    BTreeMap<DeploymentResourceId, &DeploymentResource>,
    Vec<PlanningFinding>,
) {
    let mut declarations = BTreeMap::<DeploymentResourceId, Vec<&DeploymentResource>>::new();
    for resource in resources {
        declarations
            .entry(resource.identity().clone())
            .or_default()
            .push(resource);
    }
    let mut indexed = BTreeMap::new();
    let mut findings = Vec::new();
    for (identity, declarations) in declarations {
        let [first, rest @ ..] = declarations.as_slice() else {
            continue;
        };
        if rest.is_empty() {
            indexed.insert(identity, *first);
        } else if rest.iter().all(|resource| **resource == **first) {
            indexed.insert(identity.clone(), *first);
            findings.push(
                PlanningFinding::detailed(
                    DiagnosticCode::DeploymentDuplicateResource,
                    Some(identity),
                    Vec::new(),
                    Some("resources"),
                    None,
                )
                .with_count(declarations.len()),
            );
        } else {
            findings.push(
                PlanningFinding::detailed(
                    DiagnosticCode::DeploymentConflictingResource,
                    Some(identity),
                    Vec::new(),
                    Some("resources"),
                    None,
                )
                .with_count(declarations.len()),
            );
        }
    }
    (indexed, findings)
}

fn external_preconditions(
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
) -> Vec<ExternalPrecondition> {
    resources
        .values()
        .filter_map(|resource| match resource {
            DeploymentResource::ExternalPrecondition(precondition) => Some(precondition.clone()),
            _ => None,
        })
        .collect()
}

fn validate_resources(
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    target: &TargetProfile,
    findings: &mut Vec<PlanningFinding>,
) {
    for resource in resources.values() {
        match resource {
            DeploymentResource::ExternalPrecondition(_)
            | DeploymentResource::Network(_)
            | DeploymentResource::Volume(_)
            | DeploymentResource::Image(_) => {}
            DeploymentResource::Secret(secret) => {
                if secret.material().as_str().is_empty() {
                    findings.push(PlanningFinding::new(
                        DiagnosticCode::SensitivePayloadEmbedded,
                        Some(secret.identity().clone()),
                        Some("material"),
                    ));
                }
            }
            DeploymentResource::Pod(pod) => {
                validate_pod(resources, pod, target.execution_context(), findings);
            }
            DeploymentResource::Container(container) => {
                validate_container(resources, container, target, findings);
            }
        }
    }
}

fn validate_pod(
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    pod: &PodIntent,
    execution_context: TargetExecutionContext,
    findings: &mut Vec<PlanningFinding>,
) {
    validate_network_attachments(pod.networks(), pod.identity(), "networks", findings);
    validate_static_network_addresses(pod.networks(), pod.identity(), execution_context, findings);
    validate_mounts(resources, pod.infra_mounts(), pod.identity(), "infra_mounts", findings);
    validate_distinct(pod.members(), pod.identity(), "members", findings);
    for network in pod.networks() {
        require_resolved(
            resources,
            network.network(),
            ResourceKind::Network,
            pod.identity(),
            "networks",
            findings,
        );
    }
    for member in pod.members() {
        let member_resource = resolved(resources, member, ResourceKind::Container);
        if !matches!(member_resource, Some(DeploymentResource::Container(container)) if container.pod() == Some(pod.identity()))
        {
            findings.push(PlanningFinding::detailed(
                DiagnosticCode::DeploymentPodMembership,
                Some(pod.identity().clone()),
                vec![member.clone()],
                Some("members"),
                None,
            ));
        }
    }
}

#[allow(clippy::too_many_lines)] // Every pod/member namespace invariant is validated together.
fn validate_container(
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    container: &ContainerIntent,
    target: &TargetProfile,
    findings: &mut Vec<PlanningFinding>,
) {
    validate_network_attachments(container.networks(), container.identity(), "networks", findings);
    validate_static_network_addresses(
        container.networks(),
        container.identity(),
        target.execution_context(),
        findings,
    );
    validate_mounts(resources, container.mounts(), container.identity(), "mounts", findings);
    validate_secret_grants(resources, container.secret_grants(), container, findings);
    require_resolved(
        resources,
        container.image(),
        ResourceKind::Image,
        container.identity(),
        "image",
        findings,
    );
    if let Some(pod) = container.pod() {
        if !container.runtime().namespaces().is_empty() {
            findings.push(PlanningFinding::new(
                DiagnosticCode::DeploymentUnsupportedCombination,
                Some(container.identity().clone()),
                Some("runtime.namespaces.pod_member"),
            ));
        }
        if container.settings().hostname().is_some() {
            findings.push(PlanningFinding::new(
                DiagnosticCode::DeploymentUnsupportedCombination,
                Some(container.identity().clone()),
                Some("hostname"),
            ));
        }
        if !container.networks().is_empty() {
            findings.push(PlanningFinding::new(
                DiagnosticCode::DeploymentUnsupportedCombination,
                Some(container.identity().clone()),
                Some("networks"),
            ));
        }
        if !container.ports().is_empty() {
            findings.push(PlanningFinding::new(
                DiagnosticCode::DeploymentUnsupportedCombination,
                Some(container.identity().clone()),
                Some("ports"),
            ));
        }
        if !container.dns().servers().is_empty()
            || !container.dns().search().is_empty()
            || !container.dns().options().is_empty()
        {
            findings.push(PlanningFinding::new(
                DiagnosticCode::DeploymentUnsupportedCombination,
                Some(container.identity().clone()),
                Some("dns"),
            ));
        }
        if !container.host_aliases().is_empty() {
            findings.push(PlanningFinding::new(
                DiagnosticCode::DeploymentUnsupportedCombination,
                Some(container.identity().clone()),
                Some("host_aliases"),
            ));
        }
        if container.network_order().is_some() {
            findings.push(PlanningFinding::new(
                DiagnosticCode::DeploymentUnsupportedCombination,
                Some(container.identity().clone()),
                Some("network_order"),
            ));
        }
        if resolved(resources, pod, ResourceKind::Pod).is_none() {
            findings.push(PlanningFinding::detailed(
                DiagnosticCode::DeploymentUnresolvedPrerequisite,
                Some(container.identity().clone()),
                vec![pod.clone()],
                Some("pod"),
                None,
            ));
        }
        if let Some(DeploymentResource::Pod(pod_resource)) = resolved(resources, pod, ResourceKind::Pod) {
            if !pod_resource.members().contains(container.identity()) {
                findings.push(PlanningFinding::detailed(
                    DiagnosticCode::DeploymentPodMembership,
                    Some(container.identity().clone()),
                    vec![pod.clone()],
                    Some("pod"),
                    None,
                ));
            }
        }
    }
    for network in container.networks() {
        require_resolved(
            resources,
            network.network(),
            ResourceKind::Network,
            container.identity(),
            "networks",
            findings,
        );
    }
    if let Some(order) = container.network_order() {
        let declared = container
            .networks()
            .iter()
            .map(NetworkAttachment::network)
            .collect::<BTreeSet<_>>();
        let ordered = order.iter().collect::<BTreeSet<_>>();
        if declared.len() != container.networks().len() || order.len() != ordered.len() || declared != ordered {
            findings.push(PlanningFinding::new(
                DiagnosticCode::DeploymentUnsupportedCombination,
                Some(container.identity().clone()),
                Some("network_order"),
            ));
        }
    }
    validate_runtime_settings(container, target, findings);
}

#[allow(clippy::too_many_lines)] // Health, logging, security, and cgroup invariants share one outcome.
fn validate_runtime_settings(container: &ContainerIntent, target: &TargetProfile, findings: &mut Vec<PlanningFinding>) {
    let runtime = container.runtime();
    if runtime.namespaces().uts() == Some(crate::NamespaceMode::Host) && container.settings().hostname().is_some() {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.namespaces.uts_host_with_hostname"),
        ));
    }
    if runtime.namespaces().cgroup() == Some(crate::NamespaceMode::Private)
        && !target
            .cgroup_capabilities()
            .is_some_and(|evidence| evidence.version() == crate::CgroupVersion::V2)
    {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.namespaces.cgroup_private_requires_v2"),
        ));
    }
    if runtime.startup_health().is_some() && !matches!(runtime.health(), Some(crate::HealthCheck::Command(_))) {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.startup_health_requires_health"),
        ));
    }
    if runtime.logging().driver().is_none()
        && (runtime.logging().max_size().is_some() || !runtime.logging().journald_labels().is_empty())
    {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.logging.driver"),
        ));
    }
    if !runtime.logging().journald_labels().is_empty() && runtime.logging().driver() != Some(crate::LogDriver::Journald)
    {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.logging.journald_labels"),
        ));
    }
    if !runtime.logging().journald_labels().is_empty()
        && target.podman_version().as_semver() < &semver::Version::new(6, 0, 0)
    {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.logging.journald_labels.target_version"),
        ));
    }
    if runtime.logging().max_size().is_some() && runtime.logging().driver() != Some(crate::LogDriver::K8sFile) {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.logging.max_size"),
        ));
    }
    if runtime.security().privileged() == Some(true)
        && (!runtime.security().cap_add().is_empty() || !runtime.security().cap_drop().is_empty())
    {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.security.privileged_capabilities"),
        ));
    }
    if runtime
        .security()
        .cap_add()
        .iter()
        .any(|capability| runtime.security().cap_drop().contains(capability))
    {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.security.capability_overlap"),
        ));
    }
    if runtime.security().read_write_tmpfs() == Some(true) && runtime.security().read_only_filesystem() != Some(true) {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.security.read_write_tmpfs"),
        ));
    }
    let resources = runtime.resources();
    if resources.rlimits().iter().any(|limit| {
        matches!(limit.soft(), crate::RlimitValue::Unlimited) || matches!(limit.hard(), crate::RlimitValue::Unlimited)
    }) && target.podman_version().as_semver() < &semver::Version::new(5, 6, 0)
    {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.resources.rlimits.unlimited.target_version"),
        ));
    }
    let controls_requested = resources.cpu_shares().is_some()
        || resources.cpu_period().is_some()
        || resources.cpu_quota().is_some()
        || resources.memory_bytes().is_some()
        || resources.pids().is_some();
    if controls_requested && target.cgroup_capabilities().is_none() {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.resources.cgroup_evidence"),
        ));
        return;
    }
    if controls_requested
        && target.cgroup_capabilities().is_some_and(|evidence| {
            evidence.version() == crate::CgroupVersion::V1
                && target.execution_context() != TargetExecutionContext::Rootful
        })
    {
        findings.push(PlanningFinding::new(
            DiagnosticCode::DeploymentUnsupportedCombination,
            Some(container.identity().clone()),
            Some("runtime.resources.cgroup_v1_requires_rootful"),
        ));
    }
    for (configured, controller, field) in [
        (
            resources.cpu_shares().is_some() || resources.cpu_period().is_some() || resources.cpu_quota().is_some(),
            CgroupController::Cpu,
            "runtime.resources.cpu",
        ),
        (
            resources.memory_bytes().is_some(),
            CgroupController::Memory,
            "runtime.resources.memory_bytes",
        ),
        (
            resources.pids().is_some(),
            CgroupController::Pids,
            "runtime.resources.pids",
        ),
    ] {
        if configured
            && !target
                .cgroup_capabilities()
                .is_some_and(|evidence| evidence.supports(controller))
        {
            findings.push(PlanningFinding::new(
                DiagnosticCode::DeploymentUnsupportedCombination,
                Some(container.identity().clone()),
                Some(field),
            ));
        }
    }
}

fn validate_static_network_addresses(
    attachments: &[NetworkAttachment],
    owner: &DeploymentResourceId,
    execution_context: TargetExecutionContext,
    findings: &mut Vec<PlanningFinding>,
) {
    if execution_context == TargetExecutionContext::Rootful {
        return;
    }
    for attachment in attachments {
        for (configured, field) in [
            (
                attachment.static_ipv4().is_some(),
                "networks.static_ipv4_requires_rootful",
            ),
            (
                attachment.static_ipv6().is_some(),
                "networks.static_ipv6_requires_rootful",
            ),
            (
                attachment.static_mac().is_some(),
                "networks.static_mac_requires_rootful",
            ),
        ] {
            if configured {
                findings.push(PlanningFinding::new(
                    DiagnosticCode::DeploymentUnsupportedCombination,
                    Some(owner.clone()),
                    Some(field),
                ));
            }
        }
    }
}

fn validate_network_attachments(
    attachments: &[NetworkAttachment],
    owner: &DeploymentResourceId,
    field: &'static str,
    findings: &mut Vec<PlanningFinding>,
) {
    for (index, attachment) in attachments.iter().enumerate() {
        if attachments[..index]
            .iter()
            .any(|previous| previous.network() == attachment.network())
        {
            findings.push(PlanningFinding::detailed(
                DiagnosticCode::DeploymentDuplicateResource,
                Some(owner.clone()),
                vec![attachment.network().clone()],
                Some(field),
                Some(index + 1),
            ));
        }
    }
}

fn validate_mounts(
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    mounts: &[MountIntent],
    owner: &DeploymentResourceId,
    field: &'static str,
    findings: &mut Vec<PlanningFinding>,
) {
    for (index, mount) in mounts.iter().enumerate() {
        if mounts[..index]
            .iter()
            .any(|previous| previous.destination() == mount.destination())
        {
            findings.push(PlanningFinding::detailed(
                DiagnosticCode::DeploymentDuplicateResource,
                Some(owner.clone()),
                mount.volume_source().cloned().into_iter().collect(),
                Some(field),
                Some(index + 1),
            ));
        }
        if let Some(source) = mount.volume_source() {
            require_resolved(resources, source, ResourceKind::Volume, owner, field, findings);
        }
    }
}

fn validate_secret_grants(
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    grants: &[SecretGrant],
    container: &ContainerIntent,
    findings: &mut Vec<PlanningFinding>,
) {
    let mut mount_destinations = BTreeSet::new();
    let mut environment_targets = BTreeSet::new();
    for (index, grant) in grants.iter().enumerate() {
        require_resolved(
            resources,
            grant.source(),
            ResourceKind::Secret,
            container.identity(),
            "secret_grants",
            findings,
        );
        if let Some(target) = grant.mount_target() {
            if !mount_destinations.insert(target.as_str()) {
                findings.push(PlanningFinding::detailed(
                    DiagnosticCode::DeploymentDuplicateResource,
                    Some(container.identity().clone()),
                    vec![grant.source().clone()],
                    Some("secret_grants.mount_target"),
                    Some(index + 1),
                ));
            }
        }
        if let Some(target) = grant.environment_target() {
            if !environment_targets.insert(target.as_str())
                || container
                    .settings()
                    .environment()
                    .iter()
                    .any(|assignment| assignment.name() == target)
            {
                findings.push(PlanningFinding::detailed(
                    DiagnosticCode::DeploymentDuplicateResource,
                    Some(container.identity().clone()),
                    vec![grant.source().clone()],
                    Some("secret_grants.environment_target"),
                    Some(index + 1),
                ));
            }
        }
    }
}

fn create_dependencies(
    resource: &DeploymentResource,
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
) -> BTreeSet<DeploymentOperationId> {
    let mut dependencies = BTreeSet::new();
    match resource {
        DeploymentResource::ExternalPrecondition(_)
        | DeploymentResource::Image(_)
        | DeploymentResource::Network(_)
        | DeploymentResource::Volume(_)
        | DeploymentResource::Secret(_) => {}
        DeploymentResource::Pod(pod) => {
            for network in pod.networks() {
                if is_managed(resources, network.network()) {
                    dependencies.insert(create_operation(network.network()));
                }
            }
            for mount in pod.infra_mounts() {
                if let Some(source) = mount.volume_source().filter(|source| is_managed(resources, source)) {
                    dependencies.insert(create_operation(source));
                }
            }
        }
        DeploymentResource::Container(container) => {
            if is_managed(resources, container.image()) {
                dependencies.insert(DeploymentOperationId::new(
                    SemanticOperationAction::EnsureImage,
                    container.image().clone(),
                ));
            }
            if let Some(pod) = container.pod() {
                if is_managed(resources, pod) {
                    dependencies.insert(create_operation(pod));
                }
            }
            for network in container.networks() {
                if is_managed(resources, network.network()) {
                    dependencies.insert(create_operation(network.network()));
                }
            }
            for mount in container.mounts() {
                if let Some(source) = mount.volume_source().filter(|source| is_managed(resources, source)) {
                    dependencies.insert(create_operation(source));
                }
            }
            for grant in container.secret_grants() {
                if is_managed(resources, grant.source()) {
                    dependencies.insert(create_operation(grant.source()));
                }
            }
        }
    }
    dependencies
}

fn add_start_operations(
    intent: &DeploymentIntent,
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    nodes: &mut BTreeMap<DeploymentOperationId, BTreeSet<DeploymentOperationId>>,
) {
    for (identity, resource) in resources {
        match resource {
            DeploymentResource::Pod(pod) if !pod.members().is_empty() => {
                let id = DeploymentOperationId::new(SemanticOperationAction::StartPod, identity.clone());
                let mut dependencies = BTreeSet::from([create_operation(identity)]);
                for member in pod.members() {
                    if is_managed(resources, member) {
                        dependencies.insert(create_operation(member));
                    }
                }
                nodes.insert(id, dependencies);
            }
            DeploymentResource::Container(container) if container.pod().is_none() => {
                nodes.insert(
                    DeploymentOperationId::new(SemanticOperationAction::StartContainer, identity.clone()),
                    BTreeSet::from([create_operation(identity)]),
                );
            }
            _ => {}
        }
    }
    let mut dependencies = BTreeSet::new();
    for dependency in intent.startup_dependencies() {
        if let (Some(predecessor), Some(dependent)) = (
            start_anchor(resources, dependency.predecessor()),
            start_anchor(resources, dependency.dependent()),
        ) {
            dependencies.insert((predecessor, dependent));
        }
    }
    for (predecessor, dependent) in dependencies {
        if let Some(values) = nodes.get_mut(&dependent) {
            values.insert(predecessor);
        }
    }
}

fn validate_startup_dependencies(
    intent: &DeploymentIntent,
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    findings: &mut Vec<PlanningFinding>,
) {
    let mut edges = BTreeMap::<DeploymentOperationId, BTreeSet<DeploymentOperationId>>::new();
    let mut seen = BTreeSet::new();
    for (index, dependency) in intent.startup_dependencies().iter().enumerate() {
        let occurrence = Some(index + 1);
        let predecessor = start_anchor(resources, dependency.predecessor());
        let dependent = start_anchor(resources, dependency.dependent());
        if predecessor.is_none() {
            findings.push(PlanningFinding::detailed(
                DiagnosticCode::DeploymentUnresolvedPrerequisite,
                Some(dependency.predecessor().clone()),
                vec![dependency.dependent().clone()],
                Some("startup_dependencies"),
                occurrence,
            ));
        }
        if dependent.is_none() {
            findings.push(PlanningFinding::detailed(
                DiagnosticCode::DeploymentUnresolvedPrerequisite,
                Some(dependency.dependent().clone()),
                vec![dependency.predecessor().clone()],
                Some("startup_dependencies"),
                occurrence,
            ));
        }
        let (Some(predecessor), Some(dependent)) = (predecessor, dependent) else {
            continue;
        };
        if predecessor == dependent && predecessor.action() == SemanticOperationAction::StartPod {
            findings.push(PlanningFinding::detailed(
                DiagnosticCode::SamePodStartupDependency,
                Some(dependency.dependent().clone()),
                vec![dependency.predecessor().clone()],
                Some("startup_dependencies"),
                occurrence,
            ));
            continue;
        }
        if !seen.insert((predecessor.clone(), dependent.clone())) {
            findings.push(PlanningFinding::detailed(
                DiagnosticCode::DeploymentDuplicateResource,
                Some(dependency.dependent().clone()),
                vec![dependency.predecessor().clone()],
                Some("startup_dependencies"),
                occurrence,
            ));
            continue;
        }
        edges.entry(predecessor.clone()).or_default();
        edges.entry(dependent).or_default().insert(predecessor);
    }
    if let Some(operations) = operation_cycle(&edges) {
        findings.push(PlanningFinding::detailed(
            DiagnosticCode::DeploymentCycle,
            operations.first().map(|operation| operation.resource().clone()),
            operations
                .into_iter()
                .map(|operation| operation.resource().clone())
                .collect(),
            Some("startup_dependencies"),
            None,
        ));
    }
}

fn create_operation(resource: &DeploymentResourceId) -> DeploymentOperationId {
    DeploymentOperationId::new(SemanticOperationAction::Create, resource.clone())
}

fn start_anchor(
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    container: &DeploymentResourceId,
) -> Option<DeploymentOperationId> {
    let DeploymentResource::Container(intent) = resources.get(container).copied()? else {
        return None;
    };
    match intent.pod() {
        Some(pod) if matches!(resources.get(pod), Some(DeploymentResource::Pod(_))) => Some(
            DeploymentOperationId::new(SemanticOperationAction::StartPod, pod.clone()),
        ),
        Some(_) => None,
        None => Some(DeploymentOperationId::new(
            SemanticOperationAction::StartContainer,
            container.clone(),
        )),
    }
}

fn topological_operations(
    mut nodes: BTreeMap<DeploymentOperationId, BTreeSet<DeploymentOperationId>>,
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
) -> Result<Vec<DeploymentOperation>, Vec<DeploymentOperationId>> {
    let declared_dependencies = nodes.clone();
    let mut result = Vec::with_capacity(nodes.len());
    while !nodes.is_empty() {
        let Some(id) = nodes
            .iter()
            .filter(|(_, dependencies)| dependencies.is_empty())
            .map(|(id, _)| id.clone())
            .min_by_key(operation_sort_key)
        else {
            return Err(nodes.keys().cloned().collect());
        };
        let dependencies = nodes
            .remove(&id)
            .ok_or_else(|| nodes.keys().cloned().collect::<Vec<_>>())?;
        for remaining in nodes.values_mut() {
            remaining.remove(&id);
        }
        let declared = declared_dependencies.get(&id).cloned().unwrap_or(dependencies);
        let Some(resource_intent) = resources.get(id.resource()) else {
            return Err(vec![id]);
        };
        result.push(DeploymentOperation {
            resource_intent: (*resource_intent).clone(),
            image_pull_policy: match resources.get(id.resource()) {
                Some(DeploymentResource::Image(image)) if id.action() == SemanticOperationAction::EnsureImage => {
                    Some(image.pull_policy())
                }
                _ => None,
            },
            id,
            depends_on: declared.into_iter().collect(),
        });
    }
    Ok(result)
}

fn operation_cycle(
    edges: &BTreeMap<DeploymentOperationId, BTreeSet<DeploymentOperationId>>,
) -> Option<Vec<DeploymentOperationId>> {
    let mut remaining = edges.clone();
    while let Some(id) = remaining
        .iter()
        .filter(|(_, dependencies)| dependencies.is_empty())
        .map(|(id, _)| id.clone())
        .min_by_key(operation_sort_key)
    {
        remaining.remove(&id);
        for dependencies in remaining.values_mut() {
            dependencies.remove(&id);
        }
    }
    (!remaining.is_empty()).then(|| remaining.into_keys().collect())
}

fn require_kind(identity: &DeploymentResourceId, expected: ResourceKind) -> PodmanLensResult<()> {
    if identity.kind() == expected {
        Ok(())
    } else {
        Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent))
    }
}

fn resolved<'a>(
    resources: &'a BTreeMap<DeploymentResourceId, &DeploymentResource>,
    identity: &DeploymentResourceId,
    expected: ResourceKind,
) -> Option<&'a DeploymentResource> {
    (identity.kind() == expected)
        .then(|| resources.get(identity).copied())
        .flatten()
}

fn require_resolved(
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    reference: &DeploymentResourceId,
    expected: ResourceKind,
    subject: &DeploymentResourceId,
    field: &'static str,
    findings: &mut Vec<PlanningFinding>,
) {
    if resolved(resources, reference, expected).is_none() {
        findings.push(PlanningFinding::detailed(
            DiagnosticCode::DeploymentUnresolvedPrerequisite,
            Some(subject.clone()),
            vec![reference.clone()],
            Some(field),
            None,
        ));
    }
}

fn validate_distinct(
    values: &[DeploymentResourceId],
    subject: &DeploymentResourceId,
    field: &'static str,
    findings: &mut Vec<PlanningFinding>,
) {
    let mut distinct = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !distinct.insert(value) {
            findings.push(PlanningFinding::detailed(
                DiagnosticCode::DeploymentDuplicateResource,
                Some(subject.clone()),
                vec![value.clone()],
                Some(field),
                Some(index + 1),
            ));
        }
    }
}

fn is_managed(
    resources: &BTreeMap<DeploymentResourceId, &DeploymentResource>,
    identity: &DeploymentResourceId,
) -> bool {
    !matches!(
        resources.get(identity),
        Some(DeploymentResource::ExternalPrecondition(_))
    )
}

fn classify_image_source(value: &str) -> Option<ImageSourceClassification> {
    if value.is_empty()
        || value.len() > MAX_REFERENCE_BYTES
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || value.contains(['@', '\\']) && !value.contains("@sha256:")
        || value.contains("//")
        || value.contains('@') && value.matches('@').count() != 1
    {
        return None;
    }
    if is_image_id(value) {
        return Some(ImageSourceClassification::Local);
    }
    let (name, digest) = match value.split_once('@') {
        Some((name, digest)) if !name.is_empty() && is_sha256_digest(digest) => (name, Some(digest)),
        Some(_) => return None,
        None => (value, None),
    };
    let components = name.split('/').collect::<Vec<_>>();
    if components.iter().any(|component| component.is_empty()) {
        return None;
    }
    let first = components[0];
    let registry_qualified =
        components.len() > 1 && (first == "localhost" || first.contains('.') || first.contains(':'));
    let (registry, repository) = if registry_qualified {
        if components.len() < 2 || !is_valid_registry(first) {
            return None;
        }
        (Some(first), &components[1..])
    } else {
        (None, &components[..])
    };
    let (last, tag) = split_tag(repository.last()?);
    if !repository[..repository.len() - 1]
        .iter()
        .copied()
        .chain(std::iter::once(last))
        .all(is_repository_component)
    {
        return None;
    }
    if tag.is_some_and(|tag| !is_tag(tag)) {
        return None;
    }
    if digest.is_some() && tag.is_some() {
        return None;
    }
    match (registry, tag, digest) {
        (Some("localhost"), _, _) => Some(ImageSourceClassification::Local),
        (Some(_), Some(_), None) | (Some(_), None, Some(_)) => Some(ImageSourceClassification::Portable),
        (Some(_), None, None) => Some(ImageSourceClassification::Tagless),
        (None, _, _) => Some(ImageSourceClassification::Unqualified),
        _ => None,
    }
}

fn is_valid_registry(value: &str) -> bool {
    if value == "localhost" {
        return true;
    }
    let (host, port) = value.rsplit_once(':').unwrap_or((value, ""));
    !host.is_empty()
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
        && (port.is_empty() || (port.parse::<u16>().is_ok_and(|port| port != 0)))
}

fn is_image_id(value: &str) -> bool {
    let hex = value.strip_prefix("sha256:").unwrap_or(value);
    (hex.len() == 64 || (value.starts_with("sha256:") && hex.len() == 64))
        && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn split_tag(repository: &str) -> (&str, Option<&str>) {
    let Some((name, tag)) = repository.rsplit_once(':') else {
        return (repository, None);
    };
    if tag.is_empty() {
        (repository, None)
    } else {
        (name, Some(tag))
    }
}

fn is_repository_component(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_sha256_digest(value: &str) -> bool {
    value.len() == 71 && value.starts_with("sha256:") && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn operation_sort_key(id: &DeploymentOperationId) -> (u8, DeploymentResourceId) {
    let rank = match (id.action(), id.resource().kind()) {
        (SemanticOperationAction::Create, ResourceKind::Network) => 0,
        (SemanticOperationAction::Create, ResourceKind::Volume) => 1,
        (SemanticOperationAction::Create, ResourceKind::Secret) => 2,
        (SemanticOperationAction::EnsureImage, ResourceKind::Image) => 3,
        (SemanticOperationAction::Create, ResourceKind::Pod) => 4,
        (SemanticOperationAction::Create, ResourceKind::Container) => 5,
        (SemanticOperationAction::StartPod | SemanticOperationAction::StartContainer, _) => 6,
        _ => 7,
    };
    (rank, id.resource().clone())
}

fn sort_findings(findings: &mut Vec<PlanningFinding>) {
    findings.sort_unstable();
    findings.dedup();
}

fn validate_identifier(value: &str) -> PodmanLensResult<()> {
    if value.is_empty() || value.len() > MAX_REFERENCE_BYTES || value.chars().any(char::is_control) {
        Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent))
    } else {
        Ok(())
    }
}

fn validate_connection_name(value: &str) -> PodmanLensResult<()> {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
    };
    if value.len() > MAX_CONNECTION_NAME_BYTES
        || !first.is_ascii_alphanumeric()
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent))
    } else {
        Ok(())
    }
}
