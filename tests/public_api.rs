//! Compile-time public API contract for the M1 transport and compatibility boundary.

#[cfg(unix)]
use std::time::Duration;

use podman_lens::{
    AcquisitionOptions, ConnectionSpec, LibpodHeaders, LibpodPath, LibpodRequest, LibpodResponse, LibpodTransport,
    LibpodTransportFuture, OpaqueReference, ResourceKind, SshConnection, TransportError, UnixConnection,
    acquire_inventory, probe_libpod_service,
};
#[cfg(unix)]
use podman_lens::{ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts, TransportLimits};

struct FixtureTransport;

impl LibpodTransport for FixtureTransport {
    fn send<'a>(&'a self, _request: &'a LibpodRequest) -> LibpodTransportFuture<'a> {
        Box::pin(async {
            LibpodResponse::new(200, LibpodHeaders::default(), Vec::new()).map_err(|_| TransportError::unavailable())
        })
    }
}

#[test]
fn external_consumer_can_construct_explicit_connections_and_an_object_safe_transport()
-> Result<(), Box<dyn std::error::Error>> {
    let unix = UnixConnection::parse("unix:///run/user/1000/podman/podman.sock")?;
    let connection = ConnectionSpec::Unix(unix);
    assert!(matches!(connection, ConnectionSpec::Unix(_)));

    let ssh = SshConnection::parse(
        "ssh://podman@example.invalid:2222/run/user/1000/podman/podman.sock",
        OpaqueReference::new("host-key-reference")?,
        OpaqueReference::new("authentication-reference")?,
    )?;
    assert_eq!(ssh.port(), 2222);

    let transport: &dyn LibpodTransport = &FixtureTransport;
    let request = LibpodRequest::new(
        podman_lens::LibpodMethod::Get,
        LibpodPath::parse("/libpod/_ping")?,
        Vec::new(),
    )?;
    drop(transport.send(&request));
    Ok(())
}

#[test]
fn crate_can_be_linked_by_an_external_consumer() {
    assert_eq!(env!("CARGO_PKG_NAME"), "podman-lens");
}

#[test]
fn external_consumer_can_select_the_redacted_inventory_contract() {
    let options = AcquisitionOptions::redacted();
    assert_eq!(options, AcquisitionOptions::default());
    assert_eq!(ResourceKind::Container, ResourceKind::Container);
    let transport: &dyn LibpodTransport = &FixtureTransport;
    drop(acquire_inventory(
        transport,
        AcquisitionOptions::include_environment_values(),
    ));
}

#[tokio::test]
#[cfg(unix)]
async fn external_consumer_can_use_the_fixed_read_only_probe_contract() -> Result<(), Box<dyn std::error::Error>> {
    let transport: &dyn LibpodTransport = &FixtureTransport;
    let error = probe_libpod_service(transport)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("empty fixture response unexpectedly probed"))?;
    assert_eq!(error.code().as_str(), "PLN0008");

    let unix = UnixConnection::new("/run/user/1000/podman/podman.sock")?;
    let timeouts = ReadOnlyUnixTransportTimeouts::new(
        Duration::from_secs(1),
        Duration::from_secs(1),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )?;
    let transport = ReadOnlyUnixTransport::new(unix, TransportLimits::default(), timeouts)?;
    assert_eq!(transport.timeouts(), timeouts);
    Ok(())
}
