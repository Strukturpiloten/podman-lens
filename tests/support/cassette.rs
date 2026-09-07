//! Strict request-aware cassette support for offline integration tests.

#![allow(dead_code)]

use std::{
    collections::{BTreeSet, VecDeque},
    fmt,
    sync::Mutex,
};

use podman_lens::{
    LibpodHeader, LibpodHeaders, LibpodMethod, LibpodRequest, LibpodResponse, LibpodTransport, LibpodTransportFuture,
    TransportError,
};
use serde::Deserialize;
use serde_json::Value;

const CASSETTE_SCHEMA: &str = include_str!("../../docs/schemas/podman-lens-cassette-v1.schema.json");

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ExecutionContext {
    Rootless,
    Rootful,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Cassette {
    schema_version: u8,
    fixture_kind: String,
    scenario_id: String,
    scenario_revision: u32,
    engine_version: String,
    api_version: String,
    execution_context: ExecutionContext,
    synthetic: bool,
    provenance: CassetteProvenance,
    sanitization: String,
    interactions: Vec<CassetteInteraction>,
}

impl Cassette {
    pub(crate) fn from_slice(source: &[u8]) -> Result<Self, CassetteError> {
        let value: Value = serde_json::from_slice(source).map_err(|_| CassetteError::InvalidJson)?;
        let schema: Value = serde_json::from_str(CASSETTE_SCHEMA).map_err(|_| CassetteError::InvalidSchema)?;
        let validator = jsonschema::validator_for(&schema).map_err(|_| CassetteError::InvalidSchema)?;
        if !validator.is_valid(&value) {
            return Err(CassetteError::SchemaViolation);
        }
        if value.get("synthetic").and_then(Value::as_bool) == Some(false) {
            validate_captured_privacy(&value)?;
        }
        serde_json::from_value(value).map_err(|_| CassetteError::SchemaViolation)
    }

    pub(crate) const fn schema_version(&self) -> u8 {
        self.schema_version
    }

    pub(crate) fn fixture_kind(&self) -> &str {
        &self.fixture_kind
    }

    pub(crate) fn scenario_id(&self) -> &str {
        &self.scenario_id
    }

    pub(crate) const fn scenario_revision(&self) -> u32 {
        self.scenario_revision
    }

    pub(crate) fn engine_version(&self) -> &str {
        &self.engine_version
    }

    pub(crate) fn api_version(&self) -> &str {
        &self.api_version
    }

    pub(crate) const fn execution_context(&self) -> ExecutionContext {
        self.execution_context
    }

    pub(crate) const fn synthetic(&self) -> bool {
        self.synthetic
    }

    pub(crate) const fn provenance(&self) -> &CassetteProvenance {
        &self.provenance
    }

    pub(crate) fn sanitization(&self) -> &str {
        &self.sanitization
    }

    pub(crate) fn interaction_count(&self) -> usize {
        self.interactions.len()
    }

    pub(crate) fn unique_interaction_mut(
        &mut self,
        method: LibpodMethod,
        path: &str,
    ) -> Result<&mut CassetteInteraction, CassetteError> {
        let index = unique_interaction_index(&self.interactions, method, path)?;
        Ok(&mut self.interactions[index])
    }

    pub(crate) fn remove_unique_interaction(
        &mut self,
        method: LibpodMethod,
        path: &str,
    ) -> Result<CassetteInteraction, CassetteError> {
        let index = unique_interaction_index(&self.interactions, method, path)?;
        Ok(self.interactions.remove(index))
    }
}

fn validate_captured_privacy(value: &Value) -> Result<(), CassetteError> {
    let interactions = value
        .get("interactions")
        .and_then(Value::as_array)
        .ok_or(CassetteError::PrivacyViolation)?;
    for interaction in interactions {
        let request_path = interaction
            .get("request")
            .and_then(Value::as_object)
            .and_then(|request| request.get("path"))
            .and_then(Value::as_str)
            .ok_or(CassetteError::PrivacyViolation)?;
        if captured_string_is_private(request_path, Some("request_path")) {
            return Err(CassetteError::PrivacyViolation);
        }

        let response = interaction
            .get("response")
            .and_then(Value::as_object)
            .ok_or(CassetteError::PrivacyViolation)?;
        let headers = response
            .get("headers")
            .and_then(Value::as_array)
            .ok_or(CassetteError::PrivacyViolation)?;
        for header in headers {
            let pair = header.as_array().ok_or(CassetteError::PrivacyViolation)?;
            let name = pair
                .first()
                .and_then(Value::as_str)
                .ok_or(CassetteError::PrivacyViolation)?;
            let header_value = pair
                .get(1)
                .and_then(Value::as_str)
                .ok_or(CassetteError::PrivacyViolation)?;
            let lower = name.to_ascii_lowercase();
            if matches!(lower.as_str(), "authorization" | "cookie" | "set-cookie")
                || (lower == "date" && header_value != "Sat, 01 Jan 2000 00:00:00 GMT")
                || (lower == "x-reference-id" && !sanitized_reference_is_exact(header_value))
                || captured_string_is_private(header_value, Some(name))
            {
                return Err(CassetteError::PrivacyViolation);
            }
        }
        if let Some(body) = response.get("body") {
            validate_captured_value(body, None)?;
        }
        if let Some(body) = response.get("body_text").and_then(Value::as_str) {
            if captured_string_is_private(body, None) {
                return Err(CassetteError::PrivacyViolation);
            }
        }
    }
    Ok(())
}

fn sanitized_reference_is_exact(value: &str) -> bool {
    let Some(reference) = value.strip_prefix("fixture-reference-") else {
        return false;
    };
    let Some((label, ordinal)) = reference.rsplit_once('-') else {
        return false;
    };
    !label.is_empty()
        && label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && label.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && label.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        && ordinal.len() == 2
        && ordinal.bytes().all(|byte| byte.is_ascii_digit())
        && ordinal != "00"
}

fn validate_captured_value(value: &Value, parent_key: Option<&str>) -> Result<(), CassetteError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let lower = key.to_ascii_lowercase();
                if matches!(lower.as_str(), "secretdata" | "authorization") {
                    return Err(CassetteError::PrivacyViolation);
                }
                validate_captured_value(child, Some(key))?;
            }
        }
        Value::Array(values) => {
            for child in values {
                validate_captured_value(child, parent_key)?;
            }
        }
        Value::String(text) if captured_string_is_private(text, parent_key) => {
            return Err(CassetteError::PrivacyViolation);
        }
        Value::Number(number)
            if parent_key.is_some_and(captured_key_is_timestamp)
                && !matches!(number.as_i64(), Some(0 | 946_684_800)) =>
        {
            return Err(CassetteError::PrivacyViolation);
        }
        _ => {}
    }
    Ok(())
}

fn captured_key_is_timestamp(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "created" | "createdat" | "startedat" | "finishedat" | "buildtime" | "builttime"
    ) || lower.ends_with(".created")
}

fn captured_string_is_private(value: &str, parent_key: Option<&str>) -> bool {
    let lower = value.to_ascii_lowercase();
    if [
        "bearer ",
        "basic ",
        "unix://",
        "tcp://",
        "ssh://",
        "/home/",
        "/root/",
        "/run/user/",
        "/tmp/",
        "/var/lib/containers",
        "/run/containers",
        "/capture-input",
        "/capture-socket",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
        || lower.contains("podman.sock")
        || lower.contains("sentinel_private")
    {
        return true;
    }
    if let Some((name, assigned)) = value.split_once('=') {
        if !name.is_empty()
            && name.bytes().all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
            && assigned != "redacted"
        {
            return true;
        }
    }
    if let Some(key) = parent_key {
        if key.eq_ignore_ascii_case("hostname")
            && !value.is_empty()
            && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return true;
        }
        if captured_key_is_timestamp(key) && !matches!(value, "2000-01-01T00:00:00Z" | "0001-01-01T00:00:00Z") {
            return true;
        }
    }
    if looks_like_mac(value) {
        return !lower.starts_with("02:00:00:00:00:");
    }
    address_is_private(value)
}

fn looks_like_mac(value: &str) -> bool {
    let parts = value.split(':').collect::<Vec<_>>();
    parts.len() == 6
        && parts
            .iter()
            .all(|part| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn address_is_private(value: &str) -> bool {
    let candidate = value.split('/').next().unwrap_or(value);
    let Ok(address) = candidate.parse::<std::net::IpAddr>() else {
        return false;
    };
    match address {
        std::net::IpAddr::V4(address) => {
            let octets = address.octets();
            !(address.is_unspecified()
                || address.is_loopback()
                || octets[..3] == [192, 0, 2]
                || octets[..3] == [198, 51, 100]
                || octets[..3] == [203, 0, 113])
        }
        std::net::IpAddr::V6(address) => {
            let segments = address.segments();
            !(address.is_unspecified() || address.is_loopback() || (segments[0] == 0x2001 && segments[1] == 0x0db8))
        }
    }
}

fn unique_interaction_index(
    interactions: &[CassetteInteraction],
    method: LibpodMethod,
    path: &str,
) -> Result<usize, CassetteError> {
    let mut matches = interactions
        .iter()
        .enumerate()
        .filter(|(_, interaction)| interaction.request.method == method.as_str() && interaction.request.path == path)
        .map(|(index, _)| index);
    let Some(index) = matches.next() else {
        return Err(CassetteError::InteractionNotFound {
            method: method.as_str().to_owned(),
            path: path.to_owned(),
        });
    };
    if matches.next().is_some() {
        return Err(CassetteError::InteractionAmbiguous {
            method: method.as_str().to_owned(),
            path: path.to_owned(),
        });
    }
    Ok(index)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CassetteProvenance {
    evidence_kind: String,
    release_tag: String,
    revision: String,
    source_urls: Vec<String>,
    capture: Option<CassetteCaptureProvenance>,
}

impl CassetteProvenance {
    pub(crate) fn evidence_kind(&self) -> &str {
        &self.evidence_kind
    }

    pub(crate) fn release_tag(&self) -> &str {
        &self.release_tag
    }

    pub(crate) fn revision(&self) -> &str {
        &self.revision
    }

    pub(crate) fn source_urls(&self) -> &[String] {
        &self.source_urls
    }

    pub(crate) const fn capture(&self) -> Option<&CassetteCaptureProvenance> {
        self.capture.as_ref()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CassetteCaptureProvenance {
    engine_build: CassetteEngineBuild,
    runtime_image: String,
    capture_manifest_sha256: String,
    runtime_repository: String,
    runtime_revision: String,
    runtime_matrix_cell: String,
    runtime_matrix_sha256: String,
    setup_script_sha256: String,
    request_recorder_source_sha256: String,
    compose_provider_binary_sha256: Option<String>,
}

impl CassetteCaptureProvenance {
    pub(crate) const fn engine_build(&self) -> &CassetteEngineBuild {
        &self.engine_build
    }

    pub(crate) fn runtime_image(&self) -> &str {
        &self.runtime_image
    }

    pub(crate) fn capture_manifest_sha256(&self) -> &str {
        &self.capture_manifest_sha256
    }

    pub(crate) fn runtime_repository(&self) -> &str {
        &self.runtime_repository
    }

    pub(crate) fn runtime_revision(&self) -> &str {
        &self.runtime_revision
    }

    pub(crate) fn runtime_matrix_cell(&self) -> &str {
        &self.runtime_matrix_cell
    }

    pub(crate) fn runtime_matrix_sha256(&self) -> &str {
        &self.runtime_matrix_sha256
    }

    pub(crate) fn setup_script_sha256(&self) -> &str {
        &self.setup_script_sha256
    }

    pub(crate) fn request_recorder_source_sha256(&self) -> &str {
        &self.request_recorder_source_sha256
    }

    pub(crate) fn compose_provider_binary_sha256(&self) -> Option<&str> {
        self.compose_provider_binary_sha256.as_deref()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case", tag = "kind")]
pub(crate) enum CassetteEngineBuild {
    SourceBuild {
        source_revision: String,
    },
    Package {
        package_name: String,
        package_revision: String,
        package_artifact_sha256: String,
    },
}

impl CassetteEngineBuild {
    pub(crate) fn source_revision(&self) -> Option<&str> {
        match self {
            Self::SourceBuild { source_revision } => Some(source_revision),
            Self::Package { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CassetteInteraction {
    request: CassetteRequest,
    response: CassetteResponse,
}

impl CassetteInteraction {
    pub(crate) fn response_mut(&mut self) -> &mut CassetteResponse {
        &mut self.response
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CassetteRequest {
    method: String,
    path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CassetteResponse {
    status: u16,
    headers: Vec<[String; 2]>,
    body: Option<Value>,
    body_text: Option<String>,
}

impl CassetteResponse {
    pub(crate) fn set_status(&mut self, status: u16) {
        self.status = status;
    }

    pub(crate) fn set_body(&mut self, body: Value) {
        self.body = Some(body);
        self.body_text = None;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CassetteError {
    InvalidJson,
    InvalidSchema,
    SchemaViolation,
    PrivacyViolation,
    InvalidResponse,
    InteractionNotFound { method: String, path: String },
    InteractionAmbiguous { method: String, path: String },
}

impl fmt::Display for CassetteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson => formatter.write_str("cassette is not valid JSON"),
            Self::InvalidSchema => formatter.write_str("cassette schema is invalid"),
            Self::SchemaViolation => formatter.write_str("cassette does not satisfy its schema"),
            Self::PrivacyViolation => formatter.write_str("captured cassette violates privacy admission"),
            Self::InvalidResponse => formatter.write_str("cassette contains an invalid response"),
            Self::InteractionNotFound { method, path } => {
                write!(formatter, "cassette has no interaction for {method} {path}")
            }
            Self::InteractionAmbiguous { method, path } => {
                write!(formatter, "cassette has multiple interactions for {method} {path}")
            }
        }
    }
}

impl std::error::Error for CassetteError {}

struct PreparedInteraction {
    request: CassetteRequest,
    response: LibpodResponse,
}

struct ReplayState {
    remaining: VecDeque<PreparedInteraction>,
    failure: Option<CassetteReplayError>,
}

pub(crate) struct CassetteTransport {
    state: Mutex<ReplayState>,
}

impl CassetteTransport {
    pub(crate) fn try_new(cassette: Cassette) -> Result<Self, CassetteError> {
        {
            let mut expected_requests = BTreeSet::new();
            for interaction in &cassette.interactions {
                let key = (interaction.request.method.as_str(), interaction.request.path.as_str());
                if !expected_requests.insert(key) {
                    return Err(CassetteError::InteractionAmbiguous {
                        method: interaction.request.method.clone(),
                        path: interaction.request.path.clone(),
                    });
                }
            }
        }
        let remaining = cassette
            .interactions
            .into_iter()
            .map(prepare_interaction)
            .collect::<Result<VecDeque<_>, _>>()?;
        Ok(Self {
            state: Mutex::new(ReplayState {
                remaining,
                failure: None,
            }),
        })
    }

    pub(crate) fn assert_consumed(&self) -> Result<(), CassetteReplayError> {
        let state = self.state.lock().map_err(|_| CassetteReplayError::StateUnavailable)?;
        if let Some(failure) = &state.failure {
            return Err(failure.clone());
        }
        let Some(next) = state.remaining.front() else {
            return Ok(());
        };
        Err(CassetteReplayError::Unconsumed {
            remaining: state.remaining.len(),
            next_method: next.request.method.clone(),
            next_path: next.request.path.clone(),
        })
    }
}

fn prepare_interaction(interaction: CassetteInteraction) -> Result<PreparedInteraction, CassetteError> {
    let headers = interaction
        .response
        .headers
        .into_iter()
        .map(|[name, value]| LibpodHeader::new(name, value).map_err(|_| CassetteError::InvalidResponse))
        .collect::<Result<Vec<_>, _>>()?;
    let body = if let Some(body_text) = interaction.response.body_text {
        body_text.into_bytes()
    } else {
        match interaction.response.body {
            Some(body) if !body.is_null() => serde_json::to_vec(&body).map_err(|_| CassetteError::InvalidResponse)?,
            _ => Vec::new(),
        }
    };
    let response = LibpodResponse::new(interaction.response.status, LibpodHeaders::new(headers), body)
        .map_err(|_| CassetteError::InvalidResponse)?;
    Ok(PreparedInteraction {
        request: interaction.request,
        response,
    })
}

impl LibpodTransport for CassetteTransport {
    fn send<'a>(&'a self, request: &'a LibpodRequest) -> LibpodTransportFuture<'a> {
        let response = self
            .state
            .lock()
            .map_err(|_| TransportError::unavailable())
            .and_then(|mut state| {
                if state.failure.is_some() {
                    return Err(TransportError::unavailable());
                }
                let actual_method = request.method().as_str().to_owned();
                let actual_path = request.path().as_str().to_owned();
                let Some(expected) = state.remaining.front() else {
                    state.failure = Some(CassetteReplayError::UnexpectedRequest {
                        actual_method,
                        actual_path,
                    });
                    return Err(TransportError::unavailable());
                };
                if expected.request.method != actual_method || expected.request.path != actual_path {
                    state.failure = Some(CassetteReplayError::RequestMismatch {
                        expected_method: expected.request.method.clone(),
                        expected_path: expected.request.path.clone(),
                        actual_method,
                        actual_path,
                    });
                    return Err(TransportError::unavailable());
                }
                if !request.body().is_empty() || request.headers().iter().len() != 0 {
                    state.failure = Some(CassetteReplayError::UnexpectedRequestShape {
                        actual_method,
                        actual_path,
                    });
                    return Err(TransportError::unavailable());
                }
                state
                    .remaining
                    .pop_front()
                    .map(|interaction| interaction.response)
                    .ok_or_else(TransportError::unavailable)
            });
        Box::pin(async move { response })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CassetteReplayError {
    RequestMismatch {
        expected_method: String,
        expected_path: String,
        actual_method: String,
        actual_path: String,
    },
    UnexpectedRequest {
        actual_method: String,
        actual_path: String,
    },
    UnexpectedRequestShape {
        actual_method: String,
        actual_path: String,
    },
    Unconsumed {
        remaining: usize,
        next_method: String,
        next_path: String,
    },
    StateUnavailable,
}

impl fmt::Display for CassetteReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestMismatch {
                expected_method,
                expected_path,
                actual_method,
                actual_path,
            } => write!(
                formatter,
                "cassette expected {expected_method} {expected_path}, received {actual_method} {actual_path}"
            ),
            Self::UnexpectedRequest {
                actual_method,
                actual_path,
            } => write!(formatter, "cassette received unexpected {actual_method} {actual_path}"),
            Self::UnexpectedRequestShape {
                actual_method,
                actual_path,
            } => write!(
                formatter,
                "cassette request {actual_method} {actual_path} had unexpected headers or body"
            ),
            Self::Unconsumed {
                remaining,
                next_method,
                next_path,
            } => write!(
                formatter,
                "cassette has {remaining} unconsumed interaction(s), next is {next_method} {next_path}"
            ),
            Self::StateUnavailable => formatter.write_str("cassette replay state is unavailable"),
        }
    }
}

impl std::error::Error for CassetteReplayError {}
