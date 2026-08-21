//! Focused contract coverage for request-aware offline Libpod cassettes.

#![allow(clippy::expect_used)]

mod support;

use podman_lens::{LibpodMethod, LibpodPath, LibpodRequest, LibpodTransport};
use serde_json::{Value, json};
use support::cassette::{Cassette, CassetteError, CassetteReplayError, CassetteTransport, ExecutionContext};

fn cassette_value() -> Value {
    json!({
        "schema_version": 1,
        "fixture_kind": "libpod-cassette",
        "scenario_id": "contract-replay",
        "scenario_revision": 1,
        "engine_version": "6.1.0",
        "api_version": "6.1.0",
        "execution_context": "rootless",
        "synthetic": true,
        "provenance": {
            "evidence_kind": "source-derived-synthetic-sanitized",
            "release_tag": "v6.1.0",
            "revision": "cade97a52ebdf9dbf9e81de8009015776837a074",
            "source_urls": [
                "https://github.com/containers/podman/tree/cade97a52ebdf9dbf9e81de8009015776837a074"
            ]
        },
        "sanitization": "Synthetic identifiers and values only; no endpoint, credential, or secret material.",
        "interactions": [
            {
                "request": { "method": "GET", "path": "/libpod/_ping" },
                "response": {
                    "status": 200,
                    "headers": [
                        ["set-cookie", "first=synthetic"],
                        ["Set-Cookie", "second=synthetic"],
                        ["libpod-api-version", "6.1.0"]
                    ],
                    "body": null
                }
            },
            {
                "request": { "method": "GET", "path": "/v4.0.0/libpod/version" },
                "response": {
                    "status": 200,
                    "headers": [["content-type", "application/json"]],
                    "body": {
                        "Components": [],
                        "private": "DISTINCTIVE_RESPONSE_BODY"
                    }
                }
            }
        ]
    })
}

fn cassette(value: &Value) -> Result<Cassette, CassetteError> {
    Cassette::from_slice(&serde_json::to_vec(value).expect("test value serializes"))
}

fn get(path: &str) -> Result<LibpodRequest, Box<dyn std::error::Error>> {
    Ok(LibpodRequest::new(
        LibpodMethod::Get,
        LibpodPath::parse(path)?,
        Vec::new(),
    )?)
}

#[tokio::test]
async fn valid_cassette_parses_and_replays_exact_requests_in_order() -> Result<(), Box<dyn std::error::Error>> {
    let cassette = cassette(&cassette_value())?;
    assert_eq!(cassette.schema_version(), 1);
    assert_eq!(cassette.fixture_kind(), "libpod-cassette");
    assert_eq!(cassette.scenario_id(), "contract-replay");
    assert_eq!(cassette.scenario_revision(), 1);
    assert_eq!(cassette.engine_version(), "6.1.0");
    assert_eq!(cassette.api_version(), "6.1.0");
    assert_eq!(cassette.execution_context(), ExecutionContext::Rootless);
    assert!(cassette.synthetic());
    assert_eq!(
        cassette.provenance().evidence_kind(),
        "source-derived-synthetic-sanitized"
    );
    assert_eq!(cassette.provenance().release_tag(), "v6.1.0");
    assert_eq!(
        cassette.provenance().revision(),
        "cade97a52ebdf9dbf9e81de8009015776837a074"
    );
    assert_eq!(cassette.provenance().source_urls().len(), 1);
    assert!(cassette.sanitization().contains("Synthetic"));
    assert_eq!(cassette.interaction_count(), 2);

    let transport = CassetteTransport::try_new(cassette)?;
    let ping_request = get("/libpod/_ping")?;
    let ping = transport.send(&ping_request).await?;
    assert_eq!(ping.status(), 200);
    assert!(ping.body().is_empty());
    assert_eq!(
        ping.headers().values("set-cookie").collect::<Vec<_>>(),
        ["first=synthetic", "second=synthetic"]
    );
    let version_request = get("/v4.0.0/libpod/version")?;
    let version = transport.send(&version_request).await?;
    assert_eq!(version.status(), 200);
    assert_eq!(
        serde_json::from_slice::<Value>(version.body())?["Components"],
        json!([])
    );
    transport.assert_consumed()?;
    Ok(())
}

#[tokio::test]
async fn wrong_or_reordered_request_is_rejected_without_exposing_response_body()
-> Result<(), Box<dyn std::error::Error>> {
    let transport = CassetteTransport::try_new(cassette(&cassette_value())?)?;
    let request = get("/v4.0.0/libpod/version")?;
    assert!(transport.send(&request).await.is_err());
    let failure = transport.assert_consumed().expect_err("request order must be exact");
    assert!(matches!(
        &failure,
        CassetteReplayError::RequestMismatch { expected_path, actual_path, .. }
            if expected_path == "/libpod/_ping" && actual_path == "/v4.0.0/libpod/version"
    ));
    let report = format!("{failure:?} {failure}");
    assert!(!report.contains("DISTINCTIVE_RESPONSE_BODY"));
    Ok(())
}

#[tokio::test]
async fn repeated_request_is_rejected_after_its_single_interaction_is_consumed()
-> Result<(), Box<dyn std::error::Error>> {
    let transport = CassetteTransport::try_new(cassette(&cassette_value())?)?;
    let request = get("/libpod/_ping")?;
    transport.send(&request).await?;
    assert!(transport.send(&request).await.is_err());
    assert!(matches!(
        transport.assert_consumed(),
        Err(CassetteReplayError::RequestMismatch { expected_path, actual_path, .. })
            if expected_path == "/v4.0.0/libpod/version" && actual_path == "/libpod/_ping"
    ));
    Ok(())
}

#[tokio::test]
async fn query_string_and_parameter_order_are_part_of_exact_path_matching() -> Result<(), Box<dyn std::error::Error>> {
    let mut value = cassette_value();
    value["interactions"][0]["request"]["path"] = json!("/v6.1.0/libpod/containers/json?all=true&sync=true");
    let transport = CassetteTransport::try_new(cassette(&value)?)?;
    let request = get("/v6.1.0/libpod/containers/json?sync=true&all=true")?;
    assert!(transport.send(&request).await.is_err());
    assert!(matches!(
        transport.assert_consumed(),
        Err(CassetteReplayError::RequestMismatch { expected_path, actual_path, .. })
            if expected_path == "/v6.1.0/libpod/containers/json?all=true&sync=true"
                && actual_path == "/v6.1.0/libpod/containers/json?sync=true&all=true"
    ));
    Ok(())
}

#[tokio::test]
async fn missing_expected_interaction_rejects_the_extra_request() -> Result<(), Box<dyn std::error::Error>> {
    let mut cassette = cassette(&cassette_value())?;
    cassette.remove_unique_interaction(LibpodMethod::Get, "/v4.0.0/libpod/version")?;
    let transport = CassetteTransport::try_new(cassette)?;
    let ping_request = get("/libpod/_ping")?;
    transport.send(&ping_request).await?;
    let version_request = get("/v4.0.0/libpod/version")?;
    assert!(transport.send(&version_request).await.is_err());
    assert!(matches!(
        transport.assert_consumed(),
        Err(CassetteReplayError::UnexpectedRequest { actual_path, .. })
            if actual_path == "/v4.0.0/libpod/version"
    ));
    Ok(())
}

#[tokio::test]
async fn unconsumed_extra_interaction_is_reported() -> Result<(), Box<dyn std::error::Error>> {
    let transport = CassetteTransport::try_new(cassette(&cassette_value())?)?;
    let request = get("/libpod/_ping")?;
    transport.send(&request).await?;
    assert!(matches!(
        transport.assert_consumed(),
        Err(CassetteReplayError::Unconsumed { remaining: 1, next_path, .. })
            if next_path == "/v4.0.0/libpod/version"
    ));
    Ok(())
}

#[tokio::test]
async fn unique_interaction_mutation_targets_method_and_path_not_an_offset() -> Result<(), Box<dyn std::error::Error>> {
    let mut cassette = cassette(&cassette_value())?;
    let response = cassette
        .unique_interaction_mut(LibpodMethod::Get, "/v4.0.0/libpod/version")?
        .response_mut();
    response.set_status(503);
    response.set_body(Value::Null);
    let transport = CassetteTransport::try_new(cassette)?;
    let ping_request = get("/libpod/_ping")?;
    transport.send(&ping_request).await?;
    let version_request = get("/v4.0.0/libpod/version")?;
    let version = transport.send(&version_request).await?;
    assert_eq!(version.status(), 503);
    assert!(version.body().is_empty());
    transport.assert_consumed()?;
    Ok(())
}

#[test]
fn malformed_or_non_strict_cassettes_are_rejected_by_the_schema() {
    let mut cases = Vec::new();

    let mut extra_property = cassette_value();
    extra_property["unexpected"] = json!(true);
    cases.push(extra_property);

    let mut wrong_method = cassette_value();
    wrong_method["interactions"][0]["request"]["method"] = json!("POST");
    cases.push(wrong_method);

    let mut malformed_header = cassette_value();
    malformed_header["interactions"][0]["response"]["headers"] = json!([["only-one-value"]]);
    cases.push(malformed_header);

    let mut missing_context = cassette_value();
    missing_context
        .as_object_mut()
        .expect("object fixture")
        .remove("execution_context");
    cases.push(missing_context);

    for case in cases {
        assert!(matches!(cassette(&case), Err(CassetteError::SchemaViolation)));
    }
}

#[test]
fn duplicate_request_keys_are_rejected_for_mutation_and_transport_construction()
-> Result<(), Box<dyn std::error::Error>> {
    let mut value = cassette_value();
    let duplicate = value["interactions"][0].clone();
    value["interactions"]
        .as_array_mut()
        .expect("interactions array")
        .push(duplicate);
    let mut cassette = cassette(&value)?;
    assert!(matches!(
        cassette.unique_interaction_mut(LibpodMethod::Get, "/libpod/_ping"),
        Err(CassetteError::InteractionAmbiguous { .. })
    ));
    assert!(matches!(
        CassetteTransport::try_new(cassette),
        Err(CassetteError::InteractionAmbiguous { method, path })
            if method == "GET" && path == "/libpod/_ping"
    ));
    Ok(())
}
