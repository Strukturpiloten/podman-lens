//! Offline M2 acquisition coverage using bounded fixture transports.

#![allow(clippy::expect_used)] // Test-only fixture access reports concise assertion failures.

use std::{collections::VecDeque, fs, path::PathBuf, sync::Mutex};

use podman_lens::{
    AcquisitionOptions, DiagnosticCode, EnvironmentValue, InventorySection, LibpodHeader, LibpodHeaders, LibpodMethod,
    LibpodRequest, LibpodResponse, LibpodTransport, LibpodTransportFuture, ObservationState, ResourceKind,
    TransportError, acquire_inventory,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct InventoryFixtureManifest {
    schema_version: u8,
    evidence_kind: String,
    sanitization: String,
    fixtures: Vec<InventoryFixture>,
}

#[derive(Deserialize)]
struct InventoryFixture {
    engine_version: String,
    api_version: String,
    release_tag: String,
    commit: String,
    source_url: String,
    artifact: String,
    sha256: String,
}

struct RecordingTransport {
    responses: Mutex<VecDeque<LibpodResponse>>,
    requests: Mutex<Vec<(LibpodMethod, String)>>,
}

impl RecordingTransport {
    fn new(responses: Vec<LibpodResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<(LibpodMethod, String)> {
        self.requests.lock().expect("test lock").clone()
    }
}

impl LibpodTransport for RecordingTransport {
    fn send<'a>(&'a self, request: &'a LibpodRequest) -> LibpodTransportFuture<'a> {
        let response = self
            .requests
            .lock()
            .map_err(|_| TransportError::unavailable())
            .and_then(|mut requests| {
                requests.push((request.method(), request.path().as_str().to_owned()));
                self.responses
                    .lock()
                    .map_err(|_| TransportError::unavailable())
                    .and_then(|mut responses| responses.pop_front().ok_or_else(TransportError::unavailable))
            });
        Box::pin(async move { response })
    }
}

fn json(body: impl AsRef<[u8]>) -> Result<LibpodResponse, Box<dyn std::error::Error>> {
    Ok(LibpodResponse::new(
        200,
        LibpodHeaders::new(vec![LibpodHeader::new("content-type", "application/json")?]),
        body.as_ref().to_vec(),
    )?)
}

fn status(status: u16) -> Result<LibpodResponse, Box<dyn std::error::Error>> {
    Ok(LibpodResponse::new(status, LibpodHeaders::default(), Vec::new())?)
}

fn fixture_responses(version: &str) -> Result<Vec<LibpodResponse>, Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/inventory");
    let artifact = fs::read(root.join(format!("podman-{version}.responses.json")))?;
    let fixture: Value = serde_json::from_slice(&artifact)?;
    assert_eq!(fixture.get("engine_version").and_then(Value::as_str), Some(version));
    let responses = fixture
        .get("responses")
        .and_then(Value::as_array)
        .ok_or("fixture must provide ordered responses")?;
    responses
        .iter()
        .map(|response| {
            let status = response
                .get("status")
                .and_then(Value::as_u64)
                .ok_or("fixture response status must be an unsigned integer")?
                .try_into()?;
            let headers = response
                .get("headers")
                .and_then(Value::as_array)
                .ok_or("fixture response headers must be an array")?
                .iter()
                .map(|header| {
                    let pair = header.as_array().ok_or("fixture header must be a pair")?;
                    let [name, value] = pair.as_slice() else {
                        return Err("fixture header must contain exactly two values".into());
                    };
                    LibpodHeader::new(
                        name.as_str().ok_or("fixture header name must be a string")?,
                        value.as_str().ok_or("fixture header value must be a string")?,
                    )
                    .map_err(Into::into)
                })
                .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
            let body = response.get("body").cloned().unwrap_or(Value::Null);
            let body = if body.is_null() {
                Vec::new()
            } else {
                serde_json::to_vec(&body)?
            };
            Ok(LibpodResponse::new(status, LibpodHeaders::new(headers), body)?)
        })
        .collect()
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Exact ordered native request assertions are intentionally adjacent.
async fn acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids()
-> Result<(), Box<dyn std::error::Error>> {
    let transport = RecordingTransport::new(fixture_responses("6.1.0")?);
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    assert_eq!(inventory.sections().len(), 6);
    assert_eq!(inventory.service().api_version().original(), "6.1.0");
    let requests = transport.requests();
    assert!(requests.iter().all(|(method, _)| *method == LibpodMethod::Get));
    assert_eq!(
        requests,
        [
            "/libpod/_ping",
            "/v4.0.0/libpod/version",
            "/v6.1.0/libpod/containers/json?all=true&sync=true",
            "/v6.1.0/libpod/pods/json",
            "/v6.1.0/libpod/networks/json",
            "/v6.1.0/libpod/volumes/json",
            "/v6.1.0/libpod/images/json?all=true",
            "/v6.1.0/libpod/secrets/json",
            "/v6.1.0/libpod/containers/container-a/json",
            "/v6.1.0/libpod/containers/container-z/json",
            "/v6.1.0/libpod/pods/pod-1/json",
            "/v6.1.0/libpod/networks/network-1/json",
            "/v6.1.0/libpod/volumes/database-data/json",
            "/v6.1.0/libpod/images/sha256%3Aabc/json",
            "/v6.1.0/libpod/secrets/secret-1/json",
        ]
        .into_iter()
        .map(|path| (LibpodMethod::Get, path.to_owned()))
        .collect::<Vec<_>>()
    );
    assert!(
        requests
            .iter()
            .all(|(_, path)| !path.contains("showsecret") && !path.contains("/secrets/secret-1/data"))
    );

    let containers = inventory.section(ResourceKind::Container).expect("containers");
    assert_eq!(containers.records()[0].identity().id(), "container-a");
    assert_eq!(containers.records()[0].environment().len(), 2);
    assert!(
        containers.records()[0]
            .environment()
            .iter()
            .all(|entry| matches!(entry.value(), EnvironmentValue::Redacted))
    );
    assert!(
        containers.records()[0]
            .unknown_fields()
            .iter()
            .any(|field| field.path() == "$.FutureField")
    );
    assert!(
        containers.records()[0]
            .unknown_fields()
            .iter()
            .any(|field| field.path() == "$.Config.FutureConfig")
    );
    assert_eq!(
        containers.records()[0].unknown_fields()[0].evidence().api_version(),
        "6.1.0"
    );
    assert!(
        containers.records()[0]
            .relationships()
            .iter()
            .any(|relationship| relationship.kind() == ResourceKind::Secret && relationship.target_id() == "secret-1")
    );
    assert_eq!(containers.records()[1].state(), ObservationState::Complete);
    assert!(containers.records()[1].labels().is_empty());
    assert!(containers.records()[1].environment().is_empty());
    assert_eq!(containers.records()[0].memory_swappiness(), Some(10));
    assert!(
        !containers.records()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::RelationshipConflict)
    );
    assert!(!format!("{:?}", containers.records()[0]).contains("fixture-label"));
    let network = inventory.section(ResourceKind::Network).expect("network").records()[0]
        .network()
        .expect("network details");
    assert_eq!(network.internal(), Some(true));
    assert_eq!(network.options().get("mtu"), Some(&"fixture-option".to_owned()));
    assert_eq!(network.subnets(), ["10.88.0.0/16"]);
    let secret = &inventory.section(ResourceKind::Secret).expect("secret").records()[0];
    assert_eq!(secret.secret_driver(), Some("file"));
    assert!(
        secret
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::SecretPayloadDiscarded)
    );
    let image = &inventory.section(ResourceKind::Image).expect("images").records()[0];
    assert_eq!(
        image.image_aliases(),
        ["registry.example.invalid/team/image:1@sha256:abc", "image:latest"]
    );
    assert!(
        !format!(
            "{:?}",
            inventory.section(ResourceKind::Network).expect("network").records()[0].network()
        )
        .contains("fixture-option")
    );
    Ok(())
}

#[tokio::test]
async fn explicit_environment_inclusion_is_opaque_and_preserves_duplicate_order()
-> Result<(), Box<dyn std::error::Error>> {
    let mut responses = fixture_responses("5.4.0")?;
    responses[8] = json(
        r#"{"Id":"container-a","Name":"a","Config":{"Env":["A=one","A=two"]},"HostConfig":{"MemorySwappiness":null}}"#,
    )?;
    let transport = RecordingTransport::new(responses);
    let inventory = acquire_inventory(&transport, AcquisitionOptions::include_environment_values()).await?;
    let entries = inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .records()[0]
        .environment();
    assert_eq!(
        entries
            .iter()
            .map(podman_lens::EnvironmentEntry::name)
            .collect::<Vec<_>>(),
        ["A", "A"]
    );
    let EnvironmentValue::Included(value) = entries[0].value() else {
        return Err("authorized environment value was unexpectedly redacted".into());
    };
    assert_eq!(value.expose(ToOwned::to_owned), "one");
    assert!(!format!("{value:?} {value}").contains("one"));
    assert!(
        inventory
            .section(ResourceKind::Container)
            .expect("containers")
            .records()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::VersionInapplicableField)
    );
    Ok(())
}

#[tokio::test]
async fn malformed_and_duplicate_list_entries_do_not_hide_valid_resources() -> Result<(), Box<dyn std::error::Error>> {
    let mut responses = fixture_responses("6.1.0")?;
    responses[2] = json(r#"[{"Id":"container-a"},{"Id":"container-a"},{"missing":"id"},{"Id":"container-z"}]"#)?;
    let transport = RecordingTransport::new(responses);
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    let containers = inventory.section(ResourceKind::Container).expect("containers");
    assert!(containers.available());
    assert_eq!(containers.records().len(), 2);
    assert_eq!(containers.records()[0].identity().id(), "container-a");
    assert_eq!(containers.findings().len(), 2);
    assert!(
        containers
            .findings()
            .iter()
            .all(|finding| finding.code() == DiagnosticCode::ResourceMalformed)
    );
    Ok(())
}

#[tokio::test]
async fn unavailable_lists_and_disappeared_inspects_remain_visible_as_partial_inventory()
-> Result<(), Box<dyn std::error::Error>> {
    let mut responses = fixture_responses("6.1.0")?;
    responses[4] = status(500)?;
    // Lists remain six responses; network inspection is omitted because the network list failed.
    responses.remove(11);
    responses[8] = status(404)?;
    let transport = RecordingTransport::new(responses);
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    let containers = inventory.section(ResourceKind::Container).expect("containers");
    assert_eq!(containers.records()[0].state(), ObservationState::Partial);
    assert_eq!(
        containers.records()[0].findings()[0].code(),
        DiagnosticCode::ResourceUnavailable
    );
    let network = inventory.section(ResourceKind::Network).expect("network");
    assert!(!network.available());
    assert_eq!(network.findings()[0].code(), DiagnosticCode::InventoryHttpStatus);
    assert!(inventory.section(ResourceKind::Secret).expect("secrets").available());
    Ok(())
}

#[tokio::test]
async fn every_list_status_and_shape_failure_leaves_only_its_section_unavailable()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (2, ResourceKind::Container, vec![9, 8], "{}"),
        (3, ResourceKind::Pod, vec![10], "{}"),
        (4, ResourceKind::Network, vec![11], "{}"),
        (5, ResourceKind::Volume, vec![12], r#"{"Volumes":{}}"#),
        (6, ResourceKind::Image, vec![13], "{}"),
        (7, ResourceKind::Secret, vec![14], "{}"),
    ];
    for (list_index, kind, mut inspections, malformed_body) in cases {
        for replacement in [status(503)?, json(malformed_body)?] {
            let mut responses = fixture_responses("6.1.0")?;
            responses[list_index] = replacement.clone();
            inspections.sort_unstable_by(|left, right| right.cmp(left));
            for inspection in &inspections {
                responses.remove(*inspection);
            }
            let inventory =
                acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
            let section = inventory.section(kind).expect("fixed section");
            assert!(!section.available(), "{kind:?} must be unavailable");
            assert_eq!(section.findings().len(), 1);
            assert!(
                inventory
                    .sections()
                    .iter()
                    .filter(|section| section.kind() != kind)
                    .all(InventorySection::available)
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn every_inspect_status_and_shape_failure_retains_a_partial_stable_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (8, ResourceKind::Container),
        (10, ResourceKind::Pod),
        (11, ResourceKind::Network),
        (12, ResourceKind::Volume),
        (13, ResourceKind::Image),
        (14, ResourceKind::Secret),
    ];
    for (response_index, kind) in cases {
        for replacement in [status(503)?, json("{}")?] {
            let mut responses = fixture_responses("6.1.0")?;
            responses[response_index] = replacement;
            let inventory =
                acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
            let record = &inventory.section(kind).expect("fixed section").records()[0];
            assert_eq!(
                record.state(),
                ObservationState::Partial,
                "{kind:?} must remain partial"
            );
            assert!(!record.identity().id().is_empty());
            assert!(
                record
                    .findings()
                    .iter()
                    .any(|finding| finding.code() == DiagnosticCode::ResourceUnavailable
                        || finding.code() == DiagnosticCode::InventoryHttpStatus
                        || finding.code() == DiagnosticCode::InventoryShape
                        || finding.code() == DiagnosticCode::ResourceMalformed)
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            8,
            ResourceKind::Container,
            r#"{"Id":"container-a","Name":"a","Config":{"Env":false}}"#,
            "$.Config.Env",
        ),
        (
            8,
            ResourceKind::Container,
            r#"{"Id":"container-a","Name":"a","NetworkSettings":{"Networks":{"app":false}}}"#,
            "$.NetworkSettings.Networks.app",
        ),
        (
            8,
            ResourceKind::Container,
            r#"{"Id":"container-a","Name":"a","Mounts":[false]}"#,
            "$.Mounts",
        ),
        (
            8,
            ResourceKind::Container,
            r#"{"Id":"container-a","Name":"a","Dependencies":[false]}"#,
            "$.Dependencies",
        ),
        (
            8,
            ResourceKind::Container,
            r#"{"Id":"container-a","Name":"a","Config":{"Secrets":[false]}}"#,
            "$.Config.Secrets",
        ),
        (
            10,
            ResourceKind::Pod,
            r#"{"Id":"pod-1","Name":"pod-one","Containers":[false]}"#,
            "$.Containers",
        ),
        (
            10,
            ResourceKind::Pod,
            r#"{"Id":"pod-1","Name":"pod-one","Networks":[false]}"#,
            "$.Networks",
        ),
        (
            11,
            ResourceKind::Network,
            r#"{"id":"network-1","name":"app","internal":"yes"}"#,
            "$.internal",
        ),
        (
            11,
            ResourceKind::Network,
            r#"{"id":"network-1","name":"app","options":false}"#,
            "$.options",
        ),
        (
            11,
            ResourceKind::Network,
            r#"{"id":"network-1","name":"app","subnets":[false]}"#,
            "$.subnets",
        ),
        (
            13,
            ResourceKind::Image,
            r#"{"Id":"sha256:abc","Names":["image:latest"],"Config":false}"#,
            "$.Config",
        ),
        (
            14,
            ResourceKind::Secret,
            r#"{"ID":"secret-1","Spec":{"Name":"db-password","Driver":false}}"#,
            "$.Spec.Driver",
        ),
    ];
    for (response_index, kind, body, expected_path) in cases {
        let mut responses = fixture_responses("6.1.0")?;
        responses[response_index] = json(body)?;
        let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
        let record = &inventory.section(kind).expect("fixed section").records()[0];
        assert_eq!(record.state(), ObservationState::Complete);
        assert!(record.findings().iter().any(|finding| {
            finding.code() == DiagnosticCode::ResourceMalformed && finding.field_path() == Some(expected_path)
        }));
    }
    Ok(())
}

#[tokio::test]
async fn environment_boundaries_preserve_valid_entries_and_report_every_bad_occurrence()
-> Result<(), Box<dyn std::error::Error>> {
    let mut responses = fixture_responses("6.1.0")?;
    responses[8] =
        json(r#"{"Id":"container-a","Name":"a","Config":{"Env":["NO_EQUALS","=empty",7,"A=value=with=equals"]}}"#)?;
    let inventory = acquire_inventory(
        &RecordingTransport::new(responses),
        AcquisitionOptions::include_environment_values(),
    )
    .await?;
    let record = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .records()[0];
    assert_eq!(record.environment().len(), 1);
    assert_eq!(record.environment()[0].name(), "A");
    let EnvironmentValue::Included(value) = record.environment()[0].value() else {
        return Err("authorized value unexpectedly redacted".into());
    };
    assert_eq!(value.expose(ToOwned::to_owned), "value=with=equals");
    let malformed = record
        .findings()
        .iter()
        .filter(|finding| finding.code() == DiagnosticCode::EnvironmentMalformed)
        .collect::<Vec<_>>();
    assert_eq!(malformed.len(), 3);
    assert!(
        malformed
            .iter()
            .all(|finding| finding.field_path() == Some("$.Config.Env"))
    );
    assert_eq!(
        malformed.iter().map(|finding| finding.occurrence()).collect::<Vec<_>>(),
        vec![Some(0), Some(1), Some(2)]
    );
    Ok(())
}

#[tokio::test]
async fn memory_swappiness_distinguishes_reviewed_null_boundary_and_invalid_values()
-> Result<(), Box<dyn std::error::Error>> {
    let inventory = acquire_inventory(
        &RecordingTransport::new(fixture_responses("5.4.0")?),
        AcquisitionOptions::redacted(),
    )
    .await?;
    assert!(
        inventory
            .section(ResourceKind::Container)
            .expect("containers")
            .records()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::VersionInapplicableField)
    );

    for invalid in ["\"not-a-number\"", "true", "-1"] {
        let mut responses = fixture_responses("6.1.0")?;
        responses[8] = json(format!(
            r#"{{"Id":"container-a","Name":"a","HostConfig":{{"MemorySwappiness":{invalid}}}}}"#
        ))?;
        let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
        assert!(
            inventory
                .section(ResourceKind::Container)
                .expect("containers")
                .records()[0]
                .findings()
                .iter()
                .any(|finding| finding.code() == DiagnosticCode::ResourceMalformed)
        );
    }

    let inventory = acquire_inventory(
        &RecordingTransport::new(fixture_responses("6.1.0")?),
        AcquisitionOptions::redacted(),
    )
    .await?;
    assert!(
        !inventory
            .section(ResourceKind::Container)
            .expect("containers")
            .records()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::VersionInapplicableField)
    );
    Ok(())
}

#[tokio::test]
async fn unknown_fields_are_bounded_per_record_and_across_the_inventory() -> Result<(), Box<dyn std::error::Error>> {
    let mut responses = fixture_responses("6.1.0")?;
    let mut record = serde_json::json!({"Id": "container-a", "Name": "a"});
    let object = record.as_object_mut().expect("object fixture");
    for index in 0..129 {
        object.insert(format!("Unknown{index}"), Value::Bool(true));
    }
    responses[8] = json(serde_json::to_vec(&record)?)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let record = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .records()[0];
    assert_eq!(
        record.unknown_fields().len(),
        podman_lens::MAX_UNKNOWN_FIELDS_PER_RECORD
    );
    assert!(
        record
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::UnknownFieldOverflow)
    );

    let mut responses = fixture_responses("6.1.0")?;
    let ids = (0..17).map(|index| format!("container-{index:02}")).collect::<Vec<_>>();
    responses[2] = json(serde_json::to_vec(
        &ids.iter().map(|id| serde_json::json!({"Id": id})).collect::<Vec<_>>(),
    )?)?;
    responses.remove(9);
    responses.remove(8);
    for (offset, id) in ids.iter().enumerate() {
        let mut record = serde_json::json!({"Id": id});
        let object = record.as_object_mut().expect("object fixture");
        for field in 0..podman_lens::MAX_UNKNOWN_FIELDS_PER_RECORD {
            object.insert(format!("Unknown{field}"), Value::Number(1.into()));
        }
        responses.insert(8 + offset, json(serde_json::to_vec(&record)?)?);
    }
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let records = inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .records();
    assert_eq!(
        records
            .iter()
            .map(|record| record.unknown_fields().len())
            .sum::<usize>(),
        podman_lens::MAX_UNKNOWN_FIELDS_PER_INVENTORY
    );
    assert!(records.iter().any(|record| {
        record
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::UnknownFieldOverflow)
    }));
    Ok(())
}

#[tokio::test]
async fn reconciliation_resolves_aliases_and_reports_missing_or_disagreeing_memberships()
-> Result<(), Box<dyn std::error::Error>> {
    let mut responses = fixture_responses("6.1.0")?;
    responses[8] = json(
        r#"{"Id":"container-a","Name":"a","Image":"sha256:abc","ImageName":"image:latest","Pod":"pod-one","NetworkSettings":{"Networks":{"missing-network":{}}}}"#,
    )?;
    responses[10] = json(r#"{"Id":"pod-1","Name":"pod-one","Containers":[{"Id":"a"}]}"#)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let containers = inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .records();
    let first = &containers[0];
    assert!(
        first
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::UnresolvedRelationship)
    );
    assert!(
        first
            .findings()
            .iter()
            .all(|finding| finding.code() != DiagnosticCode::PodMembershipConflict)
    );
    assert!(
        inventory.section(ResourceKind::Pod).expect("pods").records()[0]
            .findings()
            .iter()
            .all(|finding| finding.code() != DiagnosticCode::PodMembershipConflict)
    );

    let mut responses = fixture_responses("6.1.0")?;
    responses[10] = json(r#"{"Id":"pod-1","Name":"pod-one","Containers":[{"Id":"container-z"}]}"#)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    assert!(
        inventory.section(ResourceKind::Pod).expect("pods").records()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::PodMembershipConflict)
    );
    Ok(())
}

#[tokio::test]
async fn debug_output_never_leaks_labels_network_options_or_environment_values()
-> Result<(), Box<dyn std::error::Error>> {
    let inventory = acquire_inventory(
        &RecordingTransport::new(fixture_responses("6.1.0")?),
        AcquisitionOptions::include_environment_values(),
    )
    .await?;
    let rendered = format!("{inventory:?}");
    for secret in ["fixture-label", "fixture-option", "A=one", "IMAGE_ENV=value"] {
        assert!(!rendered.contains(secret), "debug output leaked {secret}");
    }
    Ok(())
}

#[test]
fn pinned_inventory_fixture_manifest_covers_reviewed_5_4_and_6_1_boundaries() -> Result<(), Box<dyn std::error::Error>>
{
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/inventory");
    let manifest: InventoryFixtureManifest = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.evidence_kind, "source-derived-minimal");
    assert!(!manifest.sanitization.is_empty());
    assert_eq!(manifest.fixtures.len(), 2);
    for fixture in manifest.fixtures {
        assert_eq!(fixture.release_tag, format!("v{}", fixture.engine_version));
        assert_eq!(fixture.engine_version, fixture.api_version);
        assert_eq!(fixture.commit.len(), 40);
        assert!(
            fixture
                .commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        );
        assert!(fixture.source_url.starts_with("https://docs.podman.io/en/v"));
        let artifact = fs::read(root.join(&fixture.artifact))?;
        assert_eq!(hex_digest(&Sha256::digest(&artifact)), fixture.sha256);
        let response: Value = serde_json::from_slice(&artifact)?;
        assert_eq!(
            response.get("engine_version").and_then(Value::as_str),
            Some(fixture.engine_version.as_str())
        );
        assert_eq!(
            response.get("api_version").and_then(Value::as_str),
            Some(fixture.api_version.as_str())
        );
        assert!(
            response
                .get("provenance")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
        );
        assert_eq!(
            response.get("responses").and_then(Value::as_array).map(Vec::len),
            Some(15)
        );
    }
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}
