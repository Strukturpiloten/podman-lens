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
    /// Creates a validated argument array. Empty arrays and empty individual arguments are valid.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for more than 128 arguments or arguments containing controls or exceeding
    /// 4096 bytes.
    pub fn new<I, S>(arguments: I) -> PodmanLensResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let arguments = arguments.into_iter().map(Into::into).collect::<Vec<String>>();
        if arguments.len() > MAX_ARGUMENTS
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

/// One named volume mounted at an exact normalized container destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedVolumeMount {
    source: DeploymentResourceId,
    destination: AbsoluteContainerPath,
    read_only: bool,
    copy_mode: NamedVolumeCopyMode,
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
        read_only: bool,
        copy_mode: NamedVolumeCopyMode,
    ) -> PodmanLensResult<Self> {
        if source.kind() != ResourceKind::Volume {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self {
            source,
            destination,
            read_only,
            copy_mode,
        })
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
        self.read_only
    }

    /// Returns image-content copy behavior.
    #[must_use]
    pub const fn copy_mode(&self) -> NamedVolumeCopyMode {
        self.copy_mode
    }
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
