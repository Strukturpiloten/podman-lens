//! Typed, bounded deployment settings that do not expose raw configuration strings by default.

use std::fmt;

use crate::{
    DeploymentResourceId, Diagnostic, DiagnosticCode, PodmanLensResult, ResourceKind, SensitiveInputReference,
};

const MAX_ARGUMENTS: usize = 128;
const MAX_ARGUMENT_BYTES: usize = 4096;
const MAX_VALUE_BYTES: usize = 4096;
const MAX_LABELS: usize = 128;
const MAX_ENVIRONMENT: usize = 128;
const MAX_PATH_BYTES: usize = 4096;

/// An ordered, bounded command or entrypoint argument array.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentArray(Vec<String>);

impl ArgumentArray {
    /// Creates a validated nonempty argument array. Empty individual arguments are valid.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an empty array, more than 128 arguments, or arguments containing
    /// controls or exceeding 4096 bytes.
    pub fn new<I, S>(arguments: I) -> PodmanLensResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let arguments = arguments.into_iter().map(Into::into).collect::<Vec<String>>();
        if arguments.is_empty()
            || arguments.len() > MAX_ARGUMENTS
            || arguments
                .iter()
                .any(|argument| !valid_non_control(argument, MAX_ARGUMENT_BYTES))
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(arguments))
    }

    /// Returns arguments in their declared order.
    #[must_use]
    pub fn values(&self) -> &[String] {
        &self.0
    }
}

/// A bounded `user` or `user:group` spelling passed to a container runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerUser(String);

impl ContainerUser {
    /// Creates a user setting from a safe runtime spelling.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an empty, oversized, unsafe, or malformed component list. Exactly
    /// one nonempty user or UID component is required; one nonempty group or GID component may
    /// follow after a single colon.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        let mut components = value.split(':');
        let Some(user) = components.next() else {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        };
        let group = components.next();
        if value.len() > MAX_VALUE_BYTES
            || !valid_user_component(user)
            || group.is_some_and(|component| !valid_user_component(component))
            || components.next().is_some()
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the validated user spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A normalized absolute path inside a container namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbsoluteContainerPath(String);

impl AbsoluteContainerPath {
    /// Creates an absolute normalized container path.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` unless the path begins with `/`, contains no empty, `.` or `..`
    /// components (except the root path), controls, backslashes, or more than 4096 bytes.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if !is_absolute_normalized_path(&value) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the normalized absolute path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A container working directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerWorkdir(AbsoluteContainerPath);

impl ContainerWorkdir {
    /// Creates a working directory from one normalized absolute container path.
    #[must_use]
    pub const fn new(path: AbsoluteContainerPath) -> Self {
        Self(path)
    }

    /// Returns the normalized working-directory path.
    #[must_use]
    pub fn path(&self) -> &AbsoluteContainerPath {
        &self.0
    }
}

/// A bounded RFC-style container hostname.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerHostname(String);

impl ContainerHostname {
    /// Creates a hostname from ASCII labels separated by dots.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an empty, oversized, or malformed hostname.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 253
            || value.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            })
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the validated hostname.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A label key retained in insertion order by [`ContainerSettings`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LabelKey(String);

impl LabelKey {
    /// Creates a bounded label key.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for empty, oversized, control-containing, or `=`-containing values.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if value.is_empty() || !valid_non_control(&value, MAX_VALUE_BYTES) || value.contains('=') {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the validated key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An explicitly caller-authorized public label value. Empty values are valid.
///
/// Constructing this type is an explicit declassification decision: it authorizes this value for
/// deployment artifacts, CLI arguments, Libpod JSON, and shell-review output once the matching
/// renderer exists. Do not construct it from observed sensitive runtime values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicLabelValue(String);

impl PublicLabelValue {
    /// Creates a bounded explicitly public label value.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for oversized or control-containing values.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if !valid_value(&value) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the validated value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One ordered container label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    key: LabelKey,
    value: PublicLabelValue,
}

impl Label {
    /// Creates one label from validated key and value types.
    #[must_use]
    pub const fn new(key: LabelKey, value: PublicLabelValue) -> Self {
        Self { key, value }
    }

    /// Returns the label key.
    #[must_use]
    pub fn key(&self) -> &LabelKey {
        &self.key
    }

    /// Returns the label value.
    #[must_use]
    pub fn value(&self) -> &PublicLabelValue {
        &self.value
    }
}

/// A validated environment-variable name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentName(String);

impl EnvironmentName {
    /// Creates a POSIX-style environment-variable name.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an empty, oversized, or non-POSIX identifier.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        let mut bytes = value.bytes();
        let Some(first) = bytes.next() else {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        };
        if value.len() > 256
            || !(first.is_ascii_alphabetic() || first == b'_')
            || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the environment-variable name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An explicitly caller-authorized public environment value. Empty values are valid.
///
/// Constructing this type is an explicit declassification decision: it authorizes this value for
/// deployment artifacts, CLI arguments, Libpod JSON, and shell-review output once the matching
/// renderer exists. Do not construct it from observed sensitive runtime values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicEnvironmentValue(String);

impl PublicEnvironmentValue {
    /// Creates one bounded explicitly public environment value.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for oversized or control-containing values.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if !valid_value(&value) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the plain value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An inline environment value that must never be exposed by diagnostics or debug output.
#[derive(Clone, Eq, PartialEq)]
pub struct SensitiveInlineEnvironmentValue(String);

impl SensitiveInlineEnvironmentValue {
    /// Creates a bounded sensitive inline value. Empty values are valid.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for oversized or control-containing values.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if !valid_value(&value) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }
}

impl fmt::Debug for SensitiveInlineEnvironmentValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SensitiveInlineEnvironmentValue([redacted])")
    }
}

/// The source of an environment value.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DeploymentEnvironmentValue {
    /// An explicitly caller-authorized public, directly declared value.
    Public(PublicEnvironmentValue),
    /// A directly declared sensitive value that remains redacted.
    SensitiveInline(SensitiveInlineEnvironmentValue),
    /// A sensitive value supplied by a caller-owned external input.
    External(SensitiveInputReference),
}

/// One ordered environment assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentAssignment {
    name: EnvironmentName,
    value: DeploymentEnvironmentValue,
}

impl EnvironmentAssignment {
    /// Creates one typed environment assignment.
    #[must_use]
    pub const fn new(name: EnvironmentName, value: DeploymentEnvironmentValue) -> Self {
        Self { name, value }
    }

    /// Returns the variable name.
    #[must_use]
    pub fn name(&self) -> &EnvironmentName {
        &self.name
    }

    /// Returns the typed value source.
    #[must_use]
    pub fn value(&self) -> &DeploymentEnvironmentValue {
        &self.value
    }
}

/// A supported container restart policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RestartPolicy {
    /// Do not automatically restart the container.
    No,
    /// Restart only after a non-zero exit status.
    OnFailure,
    /// Always restart the container.
    Always,
    /// Restart unless the user explicitly stopped the container.
    UnlessStopped,
}

/// Copy behavior for a named volume initialized by a container image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NamedVolumeCopyMode {
    /// Copy image content into a newly created volume when the runtime supports it.
    Copy,
    /// Do not copy image content into the mounted volume.
    NoCopy,
}

/// Explicit read/write access for one typed mount.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MountAccess {
    /// The mounted path is writable from the container.
    ReadWrite,
    /// The mounted path is read-only from the container.
    ReadOnly,
}

impl MountAccess {
    /// Returns whether the access declaration is read-only.
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        matches!(self, Self::ReadOnly)
    }
}

/// A normalized absolute path rooted at the named volume root.
///
/// Podman's native `SubPath` is absolute relative to a volume root, not relative to the container
/// filesystem. Empty, relative, `.` and `..` components are rejected before a renderer can build
/// a native mount representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeSubpath(String);

impl VolumeSubpath {
    /// Creates one normalized absolute volume-root subpath.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an empty, relative, unsafe, or non-normalized spelling.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if value.len() > MAX_PATH_BYTES
            || !value.starts_with('/')
            || value.contains('\\')
            || value.chars().any(char::is_control)
            || value
                .split('/')
                .skip(1)
                .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the validated absolute volume-root subpath.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One named volume mounted at an exact normalized container destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedVolumeMount {
    source: DeploymentResourceId,
    destination: AbsoluteContainerPath,
    access: MountAccess,
    copy_mode: NamedVolumeCopyMode,
    subpath: Option<VolumeSubpath>,
}

impl NamedVolumeMount {
    /// Creates one named-volume mount.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when `source` is not a volume identity.
    pub fn new(
        source: DeploymentResourceId,
        destination: AbsoluteContainerPath,
        access: MountAccess,
        copy_mode: NamedVolumeCopyMode,
    ) -> PodmanLensResult<Self> {
        if source.kind() != ResourceKind::Volume {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self {
            source,
            destination,
            access,
            copy_mode,
            subpath: None,
        })
    }

    /// Adds one volume-root-relative subpath.
    ///
    /// Podman's dual CLI/API representation is exact only with normal copy behavior. `nocopy`
    /// plus `subpath` is deliberately rejected rather than silently changing initialization.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` for a repeated subpath or for `NoCopy`.
    pub fn set_subpath(&mut self, subpath: VolumeSubpath) -> PodmanLensResult<()> {
        if self.subpath.is_some() || self.copy_mode == NamedVolumeCopyMode::NoCopy {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
        }
        self.subpath = Some(subpath);
        Ok(())
    }

    /// Returns the named-volume prerequisite identity.
    #[must_use]
    pub fn source(&self) -> &DeploymentResourceId {
        &self.source
    }

    /// Returns the normalized mount destination.
    #[must_use]
    pub fn destination(&self) -> &AbsoluteContainerPath {
        &self.destination
    }

    /// Returns whether the mount is read-only.
    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        self.access.is_read_only()
    }

    /// Returns the explicit mount access mode.
    #[must_use]
    pub const fn access(&self) -> MountAccess {
        self.access
    }

    /// Returns image-content copy behavior.
    #[must_use]
    pub const fn copy_mode(&self) -> NamedVolumeCopyMode {
        self.copy_mode
    }

    /// Returns the optional volume-root-relative source subpath.
    #[must_use]
    pub fn subpath(&self) -> Option<&VolumeSubpath> {
        self.subpath.as_ref()
    }
}

/// One host bind mount at an exact normalized container destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindMount {
    source: AbsoluteContainerPath,
    destination: AbsoluteContainerPath,
    access: MountAccess,
}

impl BindMount {
    /// Creates one normalized host-path bind mount.
    #[must_use]
    pub const fn new(source: AbsoluteContainerPath, destination: AbsoluteContainerPath, access: MountAccess) -> Self {
        Self {
            source,
            destination,
            access,
        }
    }

    /// Returns the normalized absolute host source path.
    #[must_use]
    pub fn source(&self) -> &AbsoluteContainerPath {
        &self.source
    }

    /// Returns the normalized container destination path.
    #[must_use]
    pub fn destination(&self) -> &AbsoluteContainerPath {
        &self.destination
    }

    /// Returns the declared access mode.
    #[must_use]
    pub const fn access(&self) -> MountAccess {
        self.access
    }
}

/// One tmpfs mount at an exact normalized container destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TmpfsMount {
    destination: AbsoluteContainerPath,
    access: MountAccess,
}

impl TmpfsMount {
    /// Creates one tmpfs mount.
    #[must_use]
    pub const fn new(destination: AbsoluteContainerPath, access: MountAccess) -> Self {
        Self { destination, access }
    }

    /// Returns the normalized container destination path.
    #[must_use]
    pub fn destination(&self) -> &AbsoluteContainerPath {
        &self.destination
    }

    /// Returns the declared access mode.
    #[must_use]
    pub const fn access(&self) -> MountAccess {
        self.access
    }
}

/// A typed exact container mount. Raw `--volume` spelling is deliberately not public API.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MountIntent {
    /// A named Podman volume.
    NamedVolume(NamedVolumeMount),
    /// A host filesystem bind mount.
    Bind(BindMount),
    /// An in-memory tmpfs mount.
    Tmpfs(TmpfsMount),
}

impl From<NamedVolumeMount> for MountIntent {
    fn from(mount: NamedVolumeMount) -> Self {
        Self::NamedVolume(mount)
    }
}

impl From<BindMount> for MountIntent {
    fn from(mount: BindMount) -> Self {
        Self::Bind(mount)
    }
}

impl From<TmpfsMount> for MountIntent {
    fn from(mount: TmpfsMount) -> Self {
        Self::Tmpfs(mount)
    }
}

impl MountIntent {
    /// Returns the normalized container destination shared by all mount forms.
    #[must_use]
    pub fn destination(&self) -> &AbsoluteContainerPath {
        match self {
            Self::NamedVolume(mount) => mount.destination(),
            Self::Bind(mount) => mount.destination(),
            Self::Tmpfs(mount) => mount.destination(),
        }
    }

    /// Returns the named-volume source identity when this is a named-volume mount.
    #[must_use]
    pub fn volume_source(&self) -> Option<&DeploymentResourceId> {
        match self {
            Self::NamedVolume(mount) => Some(mount.source()),
            Self::Bind(_) | Self::Tmpfs(_) => None,
        }
    }
}

/// A bounded Unix ownership value for a volume, mount, or secret declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnixId(u32);

impl UnixId {
    /// Creates one ownership value in Podman's conservative signed 32-bit range.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for values greater than `i32::MAX`.
    pub fn new(value: u32) -> PodmanLensResult<Self> {
        if value > i32::MAX as u32 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the native numeric ownership value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A bounded Unix file mode for a mounted secret.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecretMode(u16);

impl SecretMode {
    /// Creates an ordinary Unix permission mode (`0o000` through `0o777`).
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for bits outside the portable permission range.
    pub fn new(value: u16) -> PodmanLensResult<Self> {
        if value > 0o777 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the numeric Unix mode.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// A typed secret attachment to a container mount or environment name.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SecretGrant {
    /// Mount a secret. An omitted target uses Podman's native secret-name destination.
    Mount {
        /// The declared managed or external secret identity.
        source: DeploymentResourceId,
        /// The optional target path.
        target: Option<AbsoluteContainerPath>,
        /// Optional mount UID.
        uid: Option<UnixId>,
        /// Optional mount GID.
        gid: Option<UnixId>,
        /// Optional mount mode.
        mode: Option<SecretMode>,
    },
    /// Inject a secret into one exact environment variable name.
    Environment {
        /// The declared managed or external secret identity.
        source: DeploymentResourceId,
        /// The target environment name.
        target: EnvironmentName,
    },
}

impl SecretGrant {
    /// Creates one mount-form secret grant with Podman's default target and mode.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when `source` is not a secret identity.
    pub fn mount(source: DeploymentResourceId) -> PodmanLensResult<Self> {
        if source.kind() != ResourceKind::Secret {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self::Mount {
            source,
            target: None,
            uid: None,
            gid: None,
            mode: None,
        })
    }

    /// Creates one environment-form secret grant.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when `source` is not a secret identity.
    pub fn environment(source: DeploymentResourceId, target: EnvironmentName) -> PodmanLensResult<Self> {
        if source.kind() != ResourceKind::Secret {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self::Environment { source, target })
    }

    /// Sets the optional mount target. Environment grants reject this operation.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` for an environment grant or a repeated mount target.
    pub fn set_mount_target(&mut self, target: AbsoluteContainerPath) -> PodmanLensResult<()> {
        match self {
            Self::Mount { target: slot, .. } if slot.is_none() => {
                *slot = Some(target);
                Ok(())
            }
            Self::Mount { .. } | Self::Environment { .. } => {
                Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination))
            }
        }
    }

    /// Sets one optional mount UID. Environment grants reject this operation.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` for an environment grant or a repeated UID.
    pub fn set_mount_uid(&mut self, uid: UnixId) -> PodmanLensResult<()> {
        set_secret_mount_option(self, uid, |grant| match grant {
            Self::Mount { uid, .. } => uid,
            Self::Environment { .. } => unreachable!("environment grants are rejected before access"),
        })
    }

    /// Sets one optional mount GID. Environment grants reject this operation.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` for an environment grant or a repeated GID.
    pub fn set_mount_gid(&mut self, gid: UnixId) -> PodmanLensResult<()> {
        set_secret_mount_option(self, gid, |grant| match grant {
            Self::Mount { gid, .. } => gid,
            Self::Environment { .. } => unreachable!("environment grants are rejected before access"),
        })
    }

    /// Sets one optional mount file mode. Environment grants reject this operation.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` for an environment grant or a repeated mode.
    pub fn set_mount_mode(&mut self, mode: SecretMode) -> PodmanLensResult<()> {
        set_secret_mount_option(self, mode, |grant| match grant {
            Self::Mount { mode, .. } => mode,
            Self::Environment { .. } => unreachable!("environment grants are rejected before access"),
        })
    }

    /// Returns the referenced secret identity.
    #[must_use]
    pub fn source(&self) -> &DeploymentResourceId {
        match self {
            Self::Mount { source, .. } | Self::Environment { source, .. } => source,
        }
    }

    /// Returns the mount target, when this is a mount grant and a target was explicitly set.
    #[must_use]
    pub fn mount_target(&self) -> Option<&AbsoluteContainerPath> {
        match self {
            Self::Mount { target, .. } => target.as_ref(),
            Self::Environment { .. } => None,
        }
    }

    /// Returns the environment target when this is an environment grant.
    #[must_use]
    pub fn environment_target(&self) -> Option<&EnvironmentName> {
        match self {
            Self::Environment { target, .. } => Some(target),
            Self::Mount { .. } => None,
        }
    }

    /// Returns optional mount UID.
    #[must_use]
    pub fn mount_uid(&self) -> Option<UnixId> {
        match self {
            Self::Mount { uid, .. } => *uid,
            Self::Environment { .. } => None,
        }
    }

    /// Returns optional mount GID.
    #[must_use]
    pub fn mount_gid(&self) -> Option<UnixId> {
        match self {
            Self::Mount { gid, .. } => *gid,
            Self::Environment { .. } => None,
        }
    }

    /// Returns optional mount mode.
    #[must_use]
    pub fn mount_mode(&self) -> Option<SecretMode> {
        match self {
            Self::Mount { mode, .. } => *mode,
            Self::Environment { .. } => None,
        }
    }
}

fn set_secret_mount_option<T: Eq>(
    grant: &mut SecretGrant,
    value: T,
    member: impl FnOnce(&mut SecretGrant) -> &mut Option<T>,
) -> PodmanLensResult<()> {
    if !matches!(grant, SecretGrant::Mount { .. }) {
        return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
    }
    let slot = member(grant);
    if slot.is_some() {
        return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
    }
    *slot = Some(value);
    Ok(())
}

/// Typed optional container settings retained separately from topology.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContainerSettings {
    command: Option<ArgumentArray>,
    entrypoint: Option<ArgumentArray>,
    user: Option<ContainerUser>,
    workdir: Option<ContainerWorkdir>,
    hostname: Option<ContainerHostname>,
    labels: Vec<Label>,
    environment: Vec<EnvironmentAssignment>,
    restart_policy: Option<RestartPolicy>,
}

impl ContainerSettings {
    /// Assigns one command array, rejecting repeated or conflicting assignments.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for an identical repeat and `PLN0038` for a conflict.
    pub fn set_command(&mut self, command: ArgumentArray) -> PodmanLensResult<()> {
        set_once(&mut self.command, command)
    }

    /// Assigns one entrypoint array, rejecting repeated or conflicting assignments.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for an identical repeat and `PLN0038` for a conflict.
    pub fn set_entrypoint(&mut self, entrypoint: ArgumentArray) -> PodmanLensResult<()> {
        set_once(&mut self.entrypoint, entrypoint)
    }

    /// Assigns one user, rejecting repeated or conflicting assignments.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for an identical repeat and `PLN0038` for a conflict.
    pub fn set_user(&mut self, user: ContainerUser) -> PodmanLensResult<()> {
        set_once(&mut self.user, user)
    }

    /// Assigns one working directory, rejecting repeated or conflicting assignments.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for an identical repeat and `PLN0038` for a conflict.
    pub fn set_workdir(&mut self, workdir: ContainerWorkdir) -> PodmanLensResult<()> {
        set_once(&mut self.workdir, workdir)
    }

    /// Assigns one hostname, rejecting repeated or conflicting assignments.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for an identical repeat and `PLN0038` for a conflict.
    pub fn set_hostname(&mut self, hostname: ContainerHostname) -> PodmanLensResult<()> {
        set_once(&mut self.hostname, hostname)
    }

    /// Adds one label while preserving declared order.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for a duplicate key and `PLN0034` when the bounded collection is full.
    pub fn add_label(&mut self, label: Label) -> PodmanLensResult<()> {
        if self.labels.len() == MAX_LABELS {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        if self.labels.iter().any(|existing| existing.key == label.key) {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
        }
        self.labels.push(label);
        Ok(())
    }

    /// Adds one environment assignment while preserving declared order.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for a duplicate name and `PLN0034` when the bounded collection is full.
    pub fn add_environment(&mut self, assignment: EnvironmentAssignment) -> PodmanLensResult<()> {
        if self.environment.len() == MAX_ENVIRONMENT {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        if self.environment.iter().any(|existing| existing.name == assignment.name) {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
        }
        self.environment.push(assignment);
        Ok(())
    }

    /// Assigns one restart policy, rejecting repeated or conflicting assignments.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for an identical repeat and `PLN0038` for a conflict.
    pub fn set_restart_policy(&mut self, restart_policy: RestartPolicy) -> PodmanLensResult<()> {
        set_once(&mut self.restart_policy, restart_policy)
    }

    /// Returns the optional command.
    #[must_use]
    pub fn command(&self) -> Option<&ArgumentArray> {
        self.command.as_ref()
    }

    /// Returns the optional entrypoint.
    #[must_use]
    pub fn entrypoint(&self) -> Option<&ArgumentArray> {
        self.entrypoint.as_ref()
    }

    /// Returns the optional user.
    #[must_use]
    pub fn user(&self) -> Option<&ContainerUser> {
        self.user.as_ref()
    }

    /// Returns the optional working directory.
    #[must_use]
    pub fn workdir(&self) -> Option<&ContainerWorkdir> {
        self.workdir.as_ref()
    }

    /// Returns the optional hostname.
    #[must_use]
    pub fn hostname(&self) -> Option<&ContainerHostname> {
        self.hostname.as_ref()
    }

    /// Returns labels in declared order.
    #[must_use]
    pub fn labels(&self) -> &[Label] {
        &self.labels
    }

    /// Returns environment assignments in declared order.
    #[must_use]
    pub fn environment(&self) -> &[EnvironmentAssignment] {
        &self.environment
    }

    /// Returns the optional restart policy.
    #[must_use]
    pub const fn restart_policy(&self) -> Option<RestartPolicy> {
        self.restart_policy
    }
}

fn set_once<T: Eq>(slot: &mut Option<T>, value: T) -> PodmanLensResult<()> {
    match slot {
        None => {
            *slot = Some(value);
            Ok(())
        }
        Some(existing) if existing == &value => Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource)),
        Some(_) => Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination)),
    }
}

fn valid_value(value: &str) -> bool {
    valid_non_control(value, MAX_VALUE_BYTES)
}

fn valid_non_control(value: &str, maximum_bytes: usize) -> bool {
    value.len() <= maximum_bytes && !value.chars().any(char::is_control)
}

fn valid_user_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_absolute_normalized_path(value: &str) -> bool {
    value.len() <= MAX_PATH_BYTES
        && value.starts_with('/')
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && (value == "/"
            || value
                .split('/')
                .skip(1)
                .all(|component| !component.is_empty() && component != "." && component != ".."))
}
