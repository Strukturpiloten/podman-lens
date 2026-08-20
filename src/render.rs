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

use crate::{
    DeploymentConnectionReference, DeploymentOperation, DeploymentPlan, DeploymentResource, DeploymentResourceId,
    Diagnostic, DiagnosticCode, ExternalPrecondition, NamedVolumeMount, ResourceKind, RestartPolicy,
    SensitiveInputReference,
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
    reviewed_lines: Vec<ReviewedRenderingLine>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedRenderingLine {
    version: String,
    revision: String,
    tag: String,
    operations: Vec<ReviewedRenderingOperation>,
    field_evidence: Vec<ReviewedFieldEvidence>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedRenderingOperation {
    category: RenderingOperationCategory,
    cli_source: String,
    libpod_endpoint_source: String,
    body_source: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedFieldEvidence {
    field: RenderedField,
    operation: RenderingOperationCategory,
    cli: CliFieldClaim,
    libpod: LibpodFieldClaim,
    cli_source: String,
    model_sources: Vec<String>,
    handler_source: String,
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

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
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
    PodInfraNamedVolumeMount,
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
        let unsupported_fields = unsupported_fields(operation.resource_intent());
        if !unsupported_fields.is_empty() {
            blocked_resources.insert(operation.id().resource().clone());
            findings.extend(unsupported_fields.into_iter().map(|field| {
                RenderingFinding::new(
                    DiagnosticCode::RenderingUnsupported,
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
        DeploymentResource::Network(network) => (
            RenderStatus::Exact,
            vec![
                "network".to_owned(),
                "create".to_owned(),
                network.identity().name().to_owned(),
            ],
            RenderedHttpMethod::Post,
            format!("/v{version}/libpod/networks/create"),
            RenderedHttpBody::Json(json!({"name": network.identity().name()})),
            None,
        ),
        DeploymentResource::Volume(volume) => (
            RenderStatus::Exact,
            vec![
                "volume".to_owned(),
                "create".to_owned(),
                volume.identity().name().to_owned(),
            ],
            RenderedHttpMethod::Post,
            format!("/v{version}/libpod/volumes/create"),
            RenderedHttpBody::Json(json!({"Name": volume.identity().name()})),
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
                "--policy=missing".to_owned(),
                image.source().to_owned(),
            ],
            RenderedHttpMethod::Post,
            format!(
                "/v{version}/libpod/images/pull?reference={}&policy=missing",
                percent_encode(image.source())
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
                append_network_arguments(&mut cli_suffix, pod.networks());
                if !append_named_volume_arguments(&mut cli_suffix, pod.infra_mounts()) {
                    return Err(RenderingFinding::new(
                        DiagnosticCode::RenderingUnsupported,
                        Some(id.clone()),
                        Some("infra_mounts.cli_ambiguous"),
                    ));
                }
                let mut body = Map::new();
                body.insert("name".to_owned(), Value::String(pod.identity().name().to_owned()));
                body.insert("networks".to_owned(), networks);
                if !pod.infra_mounts().is_empty() {
                    body.insert("volumes".to_owned(), named_volume_json(pod.infra_mounts()));
                }
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
                    append_network_arguments(&mut cli_suffix, container.networks());
                    json!({"image": image, "networks": network_configuration(container.networks())})
                };
                if !append_named_volume_arguments(&mut cli_suffix, container.mounts()) {
                    return Err(RenderingFinding::new(
                        DiagnosticCode::RenderingUnsupported,
                        Some(id.clone()),
                        Some("mounts.cli_ambiguous"),
                    ));
                }
                append_container_setting_arguments(&mut cli_suffix, container, id)?;
                let Some(body_map) = body.as_object_mut() else {
                    return Err(RenderingFinding::new(
                        DiagnosticCode::RenderingUnsupported,
                        Some(id.clone()),
                        Some("container_body"),
                    ));
                };
                append_container_setting_json(body_map, container, id)?;
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
    if catalogue.schema_version != 3
        || catalogue.provenance.trim().is_empty()
        || catalogue.reviewed_lines.len() != capabilities.len()
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
            || !validated_renderer_operations(&line, &expected_operations)
            || !validated_field_evidence(&line)
        {
            return Err(());
        }
        versions.push(line.version);
    }
    Ok(versions)
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
            operation.cli_source == immutable_source_url(&line.revision, cli_path)
                && operation.libpod_endpoint_source == immutable_source_url(&line.revision, libpod_path)
                && match body_path {
                    Some(body_path) => {
                        operation.body_source.as_deref()
                            == Some(immutable_source_url(&line.revision, body_path).as_str())
                    }
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
            let (operation, flag, shape, member, cli_path, model_paths, handler_path) =
                expected_field_claim(evidence.field);
            evidence.operation == operation
                && evidence.cli.flag == flag
                && evidence.cli.value_shape == shape
                && evidence.libpod.json_member == member
                && evidence.cli_source == immutable_source_url(&line.revision, cli_path)
                && evidence.model_sources
                    == model_paths
                        .iter()
                        .map(|path| immutable_source_url(&line.revision, path))
                        .collect::<Vec<_>>()
                && evidence.handler_source == immutable_source_url(&line.revision, handler_path)
        })
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
        RenderedField::PodInfraNamedVolumeMount,
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
    const POD_VOLUME_MODEL: &[&str] = &["pkg/specgen/podspecgen.go", "pkg/specgen/volumes.go"];
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
        RenderedField::PodInfraNamedVolumeMount => (
            RenderingOperationCategory::PodCreate,
            Some(CliFlag::Volume),
            CliValueShape::NamedVolumeMount,
            LibpodBodyMember::Volumes,
            "cmd/podman/pods/create.go",
            POD_VOLUME_MODEL,
            "pkg/api/handlers/libpod/pods.go",
        ),
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

fn immutable_source_url(revision: &str, path: &str) -> String {
    format!("https://github.com/containers/podman/blob/{revision}/{path}")
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
            DeploymentResource::Image(image) => Some((image.identity().clone(), image.source().to_owned())),
            _ => None,
        })
        .collect()
}

fn unsupported_fields(resource: &DeploymentResource) -> Vec<&'static str> {
    match resource {
        DeploymentResource::Container(container) => {
            let mut fields = Vec::new();
            if container.pod().is_some() && !container.networks().is_empty() {
                fields.push("networks");
            }
            if !container.secrets().is_empty() {
                fields.push("secrets");
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

fn append_network_arguments(arguments: &mut Vec<String>, networks: &[DeploymentResourceId]) {
    for network in networks {
        arguments.push("--network".to_owned());
        arguments.push(network.name().to_owned());
    }
}

fn network_configuration(networks: &[DeploymentResourceId]) -> Value {
    let networks = networks
        .iter()
        .map(|network| (network.name().to_owned(), Value::Object(Map::new())))
        .collect::<Map<String, Value>>();
    Value::Object(networks)
}

fn append_named_volume_arguments(arguments: &mut Vec<String>, mounts: &[NamedVolumeMount]) -> bool {
    for mount in mounts {
        if !cli_safe_mount_component(mount.source().name()) || !cli_safe_mount_component(mount.destination().as_str()) {
            return false;
        }
        let access = if mount.is_read_only() { "ro" } else { "rw" };
        let copy = match mount.copy_mode() {
            crate::NamedVolumeCopyMode::Copy => "copy",
            crate::NamedVolumeCopyMode::NoCopy => "nocopy",
        };
        arguments.push("--volume".to_owned());
        arguments.push(format!(
            "{}:{}:{access},{copy}",
            mount.source().name(),
            mount.destination().as_str()
        ));
    }
    true
}

fn cli_safe_mount_component(value: &str) -> bool {
    !value.contains([':', ','])
}

fn named_volume_json(mounts: &[NamedVolumeMount]) -> Value {
    Value::Array(
        mounts
            .iter()
            .map(|mount| {
                let access = if mount.is_read_only() { "ro" } else { "rw" };
                let copy = match mount.copy_mode() {
                    crate::NamedVolumeCopyMode::Copy => "copy",
                    crate::NamedVolumeCopyMode::NoCopy => "nocopy",
                };
                json!({
                    "Name": mount.source().name(),
                    "Dest": mount.destination().as_str(),
                    "Options": [access, copy],
                })
            })
            .collect(),
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
        body.insert("volumes".to_owned(), named_volume_json(container.mounts()));
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

    use super::{RENDERING_CATALOGUE_JSON, parse_renderer_catalogue};

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
        let mut wrong_source = decoded_catalogue();
        wrong_source["reviewed_lines"][0]["field_evidence"][0]["cli_source"] =
            serde_json::json!("https://github.com/containers/podman/blob/main/cmd/podman/containers/create.go");
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_source)).is_err());

        let mut wrong_model = decoded_catalogue();
        wrong_model["reviewed_lines"][0]["field_evidence"][8]["model_sources"][1] = serde_json::json!(
            "https://github.com/containers/podman/blob/f9f7d48b24b1ca4403f189caaeab1cb8ff4a9aa2/pkg/specgen/not-volumes.go"
        );
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_model)).is_err());

        let mut wrong_handler = decoded_catalogue();
        wrong_handler["reviewed_lines"][0]["field_evidence"][9]["handler_source"] = serde_json::json!(
            "https://github.com/containers/podman/blob/f9f7d48b24b1ca4403f189caaeab1cb8ff4a9aa2/pkg/api/handlers/libpod/containers_create.go"
        );
        assert!(parse_renderer_catalogue(&encoded_catalogue(&wrong_handler)).is_err());

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
    fn renderer_catalogue_rejects_unknown_and_duplicate_json_keys() {
        let unknown = RENDERING_CATALOGUE_JSON.replacen(
            "\"schema_version\": 3,",
            "\"schema_version\": 3, \"unexpected\": true,",
            1,
        );
        assert!(parse_renderer_catalogue(&unknown).is_err());

        let duplicate_root = RENDERING_CATALOGUE_JSON.replacen(
            "\"schema_version\": 3,",
            "\"schema_version\": 3, \"schema_version\": 3,",
            1,
        );
        assert!(parse_renderer_catalogue(&duplicate_root).is_err());

        let duplicate_nested =
            RENDERING_CATALOGUE_JSON.replacen("\"flag\": null,", "\"flag\": null, \"flag\": null,", 1);
        assert!(parse_renderer_catalogue(&duplicate_nested).is_err());
    }
}
