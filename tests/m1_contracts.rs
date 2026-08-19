//! Positive and negative M1 public-contract tests.

use podman_lens::{
    ConnectionKind, ConnectionSpec, DiagnosticCode, LibpodHeader, LibpodHeaders, LibpodMethod, LibpodPath,
    LibpodRequest, LibpodResponse, MAX_PATH_AND_QUERY_BYTES, MutualTlsPolicy, ObservedApiVersion,
    ObservedPodmanVersion, OpaqueReference, SshConnection, SupportedPodmanRange, TargetProfile, TcpMutualTlsConnection,
    TransportLimits, UnixConnection, capability_catalogue,
};

fn reference(value: &str) -> Result<OpaqueReference, Box<dyn std::error::Error>> {
    Ok(OpaqueReference::new(value)?)
}

fn mutual_tls_policy() -> Result<MutualTlsPolicy, Box<dyn std::error::Error>> {
    Ok(MutualTlsPolicy::new(
        "podman.example.invalid",
        reference("ca-reference")?,
        reference("certificate-reference")?,
        reference("private-key-reference")?,
    )?)
}

#[test]
fn connection_endpoints_accept_only_explicit_secure_forms() -> Result<(), Box<dyn std::error::Error>> {
    let unix = UnixConnection::new("/run/user/1000/podman/podman.sock")?;
    assert_eq!(unix.socket_path().to_str(), Some("/run/user/1000/podman/podman.sock"));
    assert!(UnixConnection::parse("unix:///run/user/1000/podman/podman.sock").is_ok());

    let ssh = SshConnection::parse(
        "ssh://podman@[2001:db8::1]:2222/run/user/1000/podman/podman.sock",
        reference("verified-host-key")?,
        reference("ssh-authentication")?,
    )?;
    assert_eq!(ssh.port(), 2222);

    let tcp = TcpMutualTlsConnection::parse("tcp://[2001:db8::1]:8443", mutual_tls_policy()?)?;
    assert_eq!(tcp.port(), 8443);
    assert_eq!(ConnectionSpec::TcpMutualTls(tcp).kind(), ConnectionKind::TcpMutualTls);
    Ok(())
}

#[test]
fn connection_endpoints_reject_ambient_insecure_and_ambiguous_forms() -> Result<(), Box<dyn std::error::Error>> {
    for socket in [
        "podman.sock",
        "/",
        "/run/user/1000/../podman.sock",
        "/run/user/1000/./podman.sock",
        "/run/user/1000/podman\0.sock",
    ] {
        assert!(UnixConnection::new(socket).is_err(), "must reject {socket:?}");
    }
    assert!(UnixConnection::new(format!("/{}", "a".repeat(108))).is_err());
    for endpoint in [
        "unix://host/run/user/1000/podman.sock",
        "unix:///run/user/1000/podman%20socket",
        "unix:///run/user/1000/../podman.sock",
        "unix:///run/user/1000/%2e%2e/podman.sock",
        "unix:///run/user/1000/%2E%2E/podman.sock",
        "unix:///run/user/1000%2Fpodman.sock",
        "ssh://podman@example.invalid/run/user/1000/podman.sock",
        "ssh://podman:password@example.invalid:22/run/user/1000/podman.sock",
        "ssh://podman@example.invalid:22",
        "ssh://podman@example.invalid:22/run/user/1000/../podman.sock",
        "ssh://podman@example.invalid:22/run/user/1000/%2e%2e/podman.sock",
        "tcp://example.invalid",
        "tcp://user@example.invalid:8443",
        "tcp://example.invalid:8443/extra",
        "tcp://example.invalid:8443?untrusted=true",
        "http://example.invalid:8080",
    ] {
        let result = if endpoint.starts_with("unix:") {
            UnixConnection::parse(endpoint).map(|_| ())
        } else if endpoint.starts_with("ssh:") {
            SshConnection::parse(endpoint, reference("host-key")?, reference("authentication")?).map(|_| ())
        } else {
            TcpMutualTlsConnection::parse(endpoint, mutual_tls_policy()?).map(|_| ())
        };
        assert!(result.is_err(), "must reject {endpoint:?}");
    }
    assert!(OpaqueReference::new("\n").is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn unix_socket_paths_reject_non_utf8_spelling() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

    let non_utf8 = PathBuf::from(OsString::from_vec(vec![b'/', 0xff, b'.', b's', b'o', b'c', b'k']));
    assert!(UnixConnection::new(non_utf8).is_err());
}

#[test]
fn endpoint_formatting_and_diagnostics_are_redacted() -> Result<(), Box<dyn std::error::Error>> {
    let unix = UnixConnection::new("/very/private/podman.sock")?;
    let ssh = SshConnection::new(
        "private.example.invalid",
        2222,
        "private-user",
        "/very/private/podman.sock",
        reference("very-private-host-key")?,
        reference("very-private-authentication")?,
    )?;
    let tcp = TcpMutualTlsConnection::parse("tcp://private.example.invalid:8443", mutual_tls_policy()?)?;
    let output = format!("{unix:?} {unix} {ssh:?} {ssh} {tcp:?} {tcp}");
    for secret in [
        "private.example.invalid",
        "private-user",
        "very/private",
        "very-private-host-key",
        "very-private-authentication",
    ] {
        assert!(!output.contains(secret), "formatting leaked {secret}");
    }
    let error = UnixConnection::new("not-an-absolute-private-socket")
        .err()
        .ok_or_else(|| std::io::Error::other("invalid socket accepted"))?;
    assert_eq!(error.code(), DiagnosticCode::InvalidConnection);
    assert!(!error.to_string().contains("private"));
    Ok(())
}

#[test]
fn transport_preserves_duplicate_headers_and_validates_paths() -> Result<(), Box<dyn std::error::Error>> {
    let headers = LibpodHeaders::new(vec![
        LibpodHeader::new("Libpod-API-Version", "5.8.0")?,
        LibpodHeader::new("libpod-api-version", "6.1.0")?,
        LibpodHeader::new("content-length", "0")?,
        LibpodHeader::new("Content-Length", "1")?,
    ]);
    assert_eq!(headers.len(), 4);
    assert_eq!(
        headers.values("LIBPOD-API-VERSION").collect::<Vec<_>>(),
        ["5.8.0", "6.1.0"]
    );

    assert_eq!(LibpodPath::parse("/libpod/_ping")?.as_str(), "/libpod/_ping");
    assert_eq!(
        LibpodPath::parse("/v6.1.0/libpod/containers/json?all=true&external=true")?.as_str(),
        "/v6.1.0/libpod/containers/json?all=true&external=true"
    );
    assert_eq!(
        LibpodPath::parse(
            "/v6.1.0/libpod/images/registry.example.invalid/team/image:1.2@sha256:abcdef?reference=team%3Aimage"
        )?
        .as_str(),
        "/v6.1.0/libpod/images/registry.example.invalid/team/image:1.2@sha256:abcdef?reference=team%3Aimage"
    );
    for path in [
        "/libpod/containers/json",
        "/v6.1.0-rc.1/libpod/containers/json",
        "/v6.1.0/libpod/images/../private",
        "/v6.1.0/libpod/images/./private",
        "/v6.1.0/libpod/images/%2e/private",
        "/v6.1.0/libpod/images/%2E%2E/private",
        "/v6.1.0/libpod/images/private%2Fother",
        "/v6.1.0/libpod/images/private%5Cother",
        "/v6.1.0/libpod/images/private image",
        "/v6.1.0/libpod/images/private\"image",
        "/v6.1.0/libpod/images/private<image>",
        "/v6.1.0/libpod/images/private[image]",
        "/v6.1.0/libpod/images/private\nHTTP/1.1",
        "/v6.1.0/libpod/images/café",
        "/v6.1.0/libpod/containers/json?all=%",
        "/v6.1.0/libpod/containers/json?all=%0",
        "/v6.1.0/libpod/containers/json?all=%GG",
        "/v6.1.0/libpod/containers/json?filter=one%20two",
        "/v6.1.0/libpod/containers/json?filter=one?two",
        "/v6.1.0/libpod/containers/json?filter=one\ttwo",
        "https://example.invalid/v6.1.0/libpod/containers/json",
    ] {
        assert!(LibpodPath::parse(path).is_err(), "must reject {path}");
    }
    assert!(
        LibpodPath::parse(format!(
            "/v6.1.0/libpod/containers/json?filter={}",
            "a".repeat(MAX_PATH_AND_QUERY_BYTES)
        ))
        .is_err()
    );
    Ok(())
}

#[test]
fn transport_limits_and_formatting_are_bounded_and_redacted() -> Result<(), Box<dyn std::error::Error>> {
    for limits in [
        TransportLimits::new(0, 1, 1),
        TransportLimits::new(1, 0, 1),
        TransportLimits::new(1, 1, 0),
    ] {
        assert!(limits.is_err());
    }
    let limits = TransportLimits::new(3, 1, 64)?;
    let path = LibpodPath::parse("/v6.1.0/libpod/containers/json?token=private-value")?;
    let headers = LibpodHeaders::new(vec![LibpodHeader::new("authorization", "private-header")?]);
    let request = LibpodRequest::with_limits(limits, LibpodMethod::Get, path, headers, b"abc")?;
    let output = format!("{request:?}");
    assert!(!output.contains("private-value"));
    assert!(!output.contains("private-header"));
    assert!(!output.contains("abc"));
    assert!(
        LibpodRequest::with_limits(
            limits,
            LibpodMethod::Get,
            LibpodPath::parse("/libpod/_ping")?,
            LibpodHeaders::default(),
            b"abcd",
        )
        .is_err()
    );
    assert!(LibpodResponse::with_limits(limits, 99, LibpodHeaders::default(), Vec::new()).is_err());
    assert!(LibpodResponse::with_limits(limits, 200, LibpodHeaders::default(), b"abcd").is_err());
    assert!(LibpodHeader::new("bad header", "value").is_err());
    assert!(LibpodHeader::new("valid", "bad\r\nvalue").is_err());
    Ok(())
}

#[test]
fn versions_preserve_spelling_reject_prereleases_and_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let podman = ObservedPodmanVersion::parse("5.8.6+vendor.1")?;
    let api = ObservedApiVersion::parse("5.8.6+vendor.1")?;
    assert_eq!(podman.original(), "5.8.6+vendor.1");
    assert_eq!(api.original(), "5.8.6+vendor.1");
    assert!(ObservedPodmanVersion::parse("5.8").is_err());
    assert!(ObservedApiVersion::parse("5.8.0-rc.1").is_err());
    assert!(!SupportedPodmanRange.contains(&ObservedPodmanVersion::parse("5.3.9")?));
    assert!(SupportedPodmanRange.contains(&ObservedPodmanVersion::parse("5.4.0")?));
    assert!(SupportedPodmanRange.contains(&ObservedPodmanVersion::parse("6.1.99")?));
    assert!(!SupportedPodmanRange.contains(&ObservedPodmanVersion::parse("6.2.0")?));

    assert!(TargetProfile::new(podman, api).is_ok());
    assert!(
        TargetProfile::new(
            ObservedPodmanVersion::parse("5.8.6")?,
            ObservedApiVersion::parse("5.8.0")?,
        )
        .is_ok()
    );
    assert!(
        TargetProfile::new(
            ObservedPodmanVersion::parse("5.8.6")?,
            ObservedApiVersion::parse("4.0.0")?,
        )
        .is_ok()
    );
    assert!(
        TargetProfile::new(
            ObservedPodmanVersion::parse("5.8.6")?,
            ObservedApiVersion::parse("3.9.9")?,
        )
        .is_err()
    );
    assert!(
        TargetProfile::new(
            ObservedPodmanVersion::parse("5.8.6")?,
            ObservedApiVersion::parse("5.8.7")?,
        )
        .is_err()
    );
    assert!(
        TargetProfile::new(
            ObservedPodmanVersion::parse("6.2.0")?,
            ObservedApiVersion::parse("6.2.0")?,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn catalogue_covers_every_reviewed_minor_line_with_immutable_evidence() -> Result<(), Box<dyn std::error::Error>> {
    let catalogue = capability_catalogue()?;
    assert_eq!(catalogue.len(), 7);
    let lines = catalogue
        .iter()
        .map(podman_lens::CapabilityCatalogueEntry::podman_minor_line)
        .collect::<Vec<_>>();
    assert_eq!(lines, ["5.4", "5.5", "5.6", "5.7", "5.8", "6.0", "6.1"]);
    for entry in &catalogue {
        assert!(entry.evidence().source().contains(entry.evidence().revision()));
        assert_eq!(entry.evidence().revision().len(), 40);
    }
    assert_eq!(catalogue[4].evidence().release_tag(), "v5.8.6");
    assert_eq!(catalogue[4].observed_podman_version(), "5.8.6");
    assert_eq!(catalogue[4].observed_libpod_api_version(), "5.8.6");
    assert!(
        catalogue
            .iter()
            .all(|entry| entry.minimum_libpod_api_version() == "4.0.0")
    );
    assert_eq!(catalogue[6].evidence().release_tag(), "v6.1.0");
    Ok(())
}
