//! Offline and negative coverage for the fixed Libpod service probe.

use std::{
    collections::{BTreeSet, VecDeque},
    fs,
    path::PathBuf,
    sync::Mutex,
};

use podman_lens::{
    DiagnosticCode, LibpodHeader, LibpodHeaders, LibpodMethod, LibpodPath, LibpodRequest, LibpodResponse,
    LibpodTransport, LibpodTransportFuture, TransportError, probe_libpod_service,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

struct RecordingTransport {
    responses: Mutex<VecDeque<LibpodResponse>>,
    requests: Mutex<Vec<(LibpodMethod, String)>>,
}

impl RecordingTransport {
    fn new(responses: impl Into<Vec<LibpodResponse>>) -> Self {
        Self {
            responses: Mutex::new(responses.into().into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Result<Vec<(LibpodMethod, String)>, TransportError> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .map_err(|_| TransportError::unavailable())
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

#[derive(Deserialize)]
struct FixtureManifest {
    schema_version: u64,
    evidence_kind: String,
    sanitization: String,
    requests: Vec<ManifestRequest>,
    fixtures: Vec<ManifestFixture>,
}

#[derive(Deserialize)]
struct ManifestRequest {
    method: String,
    path: String,
    status: u16,
    content_type: Option<String>,
}

#[derive(Deserialize)]
struct ManifestFixture {
    engine_version: String,
    release_tag: String,
    minimum_api_version: String,
    current_api_version: String,
    commit: String,
    source_urls: Vec<String>,
    artifact: String,
    sha256: String,
    ping_artifact: String,
    ping_sha256: String,
}

fn response(
    status: u16,
    headers: impl Into<Vec<LibpodHeader>>,
    body: impl Into<Vec<u8>>,
) -> Result<LibpodResponse, Box<dyn std::error::Error>> {
    Ok(LibpodResponse::new(status, LibpodHeaders::new(headers), body)?)
}

fn ping(api_version: &str) -> Result<LibpodResponse, Box<dyn std::error::Error>> {
    response(
        200,
        vec![LibpodHeader::new("Libpod-API-Version", api_version)?],
        Vec::new(),
    )
}

fn version(body: impl Into<Vec<u8>>) -> Result<LibpodResponse, Box<dyn std::error::Error>> {
    response(
        200,
        vec![LibpodHeader::new("Content-Type", "application/json; charset=utf-8")?],
        body,
    )
}

fn assert_fixed_request_sequence(transport: &RecordingTransport) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        transport.requests()?,
        vec![
            (LibpodMethod::Get, "/libpod/_ping".to_owned()),
            (LibpodMethod::Get, "/v4.0.0/libpod/version".to_owned()),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn every_pinned_fixture_decodes_with_the_fixed_two_get_requests() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/api-version");
    let manifest: FixtureManifest = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.evidence_kind, "source-derived-minimal");
    assert!(!manifest.sanitization.is_empty());
    assert_eq!(manifest.requests.len(), 2);
    let expected_requests = [
        ("GET", "/libpod/_ping", 200, None),
        ("GET", "/v4.0.0/libpod/version", 200, Some("application/json")),
    ];
    for (actual, expected) in manifest.requests.iter().zip(expected_requests) {
        assert_eq!(actual.method, expected.0);
        assert_eq!(actual.path, expected.1);
        assert_eq!(actual.status, expected.2);
        assert_eq!(actual.content_type.as_deref(), expected.3);
    }

    let expected_versions = BTreeSet::from(["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"]);
    assert_eq!(manifest.fixtures.len(), expected_versions.len());
    assert_eq!(
        manifest
            .fixtures
            .iter()
            .map(|fixture| fixture.engine_version.as_str())
            .collect::<BTreeSet<_>>(),
        expected_versions
    );
    let expected_artifacts = manifest
        .fixtures
        .iter()
        .flat_map(|fixture| [fixture.artifact.clone(), fixture.ping_artifact.clone()])
        .collect::<BTreeSet<_>>();
    let actual_artifacts = fs::read_dir(&root)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<BTreeSet<_>, _>>()?;
    assert_eq!(
        actual_artifacts,
        expected_artifacts
            .iter()
            .cloned()
            .chain(std::iter::once("manifest.json".to_owned()))
            .collect()
    );
    for fixture in manifest.fixtures {
        assert!(is_lowercase_hex(&fixture.commit, 40));
        assert_eq!(fixture.release_tag, format!("v{}", fixture.engine_version));
        assert_eq!(fixture.minimum_api_version, "4.0.0");
        assert_eq!(fixture.current_api_version, fixture.engine_version);
        let expected_source_urls = [
            "pkg/api/server/register_ping.go",
            "pkg/api/server/handler_api.go",
            "pkg/api/handlers/compat/version.go",
            "pkg/domain/entities/types/types.go",
            "version/version.go",
        ]
        .map(|path| format!("https://github.com/containers/podman/blob/{}/{path}", fixture.commit));
        assert_eq!(fixture.source_urls, expected_source_urls);
        assert!(
            fixture
                .artifact
                .starts_with(&format!("podman-{}", fixture.engine_version))
        );
        assert!(is_lowercase_hex(&fixture.sha256, 64));
        assert!(is_lowercase_hex(&fixture.ping_sha256, 64));
        let bytes = fs::read(root.join(&fixture.artifact))?;
        assert_eq!(hex_digest(&Sha256::digest(&bytes)), fixture.sha256);
        let ping_bytes = fs::read(root.join(&fixture.ping_artifact))?;
        assert_eq!(hex_digest(&Sha256::digest(&ping_bytes)), fixture.ping_sha256);
        assert_eq!(
            ping_bytes,
            format!("Libpod-API-Version: {}\n", fixture.current_api_version).as_bytes()
        );

        let transport = RecordingTransport::new(vec![ping(&fixture.current_api_version)?, version(bytes)?]);
        let observation = probe_libpod_service(&transport).await?;
        assert_eq!(observation.engine_version().original(), fixture.engine_version);
        assert_eq!(observation.api_version().original(), fixture.current_api_version);
        assert_fixed_request_sequence(&transport)?;
    }
    Ok(())
}

#[tokio::test]
async fn reviewed_rhel_vendor_spelling_reaches_the_ubi_8_input_anchor() -> Result<(), Box<dyn std::error::Error>> {
    let transport = RecordingTransport::new(vec![
        ping("4.9.4-rhel")?,
        version(br#"{"Components":[{"Name":"Podman Engine","Version":"4.9.4-rhel"}]}"#)?,
    ]);

    let observation = probe_libpod_service(&transport).await?;

    assert_eq!(observation.engine_version().original(), "4.9.4-rhel");
    assert_eq!(observation.engine_version().as_semver().to_string(), "4.9.4");
    assert_eq!(observation.api_version().original(), "4.9.4-rhel");
    assert_eq!(observation.api_version().as_semver().to_string(), "4.9.4");
    assert!(!observation.input_capability().output_supported());
    assert!(observation.output_target_profile().is_none());
    assert_fixed_request_sequence(&transport)?;
    Ok(())
}

#[tokio::test]
async fn unreviewed_vendor_prerelease_spelling_remains_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let transport = RecordingTransport::new(vec![ping("4.9.4-vendor")?]);

    let error = probe_libpod_service(&transport)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("unreviewed vendor spelling unexpectedly passed"))?;

    assert_eq!(error.code(), DiagnosticCode::ProbeHeader);
    assert_eq!(transport.requests()?.len(), 1);
    Ok(())
}

#[tokio::test]
async fn unreviewed_build_metadata_cannot_select_a_legacy_anchor() -> Result<(), Box<dyn std::error::Error>> {
    let transport = RecordingTransport::new(vec![
        ping("4.9.4")?,
        version(br#"{"Components":[{"Name":"Podman Engine","Version":"4.9.4+vendor"}]}"#)?,
    ]);

    let error = probe_libpod_service(&transport)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("unreviewed build metadata unexpectedly passed"))?;

    assert_eq!(error.code(), DiagnosticCode::ObservedCompatibility);
    assert_fixed_request_sequence(&transport)?;
    Ok(())
}

#[tokio::test]
async fn reviewed_rhel_spelling_requires_the_matching_rhel_api_version() -> Result<(), Box<dyn std::error::Error>> {
    let transport = RecordingTransport::new(vec![
        ping("4.9.4")?,
        version(br#"{"Components":[{"Name":"Podman Engine","Version":"4.9.4-rhel"}]}"#)?,
    ]);

    let error = probe_libpod_service(&transport)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("mixed RHEL/canonical version pair unexpectedly passed"))?;

    assert_eq!(error.code(), DiagnosticCode::ObservedCompatibility);
    assert_fixed_request_sequence(&transport)?;
    Ok(())
}

#[tokio::test]
async fn reviewed_rhel_spelling_requires_the_matching_rhel_engine_version() -> Result<(), Box<dyn std::error::Error>> {
    let transport = RecordingTransport::new(vec![
        ping("4.9.4-rhel")?,
        version(br#"{"Components":[{"Name":"Podman Engine","Version":"4.9.4"}]}"#)?,
    ]);

    let error = probe_libpod_service(&transport)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("mixed canonical/RHEL version pair unexpectedly passed"))?;

    assert_eq!(error.code(), DiagnosticCode::ObservedCompatibility);
    assert_fixed_request_sequence(&transport)?;
    Ok(())
}

#[tokio::test]
async fn probe_rejects_invalid_ping_responses_without_requesting_version() -> Result<(), Box<dyn std::error::Error>> {
    let cases = vec![
        (
            "ping status",
            vec![response(503, Vec::new(), Vec::new())?],
            DiagnosticCode::ProbeHttpStatus,
            1,
        ),
        (
            "missing ping api header",
            vec![response(200, Vec::new(), Vec::new())?],
            DiagnosticCode::ProbeHeader,
            1,
        ),
        (
            "duplicate ping api header",
            vec![response(
                200,
                vec![
                    LibpodHeader::new("Libpod-API-Version", "4.0.0")?,
                    LibpodHeader::new("libpod-api-version", "4.0.1")?,
                ],
                Vec::new(),
            )?],
            DiagnosticCode::ProbeHeader,
            1,
        ),
        (
            "duplicate equal ping api header",
            vec![response(
                200,
                vec![
                    LibpodHeader::new("Libpod-API-Version", "5.8.6")?,
                    LibpodHeader::new("libpod-api-version", "5.8.6")?,
                ],
                Vec::new(),
            )?],
            DiagnosticCode::ProbeHeader,
            1,
        ),
        (
            "invalid ping api header",
            vec![ping("bad-version")?],
            DiagnosticCode::ProbeHeader,
            1,
        ),
    ];
    assert_malformed_cases(cases).await
}

#[tokio::test]
async fn probe_rejects_invalid_version_responses_without_a_third_request() -> Result<(), Box<dyn std::error::Error>> {
    let cases = vec![
        (
            "version status",
            vec![ping("4.0.0")?, response(500, Vec::new(), Vec::new())?],
            DiagnosticCode::ProbeHttpStatus,
            2,
        ),
        (
            "wrong content type",
            vec![
                ping("4.0.0")?,
                response(
                    200,
                    vec![LibpodHeader::new("Content-Type", "text/plain")?],
                    b"{}",
                )?,
            ],
            DiagnosticCode::ProbeHeader,
            2,
        ),
        (
            "invalid json",
            vec![ping("4.0.0")?, version(b"{")?],
            DiagnosticCode::ProbeJson,
            2,
        ),
        (
            "missing components",
            vec![ping("4.0.0")?, version(b"{}")?],
            DiagnosticCode::ProbeShape,
            2,
        ),
        (
            "non-array components",
            vec![ping("4.0.0")?, version(br#"{"Components":{}}"#)?],
            DiagnosticCode::ProbeShape,
            2,
        ),
        (
            "missing engine component",
            vec![ping("4.0.0")?, version(br#"{"Components":[]}"#)?],
            DiagnosticCode::ProbeComponent,
            2,
        ),
        (
            "missing engine version",
            vec![
                ping("5.8.6")?,
                version(br#"{"Components":[{"Name":"Podman Engine"}]}"#)?,
            ],
            DiagnosticCode::ProbeComponent,
            2,
        ),
        (
            "duplicate engine component",
            vec![
                ping("4.0.0")?,
                version(br#"{"Components":[{"Name":"Podman Engine","Version":"5.8.6"},{"Name":"Podman Engine","Version":"5.8.6"}]}"#)?,
            ],
            DiagnosticCode::ProbeComponent,
            2,
        ),
        (
            "invalid engine version",
            vec![
                ping("4.0.0")?,
                version(br#"{"Components":[{"Name":"Podman Engine","Version":"not-semver"}]}"#)?,
            ],
            DiagnosticCode::ProbeComponent,
            2,
        ),
        (
            "outside reviewed compatibility",
            vec![
                ping("4.0.0")?,
                version(br#"{"Components":[{"Name":"Podman Engine","Version":"6.2.0"}]}"#)?,
            ],
            DiagnosticCode::ObservedCompatibility,
            2,
        ),
    ];

    assert_malformed_cases(cases).await
}

async fn assert_malformed_cases(
    cases: Vec<(&'static str, Vec<LibpodResponse>, DiagnosticCode, usize)>,
) -> Result<(), Box<dyn std::error::Error>> {
    for (name, responses, expected, expected_request_count) in cases {
        let transport = RecordingTransport::new(responses);
        let error = probe_libpod_service(&transport)
            .await
            .err()
            .ok_or_else(|| std::io::Error::other("malformed response accepted"))?;
        assert_eq!(error.code(), expected, "{name}");
        assert_eq!(transport.requests()?.len(), expected_request_count, "{name}");
    }
    Ok(())
}

#[tokio::test]
async fn probe_rejects_truncated_and_oversize_json_without_leaking_bodies() -> Result<(), Box<dyn std::error::Error>> {
    let oversized = vec![b' '; podman_lens::MAX_PROBE_JSON_BYTES + 1];
    for body in [b"{\"Components\":".to_vec(), oversized] {
        let transport = RecordingTransport::new(vec![ping("4.0.0")?, version(body)?]);
        let error = probe_libpod_service(&transport)
            .await
            .err()
            .ok_or_else(|| std::io::Error::other("invalid body accepted"))?;
        assert_eq!(error.code(), DiagnosticCode::ProbeJson);
        assert_fixed_request_sequence(&transport)?;
        assert!(!error.to_string().contains("Components"));
    }
    Ok(())
}

#[tokio::test]
async fn fixed_probe_never_uses_mutating_methods() -> Result<(), Box<dyn std::error::Error>> {
    let transport = RecordingTransport::new(vec![
        ping("4.0.0")?,
        version(br#"{"Components":[{"Name":"Podman Engine","Version":"5.8.6"}]}"#)?,
    ]);
    let _ = probe_libpod_service(&transport).await?;
    assert!(
        transport
            .requests()?
            .iter()
            .all(|(method, _)| matches!(method, LibpodMethod::Get))
    );
    let post = LibpodRequest::new(
        LibpodMethod::Post,
        LibpodPath::parse("/v5.8.6/libpod/containers/create")?,
        Vec::new(),
    )?;
    assert_eq!(post.method(), LibpodMethod::Post);
    Ok(())
}

fn hex_digest(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn is_lowercase_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
