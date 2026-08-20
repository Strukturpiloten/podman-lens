//! Fixed, sanitized M4 corpus coverage for acquisition and discovery.

#![allow(clippy::expect_used)]

use std::{collections::VecDeque, fmt::Write as _, fs, path::PathBuf, sync::Mutex};

use podman_lens::{
    AcquisitionOptions, DiagnosticCode, DiscoveryRequest, LibpodHeader, LibpodHeaders, LibpodRequest, LibpodResponse,
    LibpodTransport, LibpodTransportFuture, ResourceKind, ResourceObservation, ResourceSelector, TransportError,
    acquire_inventory, discover, snapshot::v1,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Eq, PartialEq)]
enum ObservationState {
    Complete,
    Partial,
}
trait ObservationTestAccess {
    fn state(&self) -> ObservationState;
    fn findings(&self) -> &[podman_lens::InventoryFinding];
    fn unknown_fields(&self) -> &[podman_lens::UnmodelledField];
}
impl ObservationTestAccess for ResourceObservation {
    fn state(&self) -> ObservationState {
        if self.header().state() == podman_lens::ResourceObservationState::Complete {
            ObservationState::Complete
        } else {
            ObservationState::Partial
        }
    }
    fn findings(&self) -> &[podman_lens::InventoryFinding] {
        self.header().findings()
    }
    fn unknown_fields(&self) -> &[podman_lens::UnmodelledField] {
        self.header().unmodelled_fields()
    }
}

#[derive(Deserialize)]
struct CorpusManifest {
    schema_version: u8,
    evidence_kind: String,
    sanitization: String,
    artifacts: Vec<CorpusArtifact>,
}

#[derive(Deserialize)]
struct CorpusArtifact {
    name: String,
    engine_version: String,
    api_version: String,
    release_tag: String,
    commit: String,
    source_url: String,
    artifact: String,
    sha256: String,
    synthetic: bool,
    coverage: Vec<String>,
}

struct FixtureTransport {
    responses: Mutex<VecDeque<LibpodResponse>>,
}

impl FixtureTransport {
    fn new(responses: Vec<LibpodResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }
}

impl LibpodTransport for FixtureTransport {
    fn send<'a>(&'a self, _request: &'a LibpodRequest) -> LibpodTransportFuture<'a> {
        let response = self
            .responses
            .lock()
            .map_err(|_| TransportError::unavailable())
            .and_then(|mut responses| responses.pop_front().ok_or_else(TransportError::unavailable));
        Box::pin(async move { response })
    }
}

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/corpus")
}

fn manifest() -> Result<CorpusManifest, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(corpus_root().join("manifest.json"))?)?)
}

fn response(value: &Value) -> Result<LibpodResponse, Box<dyn std::error::Error>> {
    let status = value["status"]
        .as_u64()
        .ok_or("fixture status must be an unsigned integer")?
        .try_into()?;
    let mut headers = value["headers"]
        .as_array()
        .ok_or("fixture headers must be an array")?
        .iter()
        .map(|header| {
            let [name, value] = header.as_array().ok_or("fixture header must be a pair")?.as_slice() else {
                return Err("fixture header must contain exactly two values".into());
            };
            LibpodHeader::new(
                name.as_str().ok_or("fixture header name must be a string")?,
                value.as_str().ok_or("fixture header value must be a string")?,
            )
            .map_err(Into::into)
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    if headers.is_empty() && !value.get("body").is_some_and(Value::is_null) {
        headers.push(LibpodHeader::new("content-type", "application/json")?);
    }
    let body = value.get("body").cloned().unwrap_or(Value::Null);
    let body = if body.is_null() {
        Vec::new()
    } else {
        serde_json::to_vec(&body)?
    };
    Ok(LibpodResponse::new(status, LibpodHeaders::new(headers), body)?)
}

fn fixture_values(name: &str) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let fixture: Value = serde_json::from_slice(&fs::read(corpus_root().join(name))?)?;
    Ok(fixture["responses"]
        .as_array()
        .ok_or("fixture must have an ordered response array")?
        .clone())
}

fn fixture_responses(name: &str) -> Result<Vec<LibpodResponse>, Box<dyn std::error::Error>> {
    fixture_values(name)?.iter().map(response).collect()
}

async fn inventory(name: &str) -> Result<podman_lens::ResourceInventory, Box<dyn std::error::Error>> {
    Ok(acquire_inventory(
        &FixtureTransport::new(fixture_responses(name)?),
        AcquisitionOptions::redacted(),
    )
    .await?)
}

fn root(kind: ResourceKind, reference: &str) -> Result<DiscoveryRequest, Box<dyn std::error::Error>> {
    let mut request = DiscoveryRequest::new();
    request.add_root(ResourceSelector::exact(kind, reference)?);
    Ok(request)
}

fn status(status: u16) -> Value {
    serde_json::json!({ "status": status, "headers": [], "body": null })
}

#[test]
fn corpus_manifest_verifies_fixed_provenance_and_hashes() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = manifest()?;
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.evidence_kind, "source-derived-synthetic-sanitized");
    assert!(manifest.sanitization.contains("no real"));
    assert_eq!(manifest.artifacts.len(), 4);
    for artifact in manifest.artifacts {
        assert!(artifact.synthetic, "{} must explicitly be synthetic", artifact.name);
        assert!(artifact.engine_version.starts_with("5.") || artifact.engine_version.starts_with("6."));
        assert_eq!(artifact.engine_version, artifact.api_version);
        assert!(artifact.release_tag.starts_with('v'));
        assert_eq!(artifact.commit.len(), 40);
        assert!(artifact.commit.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(artifact.source_url.starts_with("https://"));
        assert!(!artifact.coverage.is_empty());
        let bytes = fs::read(corpus_root().join(&artifact.artifact))?;
        let mut digest = String::new();
        for byte in Sha256::digest(bytes) {
            write!(&mut digest, "{byte:02x}").expect("writing into a String cannot fail");
        }
        assert_eq!(digest, artifact.sha256, "{} hash", artifact.name);
    }
    Ok(())
}

#[tokio::test]
async fn rootless_and_rootful_corpus_cases_acquire_then_discover() -> Result<(), Box<dyn std::error::Error>> {
    let rootless = inventory("rootless-5.4.responses.json").await?;
    assert_eq!(rootless.service().engine_version().original(), "5.4.0");
    assert!(
        rootless
            .section(ResourceKind::Pod)
            .expect("fixed pod section")
            .observations()
            .is_empty()
    );
    let worker = &rootless
        .section(ResourceKind::Container)
        .expect("containers")
        .observations()[1];
    assert_eq!(worker.state(), ObservationState::Partial);
    assert!(
        worker
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::ResourceUnavailable)
    );
    let graph = discover(&rootless, &root(ResourceKind::Volume, "rootless-data")?)?;
    assert_eq!(graph.resolved_roots()[0].id(), "rootless-data");
    assert!(
        graph
            .groups()
            .iter()
            .any(|group| group.members().iter().any(|member| member.id() == "rootless-web"))
    );

    let rootful = inventory("rootful-6.1.responses.json").await?;
    assert_eq!(rootful.service().engine_version().original(), "6.1.0");
    let graph = discover(&rootful, &root(ResourceKind::Pod, "application")?)?;
    assert!(
        graph.groups()[0]
            .members()
            .iter()
            .any(|member| member.id() == "rootful-infra")
    );
    assert!(
        graph.groups()[0]
            .prerequisites()
            .iter()
            .any(|member| member.id() == "rootful-user")
    );
    assert!(
        graph.groups()[0]
            .prerequisites()
            .iter()
            .any(|member| member.id() == "rootful-bridge")
    );
    Ok(())
}

async fn every_list_failure_leaves_only_its_section_unavailable() -> Result<(), Box<dyn std::error::Error>> {
    let list_cases = [
        (2, ResourceKind::Container, vec![8, 9]),
        (3, ResourceKind::Pod, vec![]),
        (4, ResourceKind::Network, vec![10]),
        (5, ResourceKind::Volume, vec![11]),
        (6, ResourceKind::Image, vec![12, 13]),
        (7, ResourceKind::Secret, vec![14]),
    ];
    for (index, kind, mut inspections) in list_cases {
        let mut values = fixture_values("malformed-6.1.responses.json")?;
        values[index] = status(503);
        inspections.sort_unstable_by(|left, right| right.cmp(left));
        for inspection in inspections {
            values.remove(inspection);
        }
        let responses = values.iter().map(response).collect::<Result<Vec<_>, _>>()?;
        let inventory = acquire_inventory(&FixtureTransport::new(responses), AcquisitionOptions::redacted()).await?;
        assert_ne!(
            inventory.section(kind).expect("fixed section").availability(),
            podman_lens::InventorySectionAvailability::Available
        );

        let mut values = fixture_values("malformed-6.1.responses.json")?;
        let malformed_shape = match kind {
            ResourceKind::Volume => serde_json::json!({ "Volumes": {} }),
            _ => serde_json::json!({}),
        };
        values[index] = serde_json::json!({ "status": 200, "headers": [], "body": malformed_shape });
        let mut inspections = match kind {
            ResourceKind::Container => vec![8, 9],
            ResourceKind::Pod => vec![],
            ResourceKind::Network => vec![10],
            ResourceKind::Volume => vec![11],
            ResourceKind::Image => vec![12, 13],
            ResourceKind::Secret => vec![14],
            _ => unreachable!("fixed resource kinds"),
        };
        inspections.sort_unstable_by(|left, right| right.cmp(left));
        for inspection in inspections {
            values.remove(inspection);
        }
        let responses = values.iter().map(response).collect::<Result<Vec<_>, _>>()?;
        let inventory = acquire_inventory(&FixtureTransport::new(responses), AcquisitionOptions::redacted()).await?;
        assert_ne!(
            inventory.section(kind).expect("fixed section").availability(),
            podman_lens::InventorySectionAvailability::Available
        );
    }

    Ok(())
}

async fn every_inspect_failure_remains_a_partial_record() -> Result<(), Box<dyn std::error::Error>> {
    let inspect_cases = [
        (8, ResourceKind::Container),
        (10, ResourceKind::Pod),
        (11, ResourceKind::Network),
        (13, ResourceKind::Volume),
        (14, ResourceKind::Image),
        (15, ResourceKind::Secret),
    ];
    for (index, kind) in inspect_cases {
        let mut values = fixture_values("rootful-6.1.responses.json")?;
        values[index] = status(404);
        let responses = values.iter().map(response).collect::<Result<Vec<_>, _>>()?;
        let inventory = acquire_inventory(&FixtureTransport::new(responses), AcquisitionOptions::redacted()).await?;
        assert!(
            inventory
                .section(kind)
                .expect("fixed section")
                .observations()
                .iter()
                .any(|record| record.state() == ObservationState::Partial)
        );

        let mut values = fixture_values("rootful-6.1.responses.json")?;
        values[index] = serde_json::json!({ "status": 200, "headers": [], "body": [] });
        let responses = values.iter().map(response).collect::<Result<Vec<_>, _>>()?;
        let inventory = acquire_inventory(&FixtureTransport::new(responses), AcquisitionOptions::redacted()).await?;
        assert!(
            inventory
                .section(kind)
                .expect("fixed section")
                .observations()
                .iter()
                .any(|record| record
                    .findings()
                    .iter()
                    .any(|finding| finding.code() == DiagnosticCode::InventoryShape))
        );
    }

    Ok(())
}

#[tokio::test]
async fn malformed_corpus_is_structured_and_bounded_never_panics() -> Result<(), Box<dyn std::error::Error>> {
    let malformed = inventory("malformed-6.1.responses.json").await?;
    let containers = malformed.section(ResourceKind::Container).expect("containers");
    assert!(
        containers
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::ResourceMalformed)
    );
    assert!(
        containers.observations()[1]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::ResourceUnavailable)
    );
    assert!(
        containers.observations()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::EnvironmentMalformed)
    );
    assert!(
        malformed.section(ResourceKind::Secret).expect("secrets").observations()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::SecretPayloadDiscarded)
    );
    let graph = discover(&malformed, &root(ResourceKind::Image, "example.invalid/ambiguous:1")?)?;
    assert!(
        graph
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::SelectorAmbiguous)
    );

    every_list_failure_leaves_only_its_section_unavailable().await?;
    every_inspect_failure_remains_a_partial_record().await?;

    let mut values = fixture_values("malformed-6.1.responses.json")?;
    let record = values[8]["body"].as_object_mut().ok_or("record must be an object")?;
    for index in 0..=podman_lens::MAX_UNKNOWN_FIELDS_PER_RECORD {
        record.insert(format!("SyntheticUnknown{index}"), Value::Bool(true));
    }
    let inventory = acquire_inventory(
        &FixtureTransport::new(values.iter().map(response).collect::<Result<Vec<_>, _>>()?),
        AcquisitionOptions::redacted(),
    )
    .await?;
    let record = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .observations()[0];
    assert!(record.unknown_fields().len() <= podman_lens::MAX_UNKNOWN_FIELDS_PER_RECORD);
    assert!(
        record
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::UnknownFieldOverflow)
    );
    Ok(())
}

#[tokio::test]
async fn graph_boundary_corpus_is_deterministic_and_respects_crossing_rules() -> Result<(), Box<dyn std::error::Error>>
{
    let graph_inventory = inventory("graph-boundaries-6.1.responses.json").await?;
    let default = discover(&graph_inventory, &root(ResourceKind::Container, "api")?)?;
    assert!(
        !default
            .groups()
            .iter()
            .flat_map(podman_lens::ResourceGroup::members)
            .any(|member| member.id() == "c-external")
    );
    assert!(
        default
            .groups()
            .iter()
            .flat_map(podman_lens::ResourceGroup::prerequisites)
            .any(|member| member.id() == "n-shared")
    );

    for boundary in ["shared", "n-shared"] {
        let mut request = root(ResourceKind::Container, "api")?;
        request.add_network_boundary_override(boundary)?;
        let crossed = discover(&graph_inventory, &request)?;
        assert!(
            crossed
                .groups()
                .iter()
                .flat_map(podman_lens::ResourceGroup::members)
                .any(|member| member.id() == "c-external")
        );
    }

    let explicit = discover(&graph_inventory, &root(ResourceKind::Network, "shared")?)?;
    assert!(
        explicit
            .groups()
            .iter()
            .flat_map(podman_lens::ResourceGroup::members)
            .any(|member| member.id() == "c-external")
    );

    let cycle = discover(&graph_inventory, &root(ResourceKind::Container, "cycle-a")?)?;
    assert_eq!(cycle.groups().len(), 1);
    assert!(
        cycle.groups()[0]
            .members()
            .iter()
            .any(|member| member.id() == "c-cycle-b")
    );

    let mut all = DiscoveryRequest::new();
    all.select_all();
    let all_graph = discover(&graph_inventory, &all)?;
    assert!(!all_graph.resolved_roots().is_empty());
    assert!(
        all_graph
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::AdvisoryLabelIncomplete)
    );
    assert!(
        all_graph
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::AdvisoryLabelConflict)
    );

    let mut first = root(ResourceKind::Container, "api")?;
    first.add_root(ResourceSelector::exact(ResourceKind::Container, "cycle-a")?);
    let first = serde_json::to_vec(&v1::graph(&discover(&graph_inventory, &first)?))?;
    let mut second = root(ResourceKind::Container, "cycle-a")?;
    second.add_root(ResourceSelector::exact(ResourceKind::Container, "api")?);
    let second = serde_json::to_vec(&v1::graph(&discover(&graph_inventory, &second)?))?;
    assert_eq!(first, second, "selector insertion order must not affect snapshot bytes");

    let mut permuted = fixture_values("graph-boundaries-6.1.responses.json")?;
    permuted[2]["body"]
        .as_array_mut()
        .ok_or("container list must be an array")?
        .reverse();
    permuted[4]["body"]
        .as_array_mut()
        .ok_or("network list must be an array")?
        .reverse();
    let permuted = acquire_inventory(
        &FixtureTransport::new(permuted.iter().map(response).collect::<Result<Vec<_>, _>>()?),
        AcquisitionOptions::redacted(),
    )
    .await?;
    let original = serde_json::to_vec(&v1::inventory(&graph_inventory))?;
    assert_eq!(original, serde_json::to_vec(&v1::inventory(&permuted))?);
    Ok(())
}
