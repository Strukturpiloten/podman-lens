//! Explicit conformance against an isolated, caller-selected native Libpod service.
//!
//! This is deliberately ignored: the ordinary offline gate must not install Podman, open a
//! socket, or discover a host service.  The workflow provides a fresh service and its exact
//! socket path.
#![cfg(unix)]

use podman_lens::{
    AcquisitionOptions, AuthoredImageSpellingHint, InventorySectionAvailability, LibpodMethod, LibpodPath,
    LibpodRequest, LibpodTransport, ObservationOrigin, ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts,
    ResourceDetails, ResourceKind, TransportLimits, UnixConnection, acquire_inventory, probe_libpod_service,
};
use semver::Version;

const REVIEWED_IMAGE_PREFIX: &str = "registry.fedoraproject.org/fedora:45@sha256:";
const CANONICAL_IMAGE_PREFIX: &str = "registry.fedoraproject.org/fedora@sha256:";

fn matches_reviewed_image_reference(observed: &str, reviewed_pin: &str) -> bool {
    let Some(digest) = reviewed_pin.strip_prefix(REVIEWED_IMAGE_PREFIX) else {
        return false;
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return false;
    }
    observed == reviewed_pin || observed.strip_prefix(CANONICAL_IMAGE_PREFIX) == Some(digest)
}

fn conformance_inputs() -> Result<(String, String, String, String), Box<dyn std::error::Error>> {
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
    let image = std::env::var("PODMAN_LENS_CONFORMANCE_EXPECTED_IMAGE")?;
    let reviewed_pin = include_str!("../scripts/native-runtime-pins.sh")
        .lines()
        .find_map(|line| line.strip_prefix("readonly PODMAN_NATIVE_BASE_IMAGE="))
        .ok_or("candidate source is missing its reviewed native fixture image pin")?;
    if image != reviewed_pin || !matches_reviewed_image_reference(&image, reviewed_pin) {
        return Err(
            "PODMAN_LENS_CONFORMANCE_EXPECTED_IMAGE must match the exact versioned source pin and digest".into(),
        );
    }
    Ok((socket, expected, root_mode, image))
}

/// Exercises the production read-only acquisition path against every native list endpoint.
///
/// No fixture, ambient socket, Podman CLI command, or mutation is involved.  A release worker
/// supplies an isolated disposable service and a reviewed exact patch version.
#[tokio::test]
#[ignore = "requires an explicit isolated native Podman Unix socket"]
async fn acquires_every_section_from_the_explicit_native_service() -> Result<(), Box<dyn std::error::Error>> {
    let (socket, expected, observed_root_mode, expected_image) = conformance_inputs()?;
    let transport = ReadOnlyUnixTransport::new(
        UnixConnection::new(socket)?,
        TransportLimits::default(),
        ReadOnlyUnixTransportTimeouts::default(),
    )?;

    let probe = probe_libpod_service(&transport).await?;
    assert_eq!(probe.engine_version().original(), expected);
    if let Ok(reported_api) = std::env::var("PODMAN_LENS_CONFORMANCE_API_VERSION") {
        assert_eq!(probe.api_version().original(), reported_api);
    }
    let expected_engine = Version::parse(&expected)?;
    assert!(probe.api_version().as_semver() >= &Version::new(4, 0, 0));
    assert!(probe.api_version().as_semver() <= &expected_engine);

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
    let configured_image = container_details
        .configured_image()
        .observed()
        .ok_or("native container fixture lacked a configured image reference")?;
    assert!(
        matches_reviewed_image_reference(configured_image.value(), &expected_image),
        "native configured image must retain the reviewed registry, name, and exact digest"
    );
    let authored = container_details
        .creation_evidence()
        .observed()
        .ok_or_else(|| std::io::Error::other("CLI-created native fixture lacked bounded authored evidence"))?;
    assert_eq!(authored.origin(), ObservationOrigin::Configured);
    assert!(authored.value().image().observed().is_some_and(|value| {
        *value.value() == AuthoredImageSpellingHint::MatchesConfiguredImage
            && value.origin() == ObservationOrigin::Configured
    }));
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
    let missing = LibpodRequest::new(
        LibpodMethod::Get,
        LibpodPath::parse("/v4.0.0/libpod/containers/podman-lens-native-absent/json")?,
        Vec::new(),
    )?;
    let missing_response = transport.send(&missing).await?;
    assert_eq!(
        missing_response.status(),
        404,
        "native missing-container GET must retain HTTP 404"
    );
    Ok(())
}

#[test]
fn native_image_reference_accepts_only_the_reviewed_digest_and_podman_canonicalization() {
    let digest = "a".repeat(64);
    let pin = format!("{REVIEWED_IMAGE_PREFIX}{digest}");
    assert!(matches_reviewed_image_reference(&pin, &pin));
    assert!(matches_reviewed_image_reference(
        &format!("{CANONICAL_IMAGE_PREFIX}{digest}"),
        &pin
    ));
    for observed in [
        format!("registry.fedoraproject.org/fedora:45@sha256:{}", "b".repeat(64)),
        format!("registry.fedoraproject.org/fedora@sha256:{}", "b".repeat(64)),
        format!("registry.fedoraproject.org/fedora:46@sha256:{digest}"),
        format!("registry.example.invalid/fedora@sha256:{digest}"),
        format!("registry.fedoraproject.org/other@sha256:{digest}"),
        format!("registry.fedoraproject.org/fedora@sha256:{}", "a".repeat(63)),
    ] {
        assert!(
            !matches_reviewed_image_reference(&observed, &pin),
            "accepted {observed}"
        );
    }
    assert!(!matches_reviewed_image_reference(
        &pin,
        &format!("{REVIEWED_IMAGE_PREFIX}{}", "A".repeat(64))
    ));
    assert!(!matches_reviewed_image_reference(
        &pin,
        &format!("{CANONICAL_IMAGE_PREFIX}{digest}")
    ));
}
