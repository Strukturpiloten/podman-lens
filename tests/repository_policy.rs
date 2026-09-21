//! Repository-policy regression tests.

use std::{collections::BTreeSet, fmt::Write as _, fs, path::Path};

use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[test]
fn published_package_includes_the_public_contract_and_governance_documents() -> Result<(), std::io::Error> {
    let manifest = fs::read_to_string("Cargo.toml")?;

    for expected in [
        "/AGENTS.md",
        "/CHANGELOG.md",
        "/CONTRIBUTING.md",
        "/LICENSE",
        "/README.md",
        "/SECURITY.md",
        "/docs/**",
        "/docs/schemas/podman-lens-snapshot-v1.schema.json",
        "/docs/schemas/podman-lens-deployment-v1.schema.json",
        "/examples/**",
        "/catalogue/v1/podman-deployment-rendering.json",
        "/catalogue/v1/native-field-coverage.json",
        "/fixtures/**",
        "/fixtures/deployment/**",
        "/fixtures/corpus/**",
        "/fixtures/snapshots/**",
        "/src/**",
    ] {
        assert!(
            manifest.contains(expected),
            "Cargo package include list omits {expected}"
        );
    }

    Ok(())
}

#[test]
fn complete_repository_yaml_documents_start_with_a_marker() -> Result<(), std::io::Error> {
    let yaml_files = [
        ".github/workflows/ci.yml",
        ".github/workflows/documentation-links.yml",
        ".github/workflows/release-plz.yml",
        ".github/workflows/release.yml",
        ".github/workflows/native-podman-conformance.yml",
    ];

    for file in yaml_files {
        let contents = fs::read_to_string(file)?;
        assert!(
            contents.starts_with("---\n"),
            "{file} must begin with a YAML document marker"
        );
    }

    Ok(())
}

#[test]
fn required_governance_documents_exist() {
    for file in [
        "AGENTS.md",
        "CONTRIBUTING.md",
        "SECURITY.md",
        "docs/README.md",
        "docs/api-stability.md",
        "docs/architecture.md",
        "docs/dependency-policy.md",
        "docs/project-structure.md",
        "docs/releasing.md",
        "docs/roadmap.md",
        "docs/testing.md",
    ] {
        assert!(Path::new(file).is_file(), "missing {file}");
    }
}

fn documentation_word_count(text: &str) -> usize {
    text.split(|character: char| {
        !(character.is_alphanumeric() || character == '_' || character == '\'' || character == '-')
    })
    .filter(|word| !word.is_empty())
    .count()
}

fn contains_milestone_token(text: &str) -> bool {
    text.split(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
        .any(|word| {
            let bytes = word.as_bytes();
            bytes.len() >= 2 && bytes[0] == b'M' && bytes[1].is_ascii_digit() && (bytes.len() == 2 || bytes[2] == b'-')
        })
}

#[test]
fn current_documentation_stays_bounded_and_does_not_become_a_stale_ledger() -> Result<(), std::io::Error> {
    let documents = [
        ("README.md", 800),
        ("CONTRIBUTING.md", 500),
        ("SECURITY.md", 300),
        ("docs/README.md", 350),
        ("docs/api-stability.md", 900),
        ("docs/architecture.md", 1_300),
        ("docs/dependency-policy.md", 500),
        ("docs/project-structure.md", 700),
        ("docs/releasing.md", 600),
        ("docs/roadmap.md", 600),
        ("docs/testing.md", 1_300),
    ];

    for (path, word_limit) in documents {
        let text = fs::read_to_string(path)?;
        assert!(
            documentation_word_count(&text) <= word_limit,
            "{path} exceeds its {word_limit}-word current-document limit"
        );
        assert!(
            !contains_milestone_token(&text),
            "{path} contains historical implementation-batch notation"
        );
        for stale_ledger_phrase in ["input-observation rows", "output-intent rows", "total ledger rows"] {
            assert!(
                !text.contains(stale_ledger_phrase),
                "{path} contains mutable catalogue count prose: {stale_ledger_phrase}"
            );
        }
    }

    Ok(())
}

#[test]
fn obsolete_or_misowned_documents_do_not_return() {
    for file in [
        "docs/boxferry-integration.md",
        "docs/compatibility.md",
        "docs/development-environment.md",
        "docs/library-api.md",
        "docs/release-readiness.md",
    ] {
        assert!(!Path::new(file).exists(), "obsolete documentation returned: {file}");
    }
}

#[test]
fn public_documentation_and_examples_exist() {
    for file in [
        "docs/public/index.md",
        "docs/public/acquisition/index.md",
        "docs/public/discovery/index.md",
        "docs/public/grouping/index.md",
        "docs/public/planning-rendering/index.md",
        "docs/public/diagnostics-privacy/index.md",
        "docs/public/compatibility/index.md",
        "examples/read_only_discovery.rs",
        "examples/offline_plan_and_render.rs",
        "tests/public_guides.rs",
    ] {
        assert!(Path::new(file).is_file(), "missing {file}");
    }
}

#[test]
fn snapshot_schema_and_exact_golden_fixtures_exist() {
    for file in [
        "docs/schemas/podman-lens-snapshot-v1.schema.json",
        "fixtures/snapshots/inventory-v1.json",
        "fixtures/snapshots/graph-v1.json",
    ] {
        assert!(Path::new(file).is_file(), "missing {file}");
    }
}

#[test]
fn deployment_renderer_evidence_schema_and_exact_goldens_exist() {
    for file in [
        "catalogue/v1/podman-deployment-rendering.json",
        "docs/schemas/podman-lens-deployment-v1.schema.json",
        "fixtures/deployment/deployment-plan-v1.json",
        "fixtures/deployment/deployment.sh",
    ] {
        assert!(Path::new(file).is_file(), "missing {file}");
    }
}

#[test]
fn native_field_coverage_ledger_exists() {
    assert!(
        Path::new("catalogue/v1/native-field-coverage.json").is_file(),
        "missing native-field coverage ledger"
    );
}

#[test]
fn checked_in_deployment_artifacts_do_not_contain_the_sensitive_reference_sentinel() -> Result<(), std::io::Error> {
    for file in [
        "fixtures/deployment/deployment-plan-v1.json",
        "fixtures/deployment/deployment.sh",
    ] {
        let contents = fs::read_to_string(file)?;
        assert!(
            !contents.contains("vault/app-password"),
            "{file} contains the sensitive external-input reference sentinel"
        );
    }

    Ok(())
}

#[test]
fn offline_input_corpus_has_a_manifest_and_every_fixed_fixture_family() {
    for file in [
        "fixtures/corpus/manifest.json",
        "fixtures/corpus/rootless-5.4.responses.json",
        "fixtures/corpus/rootful-6.1.responses.json",
        "fixtures/corpus/malformed-6.1.responses.json",
        "fixtures/corpus/graph-boundaries-6.1.responses.json",
        "fixtures/corpus/network-ipam-routes-6.0.responses.json",
        "fixtures/corpus/boxferry-6.1.responses.json",
        "fixtures/corpus/boxferry-adapter-6.1.expected.json",
    ] {
        assert!(Path::new(file).is_file(), "missing {file}");
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn native_regression_fixtures_have_fixed_provenance_privacy_license_and_hashes()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("fixtures/native-regressions");
    let manifest: Value = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;

    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["evidence_kind"], "sanitized-native-regression-registry");
    assert_eq!(manifest["test_only"], true);
    assert_eq!(manifest["license"]["repository_fixture"], "MPL-2.0");
    assert_eq!(manifest["license"]["upstream_podman"], "Apache-2.0");
    assert_eq!(manifest["license"]["upstream_compose"], "Apache-2.0");
    assert_eq!(manifest["privacy"]["classification"], "sanitized-captured-and-derived");
    assert!(
        manifest["privacy"]["statement"]
            .as_str()
            .is_some_and(|statement| statement.contains("No real endpoint") && statement.contains("capture directory"))
    );

    let provenance = &manifest["provenance"];
    assert_eq!(provenance["podman_engine"], "6.1.0");
    assert_eq!(provenance["release_tag"], "v6.1.0");
    assert_eq!(provenance["podman_commit"], "cade97a52ebdf9dbf9e81de8009015776837a074");
    assert_eq!(provenance["source_urls"].as_array().map(Vec::len), Some(3));
    assert_eq!(provenance["compose_provider"], "5.5.0");
    assert_eq!(
        provenance["compose_provider_binary_sha256"],
        "abdf6ec2a49e5cb1f021f142d55a7494c823b11fa6f950237301a1cb70b5e5c6"
    );
    assert_eq!(
        provenance["runtime_repository"],
        "https://github.com/Strukturpiloten/boxferry"
    );
    assert_eq!(
        provenance["runtime_revision"],
        "52b7600afe936b3c688294bd9a7f43e1afa9bf1f"
    );
    assert_eq!(provenance["runtime_matrix_cell"], "podman-6.1-rootful");
    assert_eq!(
        provenance["runtime_matrix_sha256"],
        "1ed306f4b368c229bca927697156e2314b922c2ec728c55c2820c69a712bad25"
    );
    assert_eq!(
        provenance["runtime_image"],
        "ghcr.io/strukturpiloten/podman-6.1-rootful:v6.1.0@sha256:2cf1d0fa3d0776e6a87bc4fd3391abd227001ab22885c5897c04970f3f38cb2c"
    );
    assert!(
        provenance["capture_scope"]
            .as_str()
            .is_some_and(|scope| scope.contains("no host socket or repository mount"))
    );
    assert_eq!(
        provenance["capture_contract"],
        "one-off captured regression evidence; BoxFerry remains the maintained live matrix"
    );

    let expected = [
        (
            "creation-evidence-6.1.0.json",
            "focused-derived-native-observation",
            "84c6e4b8ab644661259bccf08ddfcf3cac949da01917bdbcc381a7e93db76f6c",
            false,
        ),
        (
            "capture-manifest-6.1.0-rootful.json",
            "capture-manifest",
            "92020331b1ae8f6817536c9fba3fc2780e57df5e0358694e7d625ab95800f7e2",
            false,
        ),
        (
            "captured-cli-only-6.1.0-rootful.cassette.json",
            "libpod-cassette",
            "98e003a18ec79864862bf524e004b5d9a1c90f93acae4b4a9f5215901b6beb35",
            true,
        ),
        (
            "captured-mixed-origin-6.1.0-rootful.cassette.json",
            "libpod-cassette",
            "4fff2585def69669f0cff9f8cd5b2f6275a8774ddaa6840f16ec8930523355cd",
            true,
        ),
    ];
    let artifacts = manifest["artifacts"]
        .as_array()
        .ok_or_else(|| std::io::Error::other("native artifacts must be an array"))?;
    assert_eq!(artifacts.len(), expected.len());
    let mut listed = BTreeSet::new();
    for (name, kind, expected_hash, linked) in expected {
        let artifact = artifacts
            .iter()
            .find(|artifact| artifact["artifact"] == name)
            .ok_or_else(|| std::io::Error::other(format!("missing native artifact {name}")))?;
        assert_eq!(artifact["kind"], kind);
        assert_eq!(artifact["sha256"], expected_hash);
        assert!(
            artifact["coverage"]
                .as_array()
                .is_some_and(|coverage| !coverage.is_empty())
        );
        if linked {
            assert_eq!(
                artifact["capture_manifest_sha256"],
                "92020331b1ae8f6817536c9fba3fc2780e57df5e0358694e7d625ab95800f7e2"
            );
        } else {
            assert!(artifact.get("capture_manifest_sha256").is_none());
        }
        let bytes = fs::read(root.join(name))?;
        let mut digest = String::new();
        for byte in Sha256::digest(bytes) {
            write!(&mut digest, "{byte:02x}")?;
        }
        assert_eq!(digest, expected_hash, "{name} digest");
        assert!(listed.insert(name.to_owned()));
    }
    let present = fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<BTreeSet<_>, _>>()?
        .into_iter()
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
                && name != "manifest.json"
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        listed, present,
        "native fixture registry must cover every JSON artifact"
    );

    let gate = fs::read_to_string("scripts/check-native-release-contract.sh")?;
    for required in [
        "cargo test --locked",
        "--test cassette_contract",
        "--test native_release_contract",
    ] {
        assert!(gate.contains(required), "native gate misses {required}");
    }
    assert!(!gate.contains("--ignored"), "ordinary native gate must stay offline");
    for consumer in ["scripts/check-all.sh", ".github/workflows/ci.yml"] {
        assert!(
            fs::read_to_string(consumer)?.contains("scripts/check-native-release-contract.sh"),
            "{consumer} must invoke the named native gate"
        );
    }

    let mut directories = vec![Path::new("src").to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                directories.push(entry.path());
            } else if entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
            {
                let source = fs::read_to_string(entry.path())?;
                for forbidden in [
                    "capture-manifest-6.1.0-rootful.json",
                    "captured-cli-only-6.1.0-rootful.cassette.json",
                    "captured-mixed-origin-6.1.0-rootful.cassette.json",
                ] {
                    assert!(!source.contains(forbidden), "production source imports {forbidden}");
                }
            }
        }
    }
    for (document, required) in [
        ("docs/decisions/README.md", "0016-captured-native-release-evidence.md"),
        ("docs/testing.md", "scripts/check-native-release-contract.sh"),
        ("docs/architecture.md", "test-only"),
        ("docs/api-stability.md", "capture manifest"),
        ("docs/project-structure.md", "fixtures/native-regressions"),
        ("docs/releasing.md", "check-native-release-contract.sh"),
        ("docs/public/compatibility/index.md", "captured native"),
    ] {
        assert!(
            fs::read_to_string(document)?.contains(required),
            "{document} must document {required}"
        );
    }
    Ok(())
}
#[test]
fn request_aware_complex_corpus_has_its_schema_tests_and_complete_version_context_matrix() {
    for file in [
        "docs/schemas/podman-lens-cassette-v1.schema.json",
        "tests/cassette_contract.rs",
        "tests/complex_corpus.rs",
        "fixtures/corpus/complex-5.4.0-rootless.cassette.json",
        "fixtures/corpus/complex-5.4.0-rootful.cassette.json",
        "fixtures/corpus/complex-5.5.0-rootless.cassette.json",
        "fixtures/corpus/complex-5.5.0-rootful.cassette.json",
        "fixtures/corpus/complex-5.6.0-rootless.cassette.json",
        "fixtures/corpus/complex-5.6.0-rootful.cassette.json",
        "fixtures/corpus/complex-5.7.0-rootless.cassette.json",
        "fixtures/corpus/complex-5.7.0-rootful.cassette.json",
        "fixtures/corpus/complex-5.8.6-rootless.cassette.json",
        "fixtures/corpus/complex-5.8.6-rootful.cassette.json",
        "fixtures/corpus/complex-6.0.0-rootless.cassette.json",
        "fixtures/corpus/complex-6.0.0-rootful.cassette.json",
        "fixtures/corpus/complex-6.1.0-rootless.cassette.json",
        "fixtures/corpus/complex-6.1.0-rootful.cassette.json",
    ] {
        assert!(Path::new(file).is_file(), "missing {file}");
    }
}

#[test]
fn release_controls_preserve_provenance_and_automatic_dispatch() -> Result<(), std::io::Error> {
    let release_preparation = fs::read_to_string(".github/workflows/release-plz.yml")?;
    for required in [
        "actions/workflows/release.yml/dispatches",
        "release-plz-",
        "permission-pull-requests: write",
        "paths:",
        "src/**",
    ] {
        assert!(
            release_preparation.contains(required),
            "release preparation is missing {required}"
        );
    }

    let release = fs::read_to_string(".github/workflows/release.yml")?;
    let ci = fs::read_to_string(".github/workflows/ci.yml")?;
    for required in [
        "cargo llvm-cov",
        "cargo-deny-action@",
        "bash scripts/check-public-api.sh",
        "lycheeverse/lychee-action@",
    ] {
        assert!(ci.contains(required), "CI workflow is missing {required}");
    }
    for required in [
        "actions/attest@",
        "crates-io-auth-action@",
        "Create or verify annotated release tag",
        "Create or replace draft GitHub release",
        "Publish immutable GitHub release",
    ] {
        assert!(release.contains(required), "release workflow is missing {required}");
    }

    Ok(())
}

#[test]
fn native_release_conformance_is_reusable_and_fail_closed() -> Result<(), std::io::Error> {
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    for required in [
        "validation_only",
        "release-metadata",
        "native-conformance",
        "needs.release-gate.result == 'success'",
        "needs: [deterministic, release-metadata, native-conformance, semver]",
    ] {
        assert!(release.contains(required), "release workflow is missing {required}");
    }
    for forbidden in [
        "  validate:\n",
        "podman-lens-disabled",
        "Retain this disabled historical",
    ] {
        assert!(
            !release.contains(forbidden),
            "release workflow retains dead validation configuration: {forbidden}"
        );
    }
    let (_, metadata_and_later) = release
        .split_once("\n  release-metadata:\n")
        .ok_or_else(|| policy_error("release metadata job boundary is missing"))?;
    let (metadata, _) = metadata_and_later
        .split_once("\n  native-conformance:\n")
        .ok_or_else(|| policy_error("native conformance job boundary is missing"))?;
    for required in [
        "ref: ${{ github.sha }}",
        "persist-credentials: false",
        "bash scripts/check-release-metadata.sh",
        "bash scripts/extract-release-notes.sh",
    ] {
        assert!(
            metadata.contains(required),
            "release metadata job is missing {required}"
        );
    }
    for forbidden in [
        "setup-node",
        "install-file-tools",
        "check-files.sh",
        "lychee-action",
        "cargo fmt",
        "cargo ci-",
        "cargo llvm-cov",
        "cargo-deny",
        "rustup component",
        "rustup toolchain",
    ] {
        assert!(
            !metadata.contains(forbidden),
            "release metadata must not repeat heavy validation: {forbidden}"
        );
    }
    Ok(())
}

fn assert_isolated_host_podman_inner_rootless_and_failure_evidence(native: &str) -> Result<(), std::io::Error> {
    for required in [
        "Install isolated host Podman launcher",
        "sudo apt-get install --yes --no-install-recommends podman",
        "CONTAINERS_CONF_OVERRIDE",
        "lock_type = \"file\"",
        "state_key: rf",
        "state_key: rl",
        "state_directory=\"/tmp/pl-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${state_key}\"",
        "[[ \"${#runroot_directory}\" -le 50 ]]",
        "host_podman=(sudo env \"CONTAINERS_CONF_OVERRIDE=${containers_conf}\" podman --root",
        "--runroot",
        "--tmpdir",
        "install -d -m 0770 \"${socket_directory}\"",
        "sudo chown \"${service_uid}:$(id --group)\" \"${socket_directory}\"",
        "runtime_args=(--device /dev/fuse --security-opt label=disable)",
        "runtime_args+=(--privileged)",
        "runtime_args+=(--security-opt apparmor=unconfined)",
        "--volume \"${socket_directory}:/podman-lens:Z\"",
        "podman info --format \"{{.Host.Security.Rootless}}\"",
        "sudo chmod 0600 \"${socket}\"",
        "sudo chown \"$(id --user):$(id --group)\" \"${socket}\"",
        "image rm --force",
        "sudo rm --recursive --force",
    ] {
        assert!(
            native.contains(required),
            "isolated host Podman workflow is missing {required}"
        );
    }
    let short_state = "state_directory=\"/tmp/pl-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${state_key}\"";
    assert_eq!(
        native.matches(short_state).count(),
        2,
        "setup and cleanup must independently derive the bounded state directory"
    );
    for validated in [
        "[[ \"${state_key}\" =~ ^(rf|rl)$ ]]",
        "[[ \"${GITHUB_RUN_ID}\" =~ ^[0-9]+$ ]]",
        "[[ \"${GITHUB_RUN_ATTEMPT}\" =~ ^[0-9]+$ ]]",
    ] {
        assert_eq!(
            native.matches(validated).count(),
            2,
            "setup and cleanup must independently enforce {validated}"
        );
    }
    let cleanup = native
        .split_once("      - name: Remove disposable native service, image, and state\n")
        .map(|(_, cleanup)| cleanup)
        .ok_or_else(|| policy_error("isolated native cleanup step is missing"))?;
    for forbidden in [
        "PODMAN_LENS_NATIVE_STATE_DIRECTORY",
        "PODMAN_LENS_NATIVE_ROOT_DIRECTORY",
        "PODMAN_LENS_NATIVE_RUNROOT_DIRECTORY",
        "PODMAN_LENS_NATIVE_TMP_DIRECTORY",
        "PODMAN_LENS_NATIVE_CONTAINERS_CONF",
        "PODMAN_LENS_NATIVE_SERVICE",
    ] {
        assert!(
            !cleanup.contains(forbidden),
            "cleanup removal targets must not trust {forbidden}"
        );
    }
    assert!(
        !native.contains("docker exec"),
        "native service must not use an outer Docker launcher"
    );
    assert!(
        !native.contains("docker rm"),
        "native cleanup must not use an outer Docker launcher"
    );
    for forbidden in ["docker pull", "docker run"] {
        assert!(
            !native.contains(forbidden),
            "native service must not retain an outer Docker command: {forbidden}"
        );
    }
    let rootless = native
        .split_once("else\n            runtime_args+=(--security-opt apparmor=unconfined)")
        .map(|(_, after)| after)
        .ok_or_else(|| policy_error("rootless host-Podman argument boundary missing"))?;
    assert!(
        !rootless.contains("runtime_args+=(--privileged)"),
        "rootless service must not be privileged"
    );
    for forbidden in [
        "seccomp=unconfined",
        "secrets.",
        "/run/podman/podman.sock",
        "/var/run/docker.sock",
    ] {
        assert!(
            !native.contains(forbidden),
            "isolated native service retains forbidden outer state: {forbidden}"
        );
    }
    assert_native_identity_and_failure_evidence(native)?;
    Ok(())
}

fn assert_native_identity_and_failure_evidence(native: &str) -> Result<(), std::io::Error> {
    let rootless_cell = native
        .split_once("          - id: podman-6.1-rootless\n")
        .and_then(|(_, rest)| rest.split_once("    steps:\n"))
        .map(|(cell, _)| cell)
        .ok_or_else(|| policy_error("rootless matrix cell boundary is missing"))?;
    for required in [
        "root_mode: rootless",
        "service_uid: 1000",
        "image: ghcr.io/strukturpiloten/podman-6.1-rootless:v",
        "@sha256:",
    ] {
        assert!(
            rootless_cell.contains(required),
            "rootless matrix identity is missing {required}"
        );
    }
    for required in [
        "root_mode=\"${{ matrix.root_mode }}\"",
        "[[ \"${root_mode}\" =~ ^(rootful|rootless)$ ]]",
        "if [[ \"${root_mode}\" == rootful ]]",
        "actual_service_uid=\"$(id -u)\"",
        "actual_rootless=\"$(podman info --format \"{{.Host.Security.Rootless}}\")\"",
        "expected_service_uid=$1",
        "expected_root_mode=$2",
        "test \"${actual_service_uid}\" = \"${expected_service_uid}\"",
        "test \"${actual_root_mode}\" = \"${expected_root_mode}\"",
        "printf \"%s\\n\" \"${actual_service_uid}\" > /podman-lens/service-uid.tmp",
        "mv -f /podman-lens/service-uid.tmp /podman-lens/service-uid",
        "printf \"%s\\n\" \"${actual_root_mode}\" > /podman-lens/root-mode.tmp",
        "mv -f /podman-lens/root-mode.tmp /podman-lens/root-mode",
        "IFS= read -r actual_service_uid < \"${socket_directory}/service-uid\"",
        "IFS= read -r actual_root_mode < \"${socket_directory}/root-mode\"",
        "[[ \"${actual_service_uid}\" =~ ^[0-9]+$ && \"${actual_service_uid}\" == \"${service_uid}\" ]]",
        "[[ \"${actual_root_mode}\" =~ ^(rootful|rootless)$ && \"${actual_root_mode}\" == \"${root_mode}\" ]]",
    ] {
        assert!(
            native.contains(required),
            "inner rootless identity verification is missing {required}"
        );
    }
    assert!(
        !native.contains("${host_podman[@]}\" exec") && !native.contains("${host_podman[@]} exec"),
        "host-side Podman exec must not enter the nested rootless service"
    );
    let evidence = native
        .split_once("      - name: Record current-run native evidence\n")
        .and_then(|(_, rest)| rest.split_once("      - name: Upload bounded native evidence\n"))
        .map(|(evidence, _)| evidence)
        .ok_or_else(|| policy_error("native evidence step boundary is missing"))?;
    for required in [
        "image='${{ matrix.image }}'",
        "expected_version=\"${tag##*:v}\"",
        "--arg expected_version \"${expected_version}\"",
        "--arg image \"${image}\"",
    ] {
        assert!(
            evidence.contains(required),
            "failure evidence must retain immutable image provenance: {required}"
        );
    }
    assert!(
        !evidence.contains("PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION"),
        "failure evidence must not depend on successful service setup"
    );
    Ok(())
}

const NATIVE_CONFORMANCE_REQUIRED: &[&str] = &[
    "workflow_call:",
    "workflow_dispatch:",
    "ref: ${{ github.sha }}",
    "podman system service",
    "PODMAN_LENS_CONFORMANCE_UNIX_SOCKET",
    "native_service_conformance",
    "podman-lens-native-api-${GITHUB_SHA}-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${{ matrix.id }}",
    "--arg run_attempt \"${GITHUB_RUN_ATTEMPT}\"",
    "overwrite: true",
    "steps.conformance.outcome != 'success'",
    "runtime_args+=(--privileged)",
    "service_uid: 0",
    "service_uid: 1000",
    "--device /dev/fuse",
    "--security-opt label=disable",
    "--security-opt apparmor=unconfined",
    "status=/podman-lens/setup.status",
    "trap \"code=\\$?; printf \\\"%s\\n\\\" \\\"\\${code}\\\" > \\\"\\${status}.tmp\\\"; mv -f \\\"\\${status}.tmp\\\" \\\"\\${status}\\\"\" EXIT",
    "mv -f \"${status}.tmp\" \"${status}\"",
    "printf \"0\\n\" > \"${status}.tmp\"",
    "setup_deadline=$((SECONDS + 300))",
    "while (( SECONDS < setup_deadline ))",
    "remaining_seconds=$((setup_deadline - SECONDS))",
    "timeout --foreground --signal=TERM --kill-after=1s \"${remaining_seconds}s\"",
    "inspect --format '{{.State.Running}}'",
    "logs --tail 50",
    "[[ -f \"${status_file}\" && ! -L \"${status_file}\" ]]",
    "if [[ ! -f \"${status_file}\" || -L \"${status_file}\" ]]",
    "status_bytes=\"$(stat --format=%s -- \"${status_file}\")\"",
    "[[ \"${status_bytes}\" =~ ^[0-9]+$ && \"${status_bytes}\" -le 4 ]]",
    "status_lines=\"$(wc -l < \"${status_file}\")\"",
    "[[ \"${status_lines}\" == 1 ]]",
    "[[ ! \"${setup_status}\" =~ ^[0-9]+$ || \"${setup_status}\" != 0 ]]",
    "podman pull --quiet \"${image}\" >/dev/null",
    "podman network create --label podman-lens.conformance=network podman-lens-native-network >/dev/null",
    "podman volume create --label podman-lens.conformance=volume podman-lens-native-volume >/dev/null",
    "podman secret create --label podman-lens.conformance=secret podman-lens-native-secret - >/dev/null",
    "podman pod create --infra=false --label podman-lens.conformance=pod podman-lens-native-pod >/dev/null",
    "--volume podman-lens-native-volume:/bounded:Z \"${image}\" >/dev/null",
    "for identity_file in service-uid root-mode",
    "[[ -f \"${path}\" && ! -L \"${path}\" ]]",
    "case \"${identity_file}\" in service-uid) max_bytes=11 ;; root-mode) max_bytes=9 ;; esac",
    "identity_bytes=\"$(stat --format=%s -- \"${path}\")\"",
    "[[ \"${identity_bytes}\" =~ ^[0-9]+$ && \"${identity_bytes}\" -le \"${max_bytes}\" ]]",
    "identity_lines=\"$(wc -l < \"${path}\")\"",
    "while ! test -d /podman-lens/start-api",
    "mkdir \"${socket_directory}/start-api\"",
    "for _ in {1..30}",
    "[[ -S \"${socket}\" && ! -L \"${socket}\" ]]",
    "Remove disposable native service, image, and state",
    "actions/upload-artifact@",
    "datasource=docker depName=ghcr.io/strukturpiloten/podman-6.1-rootful",
];

fn assert_reviewed_native_images(native: &str) -> Result<(), std::io::Error> {
    let images = native
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("image: "))
        .collect::<Vec<_>>();
    assert_eq!(images.len(), 2, "native matrix must declare two runtime images");
    let expected_image_names = BTreeSet::from([
        "ghcr.io/strukturpiloten/podman-6.1-rootful",
        "ghcr.io/strukturpiloten/podman-6.1-rootless",
    ]);
    let mut observed_image_names = BTreeSet::new();
    for image in images {
        let (name, version_and_digest) = image
            .split_once(":v")
            .ok_or_else(|| policy_error(format!("native image must have a v-prefixed tag: {image}")))?;
        assert!(
            expected_image_names.contains(name),
            "native image has an unreviewed repository: {name}"
        );
        assert!(
            observed_image_names.insert(name),
            "native image repository appears more than once: {name}"
        );
        let (version, digest) = version_and_digest
            .split_once("@sha256:")
            .ok_or_else(|| policy_error(format!("native image must have a SHA-256 digest: {image}")))?;
        let version = Version::parse(version)
            .map_err(|error| policy_error(format!("native image must use a semantic version ({version}): {error}")))?;
        assert!(
            version.pre.is_empty() && version.build.is_empty(),
            "native image must use a stable semantic version: {version}"
        );
        assert!(
            digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "native image must use a complete SHA-256 digest"
        );
    }
    assert_eq!(observed_image_names, expected_image_names);
    Ok(())
}

#[test]
fn native_release_worker_and_renovate_contract_are_complete() -> Result<(), std::io::Error> {
    let native = fs::read_to_string(".github/workflows/native-podman-conformance.yml")?;
    for &required in NATIVE_CONFORMANCE_REQUIRED {
        assert!(
            native.contains(required),
            "native conformance workflow is missing {required}"
        );
    }
    let identity = native
        .find("actual_service_uid=\"$(id -u)\"")
        .ok_or_else(|| policy_error("initial-process identity check is missing"))?;
    let pull = native
        .find("podman pull --quiet \"${image}\"")
        .ok_or_else(|| policy_error("native image pull is missing"))?;
    let provision = native
        .find("podman create --name podman-lens-native-container")
        .ok_or_else(|| policy_error("native resources must be provisioned"))?;
    let identity_publish = native
        .find("mv -f /podman-lens/root-mode.tmp /podman-lens/root-mode")
        .ok_or_else(|| policy_error("atomic identity publication is missing"))?;
    let setup_success = native
        .find("printf \"0\\n\" > \"${status}.tmp\"")
        .ok_or_else(|| policy_error("successful setup status is missing"))?;
    let inner_start_gate = native
        .find("while ! test -d /podman-lens/start-api")
        .ok_or_else(|| policy_error("native API wait gate is missing"))?;
    let api_service = native
        .find("exec podman system service")
        .ok_or_else(|| policy_error("native API service start is missing"))?;
    let host_status = native
        .find("IFS= read -r setup_status < \"${status_file}\"")
        .ok_or_else(|| policy_error("host setup-status validation is missing"))?;
    let status_guard = native
        .find("if [[ ! -f \"${status_file}\" || -L \"${status_file}\" ]]")
        .ok_or_else(|| policy_error("setup-status file guard is missing"))?;
    let status_size = native
        .find("status_bytes=\"$(stat --format=%s -- \"${status_file}\")\"")
        .ok_or_else(|| policy_error("setup-status byte bound is missing"))?;
    let status_lines = native
        .find("status_lines=\"$(wc -l < \"${status_file}\")\"")
        .ok_or_else(|| policy_error("setup-status line bound is missing"))?;
    let identity_guard = native
        .find("[[ -f \"${path}\" && ! -L \"${path}\" ]]")
        .ok_or_else(|| policy_error("identity-file guard is missing"))?;
    let identity_size = native
        .find("identity_bytes=\"$(stat --format=%s -- \"${path}\")\"")
        .ok_or_else(|| policy_error("identity-file byte bound is missing"))?;
    let identity_lines = native
        .find("identity_lines=\"$(wc -l < \"${path}\")\"")
        .ok_or_else(|| policy_error("identity-file line bound is missing"))?;
    let identity_read = native
        .find("IFS= read -r actual_service_uid < \"${socket_directory}/service-uid\"")
        .ok_or_else(|| policy_error("identity-file read is missing"))?;
    let identity_compare = native
        .find("[[ \"${actual_root_mode}\" =~ ^(rootful|rootless)$ && \"${actual_root_mode}\" == \"${root_mode}\" ]]")
        .ok_or_else(|| policy_error("host identity comparison is missing"))?;
    let start_api = native
        .find("mkdir \"${socket_directory}/start-api\"")
        .ok_or_else(|| policy_error("native API start handshake must be explicit"))?;
    assert!(
        identity < pull && pull < provision && provision < identity_publish && identity_publish < setup_success,
        "initial-process identity must precede provisioning and successful setup status"
    );
    assert!(
        setup_success < inner_start_gate && inner_start_gate < api_service,
        "successful provisioning must precede the gated API service"
    );
    assert!(
        host_status < identity_guard && identity_compare < start_api,
        "the host must validate setup status and exact identity before opening the API start gate"
    );
    assert!(
        status_guard < status_size && status_size < status_lines && status_lines < host_status,
        "the host must validate setup-status type and bounds before reading it"
    );
    assert!(
        identity_guard < identity_size
            && identity_size < identity_lines
            && identity_lines < identity_read
            && identity_read < identity_compare,
        "the host must validate identity-file type and bounds before reading it"
    );
    assert_isolated_host_podman_inner_rootless_and_failure_evidence(&native)?;
    assert_reviewed_native_images(&native)?;

    Ok(())
}

#[test]
fn native_runtime_renovate_and_publication_contract_are_complete() -> Result<(), std::io::Error> {
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    for required in [
        "podman-lens-native-api-${{ github.sha }}-${{ github.run_id }}-${{ github.run_attempt }}-podman-6.1-*",
        "\"${GITHUB_RUN_ID}\" \"${GITHUB_RUN_ATTEMPT}\"",
    ] {
        assert!(
            release.contains(required),
            "release must select and validate the current native evidence attempt: {required}"
        );
    }
    let renovate: Value = serde_json::from_str(&fs::read_to_string(".github/renovate.json")?)
        .map_err(|error| policy_error(format!("Renovate configuration must be valid JSON: {error}")))?;
    let managers = renovate["customManagers"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate customManagers must be an array"))?
        .iter()
        .filter(|manager| manager["description"] == "Update the native Podman release and immutable manifest together")
        .collect::<Vec<_>>();
    assert_eq!(managers.len(), 1, "native runtime image needs one Renovate owner");
    let manager = managers[0];
    assert!(manager["matchStrings"][0].as_str().is_some_and(|pattern| {
        pattern.contains("(?<currentValue>v") && pattern.contains("currentDigest>sha256:[a-f0-9]{64}")
    }));
    assert_eq!(manager["datasourceTemplate"], "docker");
    assert_eq!(manager["versioningTemplate"], "docker");
    let replacement = manager["autoReplaceStringTemplate"]
        .as_str()
        .ok_or_else(|| policy_error("native image manager needs a replacement template"))?;
    assert!(replacement.contains(":{{{newValue}}}@{{{newDigest}}}"));
    assert!(replacement.contains('\n'));
    assert!(!replacement.contains(r"\n"));
    let rendered = replacement
        .replace("{{{depName}}}", "ghcr.io/strukturpiloten/podman-6.1-rootless")
        .replace("{{{newValue}}}", "v6.1.99")
        .replace("{{{newDigest}}}", &format!("sha256:{}", "a".repeat(64)));
    assert_eq!(rendered.lines().count(), 2);
    assert!(rendered.lines().next().is_some_and(|line| {
        line == "# renovate: datasource=docker depName=ghcr.io/strukturpiloten/podman-6.1-rootless"
    }));
    assert!(rendered.lines().nth(1).is_some_and(|line| {
        line == format!(
            "            image: ghcr.io/strukturpiloten/podman-6.1-rootless:v6.1.99@sha256:{}",
            "a".repeat(64)
        )
    }));
    let review_rules = renovate["packageRules"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate packageRules must be an array"))?
        .iter()
        .filter(|rule| rule["description"] == "Require review for native conformance runtime pins")
        .collect::<Vec<_>>();
    assert_eq!(review_rules.len(), 1);
    assert_eq!(
        review_rules[0]["matchFileNames"],
        serde_json::json!([".github/workflows/native-podman-conformance.yml"])
    );
    assert_eq!(review_rules[0]["dependencyDashboardApproval"], true);
    assert_eq!(review_rules[0]["automerge"], false);
    assert!(
        !release.contains("expected_rootful") && !release.contains("expected_rootless"),
        "release validation must consume candidate-bound evidence instead of duplicating Renovate-owned image pins"
    );

    let (_, publish) = release
        .split_once("\n  publish:\n")
        .ok_or_else(|| policy_error("release publish job boundary is missing"))?;
    for forbidden in [
        "cargo fmt",
        "cargo ci-check",
        "cargo ci-clippy",
        "cargo ci-test",
        "check-native-release-contract",
        "native_service_conformance",
        "podman system service",
        "cargo llvm-cov",
        "cargo-deny",
        "rustup component",
        "rustup toolchain",
    ] {
        assert!(
            !publish.contains(forbidden),
            "credentialed publish job must not rerun {forbidden}"
        );
    }
    Ok(())
}

#[test]
fn hosted_documentation_job_exposes_locked_node_tools() -> Result<(), std::io::Error> {
    let workflow = fs::read_to_string(".github/workflows/ci.yml")?;
    assert!(workflow.contains("${GITHUB_WORKSPACE}/node_modules/.bin"));
    assert!(workflow.contains("${GITHUB_PATH}"));
    Ok(())
}

#[test]
fn public_api_compatibility_uses_the_repository_owned_release_policy() -> Result<(), std::io::Error> {
    let workflow = fs::read_to_string(".github/workflows/ci.yml")?;
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    for required in ["fetch-depth: 0", "bash scripts/check-public-api.sh"] {
        assert!(workflow.contains(required), "CI API job is missing {required}");
        assert!(release.contains(required), "release API job is missing {required}");
    }

    let script = fs::read_to_string("scripts/check-public-api.sh")?;
    for required in [
        "git tag --merged HEAD",
        "BREAKING CHANGE:",
        "--release-type major",
        "semver-checks check-release",
    ] {
        assert!(script.contains(required), "public API script is missing {required}");
    }
    assert!(
        !script.contains("mapfile"),
        "public API script must support the macOS Bash version"
    );
    Ok(())
}

#[test]
fn renovate_keeps_base_image_releases_and_digests_together() -> Result<(), std::io::Error> {
    let configuration = fs::read_to_string(".github/renovate.json")?;
    assert!(configuration.contains("currentDigest"));
    assert!(configuration.contains("Dev Container base release and digest together"));
    Ok(())
}

#[test]
fn renovate_automerge_is_green_gated_with_manual_exceptions() -> Result<(), std::io::Error> {
    let configuration = fs::read_to_string(".github/renovate.json")?;
    let renovate: Value = serde_json::from_str(&configuration)
        .map_err(|error| policy_error(format!("Renovate configuration must be valid JSON: {error}")))?;
    assert_eq!(
        renovate["minimumReleaseAge"], "3 days",
        "Renovate must retain the three-day minimum release age"
    );
    let package_rules = renovate["packageRules"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate packageRules must be an array"))?;
    let lock_maintenance = package_rules
        .iter()
        .find(|rule| rule["description"] == "Automerge green-gated lock-file maintenance")
        .ok_or_else(|| policy_error("Renovate lock-file maintenance rule is missing"))?;
    assert_eq!(
        lock_maintenance["matchUpdateTypes"],
        serde_json::json!(["lockFileMaintenance"])
    );
    assert_eq!(lock_maintenance["automerge"], true);
    assert_eq!(lock_maintenance["automergeType"], "pr");
    assert_eq!(lock_maintenance["platformAutomerge"], false);
    for required in [
        "Automerge tested non-major dependency updates",
        "Do not delay BoxFerry and Lens releases",
        r#""minimumReleaseAge": "0 days""#,
        r#""platformAutomerge": false"#,
        r#""boxferry-model""#,
        r#""compose-lens""#,
        r#""podman-lens""#,
        r#""quadlet-lens""#,
        "Hold the known Rust-1.85-incompatible yoke-derive release",
        r#""allowedVersions": "!/^(0\\.8\\.3)$/""#,
    ] {
        assert!(
            configuration.contains(required),
            "Renovate configuration is missing {required}"
        );
    }
    let yoke_rule = package_rules
        .iter()
        .find(|rule| rule["description"] == "Hold the known Rust-1.85-incompatible yoke-derive release")
        .ok_or_else(|| policy_error("Renovate yoke-derive compatibility rule is missing"))?;
    assert_eq!(yoke_rule["matchPackageNames"], serde_json::json!(["yoke-derive"]));
    assert_eq!(
        configuration.matches(r#""automerge": false"#).count(),
        3,
        "Dev Container features, native runtimes, and checksum-pinned tools must remain manual"
    );
    Ok(())
}

#[test]
fn lockfile_release_age_guard_is_immutable_and_complete() -> Result<(), std::io::Error> {
    let configuration = fs::read_to_string(".github/renovate.json")?;
    let renovate: Value = serde_json::from_str(&configuration)
        .map_err(|error| policy_error(format!("Renovate configuration must be valid JSON: {error}")))?;
    assert_eq!(renovate["minimumReleaseAge"], "3 days");
    let rules = renovate["packageRules"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate packageRules must be an array"))?;

    let (non_major_index, lock_index) = verify_automerge_rules(rules)?;
    verify_manual_rule(
        rules,
        "Require checksum review for downloaded file-quality tools",
        non_major_index,
        lock_index,
    )?;
    verify_manual_rule(
        rules,
        "Require manual review for Dev Container features",
        non_major_index,
        lock_index,
    )?;
    verify_shared_policy_manager(&renovate)?;
    verify_lockfile_guard_workflow()?;
    Ok(())
}

fn verify_automerge_rules(rules: &[Value]) -> Result<(usize, usize), std::io::Error> {
    let non_major_index = rules
        .iter()
        .position(|rule| rule["description"] == "Automerge tested non-major dependency updates")
        .ok_or_else(|| policy_error("generic non-major automerge rule is missing"))?;
    let non_major = &rules[non_major_index];
    assert_eq!(
        non_major["matchUpdateTypes"],
        serde_json::json!(["minor", "patch", "pin", "digest", "pinDigest"]),
        "generic automerge must cover exactly the safe non-major categories"
    );
    assert_eq!(non_major["automerge"], true);
    assert_eq!(non_major["automergeType"], "pr");
    assert_eq!(non_major["platformAutomerge"], false);

    let lock = rules
        .iter()
        .find(|rule| rule["description"] == "Automerge green-gated lock-file maintenance")
        .ok_or_else(|| policy_error("lock-file maintenance automerge rule is missing"))?;
    assert_eq!(
        rules
            .iter()
            .filter(|rule| rule["description"] == "Automerge green-gated lock-file maintenance")
            .count(),
        1,
        "lock-file maintenance needs exactly one rule"
    );
    let lock_index = rules
        .iter()
        .position(|rule| rule["description"] == "Automerge green-gated lock-file maintenance")
        .ok_or_else(|| policy_error("lock-file maintenance automerge rule was found above"))?;
    assert_eq!(lock["matchUpdateTypes"], serde_json::json!(["lockFileMaintenance"]));
    assert_eq!(lock["automerge"], true);
    assert_eq!(lock["automergeType"], "pr");
    assert_eq!(lock["platformAutomerge"], false);
    assert_eq!(
        lock["minimumReleaseAge"], "0 days",
        "the shared guard, rather than Renovate synthetic metadata, owns lockfile age evidence"
    );

    Ok((non_major_index, lock_index))
}

fn verify_manual_rule(
    rules: &[Value],
    description: &str,
    non_major_index: usize,
    lock_index: usize,
) -> Result<(), std::io::Error> {
    let (index, rule) = rules
        .iter()
        .enumerate()
        .find(|(_, rule)| rule["description"] == description)
        .ok_or_else(|| policy_error(format!("manual Renovate rule is missing: {description}")))?;
    assert!(
        index > non_major_index && index > lock_index,
        "manual rule {description} must follow both automerge rules"
    );
    assert_eq!(rule["automerge"], false, "manual rule {description} must opt out");
    Ok(())
}

fn verify_shared_policy_manager(renovate: &Value) -> Result<(), std::io::Error> {
    let rules = renovate["packageRules"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate packageRules must be an array"))?;
    let non_major_index = rules
        .iter()
        .position(|rule| rule["description"] == "Automerge tested non-major dependency updates")
        .ok_or_else(|| policy_error("generic non-major automerge rule is missing"))?;
    let lock_index = rules
        .iter()
        .position(|rule| rule["description"] == "Automerge green-gated lock-file maintenance")
        .ok_or_else(|| policy_error("lock-file maintenance automerge rule is missing"))?;
    let devcontainer = renovate["packageRules"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate packageRules must be an array"))?
        .iter()
        .enumerate()
        .find(|(_, rule)| rule["description"] == "Require manual review for Dev Container features")
        .ok_or_else(|| policy_error("Dev Container manual rule is missing"))?;
    assert!(
        devcontainer.0 > non_major_index && devcontainer.0 > lock_index,
        "Dev Container manual rule must follow both automerge rules"
    );
    assert_eq!(devcontainer.1["automerge"], false);

    let guard_managers = renovate["customManagers"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate customManagers must be an array"))?
        .iter()
        .filter(|manager| manager["description"] == "Track the immutable Strukturpiloten shared-policy commit")
        .collect::<Vec<_>>();
    assert_eq!(
        guard_managers.len(),
        1,
        "the shared guard pin needs exactly one Renovate owner"
    );
    let guard_manager = guard_managers
        .first()
        .ok_or_else(|| policy_error("the shared guard pin needs one Renovate owner"))?;
    assert_eq!(
        guard_manager["managerFilePatterns"],
        serde_json::json!([r"/^\.github/workflows/.*\.ya?ml$/"])
    );
    assert!(guard_manager["matchStrings"][0].as_str().is_some_and(|pattern| {
        pattern.contains("datasource=github-digest")
            && pattern.contains("currentValue>main")
            && pattern.contains("currentDigest>[a-f0-9]{40}")
    }));
    let manager_pattern = guard_manager["matchStrings"][0]
        .as_str()
        .ok_or_else(|| policy_error("shared-policy manager must include its regex"))?;
    let template = guard_manager["autoReplaceStringTemplate"]
        .as_str()
        .ok_or_else(|| policy_error("shared-policy manager must include its replacement template"))?;
    assert!(
        template.contains('\n'),
        "replacement template must contain a real newline"
    );
    assert!(
        !template.contains(r"\n"),
        "replacement template must not contain a literal backslash-n"
    );
    let expected_pattern = "(?<indentation>[ \\t]*)# renovate: datasource=github-digest depName=(?<depName>Strukturpiloten/\\.github) currentValue=(?<currentValue>main)\\n[ \\t]*ref:\\s*(?<currentDigest>[a-f0-9]{40})";
    assert_eq!(manager_pattern, expected_pattern);
    let new_digest = "b4a7d2e8f1c903b6a5d4e2f7182930c4b6d5e7f1";
    let rewritten = template
        .replace("{{{indentation}}}", "      ")
        .replace("{{{depName}}}", "Strukturpiloten/.github")
        .replace("{{{newValue}}}", "main")
        .replace("{{{newDigest}}}", new_digest);
    assert_eq!(
        reextract_shared_policy_marker(manager_pattern, &rewritten),
        Some(("Strukturpiloten/.github", "main", new_digest)),
        "the shared-policy regex must re-extract its rewritten adjacent marker and ref"
    );

    Ok(())
}

fn verify_lockfile_guard_workflow() -> Result<(), std::io::Error> {
    let workflow = fs::read_to_string(".github/workflows/ci.yml")?;
    let marker = "# renovate: datasource=github-digest depName=Strukturpiloten/.github currentValue=main";
    let lines = workflow.lines().collect::<Vec<_>>();
    let marker_line = lines
        .iter()
        .position(|line| line.trim() == marker)
        .ok_or_else(|| policy_error("CI shared-policy Renovate marker is missing"))?;
    let shared_ref = lines
        .get(marker_line + 1)
        .and_then(|line| line.trim().strip_prefix("ref: "))
        .ok_or_else(|| policy_error("CI shared-policy marker must be adjacent to its ref"))?;
    assert_eq!(workflow.matches(marker).count(), 1);
    assert_eq!(shared_ref.len(), 40);
    assert!(
        shared_ref
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "shared-policy ref must be a full lowercase commit SHA"
    );
    for required in [
        "lockfile-release-age:",
        "Lockfile release age",
        "fetch-depth: 0",
        "ref: ${{ github.event.pull_request.head.sha }}",
        "path: .github/strukturpiloten-shared",
        "--repository-root \"${GITHUB_WORKSPACE}\"",
        "--base \"${BASE_SHA}\"",
        "--head \"${HEAD_SHA}\"",
        "--minimum-age-hours 72",
        "if: github.event_name != 'pull_request'",
        "lockfile-release-age]",
        "success success success success success success success success",
    ] {
        assert!(workflow.contains(required), "CI lockfile guard is missing {required}");
    }
    let lock_job = workflow
        .split_once("\n  lockfile-release-age:\n")
        .and_then(|(_, remainder)| remainder.split_once("\n  pr-gate:\n"))
        .map(|(job, _)| job)
        .ok_or_else(|| policy_error("CI lockfile release-age job boundary is missing"))?;
    let before_steps = lock_job
        .split_once("\n    steps:\n")
        .map_or(lock_job, |(prefix, _)| prefix);
    assert!(
        !before_steps.lines().any(|line| line.trim_start().starts_with("if:")),
        "CI lockfile release-age job must not be skipped on main pushes"
    );
    Ok(())
}

fn policy_error(message: impl Into<String>) -> std::io::Error {
    std::io::Error::other(message.into())
}

#[test]
fn lockfile_maintenance_keeps_the_schema_test_graph_on_the_declared_msrv() -> Result<(), std::io::Error> {
    let manifest = fs::read_to_string("Cargo.toml")?;
    let lockfile = fs::read_to_string("Cargo.lock")?;

    assert!(
        manifest.contains("yoke-derive = \"=0.8.2\""),
        "the temporary direct constraint must prevent lock maintenance from selecting yoke-derive 0.8.3"
    );
    assert!(
        lockfile.contains("name = \"yoke-derive\"\nversion = \"0.8.2\""),
        "Cargo.lock must retain the Rust-1.85-compatible yoke-derive resolution"
    );
    assert!(
        !lockfile.contains("name = \"yoke-derive\"\nversion = \"0.8.3\""),
        "Cargo.lock must not select yoke-derive 0.8.3 until upstream restores Rust 1.85 compatibility"
    );

    Ok(())
}

#[test]
fn devcontainer_persists_private_github_cli_configuration() -> Result<(), std::io::Error> {
    let configuration = fs::read_to_string(".devcontainer/devcontainer.json")?;
    assert!(configuration.contains("GH_CONFIG_DIR"));
    assert!(configuration.contains("podman-lens-gh-${devcontainerId}"));

    let verifier = fs::read_to_string(".devcontainer/verify-tools.sh")?;
    assert!(verifier.contains("chmod 0700 \"${GH_CONFIG_DIR}\""));
    Ok(())
}

#[test]
fn agent_roles_are_explicit() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = fs::read_to_string(root.join(".codex/config.toml"))?;
    for required in [
        "model = \"gpt-6-astra\"",
        "model_reasoning_effort = \"high\"",
        "max_concurrent_threads_per_session = 3",
        "default_subagent_model = \"gpt-5.6-terra\"",
        "default_subagent_reasoning_effort = \"medium\"",
    ] {
        assert!(config.contains(required), "missing agent default: {required}");
    }
    for (role, model, effort, sandbox) in [
        ("implementation-worker", "gpt-5.6-terra", "high", "workspace-write"),
        ("specification-researcher", "gpt-5.6-terra", "high", "read-only"),
        ("reviewer", "gpt-5.6-sol", "high", "read-only"),
        ("verifier", "gpt-5.6-terra", "medium", "workspace-write"),
    ] {
        let text = fs::read_to_string(root.join(format!(".codex/agents/{role}.toml")))?;
        for (key, value) in [
            ("model", model),
            ("model_reasoning_effort", effort),
            ("sandbox_mode", sandbox),
        ] {
            assert!(
                text.contains(&format!("{key} = \"{value}\"")),
                "{role}: incorrect {key}"
            );
        }
    }
    let reviewer = fs::read_to_string(root.join(".codex/agents/reviewer.toml"))?;
    assert!(reviewer.contains("original user requirements"));
    assert!(reviewer.contains("independent expected results"));
    let verifier = fs::read_to_string(root.join(".codex/agents/verifier.toml"))?;
    assert!(verifier.contains("./scripts/check-all.sh --check"));
    assert!(verifier.contains("never run the default formatting gate"));
    let instructions = fs::read_to_string(root.join("AGENTS.md"))?;
    assert!(!instructions.contains("Sol") && !instructions.contains("Terra") && !instructions.contains("Astra"));
    Ok(())
}

// The full shell gate targets the Linux Dev Container, not the macOS portability lane.
// Keep configuration assertions above platform-independent.
#[cfg(target_os = "linux")]
#[test]
fn linux_gate_modes_and_failure_propagation_are_correct() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let result = std::process::Command::new("bash")
        .arg("scripts/test-check-all.sh")
        .current_dir(root)
        .output()?;
    assert!(
        result.status.success(),
        "gate mode regression failed:\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}
fn reextract_shared_policy_marker<'a>(
    manager_pattern: &str,
    candidate: &'a str,
) -> Option<(&'a str, &'a str, &'a str)> {
    let expected_pattern = "(?<indentation>[ \\t]*)# renovate: datasource=github-digest depName=(?<depName>Strukturpiloten/\\.github) currentValue=(?<currentValue>main)\\n[ \\t]*ref:\\s*(?<currentDigest>[a-f0-9]{40})";
    if manager_pattern != expected_pattern {
        return None;
    }
    let (marker_line, ref_line) = candidate.split_once('\n')?;
    let (indentation, marker) = marker_line.split_once('#')?;
    if !indentation.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
        return None;
    }
    let marker = marker.strip_prefix(" renovate: datasource=github-digest depName=")?;
    let (dep_name, current_value) = marker.split_once(" currentValue=")?;
    if dep_name != "Strukturpiloten/.github" || current_value != "main" {
        return None;
    }
    let ref_line = ref_line.strip_prefix(indentation)?.strip_prefix("ref: ")?;
    if ref_line.len() != 40
        || !ref_line
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return None;
    }
    Some((dep_name, current_value, ref_line))
}
