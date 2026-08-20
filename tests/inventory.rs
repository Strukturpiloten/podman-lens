//! Offline M2 acquisition coverage using bounded fixture transports.

#![allow(clippy::expect_used, clippy::panic)] // Test-only fixture access reports concise assertion failures.

use std::{collections::VecDeque, fs, path::PathBuf, sync::Mutex};

use podman_lens::{
    AcquisitionOptions, DiagnosticCode, LibpodHeader, LibpodHeaders, LibpodMethod, LibpodRequest, LibpodResponse,
    LibpodTransport, LibpodTransportFuture, ObservationField, ObservationOrigin, ProtectedEnvironmentEntry,
    ProtectedEnvironmentValue, ResourceDetails, ResourceKind, ResourceObservation, TransportError,
    UnmodelledCompleteness, acquire_inventory,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Debug, Eq, PartialEq)]
enum ObservationState {
    Complete,
    Partial,
}
enum EnvironmentValue<'a> {
    Redacted,
    Included(&'a podman_lens::SensitiveEnvironmentValue),
}
trait EnvironmentEntryAccess {
    fn test_value(&self) -> EnvironmentValue<'_>;
}
impl EnvironmentEntryAccess for ProtectedEnvironmentEntry {
    fn test_value(&self) -> EnvironmentValue<'_> {
        match self.value() {
            ProtectedEnvironmentValue::AuthorizedOpaque(value) => EnvironmentValue::Included(value),
            _ => EnvironmentValue::Redacted,
        }
    }
}
#[derive(Clone)]
struct TestNetwork {
    internal: Option<bool>,
    options: Vec<String>,
    subnets: Vec<String>,
}
impl std::fmt::Debug for TestNetwork {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TestNetwork")
            .field("internal", &self.internal)
            .field("option_count", &self.options.len())
            .field("subnet_count", &self.subnets.len())
            .finish()
    }
}
impl TestNetwork {
    fn internal(&self) -> Option<bool> {
        self.internal
    }
    fn options(&self) -> &[String] {
        &self.options
    }
    fn subnets(&self) -> &[String] {
        &self.subnets
    }
}
trait ObservationTestAccess {
    fn identity(&self) -> &podman_lens::ResourceIdentity;
    fn state(&self) -> ObservationState;
    fn environment(&self) -> Vec<&ProtectedEnvironmentEntry>;
    fn image_aliases(&self) -> Vec<String>;
    fn network(&self) -> Option<TestNetwork>;
    fn memory_swappiness(&self) -> Option<u64>;
    fn secret_driver(&self) -> Option<&str>;
    fn unknown_fields(&self) -> &[podman_lens::UnmodelledField];
    fn unknown_fields_complete(&self) -> bool;
    fn findings(&self) -> &[podman_lens::InventoryFinding];
}
impl ObservationTestAccess for ResourceObservation {
    fn identity(&self) -> &podman_lens::ResourceIdentity {
        self.header().identity()
    }
    fn state(&self) -> ObservationState {
        if self.header().state() == podman_lens::ResourceObservationState::Complete {
            ObservationState::Complete
        } else {
            ObservationState::Partial
        }
    }
    fn environment(&self) -> Vec<&ProtectedEnvironmentEntry> {
        match self.details() {
            ResourceDetails::Container(value) => value.environment(),
            ResourceDetails::Image(value) => value.environment(),
            _ => return Vec::new(),
        }
        .observed()
        .map(|value| value.value().entries().iter().collect())
        .unwrap_or_default()
    }
    fn image_aliases(&self) -> Vec<String> {
        match self.details() {
            ResourceDetails::Image(value) => value
                .aliases()
                .observed()
                .map(|value| value.value().clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }
    fn network(&self) -> Option<TestNetwork> {
        let ResourceDetails::Network(value) = self.details() else {
            return None;
        };
        Some(TestNetwork {
            internal: value.internal().observed().map(|value| *value.value()),
            options: value
                .options()
                .observed()
                .map(|value| value.value().keys().map(ToOwned::to_owned).collect())
                .unwrap_or_default(),
            subnets: value
                .subnets()
                .observed()
                .map(|value| value.value().clone())
                .unwrap_or_default(),
        })
    }
    fn memory_swappiness(&self) -> Option<u64> {
        let ResourceDetails::Container(value) = self.details() else {
            return None;
        };
        value.memory_swappiness().observed().map(|value| *value.value())
    }
    fn secret_driver(&self) -> Option<&str> {
        let ResourceDetails::Secret(value) = self.details() else {
            return None;
        };
        value.driver().observed().map(|value| value.value().as_str())
    }
    fn unknown_fields(&self) -> &[podman_lens::UnmodelledField] {
        self.header().unmodelled_fields()
    }
    fn unknown_fields_complete(&self) -> bool {
        self.header().unmodelled_completeness() == UnmodelledCompleteness::Complete
    }
    fn findings(&self) -> &[podman_lens::InventoryFinding] {
        self.header().findings()
    }
}

fn container_memory_field(observation: &ResourceObservation) -> &ObservationField<u64> {
    let ResourceDetails::Container(container) = observation.details() else {
        panic!("fixture observation must be a container");
    };
    container.memory_swappiness()
}

fn volume_owner_fields(
    observation: &ResourceObservation,
) -> (
    &ObservationField<podman_lens::VolumeOwnerIdWireValue>,
    &ObservationField<podman_lens::VolumeOwnerIdWireValue>,
) {
    let ResourceDetails::Volume(volume) = observation.details() else {
        panic!("fixture observation must be a volume");
    };
    (volume.uid(), volume.gid())
}

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
    assert_eq!(containers.observations()[0].identity().id(), "container-a");
    assert_eq!(containers.observations()[0].environment().len(), 2);
    assert!(
        containers.observations()[0]
            .environment()
            .iter()
            .all(|entry| matches!(entry.test_value(), EnvironmentValue::Redacted))
    );
    assert!(
        containers.observations()[0]
            .unknown_fields()
            .iter()
            .any(|field| field.path() == "$.FutureField")
    );
    assert!(
        containers.observations()[0]
            .unknown_fields()
            .iter()
            .any(|field| field.path() == "$.Config.FutureConfig")
    );
    assert_eq!(
        containers.observations()[0].unknown_fields()[0]
            .evidence()
            .api_version(),
        "6.1.0"
    );
    assert_eq!(containers.observations()[1].state(), ObservationState::Complete);
    let ResourceDetails::Container(second_container) = containers.observations()[1].details() else {
        return Err("second fixture observation must be a container".into());
    };
    assert!(matches!(second_container.labels(), ObservationField::Absent));
    assert!(containers.observations()[1].environment().is_empty());
    assert_eq!(containers.observations()[0].memory_swappiness(), Some(10));
    let ResourceDetails::Container(container) = containers.observations()[0].details() else {
        return Err("container fixture has the wrong detail kind".into());
    };
    let configured_image = container
        .configured_image()
        .observed()
        .ok_or("configured image must be observed")?;
    assert_eq!(configured_image.value(), "sha256:abc");
    assert_eq!(configured_image.origin(), ObservationOrigin::Configured);
    let local_image_id = container
        .local_image_id()
        .observed()
        .ok_or("local image ID must be observed")?;
    assert_eq!(local_image_id.value(), "sha256:abc");
    assert_eq!(local_image_id.origin(), ObservationOrigin::LocalResolution);
    assert!(
        !containers.observations()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::RelationshipConflict)
    );
    assert!(!format!("{:?}", containers.observations()[0]).contains("fixture-label"));
    let network = inventory
        .section(ResourceKind::Network)
        .expect("network")
        .observations()[0]
        .network()
        .expect("network details");
    assert_eq!(network.internal(), Some(true));
    assert_eq!(network.options(), ["mtu"]);
    assert_eq!(network.subnets(), ["10.88.0.0/16"]);
    let secret = &inventory.section(ResourceKind::Secret).expect("secret").observations()[0];
    assert_eq!(secret.secret_driver(), Some("file"));
    assert!(
        !secret
            .unknown_fields()
            .iter()
            .any(|field| field.path() == "$.Spec.Driver")
    );
    assert!(
        secret
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::SecretPayloadDiscarded)
    );
    let image = &inventory.section(ResourceKind::Image).expect("images").observations()[0];
    assert_eq!(
        image.image_aliases(),
        ["registry.example.invalid/team/image:1@sha256:abc", "image:latest"]
    );
    assert!(
        !format!(
            "{:?}",
            inventory
                .section(ResourceKind::Network)
                .expect("network")
                .observations()[0]
                .network()
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
        .observations()[0]
        .environment();
    assert_eq!(entries.iter().map(|entry| entry.name()).collect::<Vec<_>>(), ["A", "A"]);
    let EnvironmentValue::Included(value) = entries[0].test_value() else {
        return Err("authorized environment value was unexpectedly redacted".into());
    };
    assert_eq!(value.expose(ToOwned::to_owned), "one");
    assert!(!format!("{value:?} {value}").contains("one"));
    assert!(
        inventory
            .section(ResourceKind::Container)
            .expect("containers")
            .observations()[0]
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
    assert_eq!(
        containers.availability(),
        podman_lens::InventorySectionAvailability::Available
    );
    assert_eq!(containers.observations().len(), 2);
    assert_eq!(containers.observations()[0].identity().id(), "container-a");
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
    assert_eq!(containers.observations()[0].state(), ObservationState::Partial);
    assert_eq!(
        containers.observations()[0].findings()[0].code(),
        DiagnosticCode::ResourceUnavailable
    );
    let network = inventory.section(ResourceKind::Network).expect("network");
    assert_ne!(
        network.availability(),
        podman_lens::InventorySectionAvailability::Available
    );
    assert_eq!(network.findings()[0].code(), DiagnosticCode::InventoryHttpStatus);
    assert_eq!(
        inventory.section(ResourceKind::Secret).expect("secrets").availability(),
        podman_lens::InventorySectionAvailability::Available
    );
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
            assert_ne!(
                section.availability(),
                podman_lens::InventorySectionAvailability::Available,
                "{kind:?} must be unavailable"
            );
            assert_eq!(section.findings().len(), 1);
            assert!(
                inventory
                    .sections()
                    .iter()
                    .filter(|section| section.kind() != kind)
                    .all(|section| section.availability() == podman_lens::InventorySectionAvailability::Available)
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
            let record = &inventory.section(kind).expect("fixed section").observations()[0];
            assert_eq!(
                record.state(),
                ObservationState::Partial,
                "{kind:?} must remain partial"
            );
            assert!(!record.identity().id().is_empty());
            assert!(
                !record.unknown_fields_complete(),
                "{kind:?} partial records cannot claim exhaustive unknown metadata"
            );
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
        let record = &inventory.section(kind).expect("fixed section").observations()[0];
        assert_eq!(record.state(), ObservationState::Complete);
        assert!(record.findings().iter().any(|finding| {
            finding.code() == DiagnosticCode::ResourceMalformed && finding.field_path() == Some(expected_path)
        }));
    }
    Ok(())
}

#[tokio::test]
async fn malformed_labels_are_local_to_every_typed_resource_observation() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (8, ResourceKind::Container, "/Config", "Labels", "$.Config.Labels"),
        (10, ResourceKind::Pod, "", "Labels", "$.Labels"),
        (11, ResourceKind::Network, "", "labels", "$.labels"),
        (12, ResourceKind::Volume, "", "Labels", "$.Labels"),
        (13, ResourceKind::Image, "", "Labels", "$.Labels"),
        (14, ResourceKind::Secret, "/Spec", "Labels", "$.Spec.Labels"),
    ];
    for (response_index, kind, parent, label_key, expected_path) in cases {
        let mut responses = fixture_responses("6.1.0")?;
        let mut body: Value = serde_json::from_slice(responses[response_index].body())?;
        let object = if parent.is_empty() {
            body.as_object_mut().ok_or("fixture response must be an object")?
        } else {
            body.pointer_mut(parent)
                .and_then(Value::as_object_mut)
                .ok_or("fixture label parent must be an object")?
        };
        object.insert(label_key.to_owned(), Value::Bool(false));
        responses[response_index] = json(serde_json::to_vec(&body)?)?;

        let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
        let observation = &inventory.section(kind).expect("fixed resource section").observations()[0];
        assert_eq!(observation.state(), ObservationState::Complete, "{kind:?}");
        let labels = match observation.details() {
            ResourceDetails::Container(value) => value.labels(),
            ResourceDetails::Pod(value) => value.labels(),
            ResourceDetails::Network(value) => value.labels(),
            ResourceDetails::Volume(value) => value.labels(),
            ResourceDetails::Image(value) => value.labels(),
            ResourceDetails::Secret(value) => value.labels(),
            _ => panic!("future details variant has no labels"),
        };
        assert!(matches!(labels, ObservationField::Malformed), "{kind:?}");
        assert!(observation.findings().iter().any(|finding| {
            finding.code() == DiagnosticCode::ResourceMalformed && finding.field_path() == Some(expected_path)
        }));
    }

    let mut responses = fixture_responses("6.1.0")?;
    let mut body: Value = serde_json::from_slice(responses[8].body())?;
    body["Config"]["Labels"] = Value::Bool(false);
    responses[8] = json(serde_json::to_vec(&body)?)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let container = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .observations()[0];
    let ResourceDetails::Container(details) = container.details() else {
        return Err("container fixture must decode as a container".into());
    };
    assert!(details.configured_image().is_observed());
    assert!(details.local_image_id().is_observed());
    assert!(details.environment().is_observed());
    Ok(())
}

#[tokio::test]
async fn malformed_enclosing_config_marks_every_modeled_child_malformed() -> Result<(), Box<dyn std::error::Error>> {
    let mut responses = fixture_responses("6.1.0")?;
    responses[8] = json(r#"{"Id":"container-a","Name":"a","Config":false}"#)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let container = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .observations()[0];
    let ResourceDetails::Container(details) = container.details() else {
        return Err("fixture must remain a container".into());
    };
    assert!(matches!(details.labels(), ObservationField::Malformed));
    assert!(matches!(details.environment(), ObservationField::Malformed));
    assert!(
        container
            .findings()
            .iter()
            .any(|finding| finding.field_path() == Some("$.Config"))
    );

    let mut responses = fixture_responses("6.1.0")?;
    responses[13] = json(r#"{"Id":"sha256:abc","Names":["example.invalid/image:1"],"Config":false}"#)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let image = &inventory.section(ResourceKind::Image).expect("images").observations()[0];
    let ResourceDetails::Image(details) = image.details() else {
        return Err("fixture must remain an image".into());
    };
    assert!(matches!(details.environment(), ObservationField::Malformed));
    assert!(
        image
            .findings()
            .iter()
            .any(|finding| finding.field_path() == Some("$.Config"))
    );
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
        .observations()[0];
    let ResourceDetails::Container(details) = record.details() else {
        return Err("fixture must remain a container observation".into());
    };
    assert!(matches!(details.environment(), ObservationField::Malformed));
    assert!(record.environment().is_empty());
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
            .observations()[0]
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
                .observations()[0]
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
            .observations()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::VersionInapplicableField)
    );
    Ok(())
}

#[tokio::test]
async fn public_typed_observations_exercise_every_field_state_without_promoting_unmodelled_data()
-> Result<(), Box<dyn std::error::Error>> {
    let observed = acquire_inventory(
        &RecordingTransport::new(fixture_responses("6.1.0")?),
        AcquisitionOptions::redacted(),
    )
    .await?;
    assert!(matches!(
        container_memory_field(&observed.section(ResourceKind::Container).expect("containers").observations()[0]),
        ObservationField::Observed(value) if *value.value() == 10
    ));

    for body in [
        r#"{"Id":"container-a","Name":"a"}"#,
        r#"{"Id":"container-a","Name":"a","HostConfig":null}"#,
        r#"{"Id":"container-a","Name":"a","HostConfig":{}}"#,
    ] {
        let mut responses = fixture_responses("6.1.0")?;
        responses[8] = json(body)?;
        let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
        assert!(matches!(
            container_memory_field(
                &inventory
                    .section(ResourceKind::Container)
                    .expect("containers")
                    .observations()[0]
            ),
            ObservationField::Absent
        ));
    }

    let mut responses = fixture_responses("6.1.0")?;
    responses[8] = status(404)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    assert!(matches!(
        container_memory_field(
            &inventory
                .section(ResourceKind::Container)
                .expect("containers")
                .observations()[0]
        ),
        ObservationField::Unavailable
    ));

    let mut responses = fixture_responses("6.1.0")?;
    responses[8] = json(r#"{"Id":"container-a","Name":"a","HostConfig":{"MemorySwappiness":false}}"#)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    assert!(matches!(
        container_memory_field(
            &inventory
                .section(ResourceKind::Container)
                .expect("containers")
                .observations()[0]
        ),
        ObservationField::Malformed
    ));

    let version_inapplicable = acquire_inventory(
        &RecordingTransport::new(fixture_responses("5.4.0")?),
        AcquisitionOptions::redacted(),
    )
    .await?;
    assert!(matches!(
        container_memory_field(
            &version_inapplicable
                .section(ResourceKind::Container)
                .expect("containers")
                .observations()[0]
        ),
        ObservationField::VersionInapplicable
    ));

    assert!(matches!(
        container_memory_field(
            &observed
                .section(ResourceKind::Container)
                .expect("containers")
                .observations()[1]
        ),
        ObservationField::Absent
    ));

    let mut responses = fixture_responses("6.1.0")?;
    responses[8] = json(r#"{"Id":"container-a","Name":"a","HostConfig":{"NanoCpus":1}}"#)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let observation = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .observations()[0];
    assert!(matches!(container_memory_field(observation), ObservationField::Absent));
    assert!(
        observation
            .header()
            .unmodelled_fields()
            .iter()
            .any(|field| field.path() == "$.HostConfig.NanoCpus")
    );
    Ok(())
}

#[tokio::test]
async fn typed_section_and_resource_availability_preserve_their_failure_cause() -> Result<(), Box<dyn std::error::Error>>
{
    let mut responses = fixture_responses("6.1.0")?;
    responses[2] = status(500)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    assert_eq!(
        inventory
            .section(ResourceKind::Container)
            .expect("containers")
            .availability(),
        podman_lens::InventorySectionAvailability::Unavailable
    );

    let mut responses = fixture_responses("6.1.0")?;
    responses[2] = json("false")?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    assert_eq!(
        inventory
            .section(ResourceKind::Container)
            .expect("containers")
            .availability(),
        podman_lens::InventorySectionAvailability::Malformed
    );

    let mut responses = fixture_responses("6.1.0")?;
    responses[8] = json("false")?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let observation = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .observations()[0];
    assert_eq!(
        observation.header().state(),
        podman_lens::ResourceObservationState::Malformed
    );
    assert!(matches!(
        container_memory_field(observation),
        ObservationField::Malformed
    ));
    Ok(())
}

#[tokio::test]
async fn volume_owner_ids_preserve_absence_zero_bounds_and_unavailability() -> Result<(), Box<dyn std::error::Error>> {
    let inventory = acquire_inventory(
        &RecordingTransport::new(fixture_responses("6.1.0")?),
        AcquisitionOptions::redacted(),
    )
    .await?;
    let observation = &inventory.section(ResourceKind::Volume).expect("volumes").observations()[0];
    let (uid, gid) = volume_owner_fields(observation);
    assert!(matches!(
        uid,
        ObservationField::Observed(value)
            if matches!(value.value(), podman_lens::VolumeOwnerIdWireValue::WireAbsentMayMeanZero)
                && value.origin() == ObservationOrigin::Effective
    ));
    assert!(matches!(
        gid,
        ObservationField::Observed(value)
            if matches!(value.value(), podman_lens::VolumeOwnerIdWireValue::WireAbsentMayMeanZero)
                && value.origin() == ObservationOrigin::Effective
    ));
    assert_eq!(
        observation
            .header()
            .findings()
            .iter()
            .filter(|finding| finding.code() == DiagnosticCode::VolumeOwnerDefaultAmbiguous)
            .count(),
        2
    );

    for (wire, expected) in [("0", 0_u32), ("42", 42_u32)] {
        let mut responses = fixture_responses("6.1.0")?;
        responses[12] = json(format!(r#"{{"Name":"database-data","UID":{wire},"GID":{wire}}}"#))?;
        let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
        let observation = &inventory.section(ResourceKind::Volume).expect("volumes").observations()[0];
        for field in [volume_owner_fields(observation).0, volume_owner_fields(observation).1] {
            assert!(matches!(
                field,
                ObservationField::Observed(value)
                    if matches!(value.value(), podman_lens::VolumeOwnerIdWireValue::Explicit(id) if id.get() == expected)
                        && value.origin() == ObservationOrigin::Effective
            ));
        }
    }

    for wire in ["null", "false", "-1", "4294967296"] {
        let mut responses = fixture_responses("6.1.0")?;
        responses[12] = json(format!(r#"{{"Name":"database-data","UID":{wire},"GID":{wire}}}"#))?;
        let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
        let observation = &inventory.section(ResourceKind::Volume).expect("volumes").observations()[0];
        let (uid, gid) = volume_owner_fields(observation);
        assert!(matches!(uid, ObservationField::Malformed), "UID {wire}");
        assert!(matches!(gid, ObservationField::Malformed), "GID {wire}");
    }

    let mut responses = fixture_responses("6.1.0")?;
    responses[12] = status(404)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let observation = &inventory.section(ResourceKind::Volume).expect("volumes").observations()[0];
    let (uid, gid) = volume_owner_fields(observation);
    assert!(matches!(uid, ObservationField::Unavailable));
    assert!(matches!(gid, ObservationField::Unavailable));
    Ok(())
}

#[tokio::test]
async fn host_config_members_not_yet_modeled_are_retained_as_unknown_metadata() -> Result<(), Box<dyn std::error::Error>>
{
    let mut responses = fixture_responses("6.1.0")?;
    responses[8] = json(
        r#"{"Id":"container-a","Name":"a","HostConfig":{"MemorySwappiness":10,"NanoCpus":2000000000,"LogConfig":{"Type":"journald"}}}"#,
    )?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let record = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .observations()[0];
    assert_eq!(record.memory_swappiness(), Some(10));
    assert_eq!(
        record
            .unknown_fields()
            .iter()
            .map(podman_lens::UnmodelledField::path)
            .collect::<Vec<_>>(),
        ["$.HostConfig.LogConfig", "$.HostConfig.NanoCpus"]
    );
    assert!(record.unknown_fields_complete());
    assert!(
        record
            .findings()
            .iter()
            .filter(|finding| finding.code() == DiagnosticCode::NativeFieldUnsupported)
            .all(|finding| matches!(
                finding.field_path(),
                Some("$.HostConfig.LogConfig" | "$.HostConfig.NanoCpus")
            ))
    );
    Ok(())
}

#[tokio::test]
async fn secret_driver_is_modeled_without_unsupported_metadata() -> Result<(), Box<dyn std::error::Error>> {
    for version in ["5.4.0", "6.1.0"] {
        let mut responses = fixture_responses(version)?;
        responses[14] = json(r#"{"ID":"secret-1","Spec":{"Name":"database-password","Driver":"file"}}"#)?;
        let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
        let record = &inventory.section(ResourceKind::Secret).expect("secrets").observations()[0];
        assert_eq!(record.secret_driver(), Some("file"), "{version}");
        assert!(
            !record
                .unknown_fields()
                .iter()
                .any(|field| field.path() == "$.Spec.Driver"),
            "{version} must not classify Secret.Spec.Driver as unsupported"
        );
    }
    Ok(())
}

#[tokio::test]
async fn secret_payload_is_discarded_from_metadata_inspection() -> Result<(), Box<dyn std::error::Error>> {
    let mut responses = fixture_responses("6.1.0")?;
    responses[14] = json(
        r#"{"ID":"secret-1","Spec":{"Name":"database-password","SecretData":"must-not-be-retained"},"SecretData":"must-not-be-retained"}"#,
    )?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    let record = &inventory.section(ResourceKind::Secret).expect("secrets").observations()[0];
    assert!(
        record
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::SecretPayloadDiscarded)
    );
    assert!(!format!("{record:?}").contains("must-not-be-retained"));
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
        .observations()[0];
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
    assert!(!record.unknown_fields_complete());

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
        .observations();
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
    assert!(records.iter().any(|record| !record.unknown_fields_complete()));
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
        .observations();
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
        inventory.section(ResourceKind::Pod).expect("pods").observations()[0]
            .findings()
            .iter()
            .all(|finding| finding.code() != DiagnosticCode::PodMembershipConflict)
    );

    let mut responses = fixture_responses("6.1.0")?;
    responses[10] = json(r#"{"Id":"pod-1","Name":"pod-one","Containers":[{"Id":"container-z"}]}"#)?;
    let inventory = acquire_inventory(&RecordingTransport::new(responses), AcquisitionOptions::redacted()).await?;
    assert!(
        inventory.section(ResourceKind::Pod).expect("pods").observations()[0]
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::PodMembershipConflict)
    );
    Ok(())
}

#[tokio::test]
async fn public_debug_output_and_network_options_never_leak_protected_values() -> Result<(), Box<dyn std::error::Error>>
{
    use std::fmt::Write as _;

    let mut responses = fixture_responses("6.1.0")?;
    responses[8] = json(
        r#"{"Id":"container-a","Name":"a","Image":"SENTINEL_LOCAL_IMAGE","ImageName":"SENTINEL_CONFIGURED_IMAGE","Config":{"Labels":{"SENTINEL_LABEL_KEY":"SENTINEL_LABEL_VALUE"},"Env":["SENTINEL_ENV_NAME=SENTINEL_ENV_VALUE"]}}"#,
    )?;
    responses[11] = json(
        r#"{"id":"network-1","name":"app","labels":{"SENTINEL_NETWORK_LABEL":"SENTINEL_NETWORK_LABEL_VALUE"},"options":{"SENTINEL_OPTION_KEY":"SENTINEL_OPTION_VALUE"},"subnets":[{"subnet":"10.17.0.0/24"}]}"#,
    )?;
    responses[12] =
        json(r#"{"Name":"database-data","Labels":{"SENTINEL_VOLUME_LABEL":"SENTINEL_VOLUME_LABEL_VALUE"}}"#)?;
    responses[13] = json(
        r#"{"Id":"sha256:abc","Names":["SENTINEL_IMAGE_ALIAS"],"Labels":{"SENTINEL_IMAGE_LABEL":"SENTINEL_IMAGE_LABEL_VALUE"},"Config":{"Env":["SENTINEL_IMAGE_ENV=SENTINEL_IMAGE_ENV_VALUE"]}}"#,
    )?;
    responses[14] = json(
        r#"{"ID":"secret-1","Spec":{"Name":"db-password","Labels":{"SENTINEL_SECRET_LABEL":"SENTINEL_SECRET_LABEL_VALUE"},"Driver":"SENTINEL_SECRET_DRIVER"}}"#,
    )?;
    let inventory = acquire_inventory(
        &RecordingTransport::new(responses),
        AcquisitionOptions::include_environment_values(),
    )
    .await?;
    let mut rendered = format!("{inventory:?}");
    for section in inventory.sections() {
        for observation in section.observations() {
            write!(rendered, "{:?}", observation.details())?;
        }
    }
    let direct = ObservationField::Observed(podman_lens::ObservedValue::new(
        "SENTINEL_DIRECT_OBSERVED_VALUE",
        ObservationOrigin::Configured,
    ));
    write!(rendered, "{direct:?}")?;
    for secret in [
        "SENTINEL_LABEL_VALUE",
        "SENTINEL_NETWORK_LABEL_VALUE",
        "SENTINEL_VOLUME_LABEL_VALUE",
        "SENTINEL_IMAGE_LABEL_VALUE",
        "SENTINEL_SECRET_LABEL_VALUE",
        "SENTINEL_OPTION_VALUE",
        "SENTINEL_ENV_VALUE",
        "SENTINEL_IMAGE_ENV_VALUE",
        "SENTINEL_CONFIGURED_IMAGE",
        "SENTINEL_LOCAL_IMAGE",
        "SENTINEL_IMAGE_ALIAS",
        "SENTINEL_SECRET_DRIVER",
        "SENTINEL_DIRECT_OBSERVED_VALUE",
    ] {
        assert!(!rendered.contains(secret), "debug output leaked {secret}");
    }

    let ResourceDetails::Network(network) = inventory
        .section(ResourceKind::Network)
        .expect("networks")
        .observations()[0]
        .details()
    else {
        return Err("network fixture must have network details".into());
    };
    let options = network
        .options()
        .observed()
        .ok_or("network options must be observed")?
        .value();
    assert_eq!(options.keys().collect::<Vec<_>>(), ["SENTINEL_OPTION_KEY"]);
    assert_eq!(options.len(), 1);
    assert!(!format!("{options:?}").contains("SENTINEL_OPTION_KEY"));
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
