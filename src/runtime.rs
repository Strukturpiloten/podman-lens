//! Bounded, redaction-safe native container runtime intent retained before rendering exists.

use std::fmt;

use crate::{Diagnostic, DiagnosticCode, Label, PodmanLensResult, SensitiveInputReference};

const MAX_ITEMS: usize = 64;
const MAX_BYTES: usize = 4096;

/// An explicitly caller-declassified health command string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicHealthCommand(String);

impl PublicHealthCommand {
    /// Creates one bounded, non-control public health command value.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an empty, oversized, or control-containing value.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if value.is_empty() || !valid_text(&value) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }

    /// Returns the public command value.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A health command value retained only for planning and never exposed by `Debug`.
#[derive(Clone, Eq, PartialEq)]
pub struct SensitiveInlineHealthCommand(String);

impl SensitiveInlineHealthCommand {
    /// Creates one bounded sensitive health command value.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an empty, oversized, or control-containing value.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        if value.is_empty() || !valid_text(&value) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }
}

impl fmt::Debug for SensitiveInlineHealthCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SensitiveInlineHealthCommand([redacted])")
    }
}

/// An explicitly caller-declassified direct health command array.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicHealthArgumentArray(Vec<String>);

impl PublicHealthArgumentArray {
    /// Creates a nonempty, bounded direct health command array.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for invalid array boundaries or unsafe arguments.
    pub fn new<I, S>(arguments: I) -> PodmanLensResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let arguments = arguments.into_iter().map(Into::into).collect::<Vec<_>>();
        validate_arguments(&arguments)?;
        Ok(Self(arguments))
    }

    /// Returns direct command arguments in declared order.
    #[must_use]
    pub fn values(&self) -> &[String] {
        &self.0
    }
}

/// A sensitive inline direct health command array that redacts every argument in `Debug`.
#[derive(Clone, Eq, PartialEq)]
pub struct SensitiveInlineHealthArgumentArray(Vec<String>);

impl SensitiveInlineHealthArgumentArray {
    /// Creates a nonempty, bounded sensitive direct health command array.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for invalid array boundaries or unsafe arguments.
    pub fn new<I, S>(arguments: I) -> PodmanLensResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let arguments = arguments.into_iter().map(Into::into).collect::<Vec<_>>();
        validate_arguments(&arguments)?;
        Ok(Self(arguments))
    }
}

impl fmt::Debug for SensitiveInlineHealthArgumentArray {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SensitiveInlineHealthArgumentArray([redacted])")
    }
}

/// One declared health command, preserving shell and exec syntax distinctions.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HealthCommand {
    /// A caller-authorized shell command.
    Shell(PublicHealthCommand),
    /// A caller-authorized direct executable and argument array.
    Exec(PublicHealthArgumentArray),
    /// An inline sensitive shell command, redacted in all debug output.
    SensitiveInlineShell(SensitiveInlineHealthCommand),
    /// An inline sensitive direct command, redacted in all debug output.
    SensitiveInlineExec(SensitiveInlineHealthArgumentArray),
    /// A caller-owned sensitive shell-command source, redacted in all debug output.
    ExternalShell(SensitiveInputReference),
    /// A caller-owned sensitive direct-command source, redacted in all debug output.
    ExternalExec(SensitiveInputReference),
}

/// A positive health duration encoded in bounded native signed nanoseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HealthDuration(i64);

impl HealthDuration {
    /// Creates one positive native duration.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for zero or values outside the native signed range.
    pub fn new(nanoseconds: u64) -> PodmanLensResult<Self> {
        if nanoseconds == 0 || nanoseconds > i64::MAX as u64 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(i64::try_from(nanoseconds).map_err(|_| {
            Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent)
        })?))
    }

    /// Returns nanoseconds.
    #[must_use]
    pub const fn nanoseconds(&self) -> i64 {
        self.0
    }
}

/// A bounded native normal-health retry count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HealthRetries(u32);

impl HealthRetries {
    /// Creates one retry count.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for zero or outside the native signed range.
    pub const fn new(value: u32) -> PodmanLensResult<Self> {
        if value == 0 || value > i32::MAX as u32 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }
    /// Returns the count.
    #[must_use]
    pub const fn value(&self) -> u32 {
        self.0
    }
}

/// A bounded startup-health retry count. Zero is valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupHealthRetries(u32);

impl StartupHealthRetries {
    /// Creates one startup retry count.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` outside the native signed range.
    pub const fn new(value: u32) -> PodmanLensResult<Self> {
        if value > i32::MAX as u32 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }
    /// Returns the count.
    #[must_use]
    pub const fn value(&self) -> u32 {
        self.0
    }
}

/// A bounded startup-health success threshold. Zero is valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupHealthSuccesses(u32);

impl StartupHealthSuccesses {
    /// Creates one threshold.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` outside the native signed range.
    pub const fn new(value: u32) -> PodmanLensResult<Self> {
        if value > i32::MAX as u32 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value))
    }
    /// Returns the count.
    #[must_use]
    pub const fn value(&self) -> u32 {
        self.0
    }
}

/// A health interval encoded as disabled native `0` or a positive duration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HealthInterval {
    /// Disables scheduled checks with native interval `0`.
    Disabled,
    /// Schedules checks at a positive interval.
    Every(HealthDuration),
}

/// A native health timeout of at least one second.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HealthTimeout(HealthDuration);

impl HealthTimeout {
    /// Creates one timeout of at least one second.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` below one second or outside the native signed range.
    pub fn new(nanoseconds: u64) -> PodmanLensResult<Self> {
        if nanoseconds < 1_000_000_000 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(HealthDuration::new(nanoseconds)?))
    }
    /// Returns nanoseconds.
    #[must_use]
    pub const fn nanoseconds(&self) -> i64 {
        self.0.nanoseconds()
    }
}

/// A normal health start period; native zero is valid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HealthStartPeriod(i64);

impl HealthStartPeriod {
    /// Creates one bounded start period.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` outside the native signed range.
    pub fn new(nanoseconds: u64) -> PodmanLensResult<Self> {
        if nanoseconds > i64::MAX as u64 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(i64::try_from(nanoseconds).map_err(|_| {
            Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent)
        })?))
    }
    /// Returns nanoseconds.
    #[must_use]
    pub const fn nanoseconds(&self) -> i64 {
        self.0
    }
}

/// The action after normal health failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HealthOnFailure {
    /// Retains an unhealthy state without stopping the container.
    None,
    /// Kills the container.
    Kill,
    /// Restarts the container.
    Restart,
    /// Stops the container.
    Stop,
}

/// A normal health command plus bounded timing and failure fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredHealthCheck {
    command: HealthCommand,
    interval: Option<HealthInterval>,
    timeout: Option<HealthTimeout>,
    retries: Option<HealthRetries>,
    start_period: Option<HealthStartPeriod>,
    on_failure: Option<HealthOnFailure>,
}

#[allow(clippy::missing_errors_doc)]
impl ConfiguredHealthCheck {
    /// Starts one normal health check.
    #[must_use]
    pub const fn new(command: HealthCommand) -> Self {
        Self {
            command,
            interval: None,
            timeout: None,
            retries: None,
            start_period: None,
            on_failure: None,
        }
    }
    /// Sets interval once.
    pub fn set_interval(&mut self, value: HealthInterval) -> PodmanLensResult<()> {
        set_once(&mut self.interval, value)
    }
    /// Sets timeout once.
    pub fn set_timeout(&mut self, value: HealthTimeout) -> PodmanLensResult<()> {
        set_once(&mut self.timeout, value)
    }
    /// Sets retries once.
    pub fn set_retries(&mut self, value: HealthRetries) -> PodmanLensResult<()> {
        set_once(&mut self.retries, value)
    }
    /// Sets start period once.
    pub fn set_start_period(&mut self, value: HealthStartPeriod) -> PodmanLensResult<()> {
        set_once(&mut self.start_period, value)
    }
    /// Sets the failure action once.
    pub fn set_on_failure(&mut self, value: HealthOnFailure) -> PodmanLensResult<()> {
        set_once(&mut self.on_failure, value)
    }
    /// Returns command.
    #[must_use]
    pub const fn command(&self) -> &HealthCommand {
        &self.command
    }
    /// Returns interval.
    #[must_use]
    pub const fn interval(&self) -> Option<HealthInterval> {
        self.interval
    }
    /// Returns timeout.
    #[must_use]
    pub const fn timeout(&self) -> Option<HealthTimeout> {
        self.timeout
    }
    /// Returns retries.
    #[must_use]
    pub const fn retries(&self) -> Option<HealthRetries> {
        self.retries
    }
    /// Returns start period.
    #[must_use]
    pub const fn start_period(&self) -> Option<HealthStartPeriod> {
        self.start_period
    }
    /// Returns failure action.
    #[must_use]
    pub const fn on_failure(&self) -> Option<HealthOnFailure> {
        self.on_failure
    }
}

/// Normal container health behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HealthCheck {
    /// Explicitly disables health checking.
    Disabled,
    /// Uses the specified command.
    Command(ConfiguredHealthCheck),
}

/// Startup-only health behavior; it cannot disable or replace normal health configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupHealthCheck {
    command: HealthCommand,
    interval: Option<HealthInterval>,
    timeout: Option<HealthTimeout>,
    retries: Option<StartupHealthRetries>,
    successes: Option<StartupHealthSuccesses>,
}

#[allow(clippy::missing_errors_doc)]
impl StartupHealthCheck {
    /// Creates startup health behavior from a command.
    #[must_use]
    pub const fn new(command: HealthCommand) -> Self {
        Self {
            command,
            interval: None,
            timeout: None,
            retries: None,
            successes: None,
        }
    }

    /// Sets interval once.
    pub fn set_interval(&mut self, value: HealthInterval) -> PodmanLensResult<()> {
        set_once(&mut self.interval, value)
    }
    /// Sets timeout once.
    pub fn set_timeout(&mut self, value: HealthTimeout) -> PodmanLensResult<()> {
        set_once(&mut self.timeout, value)
    }
    /// Sets retries once.
    pub fn set_retries(&mut self, value: StartupHealthRetries) -> PodmanLensResult<()> {
        set_once(&mut self.retries, value)
    }
    /// Sets success threshold once.
    pub fn set_successes(&mut self, value: StartupHealthSuccesses) -> PodmanLensResult<()> {
        set_once(&mut self.successes, value)
    }

    /// Returns the declared startup command.
    #[must_use]
    pub const fn command(&self) -> &HealthCommand {
        &self.command
    }

    /// Returns interval.
    #[must_use]
    pub const fn interval(&self) -> Option<HealthInterval> {
        self.interval
    }
    /// Returns timeout.
    #[must_use]
    pub const fn timeout(&self) -> Option<HealthTimeout> {
        self.timeout
    }
    /// Returns retries.
    #[must_use]
    pub const fn retries(&self) -> Option<StartupHealthRetries> {
        self.retries
    }
    /// Returns success threshold.
    #[must_use]
    pub const fn successes(&self) -> Option<StartupHealthSuccesses> {
        self.successes
    }
}

/// The bounded supported container logging drivers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LogDriver {
    /// Podman's journald integration.
    Journald,
    /// Podman's local file driver.
    K8sFile,
}

/// A bounded maximum log size in bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogSize(i64);

impl LogSize {
    /// Creates a non-zero maximum log size.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for zero values.
    pub fn new(bytes: u64) -> PodmanLensResult<Self> {
        if bytes == 0 || bytes > i64::MAX as u64 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(i64::try_from(bytes).map_err(|_| {
            Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent)
        })?))
    }

    /// Returns the exact byte limit.
    #[must_use]
    pub const fn bytes(&self) -> i64 {
        self.0
    }
}

/// Bounded logging intent. Journald labels retain caller-public key/value pairs.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LoggingSettings {
    driver: Option<LogDriver>,
    max_size: Option<LogSize>,
    journald_labels: Vec<Label>,
}

#[allow(clippy::missing_errors_doc)] // Every mutator has the same bounded duplicate/conflict contract.
impl LoggingSettings {
    /// Sets one driver, rejecting duplicate or conflicting assignments.
    pub fn set_driver(&mut self, driver: LogDriver) -> PodmanLensResult<()> {
        set_once(&mut self.driver, driver)
    }

    /// Sets one maximum size, rejecting duplicate or conflicting assignments.
    pub fn set_max_size(&mut self, size: LogSize) -> PodmanLensResult<()> {
        set_once(&mut self.max_size, size)
    }

    /// Adds one public journald label, rejecting duplicate keys.
    pub fn add_journald_label(&mut self, label: Label) -> PodmanLensResult<()> {
        if self.journald_labels.len() == MAX_ITEMS {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        if self
            .journald_labels
            .iter()
            .any(|existing| existing.key() == label.key())
        {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
        }
        self.journald_labels.push(label);
        Ok(())
    }

    /// Returns the selected driver.
    #[must_use]
    pub const fn driver(&self) -> Option<LogDriver> {
        self.driver
    }

    /// Returns the selected size limit.
    #[must_use]
    pub const fn max_size(&self) -> Option<LogSize> {
        self.max_size
    }

    /// Returns declared public journald labels in source order.
    #[must_use]
    pub fn journald_labels(&self) -> &[Label] {
        &self.journald_labels
    }
}

/// A bounded Linux capability name.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LinuxCapability(&'static str);

impl LinuxCapability {
    /// Creates an exact reviewed prefix-free capability name.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for `CAP_`-prefixed, unknown, or legacy spellings.
    pub fn new(value: &str) -> PodmanLensResult<Self> {
        CAPABILITIES
            .iter()
            .copied()
            .find(|known| *known == value)
            .map(Self)
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent))
    }

    /// Returns the canonical capability name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

const CAPABILITIES: [&str; 41] = [
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

/// Container security settings that do not depend on host paths or execution.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SecuritySettings {
    privileged: Option<bool>,
    no_new_privileges: Option<bool>,
    read_only_filesystem: Option<bool>,
    read_write_tmpfs: Option<bool>,
    cap_add: Vec<LinuxCapability>,
    cap_drop: Vec<LinuxCapability>,
}

#[allow(clippy::missing_errors_doc)] // Every collection mutator uses the same bounded duplicate contract.
impl SecuritySettings {
    /// Enables or disables privileged execution explicitly.
    pub fn set_privileged(&mut self, enabled: bool) -> PodmanLensResult<()> {
        set_once(&mut self.privileged, enabled)
    }

    /// Enables or disables no-new-privileges explicitly.
    pub fn set_no_new_privileges(&mut self, enabled: bool) -> PodmanLensResult<()> {
        set_once(&mut self.no_new_privileges, enabled)
    }

    /// Enables or disables a read-only container filesystem explicitly.
    pub fn set_read_only_filesystem(&mut self, enabled: bool) -> PodmanLensResult<()> {
        set_once(&mut self.read_only_filesystem, enabled)
    }

    /// Adds one capability, rejecting duplicates and bounded overflow.
    pub fn add_capability(&mut self, capability: LinuxCapability) -> PodmanLensResult<()> {
        add_distinct(&mut self.cap_add, capability)
    }

    /// Drops one capability, rejecting duplicates and bounded overflow.
    pub fn drop_capability(&mut self, capability: LinuxCapability) -> PodmanLensResult<()> {
        add_distinct(&mut self.cap_drop, capability)
    }

    /// Records explicit writable-tmpfs behavior.
    pub fn set_read_write_tmpfs(&mut self, enabled: bool) -> PodmanLensResult<()> {
        set_once(&mut self.read_write_tmpfs, enabled)
    }

    /// Returns privileged state.
    #[must_use]
    pub const fn privileged(&self) -> Option<bool> {
        self.privileged
    }
    /// Returns no-new-privileges state.
    #[must_use]
    pub const fn no_new_privileges(&self) -> Option<bool> {
        self.no_new_privileges
    }
    /// Returns read-only filesystem state.
    #[must_use]
    pub const fn read_only_filesystem(&self) -> Option<bool> {
        self.read_only_filesystem
    }
    /// Returns added capabilities.
    #[must_use]
    pub fn cap_add(&self) -> &[LinuxCapability] {
        &self.cap_add
    }
    /// Returns dropped capabilities.
    #[must_use]
    pub fn cap_drop(&self) -> &[LinuxCapability] {
        &self.cap_drop
    }
    /// Returns explicitly requested read-write-tmpfs behavior.
    #[must_use]
    pub const fn read_write_tmpfs(&self) -> Option<bool> {
        self.read_write_tmpfs
    }
}

/// One finite or explicitly unlimited resource limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RlimitValue {
    /// An exact finite limit; zero is valid.
    Finite(u64),
    /// An explicit unlimited limit, supported by semantic planning from Podman 5.6 onward.
    Unlimited,
}

impl RlimitValue {
    /// Creates an exact finite limit; zero is valid.
    #[must_use]
    pub const fn finite(value: u64) -> Self {
        Self::Finite(value)
    }
}

/// The bounded supported rlimit names.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RlimitKind {
    /// Maximum open file descriptors.
    NoFile,
    /// Maximum processes.
    NProc,
}

/// One rlimit pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rlimit {
    kind: RlimitKind,
    soft: RlimitValue,
    hard: RlimitValue,
}

#[allow(clippy::missing_errors_doc)] // Constructor documents the only ordering failure.
impl Rlimit {
    /// Creates one rlimit. Finite soft limits must not exceed finite hard limits.
    pub fn new(kind: RlimitKind, soft: RlimitValue, hard: RlimitValue) -> PodmanLensResult<Self> {
        if matches!((soft, hard), (RlimitValue::Unlimited, RlimitValue::Finite(_)))
            || matches!((soft, hard), (RlimitValue::Finite(soft), RlimitValue::Finite(hard)) if soft > hard)
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self { kind, soft, hard })
    }
    /// Returns rlimit kind.
    #[must_use]
    pub const fn kind(&self) -> RlimitKind {
        self.kind
    }
    /// Returns soft limit.
    #[must_use]
    pub const fn soft(&self) -> RlimitValue {
        self.soft
    }
    /// Returns hard limit.
    #[must_use]
    pub const fn hard(&self) -> RlimitValue {
        self.hard
    }
}

/// Bounded container CPU, memory, PID, and rlimit declarations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContainerResourceControls {
    cpu_shares: Option<i64>,
    cpu_period: Option<i64>,
    cpu_quota: Option<i64>,
    memory_bytes: Option<i64>,
    pids: Option<i64>,
    rlimits: Vec<Rlimit>,
}

#[allow(clippy::missing_errors_doc)] // Setters share the bounded duplicate/conflict contract.
impl ContainerResourceControls {
    /// Sets direct native CPU shares in the reviewed CFS range.
    pub fn set_cpu_shares(&mut self, value: u32) -> PodmanLensResult<()> {
        if !(2..=262_144).contains(&value) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        set_once(&mut self.cpu_shares, i64::from(value))
    }
    /// Sets direct native CPU period in the reviewed 1ms–1s range.
    pub fn set_cpu_period(&mut self, value: u64) -> PodmanLensResult<()> {
        if !(1_000..=1_000_000).contains(&value) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        let value = signed_positive(value)?;
        set_once(&mut self.cpu_period, value)
    }
    /// Sets a positive direct native CPU quota in microseconds, at least one millisecond.
    pub fn set_cpu_quota(&mut self, value: i64) -> PodmanLensResult<()> {
        if value < 1_000 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        set_once(&mut self.cpu_quota, value)
    }
    /// Sets positive memory bytes within the native signed range.
    pub fn set_memory_bytes(&mut self, value: u64) -> PodmanLensResult<()> {
        set_once(&mut self.memory_bytes, signed_positive(value)?)
    }
    /// Sets direct native PID limit; `-1` is unlimited.
    pub fn set_pids(&mut self, value: i64) -> PodmanLensResult<()> {
        if value == 0 || value < -1 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        set_once(&mut self.pids, value)
    }
    /// Adds rlimit, rejecting duplicate kinds and bounded overflow.
    pub fn add_rlimit(&mut self, value: Rlimit) -> PodmanLensResult<()> {
        if self.rlimits.len() == MAX_ITEMS {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        if self.rlimits.iter().any(|existing| existing.kind == value.kind) {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
        }
        self.rlimits.push(value);
        Ok(())
    }
    /// Returns direct native CPU shares.
    #[must_use]
    pub const fn cpu_shares(&self) -> Option<i64> {
        self.cpu_shares
    }
    /// Returns direct native CPU period.
    #[must_use]
    pub const fn cpu_period(&self) -> Option<i64> {
        self.cpu_period
    }
    /// Returns the positive direct native CPU quota in microseconds.
    #[must_use]
    pub const fn cpu_quota(&self) -> Option<i64> {
        self.cpu_quota
    }
    /// Returns memory bytes.
    #[must_use]
    pub const fn memory_bytes(&self) -> Option<i64> {
        self.memory_bytes
    }
    /// Returns PID limit.
    #[must_use]
    pub const fn pids(&self) -> Option<i64> {
        self.pids
    }
    /// Returns rlimits.
    #[must_use]
    pub fn rlimits(&self) -> &[Rlimit] {
        &self.rlimits
    }
}

/// One private or host namespace mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum NamespaceMode {
    /// Creates a private namespace.
    Private,
    /// Joins the host namespace.
    Host,
}

/// One IPC namespace mode for an unpodded container.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum IpcNamespaceMode {
    /// Creates a private IPC namespace.
    Private,
    /// Joins the host IPC namespace.
    Host,
    /// Makes a private IPC namespace shareable.
    Shareable,
    /// Disables IPC namespace use.
    None,
}

/// Explicit namespace intent for an unpodded container.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContainerNamespaceSettings {
    pid: Option<NamespaceMode>,
    ipc: Option<IpcNamespaceMode>,
    uts: Option<NamespaceMode>,
    cgroup: Option<NamespaceMode>,
}

#[allow(clippy::missing_errors_doc)]
impl ContainerNamespaceSettings {
    /// Sets PID namespace mode once.
    pub fn set_pid(&mut self, value: NamespaceMode) -> PodmanLensResult<()> {
        set_once(&mut self.pid, value)
    }
    /// Sets IPC namespace mode once.
    pub fn set_ipc(&mut self, value: IpcNamespaceMode) -> PodmanLensResult<()> {
        set_once(&mut self.ipc, value)
    }
    /// Sets UTS namespace mode once.
    pub fn set_uts(&mut self, value: NamespaceMode) -> PodmanLensResult<()> {
        set_once(&mut self.uts, value)
    }
    /// Sets cgroup namespace mode once.
    pub fn set_cgroup(&mut self, value: NamespaceMode) -> PodmanLensResult<()> {
        set_once(&mut self.cgroup, value)
    }
    /// Returns PID namespace mode.
    #[must_use]
    pub const fn pid(&self) -> Option<NamespaceMode> {
        self.pid
    }
    /// Returns IPC namespace mode.
    #[must_use]
    pub const fn ipc(&self) -> Option<IpcNamespaceMode> {
        self.ipc
    }
    /// Returns UTS namespace mode.
    #[must_use]
    pub const fn uts(&self) -> Option<NamespaceMode> {
        self.uts
    }
    /// Returns cgroup namespace mode.
    #[must_use]
    pub const fn cgroup(&self) -> Option<NamespaceMode> {
        self.cgroup
    }
    pub(crate) fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// Container-only bounded runtime intent consumed by version-aware renderers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContainerRuntimeSettings {
    health: Option<HealthCheck>,
    startup_health: Option<StartupHealthCheck>,
    logging: LoggingSettings,
    security: SecuritySettings,
    resources: ContainerResourceControls,
    namespaces: ContainerNamespaceSettings,
}

#[allow(clippy::missing_errors_doc)] // Health setters share the bounded duplicate/conflict contract.
impl ContainerRuntimeSettings {
    /// Sets normal health behavior, rejecting duplicate or conflicting assignments.
    pub fn set_health(&mut self, value: HealthCheck) -> PodmanLensResult<()> {
        set_once(&mut self.health, value)
    }
    /// Sets startup health behavior, rejecting duplicate or conflicting assignments.
    pub fn set_startup_health(&mut self, value: StartupHealthCheck) -> PodmanLensResult<()> {
        set_once(&mut self.startup_health, value)
    }
    /// Returns normal health behavior.
    #[must_use]
    pub fn health(&self) -> Option<&HealthCheck> {
        self.health.as_ref()
    }
    /// Returns startup health behavior.
    #[must_use]
    pub fn startup_health(&self) -> Option<&StartupHealthCheck> {
        self.startup_health.as_ref()
    }
    /// Returns logging settings.
    #[must_use]
    pub fn logging(&self) -> &LoggingSettings {
        &self.logging
    }
    /// Returns mutable logging settings.
    #[must_use]
    pub fn logging_mut(&mut self) -> &mut LoggingSettings {
        &mut self.logging
    }
    /// Returns security settings.
    #[must_use]
    pub fn security(&self) -> &SecuritySettings {
        &self.security
    }
    /// Returns mutable security settings.
    #[must_use]
    pub fn security_mut(&mut self) -> &mut SecuritySettings {
        &mut self.security
    }
    /// Returns resource controls.
    #[must_use]
    pub fn resources(&self) -> &ContainerResourceControls {
        &self.resources
    }
    /// Returns mutable resource controls.
    #[must_use]
    pub fn resources_mut(&mut self) -> &mut ContainerResourceControls {
        &mut self.resources
    }
    /// Returns namespace settings.
    #[must_use]
    pub fn namespaces(&self) -> &ContainerNamespaceSettings {
        &self.namespaces
    }
    /// Returns mutable namespace settings.
    #[must_use]
    pub fn namespaces_mut(&mut self) -> &mut ContainerNamespaceSettings {
        &mut self.namespaces
    }
}

fn set_once<T: Eq>(slot: &mut Option<T>, value: T) -> PodmanLensResult<()> {
    match slot {
        None => {
            *slot = Some(value);
            Ok(())
        }
        Some(existing) if *existing == value => Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource)),
        Some(_) => Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination)),
    }
}
fn add_distinct<T: Eq>(values: &mut Vec<T>, value: T) -> PodmanLensResult<()> {
    if values.len() == MAX_ITEMS {
        return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
    }
    if values.contains(&value) {
        return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
    }
    values.push(value);
    Ok(())
}
fn signed_positive(value: u64) -> PodmanLensResult<i64> {
    if value == 0 || value > i64::MAX as u64 {
        return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
    }
    i64::try_from(value).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent))
}
fn validate_arguments(values: &[String]) -> PodmanLensResult<()> {
    if values.is_empty()
        || values.len() > MAX_ITEMS
        || values.first().is_some_and(String::is_empty)
        || values.iter().any(|value| !valid_text(value))
    {
        return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
    }
    Ok(())
}
fn valid_text(value: &str) -> bool {
    value.len() <= MAX_BYTES && !value.chars().any(char::is_control)
}
