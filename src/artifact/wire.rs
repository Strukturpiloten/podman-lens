//! Shared private wire fields for versioned deployment artifacts.

use serde::Serialize;
use serde_json::Value;

use crate::{DeploymentRendering, RenderStatus, RenderedHttpBody, RenderedHttpMethod, ResourceKind};

pub(super) struct ArtifactData {
    pub connection: Option<String>,
    pub external_preconditions: Vec<ResourceArtifact>,
    pub operations: Vec<OperationArtifact>,
}

impl ArtifactData {
    pub fn from_rendering(source: &DeploymentRendering) -> Self {
        Self {
            connection: source.connection().map(|connection| connection.as_str().to_owned()),
            external_preconditions: source
                .external_preconditions()
                .iter()
                .map(|precondition| resource(precondition.identity().kind(), precondition.identity().name()))
                .collect(),
            operations: source
                .operations()
                .iter()
                .map(|operation| {
                    let identity = operation.operation().id().resource();
                    let body = match operation.libpod().body() {
                        RenderedHttpBody::Empty => BodyArtifact {
                            kind: "empty",
                            json: None,
                        },
                        RenderedHttpBody::Json(value) => BodyArtifact {
                            kind: "json",
                            json: Some(value.clone()),
                        },
                        RenderedHttpBody::ExternalSensitiveInput(_) => BodyArtifact {
                            kind: "external_sensitive_input",
                            json: None,
                        },
                    };
                    OperationArtifact {
                        status: status(operation.status()),
                        action: action(operation.operation().id().action()),
                        resource: resource(identity.kind(), identity.name()),
                        cli: CliArtifact {
                            program: operation.cli().program(),
                            argv: operation.cli().argv().to_vec(),
                            external_sensitive_input_required: operation.cli().external_input().is_some(),
                        },
                        libpod: LibpodArtifact {
                            method: match operation.libpod().method() {
                                RenderedHttpMethod::Get => "GET",
                                RenderedHttpMethod::Post => "POST",
                            },
                            path_and_query: operation.libpod().path_and_query().to_owned(),
                            body,
                        },
                    }
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct ResourceArtifact {
    kind: &'static str,
    name: String,
}

#[derive(Serialize)]
pub(super) struct OperationArtifact {
    status: &'static str,
    action: &'static str,
    resource: ResourceArtifact,
    cli: CliArtifact,
    libpod: LibpodArtifact,
}

#[derive(Serialize)]
struct CliArtifact {
    program: &'static str,
    argv: Vec<String>,
    external_sensitive_input_required: bool,
}

#[derive(Serialize)]
struct LibpodArtifact {
    method: &'static str,
    path_and_query: String,
    body: BodyArtifact,
}

#[derive(Serialize)]
struct BodyArtifact {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    json: Option<Value>,
}

fn resource(kind: ResourceKind, name: &str) -> ResourceArtifact {
    ResourceArtifact {
        kind: kind_name(kind),
        name: name.to_owned(),
    }
}

fn kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Container => "container",
        ResourceKind::Pod => "pod",
        ResourceKind::Network => "network",
        ResourceKind::Volume => "volume",
        ResourceKind::Image => "image",
        ResourceKind::Secret => "secret",
    }
}

pub(super) fn status(value: RenderStatus) -> &'static str {
    match value {
        RenderStatus::Exact => "exact",
        RenderStatus::DeferredSensitiveInput => "deferred_sensitive_input",
        RenderStatus::Manual => "manual",
        RenderStatus::Approximate => "approximate",
        RenderStatus::Unsupported => "unsupported",
    }
}

fn action(value: crate::SemanticOperationAction) -> &'static str {
    match value {
        crate::SemanticOperationAction::EnsureImage => "ensure_image",
        crate::SemanticOperationAction::Create => "create",
        crate::SemanticOperationAction::StartPod => "start_pod",
        crate::SemanticOperationAction::StartContainer => "start_container",
    }
}
