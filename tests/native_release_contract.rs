//! Deterministic captured-native release contracts and bounded consumer entry point.
#![allow(
    clippy::expect_used,
    clippy::needless_pass_by_value,
    clippy::panic,
    clippy::redundant_closure_for_method_calls,
    clippy::too_many_lines
)]
mod support;

use std::fmt::Write as _;

use podman_lens::{
    AcquisitionOptions, AuthoredImageSpellingHint, AuthoredMountRelabelHint, ContainerMountKind,
    ContainerMountSelinuxRelabel, ContainerMountSource, DiagnosticCode, InventorySectionAvailability, JsonValueKind,
    LibpodMethod, ObservationField, ObservationOrigin, ProtectedEnvironmentValue, ResourceDetails, ResourceInventory,
    ResourceKind, ResourceObservation, ResourceObservationState, UnmodelledCompleteness, UnmodelledFieldId,
    acquire_inventory,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use support::cassette::{Cassette, CassetteError, CassetteTransport};

const CLI_CAPTURE: &[u8] =
    include_bytes!("../fixtures/native-regressions/captured-cli-only-6.1.0-rootful.cassette.json");
const MIXED_CAPTURE: &[u8] =
    include_bytes!("../fixtures/native-regressions/captured-mixed-origin-6.1.0-rootful.cassette.json");
const CAPTURE_MANIFEST: &[u8] = include_bytes!("../fixtures/native-regressions/capture-manifest-6.1.0-rootful.json");

async fn replay(source: &[u8]) -> Result<ResourceInventory, Box<dyn std::error::Error>> {
    let transport = CassetteTransport::try_new(Cassette::from_slice(source)?)?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    transport.assert_consumed()?;
    Ok(inventory)
}

fn container(inventory: &ResourceInventory, name: String) -> &ResourceObservation {
    inventory
        .section(ResourceKind::Container)
        .expect("container section")
        .observations()
        .iter()
        .find(|record| record.header().identity().name() == Some(name.as_str()))
        .expect("independently expected captured container")
}

fn container_details(record: &ResourceObservation) -> &podman_lens::ContainerObservation {
    let ResourceDetails::Container(details) = record.details() else {
        panic!("container details expected");
    };
    details
}

#[tokio::test]
async fn captured_cli_and_provider_inventory_matches_independent_native_expectations()
-> Result<(), Box<dyn std::error::Error>> {
    let mut manifest_hash = String::new();
    for byte in Sha256::digest(CAPTURE_MANIFEST) {
        write!(&mut manifest_hash, "{byte:02x}")?;
    }
    assert_eq!(
        manifest_hash,
        "92020331b1ae8f6817536c9fba3fc2780e57df5e0358694e7d625ab95800f7e2"
    );
    for source in [CLI_CAPTURE, MIXED_CAPTURE] {
        let cassette = Cassette::from_slice(source)?;
        assert!(!cassette.synthetic());
        assert_eq!(
            cassette
                .provenance()
                .capture()
                .expect("captured provenance")
                .capture_manifest_sha256(),
            manifest_hash
        );
    }

    let cli_inventory = replay(CLI_CAPTURE).await?;
    let cli_names = cli_inventory
        .section(ResourceKind::Container)
        .expect("container section")
        .observations()
        .iter()
        .map(|record| record.header().identity().name().expect("captured name"))
        .collect::<Vec<_>>();
    assert_eq!(cli_names, ["fixture-cli-authored", "fixture-cli-tagged"]);

    let mixed = replay(MIXED_CAPTURE).await?;
    assert_eq!(
        mixed
            .sections()
            .iter()
            .map(|section| section.kind())
            .collect::<Vec<_>>(),
        [
            ResourceKind::Container,
            ResourceKind::Pod,
            ResourceKind::Network,
            ResourceKind::Volume,
            ResourceKind::Image,
            ResourceKind::Secret,
        ]
    );
    assert!(
        mixed
            .sections()
            .iter()
            .all(|section| { section.availability() == InventorySectionAvailability::Available })
    );
    let names = mixed
        .section(ResourceKind::Container)
        .expect("container section")
        .observations()
        .iter()
        .map(|record| record.header().identity().name().expect("captured name"))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        ["fixture-provider-app", "fixture-cli-authored", "fixture-cli-tagged"]
    );

    let provider = container(&mixed, "fixture-provider-app".to_owned());
    let provider_details = container_details(provider);
    assert!(matches!(provider_details.creation_evidence(), ObservationField::Absent));
    assert_eq!(
        provider_details
            .configured_image()
            .observed()
            .map(|value| (value.value().as_str(), value.origin())),
        Some((
            "docker.io/library/fixture-native-evidence:fixture",
            ObservationOrigin::Configured
        ))
    );
    assert_eq!(
        provider_details
            .command()
            .observed()
            .map(|value| (value.value().arguments(), value.origin())),
        Some((
            ["fixture-provider-command".to_owned()].as_slice(),
            ObservationOrigin::Configured
        ))
    );
    assert_eq!(
        provider_details
            .entrypoint()
            .observed()
            .map(|value| (value.value().arguments(), value.origin())),
        Some((["/bin/false".to_owned()].as_slice(), ObservationOrigin::Configured))
    );
    let provider_environment = provider_details.environment().observed().expect("provider environment");
    assert_eq!(provider_environment.origin(), ObservationOrigin::Effective);
    assert_eq!(
        provider_environment
            .value()
            .entries()
            .iter()
            .map(|entry| entry.name())
            .collect::<Vec<_>>(),
        ["container", "FIXTURE_CANARY", "PATH"]
    );
    assert!(
        provider_environment
            .value()
            .entries()
            .iter()
            .all(|entry| { matches!(entry.value(), ProtectedEnvironmentValue::Redacted) })
    );
    let provider_mounts = provider_details.mounts().observed().expect("provider mounts");
    assert_eq!(provider_mounts.value().len(), 1);
    assert!(matches!(
        provider_mounts.value()[0].selinux_relabel(),
        ObservationField::Absent
    ));

    let authored = container(&mixed, "fixture-cli-authored".to_owned());
    let authored_details = container_details(authored);
    let authored_mounts = authored_details.mounts().observed().expect("authored mounts");
    assert_eq!(
        authored_mounts.value()[0]
            .selinux_relabel()
            .observed()
            .map(|value| (*value.value(), value.origin())),
        Some((ContainerMountSelinuxRelabel::Private, ObservationOrigin::Configured))
    );
    let authored_evidence = authored_details
        .creation_evidence()
        .observed()
        .expect("authored evidence");
    assert_eq!(authored_evidence.origin(), ObservationOrigin::Configured);
    assert!(matches!(
        authored_evidence.value().image(),
        ObservationField::Observed(value)
            if *value.value() == AuthoredImageSpellingHint::MatchesConfiguredImage
                && value.origin() == ObservationOrigin::Configured
    ));
    assert!(matches!(
        authored_evidence.value().mount_relabels(),
        ObservationField::Observed(value)
            if value.value().as_slice() == [AuthoredMountRelabelHint::Private { mount_index: 0 }]
                && value.origin() == ObservationOrigin::Configured
    ));

    let tagged = container(&mixed, "fixture-cli-tagged".to_owned());
    let tagged_details = container_details(tagged);
    assert!(matches!(tagged_details.entrypoint(), ObservationField::Absent));
    let tagged_evidence = tagged_details.creation_evidence().observed().expect("tagged evidence");
    assert!(matches!(
        tagged_evidence.value().mount_relabels(),
        ObservationField::Observed(value)
            if value.value().as_slice() == [AuthoredMountRelabelHint::Shared { mount_index: 0 }]
    ));

    for record in [provider, authored, tagged] {
        assert_eq!(record.header().state(), ResourceObservationState::Complete);
        assert_eq!(
            record.header().unmodelled_completeness(),
            UnmodelledCompleteness::Complete
        );
        assert!(
            record
                .header()
                .unmodelled_fields()
                .iter()
                .any(|field| { field.path() == "$.Config.Image" })
        );
        assert_eq!(
            container_details(record)
                .local_image_id()
                .observed()
                .map(|value| (value.value().as_str(), value.origin())),
            Some((
                "5555555555555555555555555555555555555555555555555555555555555555",
                ObservationOrigin::LocalResolution
            ))
        );
    }
    let images = mixed
        .section(ResourceKind::Image)
        .expect("image section")
        .observations();
    assert_eq!(images.len(), 1);
    assert_eq!(
        images[0].header().identity().id(),
        "5555555555555555555555555555555555555555555555555555555555555555"
    );
    Ok(())
}

#[tokio::test]
async fn captured_fault_mutations_preserve_non_atomic_inventory_contracts() -> Result<(), Box<dyn std::error::Error>> {
    let mut partial = Cassette::from_slice(MIXED_CAPTURE)?;
    let volume_list = partial.unique_interaction_mut(LibpodMethod::Get, "/v6.1.0/libpod/volumes/json")?;
    volume_list.response_mut().set_status(503);
    volume_list
        .response_mut()
        .set_body(json!({"cause": "fixture section unavailable"}));
    let transport = CassetteTransport::try_new(partial)?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    transport.assert_consumed()?;
    assert_eq!(
        inventory
            .section(ResourceKind::Volume)
            .expect("volume section")
            .availability(),
        InventorySectionAvailability::Unavailable
    );
    assert_eq!(
        container(&inventory, "fixture-cli-authored".to_owned())
            .header()
            .state(),
        ResourceObservationState::Complete
    );

    let provider_path =
        "/v6.1.0/libpod/containers/1111111111111111111111111111111111111111111111111111111111111111/json";
    let mut stale = Cassette::from_slice(MIXED_CAPTURE)?;
    let stale_response = stale
        .unique_interaction_mut(LibpodMethod::Get, provider_path)?
        .response_mut();
    stale_response.set_status(404);
    stale_response.set_body(json!({"cause": "fixture disappeared"}));
    let transport = CassetteTransport::try_new(stale)?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    transport.assert_consumed()?;
    let stale_record = container(&inventory, "fixture-provider-app".to_owned());
    assert_eq!(stale_record.header().state(), ResourceObservationState::Unavailable);
    assert_eq!(
        stale_record.header().unmodelled_completeness(),
        UnmodelledCompleteness::Incomplete
    );
    assert_eq!(
        container(&inventory, "fixture-cli-tagged".to_owned()).header().state(),
        ResourceObservationState::Complete
    );

    let mut malformed = Cassette::from_slice(MIXED_CAPTURE)?;
    malformed
        .unique_interaction_mut(LibpodMethod::Get, provider_path)?
        .response_mut()
        .set_body(json!({"Id": false, "Name": "fixture-provider-app"}));
    let transport = CassetteTransport::try_new(malformed)?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    transport.assert_consumed()?;
    assert_eq!(
        container(&inventory, "fixture-provider-app".to_owned())
            .header()
            .state(),
        ResourceObservationState::Malformed
    );
    Ok(())
}

#[tokio::test]
async fn legacy_boundary_and_snapshot_formats_remain_explicitly_distinct() -> Result<(), Box<dyn std::error::Error>> {
    let mut legacy = Cassette::from_slice(include_bytes!("../fixtures/corpus/complex-5.4.0-rootful.cassette.json"))?;
    legacy
        .unique_interaction_mut(LibpodMethod::Get, "/v5.4.0/libpod/networks/n-data/json")?
        .response_mut()
        .set_body(json!({
            "id": "n-data",
            "name": "data-net",
            "routes": [{
                "destination": "198.51.100.0/24",
                "gateway": "192.0.2.2",
                "route_type": "blackhole"
            }]
        }));
    let transport = CassetteTransport::try_new(legacy)?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    transport.assert_consumed()?;
    let network = inventory
        .section(ResourceKind::Network)
        .expect("network section")
        .observations()
        .iter()
        .find(|record| record.header().identity().id() == "n-data")
        .expect("legacy network");
    assert!(network.header().findings().iter().any(|finding| {
        finding.code() == DiagnosticCode::VersionInapplicableField
            && finding.field_path() == Some("$.routes[0].route_type")
    }));
    let ResourceDetails::Network(network) = network.details() else {
        panic!("network details expected");
    };
    let route = &network.routes().observed().expect("legacy routes").value()[0];
    assert!(matches!(route.route_type(), ObservationField::VersionInapplicable));

    assert!(matches!(
        Cassette::from_slice(include_bytes!("../fixtures/snapshots/inventory-v1.json")),
        Err(CassetteError::SchemaViolation)
    ));
    Ok(())
}

fn captured_response_body<'a>(cassette: &'a serde_json::Value, request_path: &str) -> &'a serde_json::Value {
    cassette["interactions"]
        .as_array()
        .expect("captured interactions")
        .iter()
        .find(|interaction| interaction["request"]["path"] == request_path)
        .and_then(|interaction| interaction["response"].get("body"))
        .expect("captured response body")
}

fn set_captured_response_body(cassette: &mut serde_json::Value, request_path: &str, body: serde_json::Value) {
    let interaction = cassette["interactions"]
        .as_array_mut()
        .expect("captured interactions")
        .iter_mut()
        .find(|interaction| interaction["request"]["path"] == request_path)
        .expect("captured interaction");
    let body_len = serde_json::to_vec(&body).expect("serialize mutated fixture body").len();
    interaction["response"]["body"] = body;
    interaction["response"]
        .as_object_mut()
        .expect("response")
        .remove("body_text");
    let headers = interaction["response"]["headers"].as_array_mut().expect("headers");
    let content_length = headers
        .iter_mut()
        .find(|header| {
            header[0]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("content-length"))
        })
        .expect("Content-Length header");
    content_length[1] = json!(body_len.to_string());
}

fn assert_redacted_environment(details: &podman_lens::ContainerObservation, expected_names: &[&str]) {
    let environment = details.environment().observed().expect("captured environment");
    assert_eq!(environment.origin(), ObservationOrigin::Effective);
    assert_eq!(
        environment
            .value()
            .entries()
            .iter()
            .map(|entry| entry.name())
            .collect::<Vec<_>>(),
        expected_names
    );
    assert!(
        environment
            .value()
            .entries()
            .iter()
            .all(|entry| { matches!(entry.value(), ProtectedEnvironmentValue::Redacted) })
    );
}

fn assert_captured_bind_mount(
    details: &podman_lens::ContainerObservation,
    source: &str,
    option: &str,
    propagation: &str,
    relabel: Option<ContainerMountSelinuxRelabel>,
) {
    let mounts = details.mounts().observed().expect("captured mounts");
    assert_eq!(mounts.origin(), ObservationOrigin::Effective);
    assert_eq!(mounts.value().len(), 1);
    let mount = &mounts.value()[0];
    assert_eq!(mount.kind(), ContainerMountKind::Bind);
    let observed_source = mount.source().observed().expect("bind source");
    assert_eq!(observed_source.origin(), ObservationOrigin::LocalResolution);
    assert!(matches!(
        observed_source.value(),
        ContainerMountSource::LocalBindPath(_)
    ));
    assert_eq!(observed_source.value().value(), source);
    assert!(matches!(mount.local_backing_path(), ObservationField::Absent));
    assert_eq!(
        mount
            .destination()
            .observed()
            .map(|value| (value.value().as_str(), value.origin())),
        Some(("/data", ObservationOrigin::Configured))
    );
    assert_eq!(
        mount
            .writable()
            .observed()
            .map(|value| (*value.value(), value.origin())),
        Some((true, ObservationOrigin::Effective))
    );
    assert_eq!(
        mount
            .options()
            .observed()
            .map(|value| (value.value().as_slice(), value.origin())),
        Some(([option.to_owned()].as_slice(), ObservationOrigin::Effective))
    );
    match relabel {
        Some(expected) => assert_eq!(
            mount
                .selinux_relabel()
                .observed()
                .map(|value| (*value.value(), value.origin())),
            Some((expected, ObservationOrigin::Configured))
        ),
        None => assert!(matches!(mount.selinux_relabel(), ObservationField::Absent)),
    }
    assert_eq!(
        mount
            .propagation()
            .observed()
            .map(|value| (value.value().as_str(), value.origin())),
        Some((propagation, ObservationOrigin::Effective))
    );
    assert!(matches!(mount.subpath(), ObservationField::Absent));
}

#[test]
fn captured_manifest_and_wire_lengths_are_independently_bound() -> Result<(), Box<dyn std::error::Error>> {
    let manifest: serde_json::Value = serde_json::from_slice(CAPTURE_MANIFEST)?;
    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["evidence_kind"], "privacy-reviewed-one-off-native-capture");
    assert_eq!(manifest["capture"]["owner"], "PodmanLens");
    assert_eq!(manifest["capture"]["id"], "native-regression-one-off-6.1.0-rootful");
    assert_eq!(manifest["engine"]["version"], "6.1.0");
    assert_eq!(manifest["engine"]["api_version"], "6.1.0");
    assert_eq!(manifest["engine"]["root_mode"], "rootful");
    assert_eq!(manifest["engine"]["build_kind"], "source-build");
    assert_eq!(
        manifest["engine"]["source_revision"],
        "cade97a52ebdf9dbf9e81de8009015776837a074"
    );
    assert_eq!(
        manifest["reused_boxferry_runtime"]["repository"],
        "https://github.com/Strukturpiloten/boxferry"
    );
    assert_eq!(
        manifest["reused_boxferry_runtime"]["revision"],
        "52b7600afe936b3c688294bd9a7f43e1afa9bf1f"
    );
    assert_eq!(manifest["reused_boxferry_runtime"]["matrix_cell"], "podman-6.1-rootful");
    assert_eq!(
        manifest["reused_boxferry_runtime"]["matrix_sha256"],
        "1ed306f4b368c229bca927697156e2314b922c2ec728c55c2820c69a712bad25"
    );
    assert_eq!(
        manifest["reused_boxferry_runtime"]["image"],
        "ghcr.io/strukturpiloten/podman-6.1-rootful:v6.1.0@sha256:2cf1d0fa3d0776e6a87bc4fd3391abd227001ab22885c5897c04970f3f38cb2c"
    );
    assert_eq!(manifest["compose_provider"]["version"], "5.5.0");
    assert_eq!(
        manifest["compose_provider"]["binary_sha256"],
        "abdf6ec2a49e5cb1f021f142d55a7494c823b11fa6f950237301a1cb70b5e5c6"
    );
    assert_eq!(
        manifest["capture_tools"]["setup_script_sha256"],
        "0790eb18744c8683a538c1917fc5a550b70e8a5dfb1f2448110101422dfae977"
    );
    assert_eq!(
        manifest["capture_tools"]["request_recorder_source_sha256"],
        "26a1b238826315326057e0d555c385ea5a4617568c8cd4154a5faa9d0ff7fb91"
    );
    assert_eq!(
        manifest["capture_tools"]["request_recorder_binary_sha256"],
        "0ae531821e3f4f3ad358631bcc1ac06c85e04a7b373523b326319c63f4bd761b"
    );
    assert_eq!(
        manifest["capture_tools"]["sanitizer_sha256"],
        "32a498875c5afca1d33f0d2e49bf246a4fca123c786f6ee39b5f8d46fdd54213"
    );
    let sessions = manifest["sessions"].as_array().expect("capture sessions");
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0]["id"], "cli-only");
    assert_eq!(sessions[0]["origins"], json!(["podman-cli"]));
    assert_eq!(sessions[0]["requests"].as_array().map(Vec::len), Some(12));
    assert_eq!(sessions[1]["id"], "mixed-origin");
    assert_eq!(sessions[1]["origins"], json!(["podman-cli", "docker-compose-provider"]));
    assert_eq!(sessions[1]["requests"].as_array().map(Vec::len), Some(13));

    for source in [CLI_CAPTURE, MIXED_CAPTURE] {
        let cassette: serde_json::Value = serde_json::from_slice(source)?;
        for interaction in cassette["interactions"].as_array().expect("interactions") {
            let response = &interaction["response"];
            let replay_bytes = if let Some(body_text) = response.get("body_text").and_then(serde_json::Value::as_str) {
                body_text.len()
            } else {
                serde_json::to_vec(&response["body"])?.len()
            };
            let declared = response["headers"]
                .as_array()
                .expect("response headers")
                .iter()
                .find(|header| {
                    header[0]
                        .as_str()
                        .is_some_and(|name| name.eq_ignore_ascii_case("content-length"))
                })
                .and_then(|header| header[1].as_str())
                .expect("Content-Length")
                .parse::<usize>()?;
            assert_eq!(declared, replay_bytes, "{}", interaction["request"]["path"]);
        }
    }
    Ok(())
}

#[tokio::test]
async fn captured_container_semantics_mount_origins_topology_and_unknowns_are_exact()
-> Result<(), Box<dyn std::error::Error>> {
    let inventory = replay(MIXED_CAPTURE).await?;
    let cases = [
        (
            "fixture-provider-app",
            "docker.io/library/fixture-native-evidence:fixture",
            &["fixture-provider-command"][..],
            Some(&["/bin/false"][..]),
            &["container", "FIXTURE_CANARY", "PATH"][..],
            "/fixture/provider-data",
            "bind",
            "private",
            None,
        ),
        (
            "fixture-cli-authored",
            "localhost/fixture-authored:1",
            &["fixture-post-image-command"][..],
            Some(&["/bin/false"][..]),
            &["container", "FIXTURE_CANARY", "PATH"][..],
            "/fixture/cli-data",
            "rbind",
            "rprivate",
            Some(ContainerMountSelinuxRelabel::Private),
        ),
        (
            "fixture-cli-tagged",
            "docker.io/library/fixture-native-evidence:fixture",
            &["/bin/false"][..],
            None,
            &["PATH", "container"][..],
            "/fixture/cli-data",
            "rbind",
            "rprivate",
            Some(ContainerMountSelinuxRelabel::Shared),
        ),
    ];
    for (name, image, command, entrypoint, environment, source, option, propagation, relabel) in cases {
        let record = container(&inventory, name.to_owned());
        let details = container_details(record);
        assert_eq!(
            details
                .configured_image()
                .observed()
                .map(|value| (value.value().as_str(), value.origin())),
            Some((image, ObservationOrigin::Configured))
        );
        assert_eq!(
            details.command().observed().map(|value| {
                (
                    value.value().arguments().iter().map(String::as_str).collect::<Vec<_>>(),
                    value.origin(),
                )
            }),
            Some((command.to_vec(), ObservationOrigin::Configured))
        );
        match entrypoint {
            Some(expected) => assert_eq!(
                details.entrypoint().observed().map(|value| {
                    (
                        value.value().arguments().iter().map(String::as_str).collect::<Vec<_>>(),
                        value.origin(),
                    )
                }),
                Some((expected.to_vec(), ObservationOrigin::Configured))
            ),
            None => assert!(matches!(details.entrypoint(), ObservationField::Absent)),
        }
        assert_redacted_environment(details, environment);
        assert_captured_bind_mount(details, source, option, propagation, relabel);

        let networking = details.networking().observed().expect("captured networking");
        let networks = networking
            .value()
            .networks()
            .observed()
            .expect("native network references");
        assert_eq!(networks.origin(), ObservationOrigin::Effective);
        assert_eq!(networks.value().len(), 1);
        assert_eq!(networks.value()[0].reference(), "none");
        assert_eq!(networks.value()[0].field_path(), "$.NetworkSettings.Networks.none");
        assert!(record.header().findings().iter().any(|finding| {
            finding.code() == DiagnosticCode::UnresolvedRelationship
                && finding.field_path() == Some("$.NetworkSettings.Networks.none")
        }));

        let image_unknown = record
            .header()
            .unmodelled_fields()
            .iter()
            .find(|field| field.path() == "$.Config.Image")
            .expect("Config.Image unknown evidence");
        assert_eq!(image_unknown.id(), &UnmodelledFieldId::ContainerConfig);
        assert_eq!(image_unknown.json_kind(), JsonValueKind::String);
    }
    let network = inventory
        .section(ResourceKind::Network)
        .expect("network section")
        .observations()
        .iter()
        .find(|record| record.header().identity().name() == Some("podman"))
        .expect("default network observation");
    assert_eq!(
        network.header().identity().id(),
        "4444444444444444444444444444444444444444444444444444444444444444"
    );
    assert_eq!(network.header().state(), ResourceObservationState::Complete);

    let wire: serde_json::Value = serde_json::from_slice(MIXED_CAPTURE)?;
    let provider_body = captured_response_body(
        &wire,
        "/v6.1.0/libpod/containers/1111111111111111111111111111111111111111111111111111111111111111/json",
    );
    assert!(provider_body["Config"].get("CreateCommand").is_none());
    let tagged_body = captured_response_body(
        &wire,
        "/v6.1.0/libpod/containers/3333333333333333333333333333333333333333333333333333333333333333/json",
    );
    assert_eq!(tagged_body["Config"]["Entrypoint"], serde_json::Value::Null);

    let tagged = container(&inventory, "fixture-cli-tagged".to_owned());
    let tagged_evidence = container_details(tagged)
        .creation_evidence()
        .observed()
        .expect("tagged creation evidence");
    assert!(matches!(
        tagged_evidence.value().image(),
        ObservationField::Observed(value)
            if *value.value() == AuthoredImageSpellingHint::MatchesConfiguredImage
                && value.origin() == ObservationOrigin::Configured
    ));
    Ok(())
}

#[tokio::test]
async fn future_native_object_is_retained_as_bounded_unknown_metadata() -> Result<(), Box<dyn std::error::Error>> {
    const PROVIDER_PATH: &str =
        "/v6.1.0/libpod/containers/1111111111111111111111111111111111111111111111111111111111111111/json";
    let mut source: serde_json::Value = serde_json::from_slice(MIXED_CAPTURE)?;
    let mut body = captured_response_body(&source, PROVIDER_PATH).clone();
    body["FutureNativeContract"] = json!({"nested": true});
    set_captured_response_body(&mut source, PROVIDER_PATH, body);
    let encoded = serde_json::to_vec(&source)?;
    let inventory = replay(&encoded).await?;
    let future = container(&inventory, "fixture-provider-app".to_owned())
        .header()
        .unmodelled_fields()
        .iter()
        .find(|field| field.path() == "$.FutureNativeContract")
        .expect("future native unknown");
    assert_eq!(future.id(), &UnmodelledFieldId::ContainerTopLevel);
    assert_eq!(future.json_kind(), JsonValueKind::Object);
    Ok(())
}

#[tokio::test]
async fn captured_faults_have_exact_findings_and_later_sections_remain_available()
-> Result<(), Box<dyn std::error::Error>> {
    const PROVIDER_PATH: &str =
        "/v6.1.0/libpod/containers/1111111111111111111111111111111111111111111111111111111111111111/json";
    let mut partial = Cassette::from_slice(MIXED_CAPTURE)?;
    let volume = partial.unique_interaction_mut(LibpodMethod::Get, "/v6.1.0/libpod/volumes/json")?;
    volume.response_mut().set_status(503);
    volume
        .response_mut()
        .set_body(json!({"cause": "fixture section unavailable"}));
    let transport = CassetteTransport::try_new(partial)?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    transport.assert_consumed()?;
    assert_eq!(
        inventory
            .section(ResourceKind::Volume)
            .expect("volume section")
            .availability(),
        InventorySectionAvailability::Unavailable
    );
    assert_eq!(
        inventory
            .section(ResourceKind::Image)
            .expect("image section")
            .availability(),
        InventorySectionAvailability::Available
    );
    assert_eq!(
        inventory
            .section(ResourceKind::Image)
            .expect("image section")
            .observations()
            .len(),
        1
    );
    assert_eq!(
        inventory
            .section(ResourceKind::Secret)
            .expect("secret section")
            .availability(),
        InventorySectionAvailability::Available
    );

    let mut stale = Cassette::from_slice(MIXED_CAPTURE)?;
    let response = stale
        .unique_interaction_mut(LibpodMethod::Get, PROVIDER_PATH)?
        .response_mut();
    response.set_status(404);
    response.set_body(json!({"cause": "fixture disappeared"}));
    let transport = CassetteTransport::try_new(stale)?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    transport.assert_consumed()?;
    let stale = container(&inventory, "fixture-provider-app".to_owned());
    assert_eq!(stale.header().state(), ResourceObservationState::Unavailable);
    assert!(
        stale
            .header()
            .findings()
            .iter()
            .any(|finding| { finding.code() == DiagnosticCode::ResourceUnavailable && finding.field_path().is_none() })
    );

    let mut malformed_source: serde_json::Value = serde_json::from_slice(MIXED_CAPTURE)?;
    let mut malformed_body = captured_response_body(&malformed_source, PROVIDER_PATH).clone();
    malformed_body["Config"]["Cmd"] = json!(false);
    set_captured_response_body(&mut malformed_source, PROVIDER_PATH, malformed_body);
    let malformed_bytes = serde_json::to_vec(&malformed_source)?;
    let inventory = replay(&malformed_bytes).await?;
    let malformed = container(&inventory, "fixture-provider-app".to_owned());
    assert!(matches!(
        container_details(malformed).command(),
        ObservationField::Malformed
    ));
    assert!(malformed.header().findings().iter().any(|finding| {
        finding.code() == DiagnosticCode::ResourceMalformed && finding.field_path() == Some("$.Config.Cmd")
    }));
    Ok(())
}

#[test]
fn observational_and_deployment_formats_are_not_cassette_inputs() {
    for source in [
        include_bytes!("../fixtures/snapshots/inventory-v1.json").as_slice(),
        include_bytes!("../fixtures/deployment/deployment-plan-v1.json").as_slice(),
    ] {
        assert!(matches!(
            Cassette::from_slice(source),
            Err(CassetteError::SchemaViolation)
        ));
    }
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires the explicit BoxFerry fixture socket and exact reviewed Podman version"]
async fn boxferry_consumer_replays_only_an_explicit_bounded_unix_service() -> Result<(), Box<dyn std::error::Error>> {
    use podman_lens::{ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts, TransportLimits, UnixConnection};
    let socket = std::env::var("PODMAN_LENS_BOXFERRY_UNIX_SOCKET").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "PODMAN_LENS_BOXFERRY_UNIX_SOCKET must name the explicit fixture socket",
        )
    })?;
    let expected = std::env::var("PODMAN_LENS_BOXFERRY_EXPECTED_VERSION").map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "PODMAN_LENS_BOXFERRY_EXPECTED_VERSION must be exactly 5.8.6 or 6.1.0",
        )
    })?;
    if !matches!(expected.as_str(), "5.8.6" | "6.1.0") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "PODMAN_LENS_BOXFERRY_EXPECTED_VERSION must be exactly 5.8.6 or 6.1.0",
        )
        .into());
    }
    let transport = ReadOnlyUnixTransport::new(
        UnixConnection::new(socket)?,
        TransportLimits::default(),
        ReadOnlyUnixTransportTimeouts::default(),
    )?;
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    assert_eq!(inventory.service().engine_version().original(), expected);
    assert_eq!(inventory.sections().len(), 6);
    Ok(())
}
