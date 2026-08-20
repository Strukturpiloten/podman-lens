//! Version-aware, transport-neutral deployment-plan rendering.
//!
//! This M6-A boundary turns validated M5 semantics into reviewable CLI and Libpod request
//! descriptions. It never opens a connection, sends a request, or serializes secret material.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use semver::Version;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
    DeploymentConnectionReference, DeploymentOperation, DeploymentPlan, DeploymentResource, DeploymentResourceId,
    Diagnostic, DiagnosticCode, ExternalPrecondition, ResourceKind, SensitiveInputReference,
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
struct RenderingCatalogue {
    schema_version: u8,
    provenance: String,
    reviewed_lines: Vec<ReviewedRenderingLine>,
}

#[derive(Deserialize)]
struct ReviewedRenderingLine {
    version: String,
    revision: String,
    tag: String,
    operations: Vec<ReviewedRenderingOperation>,
}

#[derive(Deserialize)]
struct ReviewedRenderingOperation {
    category: String,
    cli_source: String,
    libpod_endpoint_source: String,
    body_source: Value,
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
                (
                    RenderStatus::Exact,
                    cli_suffix,
                    RenderedHttpMethod::Post,
                    format!("/v{version}/libpod/pods/create"),
                    RenderedHttpBody::Json(json!({"name": pod.identity().name(), "networks": networks})),
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
                let body = if let Some(pod) = container.pod() {
                    cli_suffix.push("--pod".to_owned());
                    cli_suffix.push(pod.name().to_owned());
                    json!({"image": image, "pod": pod.name()})
                } else {
                    append_network_arguments(&mut cli_suffix, container.networks());
                    json!({"image": image, "networks": network_configuration(container.networks())})
                };
                cli_suffix.push(image.to_owned());
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
    let catalogue: RenderingCatalogue = serde_json::from_str(source).map_err(|_| ())?;
    let expected_operations = RENDERED_OPERATION_CATEGORIES.into_iter().collect::<BTreeSet<_>>();
    if catalogue.schema_version != 2 || catalogue.provenance.trim().is_empty() || catalogue.reviewed_lines.is_empty() {
        return Err(());
    }
    let mut versions = Vec::with_capacity(catalogue.reviewed_lines.len());
    let mut previous = None;
    for line in catalogue.reviewed_lines {
        let version = Version::parse(&line.version).map_err(|_| ())?;
        if !version.pre.is_empty()
            || !version.build.is_empty()
            || line.version != canonical_version(&version)
            || line.tag != format!("v{}", line.version)
            || !is_lowercase_sha40(&line.revision)
            || previous.is_some_and(|previous: Version| previous >= version)
            || !validated_renderer_operations(&line, &expected_operations)
        {
            return Err(());
        }
        previous = Some(version);
        versions.push(line.version);
    }
    Ok(versions)
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
                        operation.body_source == Value::String(immutable_source_url(&line.revision, body_path))
                    }
                    None => operation.body_source.is_null(),
                }
        })
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
        DeploymentResource::Pod(pod) if !pod.infra_mounts().is_empty() => vec!["infra_mounts"],
        DeploymentResource::Container(container) => {
            let mut fields = Vec::new();
            if container.pod().is_some() && !container.networks().is_empty() {
                fields.push("networks");
            }
            if !container.secrets().is_empty() {
                fields.push("secrets");
            }
            if !container.mounts().is_empty() {
                fields.push("mounts");
            }
            let settings = container.settings();
            if settings.command().is_some() {
                fields.push("command");
            }
            if settings.entrypoint().is_some() {
                fields.push("entrypoint");
            }
            if settings.user().is_some() {
                fields.push("user");
            }
            if settings.workdir().is_some() {
                fields.push("workdir");
            }
            if settings.hostname().is_some() {
                fields.push("hostname");
            }
            if !settings.labels().is_empty() {
                fields.push("labels");
            }
            if !settings.environment().is_empty() {
                fields.push("environment");
            }
            if settings.restart_policy().is_some() {
                fields.push("restart_policy");
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
    use super::{RENDERING_CATALOGUE_JSON, parse_renderer_catalogue};

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
}
