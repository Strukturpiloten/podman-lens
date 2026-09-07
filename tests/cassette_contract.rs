//! Focused contract coverage for request-aware offline Libpod cassettes.

#![allow(clippy::expect_used, clippy::too_many_lines)]

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
    assert!(cassette.provenance().capture().is_none());
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

#[test]
fn captured_cassettes_require_complete_bounded_capture_provenance() -> Result<(), Box<dyn std::error::Error>> {
    let capture = json!({
        "engine_build": {
            "kind": "source-build",
            "source_revision": "cade97a52ebdf9dbf9e81de8009015776837a074"
        },
        "runtime_image": "ghcr.io/strukturpiloten/podman-6.1-rootful:v6.1.0@sha256:2cf1d0fa3d0776e6a87bc4fd3391abd227001ab22885c5897c04970f3f38cb2c",
        "capture_manifest_sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "runtime_repository": "https://github.com/Strukturpiloten/boxferry",
        "runtime_revision": "52b7600afe936b3c688294bd9a7f43e1afa9bf1f",
        "runtime_matrix_cell": "podman-6.1-rootful",
        "runtime_matrix_sha256": "1ed306f4b368c229bca927697156e2314b922c2ec728c55c2820c69a712bad25",
        "setup_script_sha256": "0790eb18744c8683a538c1917fc5a550b70e8a5dfb1f2448110101422dfae977",
        "request_recorder_source_sha256": "26a1b238826315326057e0d555c385ea5a4617568c8cd4154a5faa9d0ff7fb91",
        "compose_provider_binary_sha256": "abdf6ec2a49e5cb1f021f142d55a7494c823b11fa6f950237301a1cb70b5e5c6"
    });
    let mut value = cassette_value();
    value["synthetic"] = json!(false);
    value["provenance"]["capture"] = capture.clone();
    value["interactions"][0]["response"]["headers"] = json!([["x-reference-id", "fixture-reference-contract-01"]]);

    let parsed = cassette(&value)?;
    let observed = parsed.provenance().capture().ok_or("captured provenance")?;
    assert_eq!(
        observed.engine_build().source_revision(),
        Some("cade97a52ebdf9dbf9e81de8009015776837a074")
    );
    assert!(observed.runtime_image().contains("@sha256:"));
    assert_eq!(
        observed.capture_manifest_sha256(),
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
    );
    assert_eq!(
        observed.runtime_repository(),
        "https://github.com/Strukturpiloten/boxferry"
    );
    assert_eq!(observed.runtime_revision(), "52b7600afe936b3c688294bd9a7f43e1afa9bf1f");
    assert_eq!(observed.runtime_matrix_cell(), "podman-6.1-rootful");
    assert_eq!(
        observed.runtime_matrix_sha256(),
        "1ed306f4b368c229bca927697156e2314b922c2ec728c55c2820c69a712bad25"
    );
    assert_eq!(
        observed.setup_script_sha256(),
        "0790eb18744c8683a538c1917fc5a550b70e8a5dfb1f2448110101422dfae977"
    );
    assert_eq!(
        observed.request_recorder_source_sha256(),
        "26a1b238826315326057e0d555c385ea5a4617568c8cd4154a5faa9d0ff7fb91"
    );
    assert_eq!(
        observed.compose_provider_binary_sha256(),
        Some("abdf6ec2a49e5cb1f021f142d55a7494c823b11fa6f950237301a1cb70b5e5c6")
    );

    let mut without_compose = value.clone();
    without_compose["provenance"]["capture"]
        .as_object_mut()
        .expect("capture provenance object")
        .remove("compose_provider_binary_sha256");
    assert!(cassette(&without_compose).is_ok());

    let mut malformed_compose = value.clone();
    malformed_compose["provenance"]["capture"]["compose_provider_binary_sha256"] = json!("not-a-digest");
    assert!(matches!(
        cassette(&malformed_compose),
        Err(CassetteError::SchemaViolation)
    ));

    let mut missing_capture = cassette_value();
    missing_capture["synthetic"] = json!(false);
    assert!(matches!(
        cassette(&missing_capture),
        Err(CassetteError::SchemaViolation)
    ));

    let mut contradictory = cassette_value();
    contradictory["provenance"]["capture"] = capture.clone();
    assert!(matches!(cassette(&contradictory), Err(CassetteError::SchemaViolation)));

    for field in [
        "engine_build",
        "runtime_image",
        "capture_manifest_sha256",
        "runtime_repository",
        "runtime_revision",
        "runtime_matrix_cell",
        "runtime_matrix_sha256",
        "setup_script_sha256",
        "request_recorder_source_sha256",
    ] {
        let mut incomplete = value.clone();
        incomplete["provenance"]["capture"]
            .as_object_mut()
            .expect("capture provenance object")
            .remove(field);
        assert!(matches!(cassette(&incomplete), Err(CassetteError::SchemaViolation)));
    }

    for invalid in [
        json!("ghcr.io/strukturpiloten/podman-6.1-rootful:v6.1.0"),
        json!("ghcr.io/strukturpiloten/podman@sha256:not-a-digest"),
    ] {
        let mut malformed = value.clone();
        malformed["provenance"]["capture"]["runtime_image"] = invalid;
        assert!(matches!(cassette(&malformed), Err(CassetteError::SchemaViolation)));
    }

    let mut malformed_build = value.clone();
    malformed_build["provenance"]["capture"]["engine_build"]["source_revision"] = json!("v6.1.0");
    assert!(matches!(
        cassette(&malformed_build),
        Err(CassetteError::SchemaViolation)
    ));

    let mut exact_package = value;
    exact_package["provenance"]["capture"]["engine_build"] = json!({
        "kind": "package",
        "package_name": "podman",
        "package_revision": "6.1.0-1.x86_64",
        "package_artifact_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    });
    assert!(cassette(&exact_package).is_ok());
    exact_package["provenance"]["capture"]["engine_build"]
        .as_object_mut()
        .expect("package provenance object")
        .remove("package_artifact_sha256");
    assert!(matches!(cassette(&exact_package), Err(CassetteError::SchemaViolation)));
    Ok(())
}

#[tokio::test]
async fn raw_text_response_replays_exact_bytes_and_is_mutually_exclusive_with_json_body()
-> Result<(), Box<dyn std::error::Error>> {
    let mut value = cassette_value();
    let response = &mut value["interactions"][0]["response"];
    response.as_object_mut().expect("response object").remove("body");
    response["body_text"] = json!("OK");

    let transport = CassetteTransport::try_new(cassette(&value)?)?;
    let replayed = transport.send(&get("/libpod/_ping")?).await?;
    assert_eq!(replayed.body(), b"OK");

    let mut both = value.clone();
    both["interactions"][0]["response"]["body"] = Value::Null;
    assert!(matches!(cassette(&both), Err(CassetteError::SchemaViolation)));

    let mut neither = value;
    neither["interactions"][0]["response"]
        .as_object_mut()
        .expect("response object")
        .remove("body_text");
    assert!(matches!(cassette(&neither), Err(CassetteError::SchemaViolation)));
    Ok(())
}

#[test]
fn captured_privacy_admission_rejects_raw_private_mutations() -> Result<(), Box<dyn std::error::Error>> {
    let captured: Value = serde_json::from_slice(include_bytes!(
        "../fixtures/native-regressions/captured-mixed-origin-6.1.0-rootful.cassette.json"
    ))?;
    let cases = [
        (vec!["body", "Platform", "Name"], json!("/home/example/private")),
        (vec!["body", "Platform", "Name"], json!("PASSWORD=raw-secret")),
        (vec!["body", "Platform", "Authorization"], json!("Bearer raw-token")),
        (vec!["body", "Platform", "Name"], json!("10.0.0.4")),
        (vec!["body", "Platform", "Name"], json!("aa:bb:cc:dd:ee:ff")),
        (vec!["body", "Platform", "Hostname"], json!("6fea848a36ae")),
    ];
    for (path, mutation) in cases {
        let mut value = captured.clone();
        let mut target = &mut value["interactions"][1]["response"];
        for segment in path {
            target = &mut target[segment];
        }
        *target = mutation;
        assert!(matches!(
            Cassette::from_slice(&serde_json::to_vec(&value)?),
            Err(CassetteError::PrivacyViolation)
        ));
    }

    let mut raw_reference = captured;
    raw_reference["interactions"][0]["response"]["headers"][0] =
        json!(["X-Reference-Id", "8d25216c-44c1-4fcb-bb0b-19d0acb88f03"]);
    assert!(matches!(
        Cassette::from_slice(&serde_json::to_vec(&raw_reference)?),
        Err(CassetteError::PrivacyViolation)
    ));
    Ok(())
}

#[test]
fn captured_privacy_admission_covers_paths_dates_timestamps_and_reference_shape()
-> Result<(), Box<dyn std::error::Error>> {
    let captured: Value = serde_json::from_slice(include_bytes!(
        "../fixtures/native-regressions/captured-mixed-origin-6.1.0-rootful.cassette.json"
    ))?;
    let assert_rejected = |value: &Value| {
        assert!(matches!(
            Cassette::from_slice(&serde_json::to_vec(value).expect("serialize privacy mutation")),
            Err(CassetteError::PrivacyViolation)
        ));
    };

    for path in [
        "/var/lib/containers/storage",
        "/run/containers/storage",
        "/capture-input/native.json",
        "/capture-socket/podman.sock",
    ] {
        let mut value = captured.clone();
        value["interactions"][0]["request"]["path"] = json!(path);
        assert_rejected(&value);
    }

    let mut hostname = captured.clone();
    hostname["interactions"][1]["response"]["body"]["Platform"]["Hostname"] = json!("abcdef");
    assert_rejected(&hostname);

    let mut timestamp = captured.clone();
    timestamp["interactions"][1]["response"]["body"]["Components"][0]["Details"]["BuildTime"] =
        json!(1_725_000_000_u64);
    assert_rejected(&timestamp);

    let mut raw_date = captured.clone();
    let headers = raw_date["interactions"][0]["response"]["headers"]
        .as_array_mut()
        .expect("headers");
    headers
        .iter_mut()
        .find(|header| header[0] == "Date")
        .expect("Date header")[1] = json!("Mon, 07 Sep 2026 12:34:56 GMT");
    assert_rejected(&raw_date);

    let mut almost_reference = captured;
    let headers = almost_reference["interactions"][0]["response"]["headers"]
        .as_array_mut()
        .expect("headers");
    headers
        .iter_mut()
        .find(|header| header[0] == "X-Reference-Id")
        .expect("reference header")[1] = json!("fixture-reference-mixed-origin-01-extra");
    assert_rejected(&almost_reference);
    Ok(())
}
