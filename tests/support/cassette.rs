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
    body: Value,
}

impl CassetteResponse {
    pub(crate) fn set_status(&mut self, status: u16) {
        self.status = status;
    }

    pub(crate) fn set_body(&mut self, body: Value) {
        self.body = body;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CassetteError {
    InvalidJson,
    InvalidSchema,
    SchemaViolation,
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
    let body = if interaction.response.body.is_null() {
        Vec::new()
    } else {
        serde_json::to_vec(&interaction.response.body).map_err(|_| CassetteError::InvalidResponse)?
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
