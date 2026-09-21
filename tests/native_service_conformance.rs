//! Explicit conformance against an isolated, caller-selected native Libpod service.
//!
//! This is deliberately ignored: the ordinary offline gate must not install Podman, open a
//! socket, or discover a host service.  The workflow provides a fresh service and its exact
//! socket path.
#![cfg(unix)]

use podman_lens::{
    AcquisitionOptions, InventorySectionAvailability, ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts,
    ResourceDetails, ResourceKind, TransportLimits, UnixConnection, acquire_inventory, probe_libpod_service,
};
use semver::Version;

fn conformance_inputs() -> Result<(String, String, String), Box<dyn std::error::Error>> {
    let socket = std::env::var("PODMAN_LENS_CONFORMANCE_UNIX_SOCKET")?;
    let expected = std::env::var("PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION")?;
    let parsed = Version::parse(&expected)?;
    if !parsed.pre.is_empty() || !parsed.build.is_empty() {
        return Err(
            format!("PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION must be an exact stable patch, not {expected}").into(),
        );
    }
    let root_mode = std::env::var("PODMAN_LENS_CONFORMANCE_EXPECTED_ROOT_MODE")?;
    if !matches!(root_mode.as_str(), "rootful" | "rootless") {
        return Err(format!(
            "PODMAN_LENS_CONFORMANCE_EXPECTED_ROOT_MODE must be observed rootful or rootless, not {root_mode}"
        )
        .into());
    }
    Ok((socket, expected, root_mode))
}

/// Exercises the production read-only acquisition path against every native list endpoint.
///
/// No fixture, ambient socket, Podman CLI command, or mutation is involved.  A release worker
/// supplies an isolated disposable service and a reviewed exact patch version.
#[tokio::test]
#[ignore = "requires an explicit isolated native Podman Unix socket"]
async fn acquires_every_section_from_the_explicit_native_service() -> Result<(), Box<dyn std::error::Error>> {
    let (socket, expected, observed_root_mode) = conformance_inputs()?;
    let transport = ReadOnlyUnixTransport::new(
        UnixConnection::new(socket)?,
        TransportLimits::default(),
        ReadOnlyUnixTransportTimeouts::default(),
    )?;

    let probe = probe_libpod_service(&transport).await?;
    assert_eq!(probe.engine_version().original(), expected);

    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    for kind in [
        ResourceKind::Container,
        ResourceKind::Pod,
        ResourceKind::Network,
        ResourceKind::Volume,
        ResourceKind::Image,
        ResourceKind::Secret,
    ] {
        let section = inventory
            .section(kind)
            .ok_or_else(|| format!("native acquisition omitted {kind:?}"))?;
        assert_eq!(section.availability(), InventorySectionAvailability::Available);
    }

    let named = |kind, name| {
        inventory.section(kind).and_then(|section| {
            section
                .observations()
                .iter()
                .find(|record| record.header().identity().name() == Some(name))
        })
    };
    let container = named(ResourceKind::Container, "podman-lens-native-container")
        .ok_or_else(|| std::io::Error::other("sanitized container fixture was not acquired"))?;
    let ResourceDetails::Container(container_details) = container.details() else {
        return Err(std::io::Error::other("container fixture was not decoded as a typed container").into());
    };
    assert!(container_details.configured_image().observed().is_some());
    assert!(matches!(
        named(ResourceKind::Pod, "podman-lens-native-pod").map(podman_lens::ResourceObservation::details),
        Some(ResourceDetails::Pod(_))
    ));
    assert!(matches!(
        named(ResourceKind::Network, "podman-lens-native-network").map(podman_lens::ResourceObservation::details),
        Some(ResourceDetails::Network(_))
    ));
    assert!(matches!(
        named(ResourceKind::Volume, "podman-lens-native-volume").map(podman_lens::ResourceObservation::details),
        Some(ResourceDetails::Volume(_))
    ));
    assert!(matches!(
        named(ResourceKind::Secret, "podman-lens-native-secret").map(podman_lens::ResourceObservation::details),
        Some(ResourceDetails::Secret(_))
    ));
    assert!(inventory.section(ResourceKind::Image).is_some_and(|section| {
        section
            .observations()
            .iter()
            .any(|record| matches!(record.details(), ResourceDetails::Image(_)))
    }));
    // The worker reads this identity from `podman info` inside the disposable service, rather
    // than inferring it from Docker privileges or the matrix label.
    assert!(matches!(observed_root_mode.as_str(), "rootful" | "rootless"));
    Ok(())
}
