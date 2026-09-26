//! Explicit opt-in conformance against a real, read-only Libpod Unix service.

#![cfg(unix)]

use podman_lens::{
    ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts, TransportLimits, UnixConnection, probe_libpod_service,
};
use semver::Version;

/// Checks the two fixed GET probe requests against a selected current reviewed patch.
///
/// This test is intentionally excluded from deterministic validation. Run it explicitly with
/// `cargo test --test current_patch_conformance -- --ignored` and provide both variables:
///
/// - `PODMAN_LENS_CONFORMANCE_UNIX_SOCKET=/absolute/podman.sock`
/// - `PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION=<reviewed stable patch>`
#[tokio::test]
#[ignore = "requires an explicit local Podman Unix socket and reviewed exact expected version"]
async fn current_reviewed_patch_probes_read_only_service() -> Result<(), Box<dyn std::error::Error>> {
    let socket = std::env::var("PODMAN_LENS_CONFORMANCE_UNIX_SOCKET").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "PODMAN_LENS_CONFORMANCE_UNIX_SOCKET must name an explicit absolute Unix socket",
        )
    })?;
    let expected = std::env::var("PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION must be an exact stable patch",
        )
    })?;
    let parsed = Version::parse(&expected)?;
    if !parsed.pre.is_empty() || !parsed.build.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION must be an exact stable patch",
        )
        .into());
    }

    let transport = ReadOnlyUnixTransport::new(
        UnixConnection::new(socket)?,
        TransportLimits::default(),
        ReadOnlyUnixTransportTimeouts::default(),
    )?;
    let observation = probe_libpod_service(&transport).await?;
    assert_eq!(observation.engine_version().original(), expected);
    if let Ok(reported_api) = std::env::var("PODMAN_LENS_CONFORMANCE_API_VERSION") {
        assert_eq!(observation.api_version().original(), reported_api);
    }
    assert!(observation.api_version().as_semver() >= &Version::new(4, 0, 0));
    assert!(observation.api_version().as_semver() <= &parsed);
    Ok(())
}
