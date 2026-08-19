//! Compile-time public API contract for the M1 transport and compatibility boundary.

use podman_lens::{
    ConnectionSpec, LibpodHeaders, LibpodPath, LibpodRequest, LibpodResponse, LibpodTransport, LibpodTransportFuture,
    OpaqueReference, SshConnection, TransportError, UnixConnection,
};

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
