//! Acquire and discover through one explicitly supplied read-only Podman Unix socket.

#[cfg(unix)]
use std::{env, io, path::PathBuf};

#[cfg(unix)]
use podman_lens::{
    AcquisitionOptions, DiscoveryRequest, ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts, TransportLimits,
    UnixConnection, acquire_inventory, discover, snapshot::v1,
};

#[cfg(unix)]
pub(crate) fn explicit_transport(socket_path: PathBuf) -> podman_lens::PodmanLensResult<ReadOnlyUnixTransport> {
    ReadOnlyUnixTransport::new(
        UnixConnection::new(socket_path)?,
        TransportLimits::default(),
        ReadOnlyUnixTransportTimeouts::default(),
    )
}

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
#[allow(dead_code)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = env::args_os().nth(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: read_only_discovery /absolute/path/to/podman.sock",
        )
    })?;
    let transport = explicit_transport(PathBuf::from(socket_path))?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    let mut request = DiscoveryRequest::new();
    request.select_all();
    let graph = discover(&inventory, &request)?;

    println!("{}", serde_json::to_string_pretty(&v1::graph(&graph))?);
    Ok(())
}

#[cfg(not(unix))]
fn main() {
    eprintln!("read_only_discovery requires a Unix-domain socket");
}
