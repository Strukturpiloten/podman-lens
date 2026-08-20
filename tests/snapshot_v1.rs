//! Public redacted snapshot coverage using only offline fixture transports.

use std::{collections::VecDeque, fs, path::PathBuf, sync::Mutex};

use podman_lens::{
    AcquisitionOptions, DiscoveryRequest, LabelSelector, LibpodHeader, LibpodHeaders, LibpodRequest, LibpodResponse,
    LibpodTransport, LibpodTransportFuture, ResourceKind, ResourceSelector, TransportError, acquire_inventory,
    discover, snapshot::v1,
};
use serde_json::{Value, json};

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

fn fixture_responses() -> Result<Vec<LibpodResponse>, Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/inventory");
    let artifact = fs::read(root.join("podman-6.1.0.responses.json"))?;
    let fixture: Value = serde_json::from_slice(&artifact)?;
    fixture["responses"]
        .as_array()
        .ok_or("fixture response list must be an array")?
        .iter()
        .map(|response| {
            let status = response["status"]
                .as_u64()
                .ok_or("fixture status must be an integer")?
                .try_into()?;
            let headers = response["headers"]
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

async fn inventory() -> Result<podman_lens::ResourceInventory, Box<dyn std::error::Error>> {
    Ok(acquire_inventory(
        &FixtureTransport::new(fixture_responses()?),
        AcquisitionOptions::redacted(),
    )
    .await?)
}

fn golden_value(source: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(source)
}

fn snapshot_schema() -> Result<Value, serde_json::Error> {
    golden_value(include_str!("../docs/schemas/podman-lens-snapshot-v1.schema.json"))
}

#[tokio::test]
async fn inventory_snapshot_matches_the_versioned_golden_json() -> Result<(), Box<dyn std::error::Error>> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(&v1::inventory(&inventory().await?))?
    );
    assert_eq!(serialized, include_str!("../fixtures/snapshots/inventory-v1.json"));
    Ok(())
}

#[tokio::test]
async fn secret_driver_is_typed_not_unsupported_in_inventory_snapshots() -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = serde_json::to_value(v1::inventory(&inventory().await?))?;
    let secret = snapshot["sections"]
        .as_array()
        .ok_or("inventory snapshot sections must be an array")?
        .iter()
        .find(|section| section["kind"] == "secret")
        .and_then(|section| section["observations"].as_array())
        .and_then(|observations| observations.first())
        .ok_or("inventory snapshot must contain the secret observation")?;
    assert_eq!(secret["details"]["secret_driver"]["state"], "observed");
    assert_eq!(secret["details"]["secret_driver"]["options"]["count"], 1);
    assert_eq!(secret["details"]["secret_created_at"]["origin"], "effective");
    assert_eq!(secret["details"]["secret_updated_at"]["origin"], "effective");
    assert!(!snapshot.to_string().contains("SENTINEL_OPTION_NAME"));
    assert!(!snapshot.to_string().contains("SENTINEL_OPTION_VALUE"));
    assert_eq!(secret["header"]["unmodelled_fields"], json!([]));
    assert!(
        secret["header"]["findings"]
            .as_array()
            .ok_or("secret snapshot findings must be an array")?
            .iter()
            .all(|finding| finding["field_path"] != "$.Spec.Driver")
    );
    Ok(())
}

#[tokio::test]
async fn graph_snapshot_matches_the_versioned_golden_json() -> Result<(), Box<dyn std::error::Error>> {
    let mut request = DiscoveryRequest::new();
    request.add_root(ResourceSelector::exact(ResourceKind::Container, "a")?);
    request.add_label_root(LabelSelector::exact("project", "fixture-label")?);
    let graph = discover(&inventory().await?, &request)?;
    let serialized = format!("{}\n", serde_json::to_string_pretty(&v1::graph(&graph))?);
    assert_eq!(serialized, include_str!("../fixtures/snapshots/graph-v1.json"));
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)] // One strict schema mutation table keeps every versioned shape boundary visible.
fn draft_2020_12_schema_accepts_golden_snapshots_and_rejects_shape_violations() -> Result<(), Box<dyn std::error::Error>>
{
    let schema = snapshot_schema()?;
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .build(&schema)?;
    let inventory = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    let graph = golden_value(include_str!("../fixtures/snapshots/graph-v1.json"))?;
    assert!(
        validator.is_valid(&inventory),
        "inventory errors: {:?}",
        validator.iter_errors(&inventory).collect::<Vec<_>>()
    );
    assert!(validator.is_valid(&graph));

    let mut missing_required = graph.clone();
    missing_required
        .as_object_mut()
        .ok_or("graph snapshot must be an object")?
        .remove("schema_version");
    assert!(!validator.is_valid(&missing_required));

    let mut unexpected_property = inventory.clone();
    unexpected_property
        .as_object_mut()
        .ok_or("inventory snapshot must be an object")?
        .insert("unexpected".to_owned(), json!(true));
    assert!(!validator.is_valid(&unexpected_property));

    let mut wrong_type = graph;
    wrong_type
        .as_object_mut()
        .ok_or("graph snapshot must be an object")?
        .insert("all_requested".to_owned(), json!("not a boolean"));
    assert!(!validator.is_valid(&wrong_type));

    let mut missing_sections = inventory.clone();
    missing_sections["sections"] = json!([]);
    assert!(!validator.is_valid(&missing_sections));

    let mut reordered_sections = inventory.clone();
    reordered_sections["sections"]
        .as_array_mut()
        .ok_or("inventory sections must be an array")?
        .swap(0, 1);
    assert!(!validator.is_valid(&reordered_sections));

    let mut duplicate_section = inventory.clone();
    let sections = duplicate_section["sections"]
        .as_array_mut()
        .ok_or("inventory sections must be an array")?;
    sections[1] = sections[0].clone();
    assert!(!validator.is_valid(&duplicate_section));

    let mut unexpected_field_summary_property = inventory.clone();
    unexpected_field_summary_property["sections"][0]["observations"][0]["details"]["labels"]["unexpected"] =
        json!(true);
    assert!(!validator.is_valid(&unexpected_field_summary_property));

    let mut invalid_uri = inventory;
    invalid_uri["sections"][0]["observations"][0]["header"]["evidence"]["evidence_source"] = json!("not a URI");
    assert!(!validator.is_valid(&invalid_uri));

    let mut mismatched_detail_kind = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    mismatched_detail_kind["sections"][0]["observations"][0]["details"]["kind"] = json!("pod");
    assert!(!validator.is_valid(&mismatched_detail_kind));

    let mut irrelevant_typed_payload = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    irrelevant_typed_payload["sections"][0]["observations"][0]["details"]["image_repo_tags"] = json!({
        "state": "observed",
        "origin": "local_resolution",
        "count": 1
    });
    assert!(!validator.is_valid(&irrelevant_typed_payload));

    let mut container_field_on_pod = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    container_field_on_pod["sections"][1]["observations"][0]["details"]["command"] = json!({
        "state": "observed",
        "origin": "configured",
        "count": 1
    });
    assert!(!validator.is_valid(&container_field_on_pod));

    let mut missing_required_container_payload = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    missing_required_container_payload["sections"][0]["observations"][0]["details"]["environment"] = json!(null);
    assert!(!validator.is_valid(&missing_required_container_payload));

    let mut observed_without_origin = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    observed_without_origin["sections"][0]["observations"][0]["details"]["labels"]["origin"] = json!(null);
    assert!(!validator.is_valid(&observed_without_origin));

    let mut absent_with_origin = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    absent_with_origin["sections"][0]["observations"][1]["details"]["labels"]["origin"] = json!("configured");
    assert!(!validator.is_valid(&absent_with_origin));

    let mut absent_with_nonzero_count = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    absent_with_nonzero_count["sections"][0]["observations"][1]["details"]["labels"]["count"] = json!(1);
    assert!(!validator.is_valid(&absent_with_nonzero_count));

    let mut absent_new_collection_with_count = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    absent_new_collection_with_count["sections"][0]["observations"][1]["details"]["mounts"]["count"] = json!(1);
    assert!(!validator.is_valid(&absent_new_collection_with_count));

    let mut missing_network_routes = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    missing_network_routes["sections"][2]["observations"][0]["details"]["network"]
        .as_object_mut()
        .ok_or("network snapshot must be an object")?
        .remove("routes");
    assert!(!validator.is_valid(&missing_network_routes));

    let mut absent_with_environment_entries = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    absent_with_environment_entries["sections"][0]["observations"][0]["details"]["environment"]["state"] =
        json!("absent");
    absent_with_environment_entries["sections"][0]["observations"][0]["details"]["environment"]["origin"] = json!(null);
    assert!(!validator.is_valid(&absent_with_environment_entries));

    let mut absent_with_volume_wire_value = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    absent_with_volume_wire_value["sections"][3]["observations"][0]["details"]["volume_uid"]["state"] = json!("absent");
    absent_with_volume_wire_value["sections"][3]["observations"][0]["details"]["volume_uid"]["origin"] = json!(null);
    assert!(!validator.is_valid(&absent_with_volume_wire_value));

    let mut missing_resource_controls = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    missing_resource_controls["sections"][0]["observations"][0]["details"]
        .as_object_mut()
        .ok_or("container details must be an object")?
        .remove("resource_controls");
    assert!(!validator.is_valid(&missing_resource_controls));

    let mut security_option_values = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    security_option_values["sections"][0]["observations"][0]["details"]["security"]["security_options"]["values"] =
        json!(["must-not-exist"]);
    assert!(!validator.is_valid(&security_option_values));

    let mut security_on_pod = golden_value(include_str!("../fixtures/snapshots/inventory-v1.json"))?;
    security_on_pod["sections"][1]["observations"][0]["details"]["security"] = json!({
        "privileged":{"state":"absent","origin":null},
        "cap_add":{"state":"absent","origin":null,"count":0},
        "cap_drop":{"state":"absent","origin":null,"count":0},
        "security_options":{"state":"absent","origin":null,"count":0},
        "read_only_root_filesystem":{"state":"absent","origin":null}
    });
    assert!(!validator.is_valid(&security_on_pod));
    Ok(())
}

#[tokio::test]
async fn snapshots_never_leak_sensitive_or_redacted_values() -> Result<(), Box<dyn std::error::Error>> {
    let mut responses = fixture_responses()?;
    let replacements = [
        (b"A=one".as_slice(), b"A=DISTINCTIVE_ENV_VALUE".as_slice()),
        (
            b"\"project\":\"fixture-label\"".as_slice(),
            b"\"project\":\"DISTINCTIVE_LABEL_VALUE\",\"com.docker.compose.project\":\"DISTINCTIVE_COMPOSE_PROJECT\",\"com.docker.compose.service\":\"web\",\"io.podman.compose.project\":\"DISTINCTIVE_COMPOSE_PROJECT\",\"io.podman.compose.service\":\"web\"".as_slice(),
        ),
        (b"fixture-option".as_slice(), b"DISTINCTIVE_DRIVER_OPTION_VALUE".as_slice()),
        (b"discarded".as_slice(), b"DISTINCTIVE_SECRET_PAYLOAD".as_slice()),
        (
            b"\"ImageName\":\"sha256:abc\"".as_slice(),
            b"\"ImageName\":\"DISTINCTIVE_IMAGE\",\"Pod\":\"DISTINCTIVE_POD\"".as_slice(),
        ),
        (
            b"\"Dependencies\":[\"container-z\"]".as_slice(),
            b"\"Dependencies\":[\"container-z\",\"DISTINCTIVE_DEPENDENCY\"]".as_slice(),
        ),
        (
            b"\"Labels\":{\"project\":\"fixture-label\"}".as_slice(),
            b"\"Cmd\":[\"DISTINCTIVE_CMD\"],\"Entrypoint\":[\"DISTINCTIVE_ENTRYPOINT\"],\"User\":\"DISTINCTIVE_USER\",\"WorkingDir\":\"DISTINCTIVE_WORKDIR\",\"Hostname\":\"DISTINCTIVE_HOSTNAME\",\"Labels\":{\"project\":\"fixture-label\"}".as_slice(),
        ),
        (
            b"\"Mounts\":[{\"Type\":\"volume\",\"Name\":\"database-data\"}]".as_slice(),
            b"\"Mounts\":[{\"Type\":\"volume\",\"Name\":\"DISTINCTIVE_MOUNT_NAME\",\"Source\":\"DISTINCTIVE_BACKING_PATH\",\"Destination\":\"DISTINCTIVE_DESTINATION\",\"RW\":true,\"Options\":[\"DISTINCTIVE_OPTION\"],\"Propagation\":\"DISTINCTIVE_PROPAGATION\",\"SubPath\":\"DISTINCTIVE_SUBPATH\"},{\"Type\":\"bind\",\"Source\":\"DISTINCTIVE_BIND_PATH\",\"Destination\":\"DISTINCTIVE_BIND_DESTINATION\"}]".as_slice(),
        ),
        (
            b"\"Secrets\":[{\"ID\":\"secret-1\",\"Name\":\"db-password\"}]".as_slice(),
            b"\"Secrets\":[{\"ID\":\"DISTINCTIVE_SECRET_ID\",\"Name\":\"DISTINCTIVE_SECRET_NAME\",\"UID\":1000,\"GID\":1001,\"Mode\":288}]".as_slice(),
        ),
        (
            b"\"subnets\":[{\"subnet\":\"10.88.0.0/16\"}]".as_slice(),
            b"\"subnets\":[{\"subnet\":\"198.18.0.0/24\",\"gateway\":\"198.18.0.1\",\"lease_range\":{\"start_ip\":\"198.18.0.10\",\"end_ip\":\"198.18.0.20\"}}],\"routes\":[{\"destination\":\"198.19.0.0/24\",\"gateway\":\"198.18.0.1\",\"metric\":0,\"route_type\":\"unicast\"}]".as_slice(),
        ),
    ];
    for response in &mut responses {
        let mut body = String::from_utf8(response.body().to_vec())?;
        for (from, to) in replacements {
            body = body.replace(std::str::from_utf8(from)?, std::str::from_utf8(to)?);
        }
        *response = LibpodResponse::new(response.status(), response.headers().clone(), body.into_bytes())?;
    }
    let inventory = acquire_inventory(
        &FixtureTransport::new(responses),
        AcquisitionOptions::include_environment_values(),
    )
    .await?;
    let mut request = DiscoveryRequest::new();
    request.add_root(ResourceSelector::exact(ResourceKind::Container, "a")?);
    request.add_label_root(LabelSelector::exact("project", "DISTINCTIVE_LABEL_VALUE")?);
    let graph = discover(&inventory, &request)?;
    let serializations = [
        serde_json::to_string(&v1::inventory(&inventory))?,
        serde_json::to_string(&v1::graph(&graph))?,
    ];
    for serialized in serializations {
        for forbidden in [
            "DISTINCTIVE_ENV_VALUE",
            "DISTINCTIVE_SECRET_PAYLOAD",
            "DISTINCTIVE_LABEL_VALUE",
            "DISTINCTIVE_COMPOSE_PROJECT",
            "DISTINCTIVE_DRIVER_OPTION_VALUE",
            "DISTINCTIVE_IMAGE",
            "DISTINCTIVE_POD",
            "DISTINCTIVE_DEPENDENCY",
            "DISTINCTIVE_CMD",
            "DISTINCTIVE_ENTRYPOINT",
            "DISTINCTIVE_USER",
            "DISTINCTIVE_WORKDIR",
            "DISTINCTIVE_HOSTNAME",
            "DISTINCTIVE_MOUNT_NAME",
            "DISTINCTIVE_BACKING_PATH",
            "DISTINCTIVE_DESTINATION",
            "DISTINCTIVE_OPTION",
            "DISTINCTIVE_PROPAGATION",
            "DISTINCTIVE_SUBPATH",
            "DISTINCTIVE_BIND_PATH",
            "DISTINCTIVE_BIND_DESTINATION",
            "DISTINCTIVE_SECRET_ID",
            "DISTINCTIVE_SECRET_NAME",
            "198.18.0.1",
            "198.18.0.10",
            "198.18.0.20",
            "198.19.0.0/24",
            "com.docker.compose.project",
            "io.podman.compose.project",
        ] {
            assert!(!serialized.contains(forbidden), "snapshot leaked {forbidden}");
        }
    }
    Ok(())
}
