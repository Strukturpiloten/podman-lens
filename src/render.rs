//! Version-aware, transport-neutral deployment-plan rendering.
//!
//! This M6-A boundary turns validated M5 semantics into reviewable CLI and Libpod request
//! descriptions. It never opens a connection, sends a request, or serializes secret material.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt::Write as _,
};

use semver::Version;
use serde::{Deserialize, de};
use serde_json::{Map, Value, json};

use crate::settings::{MountIntent, SecretGrant, SecretMode};
use crate::{
    DeploymentConnectionReference, DeploymentOperation, DeploymentPlan, DeploymentResource, DeploymentResourceId,
    Diagnostic, DiagnosticCode, ExternalPrecondition, HostAlias, NetworkAttachment, NetworkIntent, NetworkRoute,
    PortMapping, PortProtocol, ResourceKind, RestartPolicy, RouteType, SensitiveInputReference,
};

const RENDERING_CATALOGUE_JSON: &str = include_str!("../catalogue/v1/podman-deployment-rendering.json");
const RENDERED_OPERATION_CATEGORIES: [&str; 8] = [
    "network-create",
    "volume-create",
    "secret-create",
    "image-pull",
    "pod-create",
    "container-create",
    "pod-start",
    "container-start",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderingCatalogue {
    schema_version: u8,
    provenance: String,
    runtime_field_claims: Vec<RuntimeFieldClaimJson>,
    reviewed_lines: Vec<ReviewedRenderingLine>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedRenderingLine {
    version: String,
    revision: String,
    tag: String,
    common_module: ModulePin,
    b4_evidence: ReviewedB4Evidence,
    runtime_evidence: ReviewedRuntimeEvidence,
    operations: Vec<ReviewedRenderingOperation>,
    field_evidence: Vec<ReviewedFieldEvidence>,
}

/// Immutable per-release evidence for the bounded M6-B4 mount, secret, volume, and image
/// surface.  It deliberately records blocked and manual boundaries beside exact fields so a
/// future renderer cannot turn a known one-plane-only or portability-sensitive spelling into an
/// accidental exact claim.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedB4Evidence {
    exact_fields: Vec<B4RenderedField>,
    target_gated_fields: Vec<B4RenderedField>,
    manual_fields: Vec<B4RenderedField>,
    blocked_fields: Vec<B4RenderedField>,
    cli_sources: Vec<SourceReference>,
    model_sources: Vec<SourceReference>,
    handler_sources: Vec<SourceReference>,
}

/// Immutable all-fields evidence for the bounded M6-B3 container runtime surface.
///
/// The fields share the native container-create route and handler, while the explicit source
/// bundle retains the CLI flag declaration, CLI-to-SpecGenerator transform, body model,
/// namespace grammar, and health semantics for the exact reviewed Podman revision.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedRuntimeEvidence {
    exact_fields: Vec<RuntimeRenderedField>,
    target_gated_fields: Vec<RuntimeRenderedField>,
    cli_flag_source: SourceReference,
    cli_transform_source: SourceReference,
    command_route_source: SourceReference,
    model_sources: Vec<SourceReference>,
    route_source: SourceReference,
    handler_source: SourceReference,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeFieldClaimJson {
    field: RuntimeRenderedField,
    cli: CliFieldClaim,
    libpod: RuntimeLibpodFieldClaim,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeLibpodFieldClaim {
    json_member: LibpodBodyMember,
    value_shape: RuntimeLibpodValueShape,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedRenderingOperation {
    category: RenderingOperationCategory,
    cli_source: SourceReference,
    libpod_endpoint_source: SourceReference,
    body_source: Option<SourceReference>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedFieldEvidence {
    field: RenderedField,
    availability: FieldAvailability,
    operation: RenderingOperationCategory,
    cli: CliFieldClaim,
    libpod: LibpodFieldClaim,
    cli_source: SourceReference,
    model_sources: Vec<SourceReference>,
    handler_source: SourceReference,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum FieldAvailability {
    Exact,
    Unsupported,
}

/// One immutable source location backing a rendering claim.
///
/// URLs are deliberately not accepted here: they obscure repository ownership and permit a
/// revision from one repository to be paired with a path from another. The parser validates this
/// structured repository, revision, path, and module information against the canonical field
/// matrix.
#[derive(Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct SourceReference {
    repository: SourceRepository,
    revision: String,
    path: String,
    module: Option<ModulePin>,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum SourceRepository {
    ContainersPodman,
    ContainersCommon,
    ContainersContainerLibs,
    PodmanContainerToolsContainerLibs,
}

#[derive(Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
struct ModulePin {
    repository: SourceRepository,
    path: String,
    version: String,
    revision: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CliFieldClaim {
    flag: Option<CliFlag>,
    value_shape: CliValueShape,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LibpodFieldClaim {
    json_member: LibpodBodyMember,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum RenderingOperationCategory {
    NetworkCreate,
    VolumeCreate,
    SecretCreate,
    ImagePull,
    PodCreate,
    ContainerCreate,
    PodStart,
    ContainerStart,
}

impl RenderingOperationCategory {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NetworkCreate => "network-create",
            Self::VolumeCreate => "volume-create",
            Self::SecretCreate => "secret-create",
            Self::ImagePull => "image-pull",
            Self::PodCreate => "pod-create",
            Self::ContainerCreate => "container-create",
            Self::PodStart => "pod-start",
            Self::ContainerStart => "container-start",
        }
    }
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum RenderedField {
    ContainerCommand,
    ContainerEntrypoint,
    ContainerUser,
    ContainerWorkdir,
    ContainerHostname,
    ContainerLabel,
    ContainerEnvironment,
    ContainerRestartPolicy,
    ContainerNamedVolumeMount,
    ContainerNetworkAttachment,
    PodNetworkAttachment,
    ContainerNetworkAlias,
    PodNetworkAlias,
    ContainerNetworkStaticIpv4,
    PodNetworkStaticIpv4,
    ContainerNetworkStaticIpv6,
    PodNetworkStaticIpv6,
    ContainerNetworkStaticMac,
    PodNetworkStaticMac,
    ContainerPortMapping,
    PodPortMapping,
    ContainerDnsServers,
    ContainerDnsSearch,
    ContainerDnsOptions,
    PodDnsServers,
    PodDnsSearch,
    PodDnsOptions,
    ContainerHostAlias,
    PodHostAlias,
    ContainerNetworkOrder,
    NetworkIpamSubnet,
    NetworkIpamGateway,
    NetworkIpamRange,
    NetworkRouteDestination,
    NetworkRouteGateway,
    NetworkRouteMetric,
    NetworkRouteTypeUnicast,
    NetworkRouteTypeBlackhole,
    NetworkRouteTypeUnreachable,
    NetworkRouteTypeProhibit,
}

/// A field rendered by the bounded M6-B3 container-runtime surface.
#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
#[allow(clippy::enum_variant_names)] // Stable public catalogue identifiers retain the resource kind.
enum RuntimeRenderedField {
    ContainerHealthDisabled,
    ContainerHealthCommand,
    ContainerHealthInterval,
    ContainerHealthTimeout,
    ContainerHealthRetries,
    ContainerHealthStartPeriod,
    ContainerHealthOnFailure,
    ContainerStartupHealthCommand,
    ContainerStartupHealthInterval,
    ContainerStartupHealthTimeout,
    ContainerStartupHealthRetries,
    ContainerStartupHealthSuccesses,
    ContainerLogDriver,
    ContainerLogMaxSize,
    ContainerLogJournaldLabels,
    ContainerPrivileged,
    ContainerCapabilityAdd,
    ContainerCapabilityDrop,
    ContainerNoNewPrivileges,
    ContainerReadOnlyFilesystem,
    ContainerReadWriteTmpfs,
    ContainerPidNamespace,
    ContainerIpcNamespace,
    ContainerUtsNamespace,
    ContainerCgroupNamespace,
    ContainerCpuShares,
    ContainerCpuPeriod,
    ContainerCpuQuota,
    ContainerMemoryLimit,
    ContainerPidsLimit,
    ContainerRlimitFinite,
    ContainerRlimitUnlimited,
}

/// One exact, target-gated, manual, or deliberately blocked M6-B4 output surface.
#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
#[allow(clippy::enum_variant_names)] // Stable evidence identifiers retain their resource scope.
enum B4RenderedField {
    ContainerNamedVolumeCopyMount,
    ContainerNamedVolumeSubpathCopyMount,
    ContainerNamedVolumeNoCopyMount,
    ContainerNamedVolumeSubpathNoCopyMount,
    ContainerBindMount,
    ContainerTmpfsMount,
    ContainerSecretMountGrant,
    ContainerSecretEnvironmentGrant,
    VolumeUid,
    VolumeGid,
    ImagePullPolicyAlways,
    ImagePullPolicyMissing,
    ImagePullPolicyNever,
    ImagePullPolicyNewer,
    ImageSourcePortability,
    PodInfraMount,
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
enum CliFlag {
    #[serde(rename = "--entrypoint")]
    Entrypoint,
    #[serde(rename = "--user")]
    User,
    #[serde(rename = "--workdir")]
    Workdir,
    #[serde(rename = "--hostname")]
    Hostname,
    #[serde(rename = "--label")]
    Label,
    #[serde(rename = "--env")]
    Environment,
    #[serde(rename = "--restart")]
    Restart,
    #[serde(rename = "--volume")]
    Volume,
    #[serde(rename = "--network")]
    Network,
    #[serde(rename = "--network-alias")]
    NetworkAlias,
    #[serde(rename = "--ip")]
    Ip,
    #[serde(rename = "--ip6")]
    Ip6,
    #[serde(rename = "--mac-address")]
    MacAddress,
    #[serde(rename = "--publish")]
    Publish,
    #[serde(rename = "--dns")]
    Dns,
    #[serde(rename = "--dns-search")]
    DnsSearch,
    #[serde(rename = "--dns-option")]
    DnsOption,
    #[serde(rename = "--add-host")]
    AddHost,
    #[serde(rename = "--subnet")]
    Subnet,
    #[serde(rename = "--gateway")]
    Gateway,
    #[serde(rename = "--ip-range")]
    IpRange,
    #[serde(rename = "--route")]
    Route,
    #[serde(rename = "--no-healthcheck")]
    NoHealthcheck,
    #[serde(rename = "--health-cmd")]
    HealthCommand,
    #[serde(rename = "--health-interval")]
    HealthInterval,
    #[serde(rename = "--health-timeout")]
    HealthTimeout,
    #[serde(rename = "--health-retries")]
    HealthRetries,
    #[serde(rename = "--health-start-period")]
    HealthStartPeriod,
    #[serde(rename = "--health-on-failure")]
    HealthOnFailure,
    #[serde(rename = "--health-startup-cmd")]
    HealthStartupCommand,
    #[serde(rename = "--health-startup-interval")]
    HealthStartupInterval,
    #[serde(rename = "--health-startup-timeout")]
    HealthStartupTimeout,
    #[serde(rename = "--health-startup-retries")]
    HealthStartupRetries,
    #[serde(rename = "--health-startup-success")]
    HealthStartupSuccess,
    #[serde(rename = "--log-driver")]
    LogDriver,
    #[serde(rename = "--log-opt")]
    LogOption,
    #[serde(rename = "--privileged")]
    Privileged,
    #[serde(rename = "--cap-add")]
    CapabilityAdd,
    #[serde(rename = "--cap-drop")]
    CapabilityDrop,
    #[serde(rename = "--security-opt")]
    SecurityOption,
    #[serde(rename = "--read-only")]
    ReadOnly,
    #[serde(rename = "--read-only-tmpfs")]
    ReadOnlyTmpfs,
    #[serde(rename = "--pid")]
    PidNamespace,
    #[serde(rename = "--ipc")]
    IpcNamespace,
    #[serde(rename = "--uts")]
    UtsNamespace,
    #[serde(rename = "--cgroupns")]
    CgroupNamespace,
    #[serde(rename = "--cpu-shares")]
    CpuShares,
    #[serde(rename = "--cpu-period")]
    CpuPeriod,
    #[serde(rename = "--cpu-quota")]
    CpuQuota,
    #[serde(rename = "--memory")]
    Memory,
    #[serde(rename = "--pids-limit")]
    PidsLimit,
    #[serde(rename = "--ulimit")]
    Ulimit,
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum CliValueShape {
    ArgumentArray,
    ContainerUser,
    AbsoluteContainerPath,
    Hostname,
    LabelAssignment,
    EnvironmentAssignment,
    RestartPolicy,
    NamedVolumeMount,
    NetworkAttachment,
    NetworkAlias,
    StaticIp,
    StaticMac,
    PortMapping,
    DnsServer,
    DnsSearch,
    DnsOption,
    HostAlias,
    NetworkOrder,
    NetworkSubnet,
    NetworkGateway,
    NetworkIpRange,
    NetworkRoute,
    FlagOnly,
    Boolean,
    HealthCommand,
    HealthInterval,
    HealthDuration,
    HealthRetries,
    HealthOnFailure,
    StartupHealthSuccesses,
    LogDriver,
    LogMaxSize,
    JournaldLabel,
    Capability,
    NoNewPrivileges,
    NamespaceMode,
    CpuShares,
    CpuPeriod,
    CpuQuota,
    MemoryBytes,
    PidsLimit,
    Rlimit,
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
enum LibpodBodyMember {
    Command,
    Entrypoint,
    User,
    WorkDir,
    Hostname,
    Labels,
    Env,
    RestartPolicy,
    Volumes,
    #[serde(rename = "Networks")]
    Networks,
    Portmappings,
    DnsServer,
    DnsSearch,
    DnsOption,
    Hostadd,
    #[serde(rename = "networkOrder")]
    NetworkOrder,
    Subnets,
    Routes,
    Healthconfig,
    HealthCheckOnFailureAction,
    #[serde(rename = "startupHealthConfig")]
    StartupHealthConfig,
    #[serde(rename = "log_configuration.driver")]
    LogConfigurationDriver,
    #[serde(rename = "log_configuration.size")]
    LogConfigurationSize,
    #[serde(rename = "log_configuration.labels")]
    LogConfigurationLabels,
    Privileged,
    CapAdd,
    CapDrop,
    NoNewPrivileges,
    ReadOnlyFilesystem,
    ReadWriteTmpfs,
    Pidns,
    Ipcns,
    Utsns,
    Cgroupns,
    #[serde(rename = "resource_limits.cpu.shares")]
    ResourceLimitsCpuShares,
    #[serde(rename = "resource_limits.cpu.period")]
    ResourceLimitsCpuPeriod,
    #[serde(rename = "resource_limits.cpu.quota")]
    ResourceLimitsCpuQuota,
    #[serde(rename = "resource_limits.memory.limit")]
    ResourceLimitsMemoryLimit,
    #[serde(rename = "resource_limits.pids.limit")]
    ResourceLimitsPidsLimit,
    RLimits,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum RuntimeLibpodValueShape {
    HealthConfiguration,
    HealthOnFailureAction,
    StartupHealthConfiguration,
    String,
    Integer,
    StringMap,
    Boolean,
    StringArray,
    Namespace,
    ResourceLimitInteger,
    RlimitArray,
}

/// Exactness of a rendered operation or plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RenderStatus {
    /// Every required input is represented by the rendered forms.
    Exact,
    /// A secret payload must be supplied by the caller at deployment time.
    DeferredSensitiveInput,
    /// A human must complete a reviewed manual step.
    Manual,
    /// The rendered operation intentionally approximates a semantic request.
    Approximate,
    /// No reviewed representation exists for the semantic request.
    Unsupported,
}

/// A redacted, stable finding produced while rendering an M5 plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderingFinding {
    code: DiagnosticCode,
    subject: Option<DeploymentResourceId>,
    field: Option<&'static str>,
}

impl Ord for RenderingFinding {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.code.as_str(), &self.subject, self.field).cmp(&(other.code.as_str(), &other.subject, other.field))
    }
}

impl PartialOrd for RenderingFinding {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl RenderingFinding {
    fn new(code: DiagnosticCode, subject: Option<DeploymentResourceId>, field: Option<&'static str>) -> Self {
        Self { code, subject, field }
    }

    /// Returns the stable rendering rule code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the redacted rule explanation.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        Diagnostic::new(self.code).message()
    }

    /// Returns the affected safe target-side resource identity, when applicable.
    #[must_use]
    pub fn subject(&self) -> Option<&DeploymentResourceId> {
        self.subject.as_ref()
    }

    /// Returns the affected intent field, when applicable.
    #[must_use]
    pub const fn field(&self) -> Option<&'static str> {
        self.field
    }
}

/// A deterministic Podman CLI invocation represented without shell quoting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliInvocation {
    program: &'static str,
    argv: Vec<String>,
    external_input: Option<SensitiveInputReference>,
}

impl CliInvocation {
    fn new(argv: Vec<String>, external_input: Option<SensitiveInputReference>) -> Self {
        Self {
            program: "podman",
            argv,
            external_input,
        }
    }

    /// Returns the fixed executable name.
    #[must_use]
    pub const fn program(&self) -> &'static str {
        self.program
    }

    /// Returns command arguments without shell quoting or interpolation.
    #[must_use]
    pub fn argv(&self) -> &[String] {
        &self.argv
    }

    /// Returns the required secret-material reference for stdin/file provision, if any.
    #[must_use]
    pub fn external_input(&self) -> Option<&SensitiveInputReference> {
        self.external_input.as_ref()
    }
}

/// HTTP method in a rendered Libpod request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderedHttpMethod {
    /// HTTP `GET`.
    Get,
    /// HTTP `POST`.
    Post,
}

/// A Libpod request body that is either safe JSON, absent, or deferred sensitive bytes.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderedHttpBody {
    /// No HTTP body is required.
    Empty,
    /// A deterministic typed JSON body.
    Json(Value),
    /// The caller must supply raw sensitive material; the reference is never serialized.
    ExternalSensitiveInput(SensitiveInputReference),
}

/// A deterministic, non-executable Libpod request description.
#[derive(Clone, Debug, PartialEq)]
pub struct LibpodInvocation {
    method: RenderedHttpMethod,
    path_and_query: String,
    body: RenderedHttpBody,
}

impl LibpodInvocation {
    fn new(method: RenderedHttpMethod, path_and_query: String, body: RenderedHttpBody) -> Self {
        Self {
            method,
            path_and_query,
            body,
        }
    }

    /// Returns the request method.
    #[must_use]
    pub const fn method(&self) -> RenderedHttpMethod {
        self.method
    }

    /// Returns the fully versioned and percent-encoded path/query.
    #[must_use]
    pub fn path_and_query(&self) -> &str {
        &self.path_and_query
    }

    /// Returns the safe JSON, no body, or external sensitive-input requirement.
    #[must_use]
    pub fn body(&self) -> &RenderedHttpBody {
        &self.body
    }
}

/// One semantic operation with both independently reviewable transport renderings.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderedOperation {
    operation: DeploymentOperation,
    status: RenderStatus,
    cli: CliInvocation,
    libpod: LibpodInvocation,
}

impl RenderedOperation {
    /// Returns the retained semantic source operation.
    #[must_use]
    pub fn operation(&self) -> &DeploymentOperation {
        &self.operation
    }

    /// Returns the exactness of this operation.
    #[must_use]
    pub const fn status(&self) -> RenderStatus {
        self.status
    }

    /// Returns the non-executable CLI rendering.
    #[must_use]
    pub fn cli(&self) -> &CliInvocation {
        &self.cli
    }

    /// Returns the non-executable Libpod request rendering.
    #[must_use]
    pub fn libpod(&self) -> &LibpodInvocation {
        &self.libpod
    }
}

/// Complete rendering of a validated M5 deployment plan.
#[derive(Clone, Debug, PartialEq)]
pub struct DeploymentRendering {
    status: RenderStatus,
    connection: Option<DeploymentConnectionReference>,
    external_preconditions: Vec<ExternalPrecondition>,
    operations: Vec<RenderedOperation>,
}

impl DeploymentRendering {
    /// Returns the aggregate exactness state.
    #[must_use]
    pub const fn status(&self) -> RenderStatus {
        self.status
    }

    /// Returns the caller-selected non-sensitive output connection reference, when present.
    ///
    /// The rendering never opens this connection. CLI invocations retain it as their explicit
    /// `--connection` argument; the deployment JSON preserves it for API consumers.
    #[must_use]
    pub fn connection(&self) -> Option<&DeploymentConnectionReference> {
        self.connection.as_ref()
    }

    /// Returns explicit external prerequisites preserved from the semantic plan.
    #[must_use]
    pub fn external_preconditions(&self) -> &[ExternalPrecondition] {
        &self.external_preconditions
    }

    /// Returns operations in the authoritative M5 sequence.
    #[must_use]
    pub fn operations(&self) -> &[RenderedOperation] {
        &self.operations
    }

    /// Generates a review-only POSIX `sh` script from the stored CLI argument arrays.
    ///
    /// The library never executes the script. It deliberately has no command substitution, glob
    /// expansion, or `eval`; it contains only the displayed Podman invocations. Each deferred
    /// secret requires a caller-provided regular file path in deterministic
    /// `PODMAN_LENS_SECRET_INPUT_<n>` order.
    #[must_use]
    pub fn shell_script(&self) -> String {
        let mut script =
            String::from("#!/bin/sh\n# Review generated Podman commands before running this file.\nset -eu\n");
        for precondition in &self.external_preconditions {
            let _ = writeln!(
                script,
                "# Requires external {}: {}",
                resource_kind_name(precondition.identity().kind()),
                shell_quote(precondition.identity().name())
            );
        }
        let mut secret_number = 0_usize;
        for operation in &self.operations {
            if operation.cli.external_input().is_some() {
                secret_number += 1;
                let _ = writeln!(
                    script,
                    ": \"${{PODMAN_LENS_SECRET_INPUT_{secret_number}:?set to a readable secret file path}}\""
                );
            }
        }
        secret_number = 0;
        for operation in &self.operations {
            script.push_str("podman");
            for value in operation.cli.argv() {
                script.push(' ');
                script.push_str(&shell_quote(value));
            }
            if operation.cli.external_input().is_some() {
                secret_number += 1;
                let _ = write!(script, " < \"${{PODMAN_LENS_SECRET_INPUT_{secret_number}}}\"");
            }
            script.push('\n');
        }
        script
    }
}

/// Outcome of rendering a plan for its explicitly selected target profile.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderingOutcome {
    rendering: Option<DeploymentRendering>,
    findings: Vec<RenderingFinding>,
}

impl RenderingOutcome {
    /// Returns the complete rendering when no rendering errors occurred.
    #[must_use]
    pub fn rendering(&self) -> Option<&DeploymentRendering> {
        self.rendering.as_ref()
    }

    /// Returns deterministic sorted rendering findings.
    #[must_use]
    pub fn findings(&self) -> &[RenderingFinding] {
        &self.findings
    }

    /// Returns whether a rendering was produced.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.rendering.is_some()
    }
}

/// Renders a validated semantic plan into non-executable CLI and Libpod descriptions.
///
/// M6-A accepts only targets whose engine and API versions are semantically identical and exactly
/// listed in its immutable renderer evidence. Existing M5 target validation permits a broader API
/// relation; per-field wire evidence is required before that relation can be rendered.
#[must_use]
pub fn render_deployment(plan: &DeploymentPlan) -> RenderingOutcome {
    let Some(version) = reviewed_renderer_target_version(plan) else {
        return RenderingOutcome {
            rendering: None,
            findings: vec![RenderingFinding::new(
                DiagnosticCode::RenderingTargetMismatch,
                None,
                Some("target"),
            )],
        };
    };
    let connection = plan.connection().map(DeploymentConnectionReference::as_str);
    let managed_image_sources = managed_image_sources(plan);
    let mut deferred = false;
    let mut operations = Vec::with_capacity(plan.operations().len());
    let mut findings = Vec::new();
    let mut blocked_resources = BTreeSet::new();
    for operation in plan.operations() {
        if blocked_resources.contains(operation.id().resource()) {
            continue;
        }
        let unsupported_fields =
            unsupported_fields(operation.resource_intent(), &version, plan.target().execution_context());
        if !unsupported_fields.is_empty() {
            blocked_resources.insert(operation.id().resource().clone());
            findings.extend(unsupported_fields.into_iter().map(|field| {
                RenderingFinding::new(
                    if field == "source.portability" {
                        DiagnosticCode::ImagePortabilityManual
                    } else {
                        DiagnosticCode::RenderingUnsupported
                    },
                    Some(operation.id().resource().clone()),
                    Some(field),
                )
            }));
            continue;
        }
        match render_operation(operation, &version, connection, &managed_image_sources) {
            Ok(rendered) => {
                deferred |= rendered.status == RenderStatus::DeferredSensitiveInput;
                operations.push(rendered);
            }
            Err(finding) => findings.push(finding),
        }
    }
    findings.sort_unstable();
    if findings.is_empty() {
        RenderingOutcome {
            rendering: Some(DeploymentRendering {
                status: if deferred {
                    RenderStatus::DeferredSensitiveInput
                } else {
                    RenderStatus::Exact
                },
                connection: plan.connection().cloned(),
                external_preconditions: plan.external_preconditions().to_vec(),
                operations,
            }),
            findings,
        }
    } else {
        RenderingOutcome {
            rendering: None,
            findings,
        }
    }
}

#[allow(clippy::too_many_lines)] // One exhaustive match keeps the finite M5 semantic surface auditable.
fn render_operation(
    operation: &DeploymentOperation,
    version: &str,
    connection: Option<&str>,
    managed_image_sources: &BTreeMap<DeploymentResourceId, String>,
) -> Result<RenderedOperation, RenderingFinding> {
    let id = operation.id().resource();
    let mut prefix = connection.map_or_else(Vec::new, |value| vec!["--connection".to_owned(), value.to_owned()]);
    let (status, cli_suffix, method, path, body, input) = match operation.resource_intent() {
        DeploymentResource::Network(network) => {
            let mut cli_suffix = vec!["network".to_owned(), "create".to_owned()];
            append_network_create_arguments(&mut cli_suffix, network);
            cli_suffix.push(network.identity().name().to_owned());
            (
                RenderStatus::Exact,
                cli_suffix,
                RenderedHttpMethod::Post,
                format!("/v{version}/libpod/networks/create"),
                RenderedHttpBody::Json(network_create_configuration(
                    network,
                    supports_network_route_type(version),
                )),
                None,
            )
        }
        DeploymentResource::Volume(volume) => (
            RenderStatus::Exact,
            volume_create_cli_arguments(volume),
            RenderedHttpMethod::Post,
            format!("/v{version}/libpod/volumes/create"),
            RenderedHttpBody::Json(volume_create_json(volume)),
            None,
        ),
        DeploymentResource::Secret(secret) => (
            RenderStatus::DeferredSensitiveInput,
            vec![
                "secret".to_owned(),
                "create".to_owned(),
                secret.identity().name().to_owned(),
                "-".to_owned(),
            ],
            RenderedHttpMethod::Post,
            format!(
                "/v{version}/libpod/secrets/create?name={}",
                percent_encode(secret.identity().name())
            ),
            RenderedHttpBody::ExternalSensitiveInput(secret.material().clone()),
            Some(secret.material().clone()),
        ),
        DeploymentResource::Image(image) => (
            RenderStatus::Exact,
            vec![
                "image".to_owned(),
                "pull".to_owned(),
                format!("--policy={}", image.pull_policy().as_str()),
                image.source().as_str().to_owned(),
            ],
            RenderedHttpMethod::Post,
            format!(
                "/v{version}/libpod/images/pull?reference={}&policy={}",
                percent_encode(image.source().as_str()),
                image.pull_policy().as_str(),
            ),
            RenderedHttpBody::Empty,
            None,
        ),
        DeploymentResource::Pod(pod) => {
            if operation.id().action() == crate::SemanticOperationAction::StartPod {
                (
                    RenderStatus::Exact,
                    vec!["pod".to_owned(), "start".to_owned(), pod.identity().name().to_owned()],
                    RenderedHttpMethod::Post,
                    format!(
                        "/v{version}/libpod/pods/{}/start",
                        percent_encode(pod.identity().name())
                    ),
                    RenderedHttpBody::Empty,
                    None,
                )
            } else {
                let networks = network_configuration(pod.networks());
                let mut cli_suffix = vec![
                    "pod".to_owned(),
                    "create".to_owned(),
                    "--name".to_owned(),
                    pod.identity().name().to_owned(),
                ];
                if !append_network_arguments(&mut cli_suffix, pod.networks(), None) {
                    return Err(RenderingFinding::new(
                        DiagnosticCode::RenderingUnsupported,
                        Some(id.clone()),
                        Some("networks.cli_ambiguous"),
                    ));
                }
                append_port_arguments(&mut cli_suffix, pod.ports());
                append_dns_arguments(&mut cli_suffix, pod.dns());
                append_host_alias_arguments(&mut cli_suffix, pod.host_aliases());
                let mut body = Map::new();
                body.insert("name".to_owned(), Value::String(pod.identity().name().to_owned()));
                body.insert("Networks".to_owned(), networks);
                append_networking_json(&mut body, pod.ports(), pod.dns(), pod.host_aliases(), None);
                (
                    RenderStatus::Exact,
                    cli_suffix,
                    RenderedHttpMethod::Post,
                    format!("/v{version}/libpod/pods/create"),
                    RenderedHttpBody::Json(Value::Object(body)),
                    None,
                )
            }
        }
        DeploymentResource::Container(container) => {
            if operation.id().action() == crate::SemanticOperationAction::StartContainer {
                (
                    RenderStatus::Exact,
                    vec![
                        "container".to_owned(),
                        "start".to_owned(),
                        container.identity().name().to_owned(),
                    ],
                    RenderedHttpMethod::Post,
                    format!(
                        "/v{version}/libpod/containers/{}/start",
                        percent_encode(container.identity().name())
                    ),
                    RenderedHttpBody::Empty,
                    None,
                )
            } else {
                let image = managed_image_sources
                    .get(container.image())
                    .map_or_else(|| container.image().name(), String::as_str);
                let mut cli_suffix = vec![
                    "container".to_owned(),
                    "create".to_owned(),
                    "--name".to_owned(),
                    container.identity().name().to_owned(),
                    "--pull=never".to_owned(),
                ];
                let mut body = if let Some(pod) = container.pod() {
                    cli_suffix.push("--pod".to_owned());
                    cli_suffix.push(pod.name().to_owned());
                    json!({"image": image, "pod": pod.name()})
                } else {
                    if !append_network_arguments(&mut cli_suffix, container.networks(), container.network_order()) {
                        return Err(RenderingFinding::new(
                            DiagnosticCode::RenderingUnsupported,
                            Some(id.clone()),
                            Some("networks.cli_ambiguous"),
                        ));
                    }
                    append_port_arguments(&mut cli_suffix, container.ports());
                    append_dns_arguments(&mut cli_suffix, container.dns());
                    append_host_alias_arguments(&mut cli_suffix, container.host_aliases());
                    let mut body = json!({"image": image, "Networks": network_configuration(container.networks())});
                    let Some(body_map) = body.as_object_mut() else {
                        return Err(RenderingFinding::new(
                            DiagnosticCode::RenderingUnsupported,
                            Some(id.clone()),
                            Some("container_body"),
                        ));
                    };
                    append_networking_json(
                        body_map,
                        container.ports(),
                        container.dns(),
                        container.host_aliases(),
                        container.network_order(),
                    );
                    body
                };
                if !append_mount_arguments(&mut cli_suffix, container.mounts()) {
                    return Err(RenderingFinding::new(
                        DiagnosticCode::RenderingUnsupported,
                        Some(id.clone()),
                        Some("mounts.cli_ambiguous"),
                    ));
                }
                if !append_secret_grants_arguments(&mut cli_suffix, container.secret_grants()) {
                    return Err(RenderingFinding::new(
                        DiagnosticCode::RenderingUnsupported,
                        Some(id.clone()),
                        Some("secret_grants.cli_ambiguous"),
                    ));
                }
                append_container_setting_arguments(&mut cli_suffix, container, id)?;
                append_container_runtime_arguments(&mut cli_suffix, container, id, version)?;
                let Some(body_map) = body.as_object_mut() else {
                    return Err(RenderingFinding::new(
                        DiagnosticCode::RenderingUnsupported,
                        Some(id.clone()),
                        Some("container_body"),
                    ));
                };
                append_container_setting_json(body_map, container, id)?;
                append_secret_grants_json(body_map, container.secret_grants());
                append_container_runtime_json(body_map, container, id, version)?;
                cli_suffix.push(image.to_owned());
                if let Some(command) = container.settings().command() {
                    cli_suffix.extend(command.values().iter().cloned());
                }
                (
                    RenderStatus::Exact,
                    cli_suffix,
                    RenderedHttpMethod::Post,
                    format!(
                        "/v{version}/libpod/containers/create?name={}",
                        percent_encode(container.identity().name())
                    ),
                    RenderedHttpBody::Json(body),
                    None,
                )
            }
        }
        DeploymentResource::ExternalPrecondition(_) => {
            return Err(RenderingFinding::new(
                DiagnosticCode::RenderingUnsupported,
                Some(id.clone()),
                Some("resource"),
            ));
        }
    };
    prefix.extend(cli_suffix);
    Ok(RenderedOperation {
        operation: operation.clone(),
        status,
        cli: CliInvocation::new(prefix, input),
        libpod: LibpodInvocation::new(method, path, body),
    })
}

fn reviewed_renderer_target_version(plan: &DeploymentPlan) -> Option<String> {
    let engine = plan.target().podman_version().as_semver();
    let api = plan.target().api_version().as_semver();
    if engine != api || !engine.build.is_empty() || !api.build.is_empty() {
        return None;
    }
    let canonical = canonical_version(engine);
    renderer_catalogue_versions()
        .ok()?
        .into_iter()
        .find(|version| version == &canonical)
}

fn renderer_catalogue_versions() -> Result<Vec<String>, ()> {
    parse_renderer_catalogue(RENDERING_CATALOGUE_JSON)
}

fn parse_renderer_catalogue(source: &str) -> Result<Vec<String>, ()> {
    reject_duplicate_json_keys(source)?;
    let catalogue: RenderingCatalogue = serde_json::from_str(source).map_err(|_| ())?;
    let expected_operations = RENDERED_OPERATION_CATEGORIES.into_iter().collect::<BTreeSet<_>>();
    let capabilities = crate::capability_catalogue().map_err(|_| ())?;
    if catalogue.schema_version != 8
        || catalogue.provenance.trim().is_empty()
        || catalogue.reviewed_lines.len() != capabilities.len()
        || !validated_runtime_field_claims(&catalogue.runtime_field_claims)
    {
        return Err(());
    }
    let mut versions = Vec::with_capacity(catalogue.reviewed_lines.len());
    for (line, capability) in catalogue.reviewed_lines.into_iter().zip(capabilities) {
        let version = Version::parse(&line.version).map_err(|_| ())?;
        if !version.pre.is_empty()
            || !version.build.is_empty()
            || line.version != canonical_version(&version)
            || line.version != capability.observed_podman_version()
            || line.revision != capability.evidence().revision()
            || line.tag != capability.evidence().release_tag()
            || !is_lowercase_sha40(&line.revision)
            || !valid_common_module(&line.version, &line.common_module)
            || !validated_renderer_operations(&line, &expected_operations)
            || !validated_b4_evidence(&line)
            || !validated_field_evidence(&line)
            || !validated_runtime_evidence(&line)
        {
            return Err(());
        }
        versions.push(line.version);
    }
    Ok(versions)
}

fn validated_b4_evidence(line: &ReviewedRenderingLine) -> bool {
    let evidence = &line.b4_evidence;
    let exact = evidence.exact_fields.iter().copied().collect::<BTreeSet<_>>();
    let target_gated = evidence.target_gated_fields.iter().copied().collect::<BTreeSet<_>>();
    let manual = evidence.manual_fields.iter().copied().collect::<BTreeSet<_>>();
    let blocked = evidence.blocked_fields.iter().copied().collect::<BTreeSet<_>>();
    let expected_target_gated = expected_b4_target_gated_fields(&line.version);
    exact.len() == evidence.exact_fields.len()
        && target_gated.len() == evidence.target_gated_fields.len()
        && manual.len() == evidence.manual_fields.len()
        && blocked.len() == evidence.blocked_fields.len()
        && exact.is_disjoint(&target_gated)
        && exact.is_disjoint(&manual)
        && exact.is_disjoint(&blocked)
        && target_gated.is_disjoint(&manual)
        && target_gated.is_disjoint(&blocked)
        && manual.is_disjoint(&blocked)
        && exact.union(&target_gated).copied().collect::<BTreeSet<_>>() == expected_b4_rendered_fields()
        && target_gated == expected_target_gated
        && manual == expected_b4_manual_fields()
        && blocked == expected_b4_blocked_fields()
        && evidence.cli_sources.len() == 4
        && evidence
            .cli_sources
            .iter()
            .zip([
                "cmd/podman/common/create.go",
                "cmd/podman/containers/create.go",
                "cmd/podman/volumes/create.go",
                "cmd/podman/images/pull.go",
            ])
            .all(|(source, path)| source_matches_podman(source, &line.revision, path))
        && evidence.model_sources.len() == 3
        && evidence
            .model_sources
            .iter()
            .zip([
                "pkg/specgenutil/volumes.go",
                "pkg/specgen/specgen.go",
                "pkg/specgen/volumes.go",
            ])
            .all(|(source, path)| source_matches_podman(source, &line.revision, path))
        && evidence.handler_sources.len() == 3
        && evidence
            .handler_sources
            .iter()
            .zip([
                "pkg/api/handlers/libpod/containers_create.go",
                "pkg/api/handlers/libpod/volumes.go",
                "pkg/api/handlers/libpod/images_pull.go",
            ])
            .all(|(source, path)| source_matches_podman(source, &line.revision, path))
}

fn expected_b4_rendered_fields() -> BTreeSet<B4RenderedField> {
    use B4RenderedField::{
        ContainerBindMount, ContainerNamedVolumeCopyMount, ContainerNamedVolumeNoCopyMount,
        ContainerNamedVolumeSubpathCopyMount, ContainerSecretEnvironmentGrant, ContainerSecretMountGrant,
        ContainerTmpfsMount, ImagePullPolicyAlways, ImagePullPolicyMissing, ImagePullPolicyNever, ImagePullPolicyNewer,
        VolumeGid, VolumeUid,
    };
    [
        ContainerNamedVolumeCopyMount,
        ContainerNamedVolumeSubpathCopyMount,
        ContainerNamedVolumeNoCopyMount,
        ContainerBindMount,
        ContainerTmpfsMount,
        ContainerSecretMountGrant,
        ContainerSecretEnvironmentGrant,
        VolumeUid,
        VolumeGid,
        ImagePullPolicyAlways,
        ImagePullPolicyMissing,
        ImagePullPolicyNever,
        ImagePullPolicyNewer,
    ]
    .into_iter()
    .collect()
}

fn expected_b4_target_gated_fields(version: &str) -> BTreeSet<B4RenderedField> {
    if supports_image_policy(version) {
        BTreeSet::new()
    } else {
        use B4RenderedField::{
            ImagePullPolicyAlways, ImagePullPolicyMissing, ImagePullPolicyNever, ImagePullPolicyNewer, VolumeGid,
            VolumeUid,
        };
        [
            VolumeUid,
            VolumeGid,
            ImagePullPolicyAlways,
            ImagePullPolicyMissing,
            ImagePullPolicyNever,
            ImagePullPolicyNewer,
        ]
        .into_iter()
        .collect()
    }
}

fn expected_b4_manual_fields() -> BTreeSet<B4RenderedField> {
    [B4RenderedField::ImageSourcePortability].into_iter().collect()
}

fn expected_b4_blocked_fields() -> BTreeSet<B4RenderedField> {
    [
        B4RenderedField::ContainerNamedVolumeSubpathNoCopyMount,
        B4RenderedField::PodInfraMount,
    ]
    .into_iter()
    .collect()
}

fn validated_runtime_field_claims(claims: &[RuntimeFieldClaimJson]) -> bool {
    let expected = expected_runtime_field_claims();
    let observed = claims.iter().map(|claim| claim.field).collect::<BTreeSet<_>>();
    claims.len() == expected.len()
        && observed.len() == claims.len()
        && observed == expected.keys().copied().collect()
        && claims.iter().all(|claim| {
            expected.get(&claim.field).is_some_and(|expected| {
                claim.cli.flag == Some(expected.0)
                    && claim.cli.value_shape == expected.1
                    && claim.libpod.json_member == expected.2
                    && claim.libpod.value_shape == expected_runtime_libpod_value_shape(claim.field)
            })
        })
}

fn validated_runtime_evidence(line: &ReviewedRenderingLine) -> bool {
    let evidence = &line.runtime_evidence;
    let exact = evidence.exact_fields.iter().copied().collect::<BTreeSet<_>>();
    let target_gated = evidence.target_gated_fields.iter().copied().collect::<BTreeSet<_>>();
    let expected_target_gated = expected_runtime_target_gated_fields(&line.version);
    exact.len() == evidence.exact_fields.len()
        && target_gated.len() == evidence.target_gated_fields.len()
        && exact.is_disjoint(&target_gated)
        && exact.union(&target_gated).copied().collect::<BTreeSet<_>>() == expected_runtime_fields()
        && target_gated == expected_target_gated
        && source_matches_podman(&evidence.cli_flag_source, &line.revision, "cmd/podman/common/create.go")
        && source_matches_podman(
            &evidence.cli_transform_source,
            &line.revision,
            "pkg/specgenutil/specgen.go",
        )
        && source_matches_podman(
            &evidence.command_route_source,
            &line.revision,
            "cmd/podman/containers/create.go",
        )
        && source_matches_podman(
            &evidence.route_source,
            &line.revision,
            "pkg/api/server/register_containers.go",
        )
        && source_matches_podman(
            &evidence.handler_source,
            &line.revision,
            "pkg/api/handlers/libpod/containers_create.go",
        )
        && evidence.model_sources.len() == 3
        && evidence
            .model_sources
            .iter()
            .zip([
                "pkg/specgen/specgen.go",
                "pkg/specgen/namespaces.go",
                "libpod/define/healthchecks.go",
            ])
            .all(|(source, path)| source_matches_podman(source, &line.revision, path))
        && target_gated.iter().all(|field| {
            matches!(
                field,
                RuntimeRenderedField::ContainerLogJournaldLabels | RuntimeRenderedField::ContainerRlimitUnlimited
            )
        })
}

fn expected_runtime_fields() -> BTreeSet<RuntimeRenderedField> {
    [
        RuntimeRenderedField::ContainerHealthDisabled,
        RuntimeRenderedField::ContainerHealthCommand,
        RuntimeRenderedField::ContainerHealthInterval,
        RuntimeRenderedField::ContainerHealthTimeout,
        RuntimeRenderedField::ContainerHealthRetries,
        RuntimeRenderedField::ContainerHealthStartPeriod,
        RuntimeRenderedField::ContainerHealthOnFailure,
        RuntimeRenderedField::ContainerStartupHealthCommand,
        RuntimeRenderedField::ContainerStartupHealthInterval,
        RuntimeRenderedField::ContainerStartupHealthTimeout,
        RuntimeRenderedField::ContainerStartupHealthRetries,
        RuntimeRenderedField::ContainerStartupHealthSuccesses,
        RuntimeRenderedField::ContainerLogDriver,
        RuntimeRenderedField::ContainerLogMaxSize,
        RuntimeRenderedField::ContainerLogJournaldLabels,
        RuntimeRenderedField::ContainerPrivileged,
        RuntimeRenderedField::ContainerCapabilityAdd,
        RuntimeRenderedField::ContainerCapabilityDrop,
        RuntimeRenderedField::ContainerNoNewPrivileges,
        RuntimeRenderedField::ContainerReadOnlyFilesystem,
        RuntimeRenderedField::ContainerReadWriteTmpfs,
        RuntimeRenderedField::ContainerPidNamespace,
        RuntimeRenderedField::ContainerIpcNamespace,
        RuntimeRenderedField::ContainerUtsNamespace,
        RuntimeRenderedField::ContainerCgroupNamespace,
        RuntimeRenderedField::ContainerCpuShares,
        RuntimeRenderedField::ContainerCpuPeriod,
        RuntimeRenderedField::ContainerCpuQuota,
        RuntimeRenderedField::ContainerMemoryLimit,
        RuntimeRenderedField::ContainerPidsLimit,
        RuntimeRenderedField::ContainerRlimitFinite,
        RuntimeRenderedField::ContainerRlimitUnlimited,
    ]
    .into_iter()
    .collect()
}

fn expected_runtime_target_gated_fields(version: &str) -> BTreeSet<RuntimeRenderedField> {
    let mut fields = BTreeSet::new();
    if !supports_journald_labels(version) {
        fields.insert(RuntimeRenderedField::ContainerLogJournaldLabels);
    }
    if !supports_unlimited_rlimit(version) {
        fields.insert(RuntimeRenderedField::ContainerRlimitUnlimited);
    }
    fields
}

type RuntimeFieldClaim = (CliFlag, CliValueShape, LibpodBodyMember);

const fn expected_runtime_libpod_value_shape(field: RuntimeRenderedField) -> RuntimeLibpodValueShape {
    use RuntimeLibpodValueShape::{
        Boolean, HealthConfiguration, HealthOnFailureAction, Integer, Namespace, ResourceLimitInteger, RlimitArray,
        StartupHealthConfiguration, String, StringArray, StringMap,
    };
    use RuntimeRenderedField::{
        ContainerCapabilityAdd, ContainerCapabilityDrop, ContainerCgroupNamespace, ContainerCpuPeriod,
        ContainerCpuQuota, ContainerCpuShares, ContainerHealthCommand, ContainerHealthDisabled,
        ContainerHealthInterval, ContainerHealthOnFailure, ContainerHealthRetries, ContainerHealthStartPeriod,
        ContainerHealthTimeout, ContainerIpcNamespace, ContainerLogDriver, ContainerLogJournaldLabels,
        ContainerLogMaxSize, ContainerMemoryLimit, ContainerNoNewPrivileges, ContainerPidNamespace, ContainerPidsLimit,
        ContainerPrivileged, ContainerReadOnlyFilesystem, ContainerReadWriteTmpfs, ContainerRlimitFinite,
        ContainerRlimitUnlimited, ContainerStartupHealthCommand, ContainerStartupHealthInterval,
        ContainerStartupHealthRetries, ContainerStartupHealthSuccesses, ContainerStartupHealthTimeout,
        ContainerUtsNamespace,
    };
    match field {
        ContainerHealthDisabled
        | ContainerHealthCommand
        | ContainerHealthInterval
        | ContainerHealthTimeout
        | ContainerHealthRetries
        | ContainerHealthStartPeriod => HealthConfiguration,
        ContainerHealthOnFailure => HealthOnFailureAction,
        ContainerStartupHealthCommand
        | ContainerStartupHealthInterval
        | ContainerStartupHealthTimeout
        | ContainerStartupHealthRetries
        | ContainerStartupHealthSuccesses => StartupHealthConfiguration,
        ContainerLogDriver => String,
        ContainerLogMaxSize => Integer,
        ContainerLogJournaldLabels => StringMap,
        ContainerPrivileged | ContainerNoNewPrivileges | ContainerReadOnlyFilesystem | ContainerReadWriteTmpfs => {
            Boolean
        }
        ContainerCapabilityAdd | ContainerCapabilityDrop => StringArray,
        ContainerPidNamespace | ContainerIpcNamespace | ContainerUtsNamespace | ContainerCgroupNamespace => Namespace,
        ContainerCpuShares | ContainerCpuPeriod | ContainerCpuQuota | ContainerMemoryLimit | ContainerPidsLimit => {
            ResourceLimitInteger
        }
        ContainerRlimitFinite | ContainerRlimitUnlimited => RlimitArray,
    }
}

#[allow(clippy::too_many_lines)] // The finite B3 claim matrix is intentionally audit-friendly.
fn expected_runtime_field_claims() -> BTreeMap<RuntimeRenderedField, RuntimeFieldClaim> {
    use RuntimeRenderedField::{
        ContainerCapabilityAdd, ContainerCapabilityDrop, ContainerCgroupNamespace, ContainerCpuPeriod,
        ContainerCpuQuota, ContainerCpuShares, ContainerHealthCommand, ContainerHealthDisabled,
        ContainerHealthInterval, ContainerHealthOnFailure, ContainerHealthRetries, ContainerHealthStartPeriod,
        ContainerHealthTimeout, ContainerIpcNamespace, ContainerLogDriver, ContainerLogJournaldLabels,
        ContainerLogMaxSize, ContainerMemoryLimit, ContainerNoNewPrivileges, ContainerPidNamespace, ContainerPidsLimit,
        ContainerPrivileged, ContainerReadOnlyFilesystem, ContainerReadWriteTmpfs, ContainerRlimitFinite,
        ContainerRlimitUnlimited, ContainerStartupHealthCommand, ContainerStartupHealthInterval,
        ContainerStartupHealthRetries, ContainerStartupHealthSuccesses, ContainerStartupHealthTimeout,
        ContainerUtsNamespace,
    };
    [
        (
            ContainerHealthDisabled,
            (
                CliFlag::NoHealthcheck,
                CliValueShape::FlagOnly,
                LibpodBodyMember::Healthconfig,
            ),
        ),
        (
            ContainerHealthCommand,
            (
                CliFlag::HealthCommand,
                CliValueShape::HealthCommand,
                LibpodBodyMember::Healthconfig,
            ),
        ),
        (
            ContainerHealthInterval,
            (
                CliFlag::HealthInterval,
                CliValueShape::HealthInterval,
                LibpodBodyMember::Healthconfig,
            ),
        ),
        (
            ContainerHealthTimeout,
            (
                CliFlag::HealthTimeout,
                CliValueShape::HealthDuration,
                LibpodBodyMember::Healthconfig,
            ),
        ),
        (
            ContainerHealthRetries,
            (
                CliFlag::HealthRetries,
                CliValueShape::HealthRetries,
                LibpodBodyMember::Healthconfig,
            ),
        ),
        (
            ContainerHealthStartPeriod,
            (
                CliFlag::HealthStartPeriod,
                CliValueShape::HealthDuration,
                LibpodBodyMember::Healthconfig,
            ),
        ),
        (
            ContainerHealthOnFailure,
            (
                CliFlag::HealthOnFailure,
                CliValueShape::HealthOnFailure,
                LibpodBodyMember::HealthCheckOnFailureAction,
            ),
        ),
        (
            ContainerStartupHealthCommand,
            (
                CliFlag::HealthStartupCommand,
                CliValueShape::HealthCommand,
                LibpodBodyMember::StartupHealthConfig,
            ),
        ),
        (
            ContainerStartupHealthInterval,
            (
                CliFlag::HealthStartupInterval,
                CliValueShape::HealthInterval,
                LibpodBodyMember::StartupHealthConfig,
            ),
        ),
        (
            ContainerStartupHealthTimeout,
            (
                CliFlag::HealthStartupTimeout,
                CliValueShape::HealthDuration,
                LibpodBodyMember::StartupHealthConfig,
            ),
        ),
        (
            ContainerStartupHealthRetries,
            (
                CliFlag::HealthStartupRetries,
                CliValueShape::HealthRetries,
                LibpodBodyMember::StartupHealthConfig,
            ),
        ),
        (
            ContainerStartupHealthSuccesses,
            (
                CliFlag::HealthStartupSuccess,
                CliValueShape::StartupHealthSuccesses,
                LibpodBodyMember::StartupHealthConfig,
            ),
        ),
        (
            ContainerLogDriver,
            (
                CliFlag::LogDriver,
                CliValueShape::LogDriver,
                LibpodBodyMember::LogConfigurationDriver,
            ),
        ),
        (
            ContainerLogMaxSize,
            (
                CliFlag::LogOption,
                CliValueShape::LogMaxSize,
                LibpodBodyMember::LogConfigurationSize,
            ),
        ),
        (
            ContainerLogJournaldLabels,
            (
                CliFlag::LogOption,
                CliValueShape::JournaldLabel,
                LibpodBodyMember::LogConfigurationLabels,
            ),
        ),
        (
            ContainerPrivileged,
            (
                CliFlag::Privileged,
                CliValueShape::Boolean,
                LibpodBodyMember::Privileged,
            ),
        ),
        (
            ContainerCapabilityAdd,
            (
                CliFlag::CapabilityAdd,
                CliValueShape::Capability,
                LibpodBodyMember::CapAdd,
            ),
        ),
        (
            ContainerCapabilityDrop,
            (
                CliFlag::CapabilityDrop,
                CliValueShape::Capability,
                LibpodBodyMember::CapDrop,
            ),
        ),
        (
            ContainerNoNewPrivileges,
            (
                CliFlag::SecurityOption,
                CliValueShape::NoNewPrivileges,
                LibpodBodyMember::NoNewPrivileges,
            ),
        ),
        (
            ContainerReadOnlyFilesystem,
            (
                CliFlag::ReadOnly,
                CliValueShape::Boolean,
                LibpodBodyMember::ReadOnlyFilesystem,
            ),
        ),
        (
            ContainerReadWriteTmpfs,
            (
                CliFlag::ReadOnlyTmpfs,
                CliValueShape::Boolean,
                LibpodBodyMember::ReadWriteTmpfs,
            ),
        ),
        (
            ContainerPidNamespace,
            (
                CliFlag::PidNamespace,
                CliValueShape::NamespaceMode,
                LibpodBodyMember::Pidns,
            ),
        ),
        (
            ContainerIpcNamespace,
            (
                CliFlag::IpcNamespace,
                CliValueShape::NamespaceMode,
                LibpodBodyMember::Ipcns,
            ),
        ),
        (
            ContainerUtsNamespace,
            (
                CliFlag::UtsNamespace,
                CliValueShape::NamespaceMode,
                LibpodBodyMember::Utsns,
            ),
        ),
        (
            ContainerCgroupNamespace,
            (
                CliFlag::CgroupNamespace,
                CliValueShape::NamespaceMode,
                LibpodBodyMember::Cgroupns,
            ),
        ),
        (
            ContainerCpuShares,
            (
                CliFlag::CpuShares,
                CliValueShape::CpuShares,
                LibpodBodyMember::ResourceLimitsCpuShares,
            ),
        ),
        (
            ContainerCpuPeriod,
            (
                CliFlag::CpuPeriod,
                CliValueShape::CpuPeriod,
                LibpodBodyMember::ResourceLimitsCpuPeriod,
            ),
        ),
        (
            ContainerCpuQuota,
            (
                CliFlag::CpuQuota,
                CliValueShape::CpuQuota,
                LibpodBodyMember::ResourceLimitsCpuQuota,
            ),
        ),
        (
            ContainerMemoryLimit,
            (
                CliFlag::Memory,
                CliValueShape::MemoryBytes,
                LibpodBodyMember::ResourceLimitsMemoryLimit,
            ),
        ),
        (
            ContainerPidsLimit,
            (
                CliFlag::PidsLimit,
                CliValueShape::PidsLimit,
                LibpodBodyMember::ResourceLimitsPidsLimit,
            ),
        ),
        (
            ContainerRlimitFinite,
            (CliFlag::Ulimit, CliValueShape::Rlimit, LibpodBodyMember::RLimits),
        ),
        (
            ContainerRlimitUnlimited,
            (CliFlag::Ulimit, CliValueShape::Rlimit, LibpodBodyMember::RLimits),
        ),
    ]
    .into_iter()
    .collect()
}

fn reject_duplicate_json_keys(source: &str) -> Result<(), ()> {
    let mut deserializer = serde_json::Deserializer::from_str(source);
    RejectDuplicateJsonKeys::deserialize(&mut deserializer).map_err(|_| ())?;
    deserializer.end().map_err(|_| ())
}

struct RejectDuplicateJsonKeys;

impl<'de> Deserialize<'de> for RejectDuplicateJsonKeys {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(RejectDuplicateJsonKeysVisitor)
    }
}

struct RejectDuplicateJsonKeysVisitor;

impl<'de> de::Visitor<'de> for RejectDuplicateJsonKeysVisitor {
    type Value = RejectDuplicateJsonKeys;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectDuplicateJsonKeys)
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectDuplicateJsonKeys)
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectDuplicateJsonKeys)
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectDuplicateJsonKeys)
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectDuplicateJsonKeys)
    }

    fn visit_string<E>(self, _value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectDuplicateJsonKeys)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectDuplicateJsonKeys)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(RejectDuplicateJsonKeys)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        while sequence.next_element::<RejectDuplicateJsonKeys>()?.is_some() {}
        Ok(RejectDuplicateJsonKeys)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            map.next_value::<RejectDuplicateJsonKeys>()?;
        }
        Ok(RejectDuplicateJsonKeys)
    }
}

fn validated_renderer_operations(line: &ReviewedRenderingLine, expected_categories: &BTreeSet<&str>) -> bool {
    let observed_categories = line
        .operations
        .iter()
        .map(|operation| operation.category.as_str())
        .collect::<BTreeSet<_>>();
    line.operations.len() == expected_categories.len()
        && observed_categories == expected_categories.clone()
        && line.operations.iter().all(|operation| {
            let Some((cli_path, libpod_path, body_path)) = rendering_source_paths(operation.category.as_str()) else {
                return false;
            };
            source_matches_podman(&operation.cli_source, &line.revision, cli_path)
                && source_matches_podman(&operation.libpod_endpoint_source, &line.revision, libpod_path)
                && match body_path {
                    Some(body_path) => operation
                        .body_source
                        .as_ref()
                        .is_some_and(|source| source_matches_podman(source, &line.revision, body_path)),
                    None => operation.body_source.is_none(),
                }
        })
}

fn validated_field_evidence(line: &ReviewedRenderingLine) -> bool {
    let observed = line
        .field_evidence
        .iter()
        .map(|evidence| evidence.field)
        .collect::<BTreeSet<_>>();
    line.field_evidence.len() == expected_rendered_fields().len()
        && observed == expected_rendered_fields()
        && line.field_evidence.iter().all(|evidence| {
            if evidence.availability != expected_field_availability(evidence.field, &line.version) {
                return false;
            }
            if let Some((operation, flag, shape, member, cli_source, model_sources, handler_source)) =
                expected_network_field_claim(evidence.field)
            {
                return evidence.operation == operation
                    && evidence.cli.flag == flag
                    && evidence.cli.value_shape == shape
                    && evidence.libpod.json_member == member
                    && source_matches_network_evidence(&evidence.cli_source, line, cli_source)
                    && evidence.model_sources.len() == model_sources.len()
                    && evidence
                        .model_sources
                        .iter()
                        .zip(model_sources)
                        .all(|(source, expected)| source_matches_network_evidence(source, line, *expected))
                    && source_matches_network_evidence(&evidence.handler_source, line, handler_source);
            }
            let (operation, flag, shape, member, cli_path, model_paths, handler_path) =
                expected_field_claim(evidence.field);
            evidence.operation == operation
                && evidence.cli.flag == flag
                && evidence.cli.value_shape == shape
                && evidence.libpod.json_member == member
                && source_matches_podman(&evidence.cli_source, &line.revision, cli_path)
                && evidence.model_sources.len() == model_paths.len()
                && evidence
                    .model_sources
                    .iter()
                    .zip(model_paths)
                    .all(|(source, path)| source_matches_podman(source, &line.revision, path))
                && source_matches_podman(&evidence.handler_source, &line.revision, handler_path)
        })
}

fn expected_field_availability(field: RenderedField, version: &str) -> FieldAvailability {
    match field {
        RenderedField::ContainerNetworkOrder
        | RenderedField::NetworkRouteTypeBlackhole
        | RenderedField::NetworkRouteTypeUnreachable
        | RenderedField::NetworkRouteTypeProhibit
            if !supports_network_order(version) =>
        {
            FieldAvailability::Unsupported
        }
        _ => FieldAvailability::Exact,
    }
}

fn expected_rendered_fields() -> BTreeSet<RenderedField> {
    [
        RenderedField::ContainerCommand,
        RenderedField::ContainerEntrypoint,
        RenderedField::ContainerUser,
        RenderedField::ContainerWorkdir,
        RenderedField::ContainerHostname,
        RenderedField::ContainerLabel,
        RenderedField::ContainerEnvironment,
        RenderedField::ContainerRestartPolicy,
        RenderedField::ContainerNamedVolumeMount,
        RenderedField::ContainerNetworkAttachment,
        RenderedField::PodNetworkAttachment,
        RenderedField::ContainerNetworkAlias,
        RenderedField::PodNetworkAlias,
        RenderedField::ContainerNetworkStaticIpv4,
        RenderedField::PodNetworkStaticIpv4,
        RenderedField::ContainerNetworkStaticIpv6,
        RenderedField::PodNetworkStaticIpv6,
        RenderedField::ContainerNetworkStaticMac,
        RenderedField::PodNetworkStaticMac,
        RenderedField::ContainerPortMapping,
        RenderedField::PodPortMapping,
        RenderedField::ContainerDnsServers,
        RenderedField::ContainerDnsSearch,
        RenderedField::ContainerDnsOptions,
        RenderedField::PodDnsServers,
        RenderedField::PodDnsSearch,
        RenderedField::PodDnsOptions,
        RenderedField::ContainerHostAlias,
        RenderedField::PodHostAlias,
        RenderedField::ContainerNetworkOrder,
        RenderedField::NetworkIpamSubnet,
        RenderedField::NetworkIpamGateway,
        RenderedField::NetworkIpamRange,
        RenderedField::NetworkRouteDestination,
        RenderedField::NetworkRouteGateway,
        RenderedField::NetworkRouteMetric,
        RenderedField::NetworkRouteTypeUnicast,
        RenderedField::NetworkRouteTypeBlackhole,
        RenderedField::NetworkRouteTypeUnreachable,
        RenderedField::NetworkRouteTypeProhibit,
    ]
    .into_iter()
    .collect()
}

#[allow(clippy::too_many_lines)]
fn expected_field_claim(
    field: RenderedField,
) -> (
    RenderingOperationCategory,
    Option<CliFlag>,
    CliValueShape,
    LibpodBodyMember,
    &'static str,
    &'static [&'static str],
    &'static str,
) {
    const CONTAINER_MODEL: &[&str] = &["pkg/specgen/specgen.go"];
    const CONTAINER_VOLUME_MODEL: &[&str] = &["pkg/specgen/specgen.go", "pkg/specgen/volumes.go"];
    const CONTAINER_HANDLER: &str = "pkg/api/handlers/libpod/containers_create.go";
    match field {
        RenderedField::ContainerCommand => (
            RenderingOperationCategory::ContainerCreate,
            None,
            CliValueShape::ArgumentArray,
            LibpodBodyMember::Command,
            "cmd/podman/containers/create.go",
            CONTAINER_MODEL,
            CONTAINER_HANDLER,
        ),
        RenderedField::ContainerEntrypoint => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Entrypoint),
            CliValueShape::ArgumentArray,
            LibpodBodyMember::Entrypoint,
            "cmd/podman/common/create.go",
            CONTAINER_MODEL,
            CONTAINER_HANDLER,
        ),
        RenderedField::ContainerUser => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::User),
            CliValueShape::ContainerUser,
            LibpodBodyMember::User,
            "cmd/podman/common/create.go",
            CONTAINER_MODEL,
            CONTAINER_HANDLER,
        ),
        RenderedField::ContainerWorkdir => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Workdir),
            CliValueShape::AbsoluteContainerPath,
            LibpodBodyMember::WorkDir,
            "cmd/podman/common/create.go",
            CONTAINER_MODEL,
            CONTAINER_HANDLER,
        ),
        RenderedField::ContainerHostname => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Hostname),
            CliValueShape::Hostname,
            LibpodBodyMember::Hostname,
            "cmd/podman/common/create.go",
            CONTAINER_MODEL,
            CONTAINER_HANDLER,
        ),
        RenderedField::ContainerLabel => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Label),
            CliValueShape::LabelAssignment,
            LibpodBodyMember::Labels,
            "cmd/podman/common/create.go",
            CONTAINER_MODEL,
            CONTAINER_HANDLER,
        ),
        RenderedField::ContainerEnvironment => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Environment),
            CliValueShape::EnvironmentAssignment,
            LibpodBodyMember::Env,
            "cmd/podman/common/create.go",
            CONTAINER_MODEL,
            CONTAINER_HANDLER,
        ),
        RenderedField::ContainerRestartPolicy => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Restart),
            CliValueShape::RestartPolicy,
            LibpodBodyMember::RestartPolicy,
            "cmd/podman/common/create.go",
            CONTAINER_MODEL,
            CONTAINER_HANDLER,
        ),
        RenderedField::ContainerNamedVolumeMount => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Volume),
            CliValueShape::NamedVolumeMount,
            LibpodBodyMember::Volumes,
            "cmd/podman/common/create.go",
            CONTAINER_VOLUME_MODEL,
            CONTAINER_HANDLER,
        ),
        _ => unreachable!("networking fields are handled by the dedicated evidence matrix"),
    }
}

#[derive(Clone, Copy)]
enum ExpectedNetworkSource {
    Podman(&'static str),
    Common,
}

type NetworkFieldClaim = (
    RenderingOperationCategory,
    Option<CliFlag>,
    CliValueShape,
    LibpodBodyMember,
    ExpectedNetworkSource,
    &'static [ExpectedNetworkSource],
    ExpectedNetworkSource,
);

#[allow(clippy::too_many_lines)] // The finite M6-B2 field matrix is deliberately audit-friendly.
fn expected_network_field_claim(field: RenderedField) -> Option<NetworkFieldClaim> {
    const CONTAINER_NETWORK: &[ExpectedNetworkSource] = &[
        ExpectedNetworkSource::Podman("pkg/specgen/specgen.go"),
        ExpectedNetworkSource::Common,
    ];
    const POD_NETWORK: &[ExpectedNetworkSource] = &[
        ExpectedNetworkSource::Podman("pkg/specgen/podspecgen.go"),
        ExpectedNetworkSource::Common,
    ];
    const CONTAINER_NETWORK_CONFIG: &[ExpectedNetworkSource] =
        &[ExpectedNetworkSource::Podman("pkg/specgen/specgen.go")];
    const POD_NETWORK_CONFIG: &[ExpectedNetworkSource] = &[ExpectedNetworkSource::Podman("pkg/specgen/podspecgen.go")];
    const CONTAINER_NETWORK_ORDER: &[ExpectedNetworkSource] = &[
        ExpectedNetworkSource::Podman("pkg/specgen/specgen.go"),
        ExpectedNetworkSource::Podman("pkg/specgen/namespaces.go"),
    ];
    const NETWORK: &[ExpectedNetworkSource] = &[ExpectedNetworkSource::Common];
    const CONTAINER_HANDLER: ExpectedNetworkSource =
        ExpectedNetworkSource::Podman("pkg/api/handlers/libpod/containers_create.go");
    const POD_HANDLER: ExpectedNetworkSource = ExpectedNetworkSource::Podman("pkg/api/handlers/libpod/pods.go");
    const NETWORK_HANDLER: ExpectedNetworkSource = ExpectedNetworkSource::Podman("pkg/api/handlers/libpod/networks.go");
    let container_network = |shape| {
        (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Network),
            shape,
            LibpodBodyMember::Networks,
            ExpectedNetworkSource::Podman("cmd/podman/common/netflags.go"),
            CONTAINER_NETWORK,
            CONTAINER_HANDLER,
        )
    };
    let pod_network = |shape| {
        (
            RenderingOperationCategory::PodCreate,
            Some(CliFlag::Network),
            shape,
            LibpodBodyMember::Networks,
            ExpectedNetworkSource::Podman("cmd/podman/pods/create.go"),
            POD_NETWORK,
            POD_HANDLER,
        )
    };
    let container_network_config = |flag, shape, member| {
        (
            RenderingOperationCategory::ContainerCreate,
            Some(flag),
            shape,
            member,
            ExpectedNetworkSource::Podman("cmd/podman/common/netflags.go"),
            CONTAINER_NETWORK_CONFIG,
            CONTAINER_HANDLER,
        )
    };
    let pod_network_config = |flag, shape, member| {
        (
            RenderingOperationCategory::PodCreate,
            Some(flag),
            shape,
            member,
            ExpectedNetworkSource::Podman("cmd/podman/pods/create.go"),
            POD_NETWORK_CONFIG,
            POD_HANDLER,
        )
    };
    let network_create = |flag, shape, member| {
        (
            RenderingOperationCategory::NetworkCreate,
            Some(flag),
            shape,
            member,
            ExpectedNetworkSource::Podman("cmd/podman/networks/create.go"),
            NETWORK,
            NETWORK_HANDLER,
        )
    };
    Some(match field {
        RenderedField::ContainerNetworkAttachment => container_network(CliValueShape::NetworkAttachment),
        RenderedField::PodNetworkAttachment => pod_network(CliValueShape::NetworkAttachment),
        RenderedField::ContainerNetworkAlias => container_network(CliValueShape::NetworkAlias),
        RenderedField::PodNetworkAlias => pod_network(CliValueShape::NetworkAlias),
        RenderedField::ContainerNetworkStaticIpv4 | RenderedField::ContainerNetworkStaticIpv6 => {
            container_network(CliValueShape::StaticIp)
        }
        RenderedField::PodNetworkStaticIpv4 | RenderedField::PodNetworkStaticIpv6 => {
            pod_network(CliValueShape::StaticIp)
        }
        RenderedField::ContainerNetworkStaticMac => container_network(CliValueShape::StaticMac),
        RenderedField::PodNetworkStaticMac => pod_network(CliValueShape::StaticMac),
        RenderedField::ContainerPortMapping => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Publish),
            CliValueShape::PortMapping,
            LibpodBodyMember::Portmappings,
            ExpectedNetworkSource::Podman("cmd/podman/common/netflags.go"),
            CONTAINER_NETWORK,
            CONTAINER_HANDLER,
        ),
        RenderedField::PodPortMapping => (
            RenderingOperationCategory::PodCreate,
            Some(CliFlag::Publish),
            CliValueShape::PortMapping,
            LibpodBodyMember::Portmappings,
            ExpectedNetworkSource::Podman("cmd/podman/pods/create.go"),
            POD_NETWORK,
            POD_HANDLER,
        ),
        RenderedField::ContainerDnsServers => {
            container_network_config(CliFlag::Dns, CliValueShape::DnsServer, LibpodBodyMember::DnsServer)
        }
        RenderedField::ContainerDnsSearch => container_network_config(
            CliFlag::DnsSearch,
            CliValueShape::DnsSearch,
            LibpodBodyMember::DnsSearch,
        ),
        RenderedField::ContainerDnsOptions => container_network_config(
            CliFlag::DnsOption,
            CliValueShape::DnsOption,
            LibpodBodyMember::DnsOption,
        ),
        RenderedField::PodDnsServers => {
            pod_network_config(CliFlag::Dns, CliValueShape::DnsServer, LibpodBodyMember::DnsServer)
        }
        RenderedField::PodDnsSearch => pod_network_config(
            CliFlag::DnsSearch,
            CliValueShape::DnsSearch,
            LibpodBodyMember::DnsSearch,
        ),
        RenderedField::PodDnsOptions => pod_network_config(
            CliFlag::DnsOption,
            CliValueShape::DnsOption,
            LibpodBodyMember::DnsOption,
        ),
        RenderedField::ContainerHostAlias => {
            container_network_config(CliFlag::AddHost, CliValueShape::HostAlias, LibpodBodyMember::Hostadd)
        }
        RenderedField::PodHostAlias => {
            pod_network_config(CliFlag::AddHost, CliValueShape::HostAlias, LibpodBodyMember::Hostadd)
        }
        RenderedField::ContainerNetworkOrder => (
            RenderingOperationCategory::ContainerCreate,
            Some(CliFlag::Network),
            CliValueShape::NetworkOrder,
            LibpodBodyMember::NetworkOrder,
            ExpectedNetworkSource::Podman("cmd/podman/common/netflags.go"),
            CONTAINER_NETWORK_ORDER,
            CONTAINER_HANDLER,
        ),
        RenderedField::NetworkIpamSubnet => {
            network_create(CliFlag::Subnet, CliValueShape::NetworkSubnet, LibpodBodyMember::Subnets)
        }
        RenderedField::NetworkIpamGateway => network_create(
            CliFlag::Gateway,
            CliValueShape::NetworkGateway,
            LibpodBodyMember::Subnets,
        ),
        RenderedField::NetworkIpamRange => network_create(
            CliFlag::IpRange,
            CliValueShape::NetworkIpRange,
            LibpodBodyMember::Subnets,
        ),
        RenderedField::NetworkRouteDestination
        | RenderedField::NetworkRouteGateway
        | RenderedField::NetworkRouteMetric
        | RenderedField::NetworkRouteTypeUnicast
        | RenderedField::NetworkRouteTypeBlackhole
        | RenderedField::NetworkRouteTypeUnreachable
        | RenderedField::NetworkRouteTypeProhibit => {
            network_create(CliFlag::Route, CliValueShape::NetworkRoute, LibpodBodyMember::Routes)
        }
        _ => return None,
    })
}

fn source_matches_network_evidence(
    source: &SourceReference,
    line: &ReviewedRenderingLine,
    expected: ExpectedNetworkSource,
) -> bool {
    match expected {
        ExpectedNetworkSource::Podman(path) => source_matches_podman(source, &line.revision, path),
        ExpectedNetworkSource::Common => {
            source.repository == line.common_module.repository
                && source.revision == line.common_module.revision
                && source.path == common_model_source_path(line)
                && source.module.as_ref() == Some(&line.common_module)
                && is_lowercase_sha40(&source.revision)
        }
    }
}

fn common_model_source_path(line: &ReviewedRenderingLine) -> &'static str {
    match line.version.as_str() {
        "5.4.0" | "5.5.0" | "5.6.0" => "libnetwork/types/network.go",
        "5.7.0" | "5.8.6" | "6.0.0" | "6.1.0" => "common/libnetwork/types/network.go",
        _ => unreachable!("reviewed renderer versions are validated before field evidence"),
    }
}

fn rendering_source_paths(category: &str) -> Option<(&'static str, &'static str, Option<&'static str>)> {
    match category {
        "network-create" => Some((
            "cmd/podman/networks/create.go",
            "pkg/api/server/register_networks.go",
            Some("pkg/api/handlers/libpod/networks.go"),
        )),
        "volume-create" => Some((
            "cmd/podman/volumes/create.go",
            "pkg/api/server/register_volumes.go",
            Some("pkg/api/handlers/libpod/volumes.go"),
        )),
        "secret-create" => Some((
            "cmd/podman/secrets/create.go",
            "pkg/api/server/register_secrets.go",
            Some("pkg/api/handlers/libpod/secrets.go"),
        )),
        "image-pull" => Some(("cmd/podman/images/pull.go", "pkg/api/server/register_images.go", None)),
        "pod-create" => Some((
            "cmd/podman/pods/create.go",
            "pkg/api/server/register_pods.go",
            Some("pkg/api/handlers/libpod/pods.go"),
        )),
        "container-create" => Some((
            "cmd/podman/containers/create.go",
            "pkg/api/server/register_containers.go",
            Some("pkg/api/handlers/libpod/containers_create.go"),
        )),
        "pod-start" => Some(("cmd/podman/pods/start.go", "pkg/api/server/register_pods.go", None)),
        "container-start" => Some((
            "cmd/podman/containers/start.go",
            "pkg/api/server/register_containers.go",
            None,
        )),
        _ => None,
    }
}

fn source_matches_podman(source: &SourceReference, revision: &str, path: &str) -> bool {
    source.repository == SourceRepository::ContainersPodman
        && source.revision == revision
        && source.path == path
        && source.module.is_none()
        && is_lowercase_sha40(&source.revision)
}

fn valid_common_module(version: &str, module: &ModulePin) -> bool {
    matches!(
        (
            version,
            module.repository,
            module.path.as_str(),
            module.version.as_str(),
            module.revision.as_str()
        ),
        (
            "5.4.0",
            SourceRepository::ContainersCommon,
            "github.com/containers/common",
            "v0.62.0",
            "cde1afdf623bdb9595d1fdc7acffa6e2f03a06b2"
        ) | (
            "5.5.0",
            SourceRepository::ContainersCommon,
            "github.com/containers/common",
            "v0.63.0",
            "92927328862e4837d87bcbc0725b713387399984"
        ) | (
            "5.6.0",
            SourceRepository::ContainersCommon,
            "github.com/containers/common",
            "v0.64.1",
            "c007f37a6c55a53a7d41419d094c815bf2e46ca3"
        ) | (
            "5.7.0",
            SourceRepository::ContainersContainerLibs,
            "go.podman.io/common",
            "v0.66.0",
            "8163ca799c317e3dea886be7228406ec8cf06abc"
        ) | (
            "5.8.6",
            SourceRepository::ContainersContainerLibs,
            "go.podman.io/common",
            "v0.67.1",
            "c8b7f74383aa8bac5a84118db1a6fa870602af63"
        ) | (
            "6.0.0",
            SourceRepository::ContainersContainerLibs,
            "go.podman.io/common",
            "v0.68.0",
            "bb6a37c8946a977f24a8eed4d97b2dc3608cd05e"
        ) | (
            "6.1.0",
            SourceRepository::PodmanContainerToolsContainerLibs,
            "go.podman.io/common",
            "v0.69.1",
            "e47c7ccf66a1b89b0807e053dd426ff26eedd7a7"
        )
    )
}

fn canonical_version(version: &Version) -> String {
    format!("{}.{}.{}", version.major, version.minor, version.patch)
}

fn is_lowercase_sha40(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn managed_image_sources(plan: &DeploymentPlan) -> BTreeMap<DeploymentResourceId, String> {
    plan.operations()
        .iter()
        .filter_map(|operation| match operation.resource_intent() {
            DeploymentResource::Image(image) => Some((image.identity().clone(), image.source().as_str().to_owned())),
            _ => None,
        })
        .collect()
}

#[allow(clippy::too_many_lines)] // One exhaustive renderer boundary keeps the semantic surface auditable.
fn unsupported_fields(
    resource: &DeploymentResource,
    version: &str,
    context: crate::TargetExecutionContext,
) -> Vec<&'static str> {
    match resource {
        DeploymentResource::Image(image) => {
            let mut fields = Vec::new();
            if !matches!(
                image.source().classification(),
                crate::ImageSourceClassification::Portable
            ) {
                fields.push("source.portability");
            }
            if !supports_image_policy(version) {
                fields.push("pull_policy.target_version");
            }
            fields
        }
        DeploymentResource::Volume(volume) => {
            if !supports_volume_ownership(version) && (volume.uid().is_some() || volume.gid().is_some()) {
                vec!["owner.target_version"]
            } else {
                Vec::new()
            }
        }
        DeploymentResource::Network(network) => {
            let mut fields = Vec::new();
            if !supports_network_route_type(version)
                && network
                    .routes()
                    .iter()
                    .any(|route| route.route_type() != RouteType::Unicast)
            {
                fields.push("routes.route_type");
            }
            fields
        }
        DeploymentResource::Pod(pod) => {
            let mut fields = Vec::new();
            if context != crate::TargetExecutionContext::Rootful
                && pod.networks().iter().any(network_attachment_has_static_address)
            {
                fields.push("networks.static_address_requires_rootful");
            }
            if !pod.infra_mounts().is_empty() {
                fields.push("infra_mounts");
            }
            fields
        }
        DeploymentResource::Container(container) => {
            let mut fields = Vec::new();
            fields.extend(unsupported_runtime_fields(container, version));
            if container.pod().is_some() && !container.networks().is_empty() {
                fields.push("networks");
            }
            if container.pod().is_some() && !container.ports().is_empty() {
                fields.push("ports");
            }
            if container.pod().is_some()
                && (!container.dns().servers().is_empty()
                    || !container.dns().search().is_empty()
                    || !container.dns().options().is_empty())
            {
                fields.push("dns");
            }
            if container.pod().is_some() && !container.host_aliases().is_empty() {
                fields.push("host_aliases");
            }
            if container.pod().is_some() && container.network_order().is_some() {
                fields.push("network_order");
            }
            if context != crate::TargetExecutionContext::Rootful
                && container.networks().iter().any(network_attachment_has_static_address)
            {
                fields.push("networks.static_address_requires_rootful");
            }
            if container.network_order().is_some() && !supports_network_order(version) {
                fields.push("network_order");
            }
            if !container.secret_grants().is_empty() && !secret_grants_renderable(container.secret_grants()) {
                fields.push("secret_grants");
            }
            if container.mounts().iter().any(mount_has_unsupported_subpath) {
                fields.push("mounts.subpath_nocopy");
            }
            let settings = container.settings();
            if settings.environment().iter().any(|assignment| {
                matches!(
                    assignment.value(),
                    crate::DeploymentEnvironmentValue::SensitiveInline(_)
                )
            }) {
                fields.push("environment.sensitive_inline");
            }
            if settings
                .environment()
                .iter()
                .any(|assignment| matches!(assignment.value(), crate::DeploymentEnvironmentValue::External(_)))
            {
                fields.push("environment.external");
            }
            if container.pod().is_some() && settings.restart_policy().is_some() {
                fields.push("restart_policy.pod_member");
            }
            fields
        }
        _ => Vec::new(),
    }
}

fn network_attachment_has_static_address(attachment: &NetworkAttachment) -> bool {
    attachment.static_ipv4().is_some() || attachment.static_ipv6().is_some() || attachment.static_mac().is_some()
}

fn supports_network_order(version: &str) -> bool {
    matches!(version, "6.0.0" | "6.1.0")
}

fn supports_image_policy(version: &str) -> bool {
    !matches!(version, "5.4.0" | "5.5.0")
}

fn supports_volume_ownership(version: &str) -> bool {
    supports_image_policy(version)
}

fn supports_network_route_type(version: &str) -> bool {
    supports_network_order(version)
}

fn append_network_arguments(
    arguments: &mut Vec<String>,
    networks: &[NetworkAttachment],
    order: Option<&[DeploymentResourceId]>,
) -> bool {
    let attachments = ordered_network_attachments(networks, order);
    let Some(attachments) = attachments else {
        return false;
    };
    for network in attachments {
        let Some(value) = network_attachment_cli_value(network) else {
            return false;
        };
        arguments.push("--network".to_owned());
        arguments.push(value);
    }
    true
}

fn ordered_network_attachments<'a>(
    attachments: &'a [NetworkAttachment],
    order: Option<&[DeploymentResourceId]>,
) -> Option<Vec<&'a NetworkAttachment>> {
    match order {
        None => Some(attachments.iter().collect()),
        Some(order) => order
            .iter()
            .map(|identity| attachments.iter().find(|attachment| attachment.network() == identity))
            .collect(),
    }
}

fn network_attachment_cli_value(attachment: &NetworkAttachment) -> Option<String> {
    let name = attachment.network().name();
    if name.contains([':', ',']) {
        return None;
    }
    let mut value = name.to_owned();
    let mut options = Vec::new();
    options.extend(attachment.aliases().iter().map(|alias| format!("alias={alias}")));
    if let Some(ipv4) = attachment.static_ipv4() {
        options.push(format!("ip={ipv4}"));
    }
    if let Some(ipv6) = attachment.static_ipv6() {
        options.push(format!("ip6={ipv6}"));
    }
    if let Some(mac) = attachment.static_mac() {
        options.push(format!("mac={}", mac.as_str()));
    }
    if !options.is_empty() {
        value.push(':');
        value.push_str(&options.join(","));
    }
    Some(value)
}

fn network_configuration(networks: &[NetworkAttachment]) -> Value {
    let networks = networks
        .iter()
        .map(|network| {
            let mut options = Map::new();
            if !network.aliases().is_empty() {
                options.insert("aliases".to_owned(), string_array(network.aliases()));
            }
            let static_ips = [network.static_ipv4(), network.static_ipv6()]
                .into_iter()
                .flatten()
                .map(|address| Value::String(address.to_string()))
                .collect::<Vec<_>>();
            if !static_ips.is_empty() {
                options.insert("static_ips".to_owned(), Value::Array(static_ips));
            }
            if let Some(mac) = network.static_mac() {
                options.insert("static_mac".to_owned(), Value::String(mac.as_str().to_owned()));
            }
            (network.network().name().to_owned(), Value::Object(options))
        })
        .collect::<Map<String, Value>>();
    Value::Object(networks)
}

fn append_port_arguments(arguments: &mut Vec<String>, ports: &[PortMapping]) {
    for port in ports {
        arguments.push("--publish".to_owned());
        arguments.push(port_mapping_cli_value(port));
    }
}

fn port_mapping_cli_value(port: &PortMapping) -> String {
    let mut value = String::new();
    if let Some(host_ip) = port.host_ip() {
        if host_ip.is_ipv6() {
            value.push('[');
            value.push_str(&host_ip.to_string());
            value.push(']');
        } else {
            value.push_str(&host_ip.to_string());
        }
        value.push(':');
    }
    let protocol = match port.protocol() {
        PortProtocol::Tcp => "tcp",
        PortProtocol::Udp => "udp",
        PortProtocol::Sctp => "sctp",
    };
    format!("{value}{}:{}/{protocol}", port.host_port(), port.container_port())
}

fn append_dns_arguments(arguments: &mut Vec<String>, dns: &crate::DnsConfiguration) {
    for server in dns.servers() {
        arguments.extend(["--dns".to_owned(), server.to_string()]);
    }
    for search in dns.search() {
        arguments.extend(["--dns-search".to_owned(), search.clone()]);
    }
    for option in dns.options() {
        arguments.extend(["--dns-option".to_owned(), option.clone()]);
    }
}

fn append_host_alias_arguments(arguments: &mut Vec<String>, aliases: &[HostAlias]) {
    for alias in aliases {
        arguments.extend(["--add-host".to_owned(), host_alias_value(alias)]);
    }
}

fn append_networking_json(
    body: &mut Map<String, Value>,
    ports: &[PortMapping],
    dns: &crate::DnsConfiguration,
    aliases: &[HostAlias],
    network_order: Option<&[DeploymentResourceId]>,
) {
    if !ports.is_empty() {
        body.insert(
            "portmappings".to_owned(),
            Value::Array(
                ports
                    .iter()
                    .map(|port| {
                        let protocol = match port.protocol() {
                            PortProtocol::Tcp => "tcp",
                            PortProtocol::Udp => "udp",
                            PortProtocol::Sctp => "sctp",
                        };
                        json!({
                            "host_ip": port.host_ip().map_or_else(String::new, |address| address.to_string()),
                            "host_port": port.host_port(),
                            "container_port": port.container_port(),
                            "range": 1,
                            "protocol": protocol,
                        })
                    })
                    .collect(),
            ),
        );
    }
    if !dns.servers().is_empty() {
        body.insert(
            "dns_server".to_owned(),
            Value::Array(
                dns.servers()
                    .iter()
                    .map(|server| Value::String(server.to_string()))
                    .collect(),
            ),
        );
    }
    if !dns.search().is_empty() {
        body.insert("dns_search".to_owned(), string_array(dns.search()));
    }
    if !dns.options().is_empty() {
        body.insert("dns_option".to_owned(), string_array(dns.options()));
    }
    if !aliases.is_empty() {
        body.insert(
            "hostadd".to_owned(),
            Value::Array(
                aliases
                    .iter()
                    .map(|alias| Value::String(host_alias_value(alias)))
                    .collect(),
            ),
        );
    }
    if let Some(order) = network_order {
        body.insert(
            "networkOrder".to_owned(),
            Value::Array(
                order
                    .iter()
                    .map(|network| Value::String(network.name().to_owned()))
                    .collect(),
            ),
        );
    }
}

fn host_alias_value(alias: &HostAlias) -> String {
    // Podman separates the hostname once at the first colon. The validated hostname grammar
    // excludes colons, so a literal IPv6 address remains unambiguous without brackets in both
    // the CLI and Libpod `hostadd` representation.
    format!("{}:{}", alias.hostname(), alias.address())
}

fn append_network_create_arguments(arguments: &mut Vec<String>, network: &NetworkIntent) {
    for subnet in network.subnets() {
        arguments.extend(["--subnet".to_owned(), subnet.subnet().as_str().to_owned()]);
    }
    for subnet in network.subnets() {
        if let Some(gateway) = subnet.gateway() {
            arguments.extend(["--gateway".to_owned(), gateway.to_string()]);
        }
    }
    for subnet in network.subnets() {
        if let Some((start, end)) = subnet.range() {
            arguments.extend(["--ip-range".to_owned(), format!("{start}-{end}")]);
        }
    }
    for route in network.routes() {
        arguments.extend(["--route".to_owned(), network_route_cli_value(route)]);
    }
}

fn network_route_cli_value(route: &NetworkRoute) -> String {
    let next_hop = match route.route_type() {
        RouteType::Unicast => route.gateway().map_or_else(String::new, |gateway| gateway.to_string()),
        RouteType::Blackhole => "blackhole".to_owned(),
        RouteType::Unreachable => "unreachable".to_owned(),
        RouteType::Prohibit => "prohibit".to_owned(),
    };
    let mut value = format!("{},{}", route.destination().as_str(), next_hop);
    if let Some(metric) = route.metric() {
        let _ = write!(value, ",{metric}");
    }
    value
}

fn network_create_configuration(network: &NetworkIntent, supports_route_type: bool) -> Value {
    let mut body = Map::new();
    body.insert("name".to_owned(), Value::String(network.identity().name().to_owned()));
    if !network.subnets().is_empty() {
        body.insert(
            "subnets".to_owned(),
            Value::Array(
                network
                    .subnets()
                    .iter()
                    .map(|subnet| {
                        let mut value = Map::new();
                        value.insert("subnet".to_owned(), Value::String(subnet.subnet().as_str().to_owned()));
                        if let Some(gateway) = subnet.gateway() {
                            value.insert("gateway".to_owned(), Value::String(gateway.to_string()));
                        }
                        if let Some((start, end)) = subnet.range() {
                            value.insert(
                                "lease_range".to_owned(),
                                json!({"start_ip": start.to_string(), "end_ip": end.to_string()}),
                            );
                        }
                        Value::Object(value)
                    })
                    .collect(),
            ),
        );
    }
    if !network.routes().is_empty() {
        body.insert(
            "routes".to_owned(),
            Value::Array(
                network
                    .routes()
                    .iter()
                    .map(|route| {
                        let mut value = Map::new();
                        value.insert(
                            "destination".to_owned(),
                            Value::String(route.destination().as_str().to_owned()),
                        );
                        if let Some(gateway) = route.gateway() {
                            value.insert("gateway".to_owned(), Value::String(gateway.to_string()));
                        }
                        if let Some(metric) = route.metric() {
                            value.insert("metric".to_owned(), json!(metric));
                        }
                        if supports_route_type {
                            let route_type = match route.route_type() {
                                RouteType::Unicast => "unicast",
                                RouteType::Blackhole => "blackhole",
                                RouteType::Unreachable => "unreachable",
                                RouteType::Prohibit => "prohibit",
                            };
                            value.insert("route_type".to_owned(), Value::String(route_type.to_owned()));
                        }
                        Value::Object(value)
                    })
                    .collect(),
            ),
        );
    }
    Value::Object(body)
}

fn append_mount_arguments(arguments: &mut Vec<String>, mounts: &[MountIntent]) -> bool {
    for mount in mounts {
        match mount {
            MountIntent::NamedVolume(mount) => {
                if mount.subpath().is_some() {
                    if !cli_safe_mount_component(mount.source().name())
                        || !cli_safe_mount_component(mount.destination().as_str())
                        || mount
                            .subpath()
                            .is_some_and(|subpath| !cli_safe_mount_component(subpath.as_str()))
                    {
                        return false;
                    }
                    arguments.extend([
                        "--mount".to_owned(),
                        mount_cli_value(
                            "volume",
                            mount.source().name(),
                            mount.destination().as_str(),
                            mount.access(),
                            mount.subpath().map(crate::VolumeSubpath::as_str),
                        ),
                    ]);
                    continue;
                }
                if !cli_safe_mount_component(mount.source().name())
                    || !cli_safe_mount_component(mount.destination().as_str())
                {
                    return false;
                }
                let access = if mount.is_read_only() { "ro" } else { "rw" };
                let copy = match mount.copy_mode() {
                    crate::NamedVolumeCopyMode::Copy => "copy",
                    crate::NamedVolumeCopyMode::NoCopy => "nocopy",
                };
                let options = format!("{access},{copy}");
                arguments.extend([
                    "--volume".to_owned(),
                    format!("{}:{}:{options}", mount.source().name(), mount.destination().as_str()),
                ]);
            }
            MountIntent::Bind(mount) => {
                if !cli_safe_mount_component(mount.source().as_str())
                    || !cli_safe_mount_component(mount.destination().as_str())
                {
                    return false;
                }
                arguments.extend([
                    "--mount".to_owned(),
                    mount_cli_value(
                        "bind",
                        mount.source().as_str(),
                        mount.destination().as_str(),
                        mount.access(),
                        None,
                    ),
                ]);
            }
            MountIntent::Tmpfs(mount) => {
                if !cli_safe_mount_component(mount.destination().as_str()) {
                    return false;
                }
                arguments.extend([
                    "--mount".to_owned(),
                    mount_cli_value("tmpfs", "", mount.destination().as_str(), mount.access(), None),
                ]);
            }
        }
    }
    true
}

fn cli_safe_mount_component(value: &str) -> bool {
    !value.contains([':', ','])
}

fn mount_json(mounts: &[MountIntent]) -> Value {
    Value::Array(
        mounts
            .iter()
            .filter_map(|mount| match mount {
                MountIntent::NamedVolume(mount) if mount.subpath().is_none() => {
                    let access = if mount.is_read_only() { "ro" } else { "rw" };
                    let copy = match mount.copy_mode() {
                        crate::NamedVolumeCopyMode::Copy => "copy",
                        crate::NamedVolumeCopyMode::NoCopy => "nocopy",
                    };
                    let options = vec![access.to_owned(), copy.to_owned()];
                    Some(json!({"Name": mount.source().name(), "Dest": mount.destination().as_str(), "Options": options}))
                }
                MountIntent::NamedVolume(mount) => Some(json!({
                    "Name": mount.source().name(),
                    "Dest": mount.destination().as_str(),
                    "Options": if mount.access().is_read_only() { vec!["ro"] } else { Vec::<&str>::new() },
                    "SubPath": mount.subpath().map(crate::VolumeSubpath::as_str),
                })),
                MountIntent::Bind(_) | MountIntent::Tmpfs(_) => None,
            })
            .collect(),
    )
}

fn mount_cli_value(
    kind: &str,
    source: &str,
    destination: &str,
    access: crate::MountAccess,
    subpath: Option<&str>,
) -> String {
    let mut value = format!("type={kind}");
    if !source.is_empty() {
        let _ = write!(value, ",source={source}");
    }
    let _ = write!(value, ",target={destination}");
    if access.is_read_only() {
        value.push_str(",readonly");
    }
    if let Some(subpath) = subpath {
        let _ = write!(value, ",subpath={subpath}");
    }
    value
}

fn native_mount_json(mounts: &[MountIntent]) -> Value {
    Value::Array(
        mounts
            .iter()
            .filter_map(|mount| match mount {
                MountIntent::Bind(mount) => Some(json!({
                    "destination": mount.destination().as_str(),
                    "type": "bind",
                    "source": mount.source().as_str(),
                    "options": if mount.access().is_read_only() { ["ro"] } else { ["rw"] },
                })),
                MountIntent::Tmpfs(mount) => Some(json!({
                    "destination": mount.destination().as_str(),
                    "type": "tmpfs",
                    "source": "tmpfs",
                    "options": if mount.access().is_read_only() { ["ro"] } else { ["rw"] },
                })),
                MountIntent::NamedVolume(_) => None,
            })
            .collect(),
    )
}

fn append_secret_grants_arguments(arguments: &mut Vec<String>, grants: &[SecretGrant]) -> bool {
    for grant in grants {
        if !cli_safe_mount_component(grant.source().name()) {
            return false;
        }
        let value = match grant {
            SecretGrant::Mount { .. } => {
                let mut value = format!("source={},type=mount", grant.source().name());
                if let Some(target) = grant.mount_target() {
                    if !cli_safe_mount_component(target.as_str()) {
                        return false;
                    }
                    let _ = write!(value, ",target={}", target.as_str());
                }
                if let Some(uid) = grant.mount_uid() {
                    let _ = write!(value, ",uid={}", uid.get());
                }
                if let Some(gid) = grant.mount_gid() {
                    let _ = write!(value, ",gid={}", gid.get());
                }
                if let Some(mode) = grant.mount_mode() {
                    let _ = write!(value, ",mode={:o}", mode.get());
                }
                value
            }
            SecretGrant::Environment { target, .. } => {
                format!("source={},type=env,target={}", grant.source().name(), target.as_str())
            }
        };
        arguments.extend(["--secret".to_owned(), value]);
    }
    true
}

fn append_secret_grants_json(body: &mut Map<String, Value>, grants: &[SecretGrant]) {
    let mut mounts = Vec::new();
    let mut environment = Map::new();
    for grant in grants {
        match grant {
            SecretGrant::Mount { .. } => {
                let mut value = Map::new();
                value.insert("Source".to_owned(), Value::String(grant.source().name().to_owned()));
                if let Some(target) = grant.mount_target() {
                    value.insert("Target".to_owned(), Value::String(target.as_str().to_owned()));
                }
                if let Some(uid) = grant.mount_uid() {
                    value.insert("UID".to_owned(), json!(uid.get()));
                }
                if let Some(gid) = grant.mount_gid() {
                    value.insert("GID".to_owned(), json!(gid.get()));
                }
                // `podman --secret` defaults omitted mount modes to 0444, while the Libpod
                // request model decodes an omitted `Mode` as zero. Emit the CLI default
                // explicitly in the API representation so both non-executing planes retain
                // the same declared semantics.
                value.insert(
                    "Mode".to_owned(),
                    json!(grant.mount_mode().map_or(0o444, SecretMode::get)),
                );
                mounts.push(Value::Object(value));
            }
            SecretGrant::Environment { target, .. } => {
                environment.insert(
                    target.as_str().to_owned(),
                    Value::String(grant.source().name().to_owned()),
                );
            }
        }
    }
    if !mounts.is_empty() {
        body.insert("secrets".to_owned(), Value::Array(mounts));
    }
    if !environment.is_empty() {
        body.insert("secret_env".to_owned(), Value::Object(environment));
    }
}

fn volume_create_cli_arguments(volume: &crate::VolumeIntent) -> Vec<String> {
    let mut arguments = vec!["volume".to_owned(), "create".to_owned()];
    if let Some(uid) = volume.uid() {
        arguments.extend(["--uid".to_owned(), uid.get().to_string()]);
    }
    if let Some(gid) = volume.gid() {
        arguments.extend(["--gid".to_owned(), gid.get().to_string()]);
    }
    arguments.push(volume.identity().name().to_owned());
    arguments
}

fn volume_create_json(volume: &crate::VolumeIntent) -> Value {
    let mut body = Map::new();
    body.insert("Name".to_owned(), Value::String(volume.identity().name().to_owned()));
    if let Some(uid) = volume.uid() {
        body.insert("UID".to_owned(), json!(uid.get()));
    }
    if let Some(gid) = volume.gid() {
        body.insert("GID".to_owned(), json!(gid.get()));
    }
    Value::Object(body)
}

fn mount_has_unsupported_subpath(mount: &MountIntent) -> bool {
    matches!(mount, MountIntent::NamedVolume(volume) if volume.subpath().is_some() && volume.copy_mode() == crate::NamedVolumeCopyMode::NoCopy)
}

fn secret_grants_renderable(grants: &[SecretGrant]) -> bool {
    let mut arguments = Vec::new();
    append_secret_grants_arguments(&mut arguments, grants)
}

fn unsupported_runtime_fields(container: &crate::ContainerIntent, version: &str) -> Vec<&'static str> {
    let runtime = container.runtime();
    let mut fields = Vec::new();
    if runtime.startup_health().is_some() && !matches!(runtime.health(), Some(crate::HealthCheck::Command(_))) {
        fields.push("runtime.startup_health_requires_health");
    }
    for (health, field) in [
        (
            runtime.health().and_then(|health| match health {
                crate::HealthCheck::Command(configured) => Some(configured.command()),
                crate::HealthCheck::Disabled => None,
            }),
            "runtime.health.command",
        ),
        (
            runtime.startup_health().map(crate::StartupHealthCheck::command),
            "runtime.startup_health.command",
        ),
    ] {
        if health.is_some_and(health_command_is_sensitive) {
            fields.push(field);
        }
    }
    if !runtime.logging().journald_labels().is_empty() && !supports_journald_labels(version) {
        fields.push("runtime.logging.journald_labels.target_version");
    }
    if runtime.logging().driver().is_none()
        && (runtime.logging().max_size().is_some() || !runtime.logging().journald_labels().is_empty())
    {
        fields.push("runtime.logging.driver");
    }
    if !runtime.logging().journald_labels().is_empty() && runtime.logging().driver() != Some(crate::LogDriver::Journald)
    {
        fields.push("runtime.logging.journald_labels");
    }
    if runtime.logging().max_size().is_some() && runtime.logging().driver() != Some(crate::LogDriver::K8sFile) {
        fields.push("runtime.logging.max_size");
    }
    if runtime.resources().rlimits().iter().any(rlimit_is_unlimited) && !supports_unlimited_rlimit(version) {
        fields.push("runtime.resources.rlimits.unlimited.target_version");
    }
    if runtime.security().privileged() == Some(true)
        && (!runtime.security().cap_add().is_empty() || !runtime.security().cap_drop().is_empty())
    {
        fields.push("runtime.security.privileged_capabilities");
    }
    if runtime
        .security()
        .cap_add()
        .iter()
        .any(|capability| runtime.security().cap_drop().contains(capability))
    {
        fields.push("runtime.security.capability_overlap");
    }
    if runtime.security().read_write_tmpfs() == Some(true) && runtime.security().read_only_filesystem() != Some(true) {
        fields.push("runtime.security.read_write_tmpfs");
    }
    if container.pod().is_some() && !runtime.namespaces().is_empty() {
        fields.push("runtime.namespaces.pod_member");
    }
    fields
}

fn health_command_is_sensitive(command: &crate::HealthCommand) -> bool {
    matches!(
        command,
        crate::HealthCommand::SensitiveInlineShell(_)
            | crate::HealthCommand::SensitiveInlineExec(_)
            | crate::HealthCommand::ExternalShell(_)
            | crate::HealthCommand::ExternalExec(_)
    )
}

fn rlimit_is_unlimited(limit: &crate::Rlimit) -> bool {
    matches!(limit.soft(), crate::RlimitValue::Unlimited) || matches!(limit.hard(), crate::RlimitValue::Unlimited)
}

fn supports_journald_labels(version: &str) -> bool {
    matches!(version, "6.0.0" | "6.1.0")
}

fn supports_unlimited_rlimit(version: &str) -> bool {
    !matches!(version, "5.4.0" | "5.5.0")
}

fn append_container_runtime_arguments(
    arguments: &mut Vec<String>,
    container: &crate::ContainerIntent,
    identity: &DeploymentResourceId,
    version: &str,
) -> Result<(), RenderingFinding> {
    let runtime = container.runtime();
    if let Some(health) = runtime.health() {
        match health {
            crate::HealthCheck::Disabled => arguments.push("--no-healthcheck".to_owned()),
            crate::HealthCheck::Command(configured) => {
                arguments.extend([
                    "--health-cmd".to_owned(),
                    health_command_cli(configured.command(), identity, "runtime.health.command")?,
                ]);
                append_health_arguments(arguments, configured);
            }
        }
    }
    if let Some(startup) = runtime.startup_health() {
        arguments.extend([
            "--health-startup-cmd".to_owned(),
            health_command_cli(startup.command(), identity, "runtime.startup_health.command")?,
        ]);
        if let Some(interval) = startup.interval() {
            arguments.extend(["--health-startup-interval".to_owned(), health_interval_cli(interval)]);
        }
        if let Some(timeout) = startup.timeout() {
            arguments.extend([
                "--health-startup-timeout".to_owned(),
                health_duration_cli(timeout.nanoseconds()),
            ]);
        }
        if let Some(retries) = startup.retries() {
            arguments.extend(["--health-startup-retries".to_owned(), retries.value().to_string()]);
        }
        if let Some(successes) = startup.successes() {
            arguments.extend(["--health-startup-success".to_owned(), successes.value().to_string()]);
        }
    }
    let logging = runtime.logging();
    if let Some(driver) = logging.driver() {
        arguments.extend(["--log-driver".to_owned(), log_driver_name(driver).to_owned()]);
    }
    if let Some(size) = logging.max_size() {
        arguments.extend(["--log-opt".to_owned(), format!("max-size={}", size.bytes())]);
    }
    if !logging.journald_labels().is_empty() && !supports_journald_labels(version) {
        return Err(rendering_runtime_finding(
            identity,
            "runtime.logging.journald_labels.target_version",
        ));
    }
    for label in logging.journald_labels() {
        arguments.extend([
            "--log-opt".to_owned(),
            format!("label={}={}", label.key().as_str(), label.value().as_str()),
        ]);
    }
    append_security_arguments(arguments, runtime.security());
    append_namespace_arguments(arguments, runtime.namespaces());
    append_resource_arguments(arguments, runtime.resources(), identity, version)?;
    Ok(())
}

fn append_health_arguments(arguments: &mut Vec<String>, health: &crate::ConfiguredHealthCheck) {
    if let Some(interval) = health.interval() {
        arguments.extend(["--health-interval".to_owned(), health_interval_cli(interval)]);
    }
    if let Some(timeout) = health.timeout() {
        arguments.extend([
            "--health-timeout".to_owned(),
            health_duration_cli(timeout.nanoseconds()),
        ]);
    }
    if let Some(retries) = health.retries() {
        arguments.extend(["--health-retries".to_owned(), retries.value().to_string()]);
    }
    if let Some(period) = health.start_period() {
        arguments.extend([
            "--health-start-period".to_owned(),
            health_duration_cli(period.nanoseconds()),
        ]);
    }
    if let Some(on_failure) = health.on_failure() {
        arguments.extend([
            "--health-on-failure".to_owned(),
            health_on_failure_name(on_failure).to_owned(),
        ]);
    }
}

fn append_security_arguments(arguments: &mut Vec<String>, security: &crate::SecuritySettings) {
    if let Some(value) = security.privileged() {
        arguments.push(format!("--privileged={value}"));
    }
    if let Some(value) = security.no_new_privileges() {
        arguments.extend(["--security-opt".to_owned(), format!("no-new-privileges={value}")]);
    }
    if let Some(value) = security.read_only_filesystem() {
        arguments.push(format!("--read-only={value}"));
    }
    if let Some(value) = security.read_write_tmpfs() {
        arguments.push(format!("--read-only-tmpfs={value}"));
    }
    for capability in security.cap_add() {
        arguments.extend(["--cap-add".to_owned(), capability.as_str().to_owned()]);
    }
    for capability in security.cap_drop() {
        arguments.extend(["--cap-drop".to_owned(), capability.as_str().to_owned()]);
    }
}

fn append_namespace_arguments(arguments: &mut Vec<String>, namespaces: &crate::ContainerNamespaceSettings) {
    if let Some(pid) = namespaces.pid() {
        arguments.extend(["--pid".to_owned(), namespace_mode_name(pid).to_owned()]);
    }
    if let Some(ipc) = namespaces.ipc() {
        arguments.extend(["--ipc".to_owned(), ipc_namespace_mode_name(ipc).to_owned()]);
    }
    if let Some(uts) = namespaces.uts() {
        arguments.extend(["--uts".to_owned(), namespace_mode_name(uts).to_owned()]);
    }
    if let Some(cgroup) = namespaces.cgroup() {
        arguments.extend(["--cgroupns".to_owned(), namespace_mode_name(cgroup).to_owned()]);
    }
}

fn append_resource_arguments(
    arguments: &mut Vec<String>,
    resources: &crate::ContainerResourceControls,
    identity: &DeploymentResourceId,
    version: &str,
) -> Result<(), RenderingFinding> {
    for (flag, value) in [
        ("--cpu-shares", resources.cpu_shares()),
        ("--cpu-period", resources.cpu_period()),
        ("--cpu-quota", resources.cpu_quota()),
        ("--memory", resources.memory_bytes()),
        ("--pids-limit", resources.pids()),
    ] {
        if let Some(value) = value {
            arguments.extend([flag.to_owned(), value.to_string()]);
        }
    }
    for rlimit in resources.rlimits() {
        if rlimit_is_unlimited(rlimit) && !supports_unlimited_rlimit(version) {
            return Err(rendering_runtime_finding(
                identity,
                "runtime.resources.rlimits.unlimited.target_version",
            ));
        }
        arguments.extend(["--ulimit".to_owned(), rlimit_cli_value(rlimit)]);
    }
    Ok(())
}

fn append_container_runtime_json(
    body: &mut Map<String, Value>,
    container: &crate::ContainerIntent,
    identity: &DeploymentResourceId,
    version: &str,
) -> Result<(), RenderingFinding> {
    let runtime = container.runtime();
    if let Some(health) = runtime.health() {
        body.insert(
            "healthconfig".to_owned(),
            health_json(health, identity, "runtime.health.command")?,
        );
        if let crate::HealthCheck::Command(configured) = health {
            if let Some(on_failure) = configured.on_failure() {
                body.insert(
                    "health_check_on_failure_action".to_owned(),
                    json!(health_on_failure_code(on_failure)),
                );
            }
        }
    }
    if let Some(startup) = runtime.startup_health() {
        let mut configuration = Map::new();
        configuration.insert(
            "Test".to_owned(),
            health_command_json(startup.command(), identity, "runtime.startup_health.command")?,
        );
        append_startup_health_json(&mut configuration, startup);
        body.insert("startupHealthConfig".to_owned(), Value::Object(configuration));
    }
    append_logging_json(body, runtime.logging(), identity, version)?;
    append_security_json(body, runtime.security());
    append_namespace_json(body, runtime.namespaces());
    append_resource_json(body, runtime.resources(), identity, version)?;
    Ok(())
}

fn health_command_cli(
    command: &crate::HealthCommand,
    identity: &DeploymentResourceId,
    field: &'static str,
) -> Result<String, RenderingFinding> {
    serde_json::to_string(&health_command_json(command, identity, field)?)
        .map_err(|_| rendering_runtime_finding(identity, field))
}

fn health_command_json(
    command: &crate::HealthCommand,
    identity: &DeploymentResourceId,
    field: &'static str,
) -> Result<Value, RenderingFinding> {
    match command {
        crate::HealthCommand::Shell(command) => Ok(json!(["CMD-SHELL", command.as_str()])),
        crate::HealthCommand::Exec(arguments) => Ok(Value::Array(
            std::iter::once(Value::String("CMD".to_owned()))
                .chain(arguments.values().iter().cloned().map(Value::String))
                .collect(),
        )),
        crate::HealthCommand::SensitiveInlineShell(_)
        | crate::HealthCommand::SensitiveInlineExec(_)
        | crate::HealthCommand::ExternalShell(_)
        | crate::HealthCommand::ExternalExec(_) => Err(rendering_runtime_finding(identity, field)),
    }
}

fn health_json(
    health: &crate::HealthCheck,
    identity: &DeploymentResourceId,
    field: &'static str,
) -> Result<Value, RenderingFinding> {
    match health {
        crate::HealthCheck::Disabled => Ok(json!({"Test": ["NONE"]})),
        crate::HealthCheck::Command(configured) => {
            let mut configuration = Map::new();
            configuration.insert(
                "Test".to_owned(),
                health_command_json(configured.command(), identity, field)?,
            );
            append_configured_health_json(&mut configuration, configured);
            Ok(Value::Object(configuration))
        }
    }
}

fn append_configured_health_json(configuration: &mut Map<String, Value>, health: &crate::ConfiguredHealthCheck) {
    if let Some(interval) = health.interval() {
        configuration.insert("Interval".to_owned(), json!(health_interval_nanoseconds(interval)));
    }
    if let Some(timeout) = health.timeout() {
        configuration.insert("Timeout".to_owned(), json!(timeout.nanoseconds()));
    }
    if let Some(retries) = health.retries() {
        configuration.insert("Retries".to_owned(), json!(retries.value()));
    }
    if let Some(period) = health.start_period() {
        configuration.insert("StartPeriod".to_owned(), json!(period.nanoseconds()));
    }
}

fn append_startup_health_json(configuration: &mut Map<String, Value>, health: &crate::StartupHealthCheck) {
    if let Some(interval) = health.interval() {
        configuration.insert("Interval".to_owned(), json!(health_interval_nanoseconds(interval)));
    }
    if let Some(timeout) = health.timeout() {
        configuration.insert("Timeout".to_owned(), json!(timeout.nanoseconds()));
    }
    if let Some(retries) = health.retries() {
        configuration.insert("Retries".to_owned(), json!(retries.value()));
    }
    if let Some(successes) = health.successes() {
        configuration.insert("Successes".to_owned(), json!(successes.value()));
    }
}

fn health_interval_cli(interval: crate::HealthInterval) -> String {
    match interval {
        crate::HealthInterval::Disabled => "disable".to_owned(),
        crate::HealthInterval::Every(duration) => health_duration_cli(duration.nanoseconds()),
    }
}

fn health_interval_nanoseconds(interval: crate::HealthInterval) -> i64 {
    match interval {
        crate::HealthInterval::Disabled => 0,
        crate::HealthInterval::Every(duration) => duration.nanoseconds(),
    }
}

fn health_duration_cli(nanoseconds: i64) -> String {
    format!("{nanoseconds}ns")
}

fn health_on_failure_name(value: crate::HealthOnFailure) -> &'static str {
    match value {
        crate::HealthOnFailure::None => "none",
        crate::HealthOnFailure::Kill => "kill",
        crate::HealthOnFailure::Restart => "restart",
        crate::HealthOnFailure::Stop => "stop",
    }
}

const fn health_on_failure_code(value: crate::HealthOnFailure) -> u8 {
    match value {
        crate::HealthOnFailure::None => 0,
        crate::HealthOnFailure::Kill => 2,
        crate::HealthOnFailure::Restart => 3,
        crate::HealthOnFailure::Stop => 4,
    }
}

fn log_driver_name(value: crate::LogDriver) -> &'static str {
    match value {
        crate::LogDriver::Journald => "journald",
        crate::LogDriver::K8sFile => "k8s-file",
    }
}

fn append_logging_json(
    body: &mut Map<String, Value>,
    logging: &crate::LoggingSettings,
    identity: &DeploymentResourceId,
    version: &str,
) -> Result<(), RenderingFinding> {
    if logging.driver().is_none() && logging.max_size().is_none() && logging.journald_labels().is_empty() {
        return Ok(());
    }
    if !logging.journald_labels().is_empty() && !supports_journald_labels(version) {
        return Err(rendering_runtime_finding(
            identity,
            "runtime.logging.journald_labels.target_version",
        ));
    }
    let mut configuration = Map::new();
    if let Some(driver) = logging.driver() {
        configuration.insert("driver".to_owned(), Value::String(log_driver_name(driver).to_owned()));
    }
    if let Some(size) = logging.max_size() {
        configuration.insert("size".to_owned(), json!(size.bytes()));
    }
    if !logging.journald_labels().is_empty() {
        configuration.insert(
            "labels".to_owned(),
            Value::Object(
                logging
                    .journald_labels()
                    .iter()
                    .map(|label| {
                        (
                            label.key().as_str().to_owned(),
                            Value::String(label.value().as_str().to_owned()),
                        )
                    })
                    .collect(),
            ),
        );
    }
    body.insert("log_configuration".to_owned(), Value::Object(configuration));
    Ok(())
}

fn append_security_json(body: &mut Map<String, Value>, security: &crate::SecuritySettings) {
    if let Some(value) = security.privileged() {
        body.insert("privileged".to_owned(), json!(value));
    }
    if let Some(value) = security.no_new_privileges() {
        body.insert("no_new_privileges".to_owned(), json!(value));
    }
    if let Some(value) = security.read_only_filesystem() {
        body.insert("read_only_filesystem".to_owned(), json!(value));
    }
    if let Some(value) = security.read_write_tmpfs() {
        body.insert("read_write_tmpfs".to_owned(), json!(value));
    }
    if !security.cap_add().is_empty() {
        body.insert(
            "cap_add".to_owned(),
            Value::Array(
                security
                    .cap_add()
                    .iter()
                    .map(|value| Value::String(value.as_str().to_owned()))
                    .collect(),
            ),
        );
    }
    if !security.cap_drop().is_empty() {
        body.insert(
            "cap_drop".to_owned(),
            Value::Array(
                security
                    .cap_drop()
                    .iter()
                    .map(|value| Value::String(value.as_str().to_owned()))
                    .collect(),
            ),
        );
    }
}

fn append_namespace_json(body: &mut Map<String, Value>, namespaces: &crate::ContainerNamespaceSettings) {
    for (member, mode) in [
        ("pidns", namespaces.pid().map(namespace_mode_name)),
        ("ipcns", namespaces.ipc().map(ipc_namespace_mode_name)),
        ("utsns", namespaces.uts().map(namespace_mode_name)),
        ("cgroupns", namespaces.cgroup().map(namespace_mode_name)),
    ] {
        if let Some(mode) = mode {
            body.insert(member.to_owned(), json!({"nsmode": mode}));
        }
    }
}

fn namespace_mode_name(value: crate::NamespaceMode) -> &'static str {
    match value {
        crate::NamespaceMode::Private => "private",
        crate::NamespaceMode::Host => "host",
    }
}

fn ipc_namespace_mode_name(value: crate::IpcNamespaceMode) -> &'static str {
    match value {
        crate::IpcNamespaceMode::Private => "private",
        crate::IpcNamespaceMode::Host => "host",
        crate::IpcNamespaceMode::Shareable => "shareable",
        crate::IpcNamespaceMode::None => "none",
    }
}

fn append_resource_json(
    body: &mut Map<String, Value>,
    resources: &crate::ContainerResourceControls,
    identity: &DeploymentResourceId,
    version: &str,
) -> Result<(), RenderingFinding> {
    let mut limits = Map::new();
    let mut cpu = Map::new();
    if let Some(value) = resources.cpu_shares() {
        cpu.insert("shares".to_owned(), json!(value));
    }
    if let Some(value) = resources.cpu_period() {
        cpu.insert("period".to_owned(), json!(value));
    }
    if let Some(value) = resources.cpu_quota() {
        cpu.insert("quota".to_owned(), json!(value));
    }
    if !cpu.is_empty() {
        limits.insert("cpu".to_owned(), Value::Object(cpu));
    }
    if let Some(value) = resources.memory_bytes() {
        limits.insert("memory".to_owned(), json!({"limit": value}));
    }
    if let Some(value) = resources.pids() {
        limits.insert("pids".to_owned(), json!({"limit": value}));
    }
    if !limits.is_empty() {
        body.insert("resource_limits".to_owned(), Value::Object(limits));
    }
    if !resources.rlimits().is_empty() {
        let mut limits = Vec::with_capacity(resources.rlimits().len());
        for rlimit in resources.rlimits() {
            if rlimit_is_unlimited(rlimit) && !supports_unlimited_rlimit(version) {
                return Err(rendering_runtime_finding(
                    identity,
                    "runtime.resources.rlimits.unlimited.target_version",
                ));
            }
            limits.push(json!({
                "type": rlimit_kind_name(rlimit.kind()),
                "soft": rlimit_value_json(rlimit.soft()),
                "hard": rlimit_value_json(rlimit.hard()),
            }));
        }
        body.insert("r_limits".to_owned(), Value::Array(limits));
    }
    Ok(())
}

fn rlimit_cli_value(rlimit: &crate::Rlimit) -> String {
    format!(
        "{}={}:{}",
        rlimit_kind_cli_name(rlimit.kind()),
        rlimit_value_cli(rlimit.soft()),
        rlimit_value_cli(rlimit.hard())
    )
}

fn rlimit_kind_cli_name(kind: crate::RlimitKind) -> &'static str {
    match kind {
        crate::RlimitKind::NoFile => "nofile",
        crate::RlimitKind::NProc => "nproc",
    }
}

fn rlimit_kind_name(kind: crate::RlimitKind) -> &'static str {
    match kind {
        crate::RlimitKind::NoFile => "RLIMIT_NOFILE",
        crate::RlimitKind::NProc => "RLIMIT_NPROC",
    }
}

fn rlimit_value_cli(value: crate::RlimitValue) -> String {
    match value {
        crate::RlimitValue::Finite(value) => value.to_string(),
        crate::RlimitValue::Unlimited => "-1".to_owned(),
    }
}

fn rlimit_value_json(value: crate::RlimitValue) -> Value {
    match value {
        crate::RlimitValue::Finite(value) => json!(value),
        crate::RlimitValue::Unlimited => json!(-1),
    }
}

fn rendering_runtime_finding(identity: &DeploymentResourceId, field: &'static str) -> RenderingFinding {
    RenderingFinding::new(
        DiagnosticCode::RenderingUnsupported,
        Some(identity.clone()),
        Some(field),
    )
}

fn append_container_setting_arguments(
    arguments: &mut Vec<String>,
    container: &crate::ContainerIntent,
    identity: &DeploymentResourceId,
) -> Result<(), RenderingFinding> {
    let settings = container.settings();
    if let Some(entrypoint) = settings.entrypoint() {
        let encoded = serde_json::to_string(entrypoint.values()).map_err(|_| {
            RenderingFinding::new(
                DiagnosticCode::RenderingUnsupported,
                Some(identity.clone()),
                Some("entrypoint"),
            )
        })?;
        arguments.push("--entrypoint".to_owned());
        arguments.push(encoded);
    }
    if let Some(user) = settings.user() {
        arguments.push("--user".to_owned());
        arguments.push(user.as_str().to_owned());
    }
    if let Some(workdir) = settings.workdir() {
        arguments.push("--workdir".to_owned());
        arguments.push(workdir.path().as_str().to_owned());
    }
    if let Some(hostname) = settings.hostname() {
        arguments.push("--hostname".to_owned());
        arguments.push(hostname.as_str().to_owned());
    }
    for label in settings.labels() {
        arguments.push("--label".to_owned());
        arguments.push(format!("{}={}", label.key().as_str(), label.value().as_str()));
    }
    for assignment in settings.environment() {
        let crate::DeploymentEnvironmentValue::Public(value) = assignment.value() else {
            return Err(RenderingFinding::new(
                DiagnosticCode::RenderingUnsupported,
                Some(identity.clone()),
                Some("environment"),
            ));
        };
        arguments.push("--env".to_owned());
        arguments.push(format!("{}={}", assignment.name().as_str(), value.as_str()));
    }
    if let Some(restart) = settings.restart_policy() {
        arguments.push("--restart".to_owned());
        arguments.push(restart_policy_name(restart).to_owned());
    }
    Ok(())
}

fn append_container_setting_json(
    body: &mut Map<String, Value>,
    container: &crate::ContainerIntent,
    identity: &DeploymentResourceId,
) -> Result<(), RenderingFinding> {
    let settings = container.settings();
    if let Some(command) = settings.command() {
        body.insert("command".to_owned(), string_array(command.values()));
    }
    if let Some(entrypoint) = settings.entrypoint() {
        body.insert("entrypoint".to_owned(), string_array(entrypoint.values()));
    }
    if let Some(user) = settings.user() {
        body.insert("user".to_owned(), Value::String(user.as_str().to_owned()));
    }
    if let Some(workdir) = settings.workdir() {
        body.insert("work_dir".to_owned(), Value::String(workdir.path().as_str().to_owned()));
    }
    if let Some(hostname) = settings.hostname() {
        body.insert("hostname".to_owned(), Value::String(hostname.as_str().to_owned()));
    }
    if !settings.labels().is_empty() {
        body.insert(
            "labels".to_owned(),
            Value::Object(
                settings
                    .labels()
                    .iter()
                    .map(|label| {
                        (
                            label.key().as_str().to_owned(),
                            Value::String(label.value().as_str().to_owned()),
                        )
                    })
                    .collect(),
            ),
        );
    }
    if !settings.environment().is_empty() {
        let mut environment = Map::new();
        for assignment in settings.environment() {
            let crate::DeploymentEnvironmentValue::Public(value) = assignment.value() else {
                return Err(RenderingFinding::new(
                    DiagnosticCode::RenderingUnsupported,
                    Some(identity.clone()),
                    Some("environment"),
                ));
            };
            environment.insert(
                assignment.name().as_str().to_owned(),
                Value::String(value.as_str().to_owned()),
            );
        }
        body.insert("env".to_owned(), Value::Object(environment));
    }
    if let Some(restart) = settings.restart_policy() {
        body.insert(
            "restart_policy".to_owned(),
            Value::String(restart_policy_name(restart).to_owned()),
        );
    }
    if !container.mounts().is_empty() {
        let volumes = mount_json(container.mounts());
        if !volumes.as_array().is_some_and(Vec::is_empty) {
            body.insert("volumes".to_owned(), volumes);
        }
        let mounts = native_mount_json(container.mounts());
        if !mounts.as_array().is_some_and(Vec::is_empty) {
            body.insert("mounts".to_owned(), mounts);
        }
    }
    Ok(())
}

fn string_array(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

fn restart_policy_name(policy: RestartPolicy) -> &'static str {
    match policy {
        RestartPolicy::No => "no",
        RestartPolicy::OnFailure => "on-failure",
        RestartPolicy::Always => "always",
        RestartPolicy::UnlessStopped => "unless-stopped",
    }
}

fn percent_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            let _ = write!(output, "%{byte:02X}");
        }
    }
    output
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn resource_kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Container => "container",
        ResourceKind::Pod => "pod",
        ResourceKind::Network => "network",
        ResourceKind::Volume => "volume",
        ResourceKind::Image => "image",
        ResourceKind::Secret => "secret",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{RENDERING_CATALOGUE_JSON, host_alias_value, parse_renderer_catalogue};
    use crate::HostAlias;

    fn decoded_catalogue() -> serde_json::Value {
        serde_json::from_str(RENDERING_CATALOGUE_JSON).expect("embedded catalogue JSON")
    }

    fn encoded_catalogue(value: &serde_json::Value) -> String {
        serde_json::to_string(value).expect("catalogue JSON")
    }

    #[test]
    fn renderer_catalogue_rejects_invalid_operation_evidence_revisions_and_noncanonical_versions() {
        let missing_category = RENDERING_CATALOGUE_JSON.replacen(
            "\"category\": \"container-start\"",
            "\"category\": \"container-run\"",
            1,
        );
        assert!(parse_renderer_catalogue(&missing_category).is_err());

        let invalid_source = RENDERING_CATALOGUE_JSON.replacen(
            "pkg/api/server/register_networks.go",
            "pkg/api/server/register_unknown.go",
            1,
        );
        assert!(parse_renderer_catalogue(&invalid_source).is_err());

        let invalid_revision = RENDERING_CATALOGUE_JSON.replacen(
            "f9f7d48b24b1ca4403f189caaeab1cb8ff4a9aa2",
            "F9f7d48b24b1ca4403f189caaeab1cb8ff4a9aa2",
            1,
        );
        assert!(parse_renderer_catalogue(&invalid_revision).is_err());

        let build_metadata =
            RENDERING_CATALOGUE_JSON.replacen("\"version\": \"5.4.0\"", "\"version\": \"5.4.0+build\"", 1);
        assert!(parse_renderer_catalogue(&build_metadata).is_err());
    }

    #[test]
    fn renderer_catalogue_requires_the_complete_strict_field_matrix() {
        let mut missing = decoded_catalogue();
        missing["reviewed_lines"][0]["field_evidence"]
            .as_array_mut()
            .expect("field evidence")
            .pop();
        assert!(parse_renderer_catalogue(&encoded_catalogue(&missing)).is_err());

        let mut duplicate = decoded_catalogue();
        let fields = duplicate["reviewed_lines"][0]["field_evidence"]
            .as_array_mut()
            .expect("field evidence");
        fields.push(fields[0].clone());
        assert!(parse_renderer_catalogue(&encoded_catalogue(&duplicate)).is_err());

        let mut wrong_operation = decoded_catalogue();
        wrong_operation["reviewed_lines"][0]["field_evidence"][0]["operation"] = serde_json::json!("pod-create");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_operation)).is_err());

        let mut wrong_claim = decoded_catalogue();
        wrong_claim["reviewed_lines"][0]["field_evidence"][1]["cli"]["flag"] = serde_json::json!("--label");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_claim)).is_err());

        let mut wrong_shape = decoded_catalogue();
        wrong_shape["reviewed_lines"][0]["field_evidence"][1]["cli"]["value_shape"] = serde_json::json!("hostname");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_shape)).is_err());

        let mut wrong_member = decoded_catalogue();
        wrong_member["reviewed_lines"][0]["field_evidence"][1]["libpod"]["json_member"] = serde_json::json!("hostname");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_member)).is_err());
    }

    #[test]
    fn renderer_catalogue_rejects_nonimmutable_field_evidence_and_capability_substitutions() {
        let mut legacy_source_shape = decoded_catalogue();
        legacy_source_shape["reviewed_lines"][0]["field_evidence"][0]["cli_source"] =
            serde_json::json!("https://github.com/containers/podman/blob/main/cmd/podman/containers/create.go");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&legacy_source_shape)).is_err());
        let mut wrong_module_version = decoded_catalogue();
        wrong_module_version["reviewed_lines"][0]["common_module"]["version"] = serde_json::json!("v0.0.0");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_module_version)).is_err());

        let mut wrong_module_revision = decoded_catalogue();
        wrong_module_revision["reviewed_lines"][0]["common_module"]["revision"] =
            serde_json::json!("0000000000000000000000000000000000000000");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_module_revision)).is_err());

        let mut wrong_module_repository = decoded_catalogue();
        wrong_module_repository["reviewed_lines"][0]["common_module"]["repository"] =
            serde_json::json!("podman-container-tools-container-libs");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_module_repository)).is_err());

        let mut wrong_module_path = decoded_catalogue();
        wrong_module_path["reviewed_lines"][0]["common_module"]["path"] = serde_json::json!("go.podman.io/common");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_module_path)).is_err());

        let mut mutable_field_source = decoded_catalogue();
        mutable_field_source["reviewed_lines"][0]["field_evidence"][0]["cli_source"]["revision"] =
            serde_json::json!("main");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&mutable_field_source)).is_err());

        let mut wrong_field_repository = decoded_catalogue();
        wrong_field_repository["reviewed_lines"][0]["field_evidence"][0]["cli_source"]["repository"] =
            serde_json::json!("containers-common");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_field_repository)).is_err());

        let mut wrong_field_path = decoded_catalogue();
        wrong_field_path["reviewed_lines"][0]["field_evidence"][0]["cli_source"]["path"] =
            serde_json::json!("cmd/podman/unknown.go");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_field_path)).is_err());

        let mut wrong_model = decoded_catalogue();
        wrong_model["reviewed_lines"][0]["field_evidence"][8]["model_sources"][1]["repository"] =
            serde_json::json!("containers-common");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_model)).is_err());

        let mut mutable_model = decoded_catalogue();
        mutable_model["reviewed_lines"][0]["field_evidence"][8]["model_sources"][1]["revision"] =
            serde_json::json!("main");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&mutable_model)).is_err());

        let mut wrong_model_path = decoded_catalogue();
        wrong_model_path["reviewed_lines"][0]["field_evidence"][8]["model_sources"][1]["path"] =
            serde_json::json!("pkg/specgen/not-volumes.go");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_model_path)).is_err());

        let mut wrong_handler = decoded_catalogue();
        wrong_handler["reviewed_lines"][0]["field_evidence"][9]["handler_source"]["repository"] =
            serde_json::json!("containers-common");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_handler)).is_err());

        let mut mutable_handler = decoded_catalogue();
        mutable_handler["reviewed_lines"][0]["field_evidence"][9]["handler_source"]["revision"] =
            serde_json::json!("main");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&mutable_handler)).is_err());

        let mut wrong_handler_path = decoded_catalogue();
        wrong_handler_path["reviewed_lines"][0]["field_evidence"][9]["handler_source"]["path"] =
            serde_json::json!("pkg/api/handlers/libpod/not-containers.go");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_handler_path)).is_err());

        let mut missing_line = decoded_catalogue();
        missing_line["reviewed_lines"]
            .as_array_mut()
            .expect("reviewed lines")
            .remove(0);
        assert!(parse_renderer_catalogue(&encoded_catalogue(&missing_line)).is_err());

        let mut substituted_line = decoded_catalogue();
        let lines = substituted_line["reviewed_lines"]
            .as_array_mut()
            .expect("reviewed lines");
        lines[0] = lines[1].clone();
        assert!(parse_renderer_catalogue(&encoded_catalogue(&substituted_line)).is_err());
    }

    #[test]
    fn renderer_catalogue_rejects_b2_field_mutations_and_fabricated_pre_six_support() {
        const FIRST_B2_FIELD: usize = 9;
        const NETWORK_ORDER_FIELD: usize = 29;
        const ROUTE_TYPE_BLACKHOLE_FIELD: usize = 37;

        let mut missing = decoded_catalogue();
        missing["reviewed_lines"][0]["field_evidence"]
            .as_array_mut()
            .expect("field evidence")
            .remove(FIRST_B2_FIELD);
        assert!(parse_renderer_catalogue(&encoded_catalogue(&missing)).is_err());

        let mut duplicate = decoded_catalogue();
        let fields = duplicate["reviewed_lines"][0]["field_evidence"]
            .as_array_mut()
            .expect("field evidence");
        fields.push(fields[FIRST_B2_FIELD].clone());
        assert!(parse_renderer_catalogue(&encoded_catalogue(&duplicate)).is_err());

        for (path, value) in [
            (["operation", "", ""], serde_json::json!("pod-create")),
            (["cli", "flag", ""], serde_json::json!("--publish")),
            (["cli", "value_shape", ""], serde_json::json!("port-mapping")),
            (["libpod", "json_member", ""], serde_json::json!("portmappings")),
        ] {
            let mut mutated = decoded_catalogue();
            let field = &mut mutated["reviewed_lines"][0]["field_evidence"][FIRST_B2_FIELD];
            if path[1].is_empty() {
                field[path[0]] = value;
            } else if path[2].is_empty() {
                field[path[0]][path[1]] = value;
            }
            assert!(parse_renderer_catalogue(&encoded_catalogue(&mutated)).is_err());
        }

        for (path, value) in [
            (["repository", ""], serde_json::json!("containers-podman")),
            (["revision", ""], serde_json::json!("main")),
            (["path", ""], serde_json::json!("libnetwork/types/fabricated.go")),
            (
                ["module", "revision"],
                serde_json::json!("0000000000000000000000000000000000000000"),
            ),
        ] {
            let mut mutated = decoded_catalogue();
            let source = &mut mutated["reviewed_lines"][0]["field_evidence"][FIRST_B2_FIELD]["model_sources"][1];
            if path[1].is_empty() {
                source[path[0]] = value;
            } else {
                source[path[0]][path[1]] = value;
            }
            assert!(parse_renderer_catalogue(&encoded_catalogue(&mutated)).is_err());
        }

        let mut fabricated_pre_six_support = decoded_catalogue();
        fabricated_pre_six_support["reviewed_lines"][0]["field_evidence"][NETWORK_ORDER_FIELD]["availability"] =
            serde_json::json!("exact");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&fabricated_pre_six_support)).is_err());

        let mut fabricated_pre_six_route_type = decoded_catalogue();
        fabricated_pre_six_route_type["reviewed_lines"][0]["field_evidence"][ROUTE_TYPE_BLACKHOLE_FIELD]["availability"] =
            serde_json::json!("exact");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&fabricated_pre_six_route_type)).is_err());

        for line_index in [3, 4, 5] {
            let mut substituted_repository = decoded_catalogue();
            substituted_repository["reviewed_lines"][line_index]["field_evidence"][FIRST_B2_FIELD]["model_sources"]
                [1]["repository"] = serde_json::json!("containers-common");
            assert!(parse_renderer_catalogue(&encoded_catalogue(&substituted_repository)).is_err());
        }

        for line_index in [3, 4, 5, 6] {
            let mut wrong_common_path = decoded_catalogue();
            wrong_common_path["reviewed_lines"][line_index]["field_evidence"][FIRST_B2_FIELD]["model_sources"][1]["path"] =
                serde_json::json!("libnetwork/types/network.go");
            assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_common_path)).is_err());
        }

        let mut missing_network_order_wire_model = decoded_catalogue();
        missing_network_order_wire_model["reviewed_lines"][5]["field_evidence"][NETWORK_ORDER_FIELD]["model_sources"]
            [0]["path"] = serde_json::json!("pkg/specgen/namespaces.go");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&missing_network_order_wire_model)).is_err());
    }

    #[test]
    fn renderer_catalogue_rejects_b3_claim_and_per_release_evidence_mutations() {
        let mut missing_claim = decoded_catalogue();
        missing_claim["runtime_field_claims"]
            .as_array_mut()
            .expect("runtime claims")
            .pop();
        assert!(parse_renderer_catalogue(&encoded_catalogue(&missing_claim)).is_err());

        let mut duplicate_claim = decoded_catalogue();
        let claims = duplicate_claim["runtime_field_claims"]
            .as_array_mut()
            .expect("runtime claims");
        claims.push(claims[0].clone());
        assert!(parse_renderer_catalogue(&encoded_catalogue(&duplicate_claim)).is_err());

        for (path, value) in [
            (["cli", "flag"], serde_json::json!("--health-timeout")),
            (["cli", "value_shape"], serde_json::json!("health-duration")),
            (["libpod", "json_member"], serde_json::json!("healthconfig_timeout")),
        ] {
            let mut substituted = decoded_catalogue();
            substituted["runtime_field_claims"][1][path[0]][path[1]] = value;
            assert!(parse_renderer_catalogue(&encoded_catalogue(&substituted)).is_err());
        }

        for (claim_index, wrong_shape) in [
            (0, "boolean"),
            (6, "boolean"),
            (7, "boolean"),
            (12, "boolean"),
            (13, "boolean"),
            (14, "boolean"),
            (15, "string"),
            (16, "boolean"),
            (21, "boolean"),
            (25, "boolean"),
            (30, "boolean"),
        ] {
            let mut substituted = decoded_catalogue();
            substituted["runtime_field_claims"][claim_index]["libpod"]["value_shape"] = serde_json::json!(wrong_shape);
            assert!(parse_renderer_catalogue(&encoded_catalogue(&substituted)).is_err());
        }

        let mut fabricated_journald = decoded_catalogue();
        let fields = fabricated_journald["reviewed_lines"][0]["runtime_evidence"]["exact_fields"]
            .as_array_mut()
            .expect("exact runtime fields");
        fields.push(serde_json::json!("container-log-journald-labels"));
        let gated = fabricated_journald["reviewed_lines"][0]["runtime_evidence"]["target_gated_fields"]
            .as_array_mut()
            .expect("target-gated runtime fields");
        gated.retain(|field| field != "container-log-journald-labels");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&fabricated_journald)).is_err());

        let mut fabricated_unlimited = decoded_catalogue();
        let fields = fabricated_unlimited["reviewed_lines"][1]["runtime_evidence"]["exact_fields"]
            .as_array_mut()
            .expect("exact runtime fields");
        fields.push(serde_json::json!("container-rlimit-unlimited"));
        let gated = fabricated_unlimited["reviewed_lines"][1]["runtime_evidence"]["target_gated_fields"]
            .as_array_mut()
            .expect("target-gated runtime fields");
        gated.retain(|field| field != "container-rlimit-unlimited");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&fabricated_unlimited)).is_err());

        for (member, source) in [
            ("cli_flag_source", "cmd/podman/common/unknown.go"),
            ("cli_transform_source", "pkg/specgenutil/unknown.go"),
            ("command_route_source", "cmd/podman/containers/unknown.go"),
            ("route_source", "pkg/api/server/register_unknown.go"),
            ("handler_source", "pkg/api/handlers/libpod/unknown.go"),
        ] {
            let mut substituted = decoded_catalogue();
            substituted["reviewed_lines"][0]["runtime_evidence"][member]["path"] = serde_json::json!(source);
            assert!(parse_renderer_catalogue(&encoded_catalogue(&substituted)).is_err());
        }

        let mut wrong_model = decoded_catalogue();
        wrong_model["reviewed_lines"][2]["runtime_evidence"]["model_sources"][1]["path"] =
            serde_json::json!("pkg/specgen/specgen.go");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_model)).is_err());

        let mut wrong_revision = decoded_catalogue();
        wrong_revision["reviewed_lines"][6]["runtime_evidence"]["handler_source"]["revision"] =
            serde_json::json!("0000000000000000000000000000000000000000");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_revision)).is_err());
    }

    #[test]
    fn renderer_catalogue_rejects_b4_boundary_and_evidence_mutations() {
        let mut missing_exact = decoded_catalogue();
        missing_exact["reviewed_lines"][0]["b4_evidence"]["exact_fields"]
            .as_array_mut()
            .expect("B4 exact fields")
            .pop();
        assert!(parse_renderer_catalogue(&encoded_catalogue(&missing_exact)).is_err());

        let mut duplicate_blocked = decoded_catalogue();
        let blocked = duplicate_blocked["reviewed_lines"][0]["b4_evidence"]["blocked_fields"]
            .as_array_mut()
            .expect("B4 blocked fields");
        blocked.push(blocked[0].clone());
        assert!(parse_renderer_catalogue(&encoded_catalogue(&duplicate_blocked)).is_err());

        let mut fabricated_pre_five_six = decoded_catalogue();
        let target_gated = fabricated_pre_five_six["reviewed_lines"][0]["b4_evidence"]["target_gated_fields"]
            .as_array_mut()
            .expect("B4 target-gated fields");
        let volume_uid = target_gated.remove(0);
        fabricated_pre_five_six["reviewed_lines"][0]["b4_evidence"]["exact_fields"]
            .as_array_mut()
            .expect("B4 exact fields")
            .push(volume_uid);
        assert!(parse_renderer_catalogue(&encoded_catalogue(&fabricated_pre_five_six)).is_err());

        let mut mutable_source = decoded_catalogue();
        mutable_source["reviewed_lines"][0]["b4_evidence"]["cli_sources"][0]["revision"] = serde_json::json!("main");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&mutable_source)).is_err());

        let mut incorrect_model = decoded_catalogue();
        incorrect_model["reviewed_lines"][0]["b4_evidence"]["model_sources"][0]["path"] =
            serde_json::json!("pkg/specgenutil/specgen.go");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&incorrect_model)).is_err());

        let mut unsupported_pod_mount = decoded_catalogue();
        let blocked = unsupported_pod_mount["reviewed_lines"][0]["b4_evidence"]["blocked_fields"]
            .as_array_mut()
            .expect("B4 blocked fields");
        blocked.retain(|field| field != "pod-infra-mount");
        unsupported_pod_mount["reviewed_lines"][0]["b4_evidence"]["exact_fields"]
            .as_array_mut()
            .expect("B4 exact fields")
            .push(serde_json::json!("pod-infra-mount"));
        assert!(parse_renderer_catalogue(&encoded_catalogue(&unsupported_pod_mount)).is_err());
    }

    #[test]
    fn renderer_catalogue_rejects_unknown_and_duplicate_json_keys() {
        let unknown = RENDERING_CATALOGUE_JSON.replacen(
            "\"schema_version\": 8,",
            "\"schema_version\": 8, \"unexpected\": true,",
            1,
        );
        assert!(parse_renderer_catalogue(&unknown).is_err());

        let duplicate_root = RENDERING_CATALOGUE_JSON.replacen(
            "\"schema_version\": 8,",
            "\"schema_version\": 8, \"schema_version\": 8,",
            1,
        );
        assert!(parse_renderer_catalogue(&duplicate_root).is_err());

        let duplicate_nested =
            RENDERING_CATALOGUE_JSON.replacen("\"flag\": null,", "\"flag\": null, \"flag\": null,", 1);
        assert!(parse_renderer_catalogue(&duplicate_nested).is_err());
    }

    #[test]
    fn host_alias_rendering_uses_podmans_single_hostname_separator_for_both_ip_families() {
        let ipv4 = HostAlias::new("192.0.2.53".parse().expect("IPv4"), "resolver.example").expect("host alias");
        assert_eq!(host_alias_value(&ipv4), "resolver.example:192.0.2.53");
        let ipv6 = HostAlias::new("2001:db8::53".parse().expect("IPv6"), "resolver-v6.example").expect("host alias");
        assert_eq!(host_alias_value(&ipv6), "resolver-v6.example:2001:db8::53");
    }
}
