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
        "/docs/schemas/podman-lens-deployment-v2.schema.json",
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
        "docs/schemas/podman-lens-deployment-v2.schema.json",
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
    for consumer in ["scripts/check-all.sh", "scripts/validation-policy.json"] {
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
        "shared-release-prepare.yml@",
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
        "shared-rust-ci.yml@",
        "shared-rust-dependencies.yml@",
        "shared-rust-semver.yml@",
        "lycheeverse/lychee-action@",
    ] {
        assert!(ci.contains(required), "CI workflow is missing {required}");
    }
    for required in [
        "actions/attest@",
        "./_automation/.github/actions/shared-crate-publish",
        "Create or verify annotated release tag",
        "Create or replace draft GitHub release",
        "Publish immutable GitHub release",
    ] {
        assert!(release.contains(required), "release workflow is missing {required}");
    }

    Ok(())
}

#[test]
fn release_metadata_context_is_explicit_for_each_validation_entry_point() -> Result<(), std::io::Error> {
    let local = fs::read_to_string("scripts/check-all.sh")?;
    assert!(local.contains("bash scripts/check-release-metadata.sh --development"));

    let ci = fs::read_to_string(".github/workflows/ci.yml")?;
    assert!(ci.split_whitespace().collect::<Vec<_>>().join(" ").contains(
        "RELEASE_METADATA_MODE: ${{ github.event_name == 'pull_request' && startsWith(github.head_ref, 'release-plz-') && '--release' || '--development' }}"
    ));
    assert!(ci.contains(
        "      - run: bash scripts/test-release-metadata.sh\n      - name: Check release metadata for the validation context"
    ));
    assert!(ci.contains("run: bash scripts/check-release-metadata.sh \"${RELEASE_METADATA_MODE}\""));

    let release = fs::read_to_string(".github/workflows/release.yml")?;
    assert!(release.contains("bash scripts/check-release-metadata.sh --release"));
    assert!(!release.contains("check-release-metadata.sh --development"));

    let validator = fs::read_to_string("scripts/check-release-metadata.sh")?;
    assert!(!validator.contains("git describe"));
    assert!(!validator.contains("latest_tag"));
    Ok(())
}

#[test]
fn native_release_conformance_is_reusable_and_fail_closed() -> Result<(), std::io::Error> {
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    let policy: Value = serde_json::from_str(&fs::read_to_string(".github/release-policy.json")?)?;
    assert_eq!(
        policy["required_jobs"],
        serde_json::json!([
            "deterministic",
            "release-metadata",
            "native-conformance",
            "semver",
            "release-gate"
        ])
    );
    assert_eq!(policy["publication_enabled"], true);
    assert!(policy["bootstrap_version"].is_null());
    assert!(release.contains("name: Read-only exact-candidate publication inventory"));
    assert!(
        fs::read_to_string(".github/workflows/native-podman-conformance.yml")?
            .contains("value: ${{ jobs.native-gate.outputs.candidate_sha }}")
    );
    for required in [
        "validation_only",
        "release-metadata",
        "native-conformance",
        "release_validation: true",
        "needs.shared-release-gate.result == 'success'",
        "needs: [deterministic, release-metadata, native-conformance, semver, release-gate]",
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
        "bash scripts/check-release-metadata.sh --release",
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

fn assert_native_cleanup_contract(native: &str) -> Result<(), std::io::Error> {
    let cleanup = native
        .split_once("      - name: Remove disposable native service, image, and state\n")
        .map(|(_, cleanup)| cleanup)
        .ok_or_else(|| policy_error("isolated native cleanup step is missing"))?;
    assert!(cleanup.contains("bash scripts/cleanup-native-runtime.sh \"${state_key}\""));
    let cleanup_script = fs::read_to_string("scripts/cleanup-native-runtime.sh")?;
    for required in [
        "state_directory=\"/tmp/pl-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${state_key}\"",
        "[[ \"${GITHUB_RUN_ID}\" =~ ^[0-9]+$ && \"${GITHUB_RUN_ATTEMPT}\" =~ ^[0-9]+$ ]]",
        "ps --all --external",
        "rm --force --volumes",
        "volume ls --format '{{.Name}}'",
        "volume rm --force",
        "unmount --all",
        "images --all",
        "remaining_volumes=",
        "remaining_mounts=",
        "Native cleanup incomplete; preserving task-owned state",
        "sudo du --summarize --human-readable -- \"${state_directory}\"",
        "sudo rm --recursive --force -- \"${state_directory}\"",
    ] {
        assert!(
            cleanup_script.contains(required),
            "native cleanup is missing {required}"
        );
    }
    let removal = cleanup_script
        .rfind("sudo rm --recursive --force")
        .ok_or_else(|| policy_error("native cleanup removal is missing"))?;
    let mount_check = cleanup_script
        .find("remaining_mounts=")
        .ok_or_else(|| policy_error("native cleanup mount check is missing"))?;
    let disk_inventory = cleanup_script
        .find("sudo du --summarize")
        .ok_or_else(|| policy_error("native cleanup disk inventory is missing"))?;
    assert!(mount_check < removal);
    assert!(disk_inventory < removal);
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
        "setup and monitored conformance teardown must independently derive the bounded state directory"
    );
    for validated in [
        "[[ \"${state_key}\" =~ ^(rf|rl)$ ]]",
        "[[ \"${GITHUB_RUN_ID}\" =~ ^[0-9]+$ ]]",
        "[[ \"${GITHUB_RUN_ATTEMPT}\" =~ ^[0-9]+$ ]]",
    ] {
        assert_eq!(native.matches(validated).count(), 1, "setup must enforce {validated}");
    }
    assert_native_cleanup_contract(native)?;
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
    for required in ["root_mode: rootless", "service_uid: 1000"] {
        assert!(
            rootless_cell.contains(required),
            "rootless matrix identity is missing {required}"
        );
    }
    for required in [
        "NATIVE_ROOT_MODE: ${{ matrix.root_mode }}",
        "root_mode=\"${NATIVE_ROOT_MODE}\"",
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
        "source scripts/native-runtime-pins.sh",
        "expected_version=\"${PODMAN_NATIVE_VERSION#v}\"",
        "--arg expected_version \"${expected_version}\"",
        "--arg image \"${image}\"",
        "NATIVE_IMAGE_ID: ${{ steps.runtime.outputs.image_id }}",
        "NATIVE_CLOSURE_SHA256: ${{ steps.runtime.outputs.closure_sha256 }}",
        "--arg image_id \"${NATIVE_IMAGE_ID}\"",
        "--arg closure_sha256 \"${NATIVE_CLOSURE_SHA256}\"",
        "--arg api_version \"${PODMAN_LENS_CONFORMANCE_API_VERSION:-}\"",
        "NATIVE_BUILD_OUTCOME: ${{ steps.runtime.outcome }}",
        "--arg build_outcome \"${NATIVE_BUILD_OUTCOME}\"",
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
    "release_validation:",
    "candidate_sha:",
    "needs: admit-candidate",
    "if: needs.admit-candidate.result == 'success'",
    "ref: ${{ needs.admit-candidate.outputs.candidate_sha }}",
    "ref: ${{ github.sha }}",
    "podman system service",
    "PODMAN_LENS_CONFORMANCE_UNIX_SOCKET",
    "native_service_conformance",
    "NATIVE_CELL_ID: ${{ matrix.id }}",
    "podman-lens-native-api-${GITHUB_SHA}-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${NATIVE_CELL_ID}",
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
    "Build pinned native runtime from fixed Fedora compose",
];

fn assert_native_timeout_budget(native: &str) -> Result<(), std::io::Error> {
    let native_worker = native
        .split_once("\n  native-api:\n")
        .map(|(_, worker)| worker)
        .ok_or_else(|| policy_error("native worker job is missing"))?;
    // The separate receipt gate follows native-api and has its own job budget.
    // Keep the nine native phase steps and cleanup reserve scoped to native-api.
    let (native_worker, native_gate) = native_worker
        .split_once("\n  native-gate:\n")
        .ok_or_else(|| policy_error("native receipt gate job boundary is missing"))?;
    let native_gate_header = native_gate
        .split_once("    steps:\n")
        .map(|(header, _)| header)
        .ok_or_else(|| policy_error("native receipt gate steps are missing"))?;
    assert!(
        native_gate_header.lines().any(|line| line == "    timeout-minutes: 5"),
        "native receipt gate must retain its five-minute job budget"
    );
    let job_header = native_worker
        .split_once("    steps:\n")
        .map(|(header, _)| header)
        .ok_or_else(|| policy_error("native worker steps are missing"))?;
    let job_minutes = job_header
        .lines()
        .find_map(|line| line.strip_prefix("    timeout-minutes: "))
        .ok_or_else(|| policy_error("native worker job timeout is missing"))?
        .parse::<u32>()
        .map_err(|error| policy_error(format!("invalid native job timeout: {error}")))?;
    let mut steps = std::collections::BTreeMap::new();
    for section in native_worker.split("      - name: ").skip(1) {
        let name = section.lines().next().unwrap_or_default();
        let minutes = section
            .lines()
            .find_map(|line| line.strip_prefix("        timeout-minutes: "))
            .ok_or_else(|| policy_error(format!("native step {name} has no timeout")))?
            .parse::<u32>()
            .map_err(|error| policy_error(format!("invalid native step {name} timeout: {error}")))?;
        assert!(steps.insert(name, minutes).is_none(), "duplicate native step {name}");
    }
    let cleanup = steps
        .remove("Remove disposable native service, image, and state")
        .ok_or_else(|| policy_error("native cleanup step is missing"))?;
    assert_eq!(steps.len(), 9, "all pre-cleanup native steps need bounded timeouts");
    let pre_cleanup: u32 = steps.values().sum();
    assert!(job_minutes <= 50, "native job must remain bounded");
    assert!(
        cleanup >= 5,
        "native cleanup needs five minutes even after a prior timeout"
    );
    assert!(
        pre_cleanup + cleanup + 2 <= job_minutes,
        "native phase maxima must leave cleanup plus two minutes of job reserve"
    );
    Ok(())
}

fn assert_reviewed_native_images(native: &str) -> Result<(), std::io::Error> {
    assert!(
        !native.contains("matrix.image"),
        "native matrix must use the built candidate image"
    );
    assert!(native.contains("bash scripts/build-native-runtime-budgeted.sh"));
    assert!(native.contains("steps.runtime.outputs.image"));
    assert!(native.contains("steps.runtime.outputs.peak_state_kib"));
    for phase in ["provision", "api", "conformance"] {
        assert!(
            native.contains(&format!(
                "scripts/run-native-runtime-budgeted.sh \"${{NATIVE_STATE_KEY}}\" {phase} --"
            )),
            "native {phase} phase must run under exact-store monitoring"
        );
    }
    for output in [
        "steps.provision.outputs.peak_state_kib",
        "steps.api_start.outputs.peak_state_kib",
        "steps.conformance.outputs.peak_state_kib",
    ] {
        assert!(native.contains(output), "native runtime peak is missing {output}");
    }
    assert!(native.contains("http://localhost/libpod/_ping"));
    let pins = fs::read_to_string("scripts/native-runtime-pins.sh")?;
    let version = pins
        .lines()
        .find_map(|line| line.strip_prefix("readonly PODMAN_NATIVE_VERSION=v"))
        .ok_or_else(|| policy_error("native Podman version pin is missing"))?;
    let parsed = Version::parse(version)
        .map_err(|error| policy_error(format!("native version must be stable semver: {error}")))?;
    assert!(parsed.pre.is_empty() && parsed.build.is_empty());
    for name in [
        "PODMAN_NATIVE_RPM_SHA256",
        "PODMAN_NATIVE_SOURCE_RPM_SHA256",
        "PODMAN_NATIVE_SOURCE_ARCHIVE_SHA256",
        "PODMAN_NATIVE_REPOMD_SHA256",
        "PODMAN_NATIVE_PRIMARY_SHA256",
    ] {
        let value = pins
            .lines()
            .find_map(|line| line.strip_prefix(&format!("readonly {name}=")))
            .ok_or_else(|| policy_error(format!("missing {name}")))?;
        assert!(value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    assert!(pins.contains("readonly PODMAN_NATIVE_BASE_IMAGE=registry.fedoraproject.org/fedora:45@sha256:"));
    assert!(pins.contains("readonly PODMAN_NATIVE_COMPOSE_URL=https://dl.fedoraproject.org/pub/fedora/linux/releases/test/45_Beta/Everything/x86_64/os"));
    let builder = fs::read_to_string("scripts/build-native-runtime.sh")?;
    for required in [
        "sha256sum --check --status",
        "PODMAN_NATIVE_REPOMD_SHA256",
        "PODMAN_NATIVE_PRIMARY_SHA256",
        "--pull=never",
        "--target \"${root_mode}\"",
        "repomd-after.xml",
        "native-package-closure.txt",
        "image_id=\"$(normalize_native_image_id \"${raw_image_id}\")\"",
    ] {
        assert!(builder.contains(required), "native image builder is missing {required}");
    }
    let budgeted_builder = fs::read_to_string("scripts/build-native-runtime-budgeted.sh")?;
    assert!(budgeted_builder.contains("run-native-runtime-budgeted.sh"));
    assert!(budgeted_builder.contains("timeout --signal=TERM --kill-after=30s 12m"));
    let watchdog = fs::read_to_string("scripts/run-native-runtime-budgeted.sh")?;
    for required in [
        "preflight_available_kib=$((10 * 1024 * 1024))",
        "available_floor_kib=$((2 * 1024 * 1024))",
        "state_max_kib=$((8 * 1024 * 1024))",
        "sample_native_storage",
        "sudo env LC_ALL=C du -skx -- \"${state_directory}\"",
        "setsid \"$@\"",
        "kill -TERM -- \"-${command_pid}\"",
        "trap finish EXIT",
        "cleanup-native-runtime.sh",
        "peak_state_kib=",
    ] {
        assert!(
            watchdog.contains(required),
            "native lifecycle budget is missing {required}"
        );
    }
    let containerfile = fs::read_to_string("containers/native-podman/Containerfile")?;
    assert!(
        fs::read_to_string("scripts/check-files.sh")?.contains("'containers/native-podman/Containerfile'"),
        "the active native Containerfile must enter Hadolint file checks"
    );
    for required in [
        "--disablerepo='*' --enablerepo=podman-lens-fixed",
        "grep -Fx \"5:${PODMAN_VERSION}-${PODMAN_RPM_RELEASE}.x86_64\"",
        "FROM runtime AS rootful",
        "FROM runtime AS rootless",
        "USER 1000",
    ] {
        assert!(
            containerfile.contains(required),
            "native Containerfile is missing {required}"
        );
    }
    Ok(())
}

fn assert_native_renovate_extraction(manager: &Value, marker: &str) -> Result<(), std::io::Error> {
    let pattern = manager["matchStrings"][0]
        .as_str()
        .ok_or_else(|| policy_error("native Renovate regex is missing"))?;
    let result = std::process::Command::new("grep")
        .args(["-Pzo", "--", pattern, "scripts/native-runtime-pins.sh"])
        .output()?;
    assert!(result.status.success(), "Renovate regex did not extract {marker}");
    assert_eq!(
        result.stdout.split(|&byte| byte == 0).count(),
        2,
        "Renovate regex must extract {marker} exactly once"
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains(marker));
    Ok(())
}

fn assert_native_admission_contract(native: &str) -> Result<(), std::io::Error> {
    let admission = native
        .split_once("\n  admit-candidate:\n")
        .and_then(|(_, after)| after.split_once("\n  native-api:\n"))
        .map(|(admission, _)| admission)
        .ok_or_else(|| policy_error("native candidate admission must precede the privileged matrix"))?;
    for required in [
        "permissions:\n      contents: read",
        "persist-credentials: false",
        "NATIVE_RELEASE_VALIDATION: ${{ inputs.release_validation == true }}",
        "NATIVE_CANDIDATE_SHA: ${{ inputs.candidate_sha }}",
        "ORIGINAL_ACTOR: ${{ github.actor }}",
        "RERUN_ACTOR: ${{ github.triggering_actor }}",
        "run: bash scripts/admit-native-candidate.sh",
    ] {
        assert!(admission.contains(required), "native admission is missing {required}");
    }
    assert!(!native.contains("pull_request_target:"));
    for &required in NATIVE_CONFORMANCE_REQUIRED {
        assert!(
            native.contains(required),
            "native conformance workflow is missing {required}"
        );
    }
    for run_block in native.split("        run: |\n").skip(1) {
        let script = run_block
            .lines()
            .take_while(|line| line.starts_with("          ") || line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !script.contains("${{"),
            "native workflow must pass expressions through step env, not interpolate them into shell scripts"
        );
    }
    Ok(())
}

#[test]
fn native_release_worker_and_renovate_contract_are_complete() -> Result<(), std::io::Error> {
    let native = fs::read_to_string(".github/workflows/native-podman-conformance.yml")?;
    assert_native_admission_contract(&native)?;
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
    assert_native_timeout_budget(&native)?;

    Ok(())
}

#[test]
fn native_runtime_renovate_and_publication_contract_are_complete() -> Result<(), std::io::Error> {
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    let normalized = release.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "podman-lens-native-api-${{ github.sha }}-${{ github.run_id }}-${{ github.run_attempt }}-podman-6.1-*",
        "bash scripts/validate-native-evidence.sh \"${RUNNER_TEMP}/native-evidence\" \"${GITHUB_SHA}\" \"${GITHUB_RUN_ID}\" \"${GITHUB_RUN_ATTEMPT}\"",
    ] {
        assert!(
            normalized.contains(required),
            "release must select and validate the current native evidence attempt: {required}"
        );
    }
    let renovate: Value = serde_json::from_str(&fs::read_to_string(".github/renovate.json")?)
        .map_err(|error| policy_error(format!("Renovate configuration must be valid JSON: {error}")))?;
    let all_managers = renovate["customManagers"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate customManagers must be an array"))?;
    let managers = all_managers
        .iter()
        .filter(|manager| {
            manager["description"]
                == serde_json::json!(["Record the fixed native Podman version for qualification policy"])
        })
        .collect::<Vec<_>>();
    assert_eq!(managers.len(), 1, "native Podman patch needs one Renovate owner");
    let manager = managers[0];
    assert!(manager["matchStrings"][0].as_str().is_some_and(|pattern| {
        pattern.contains("containers/podman") && pattern.contains("PODMAN_NATIVE_VERSION=(?<currentValue>v")
    }));
    assert_eq!(manager["versioningTemplate"], "semver");
    assert_eq!(
        manager["managerFilePatterns"],
        serde_json::json!(["/^scripts/native-runtime-pins\\.sh$/"])
    );
    assert_native_renovate_extraction(manager, "PODMAN_NATIVE_VERSION=v")?;
    let base_managers = all_managers
        .iter()
        .filter(|manager| {
            manager["description"]
                == serde_json::json!(["Record the fixed native Fedora base tag and manifest for qualification policy"])
        })
        .collect::<Vec<_>>();
    assert_eq!(base_managers.len(), 1, "native Fedora base needs one Renovate owner");
    assert!(base_managers[0]["matchStrings"][0].as_str().is_some_and(|pattern| {
        pattern.contains("PODMAN_NATIVE_BASE_IMAGE") && pattern.contains("currentDigest>sha256:[a-f0-9]{64}")
    }));
    assert_native_renovate_extraction(base_managers[0], "PODMAN_NATIVE_BASE_IMAGE=")?;
    for marker in ["PODMAN_NATIVE_VERSION", "PODMAN_NATIVE_BASE_IMAGE"] {
        assert_eq!(
            all_managers
                .iter()
                .filter(|manager| manager["matchStrings"].to_string().contains(marker))
                .count(),
            1,
            "{marker} must have one Renovate owner"
        );
    }
    assert_native_qualification_tuple_is_fixed(&renovate)?;
    assert!(
        !release.contains("expected_rootful") && !release.contains("expected_rootless"),
        "release validation must consume candidate-bound evidence instead of duplicating Renovate-owned image pins"
    );

    assert_release_gate_checkout_and_evidence_order(&release)?;

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

fn assert_native_qualification_tuple_is_fixed(renovate: &Value) -> Result<(), std::io::Error> {
    let review_rules = renovate["packageRules"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate packageRules must be an array"))?
        .iter()
        .filter(|rule| {
            rule["description"] == serde_json::json!(["Keep the coupled Podman native qualification tuple fixed"])
        })
        .collect::<Vec<_>>();
    assert_eq!(review_rules.len(), 1);
    let fixed = review_rules[0];
    assert_eq!(fixed["matchManagers"], serde_json::json!(["custom.regex"]));
    assert_eq!(
        fixed["matchFileNames"],
        serde_json::json!(["scripts/native-runtime-pins.sh"])
    );
    assert_eq!(
        fixed["matchDatasources"],
        serde_json::json!(["github-releases", "docker"])
    );
    assert_eq!(
        fixed["matchPackageNames"],
        serde_json::json!(["containers/podman", "registry.fedoraproject.org/fedora"])
    );
    assert_eq!(fixed["enabled"], false);
    assert!(fixed["dependencyDashboardApproval"].is_null());
    assert!(fixed["automerge"].is_null());
    Ok(())
}

fn assert_release_gate_checkout_and_evidence_order(release: &str) -> Result<(), std::io::Error> {
    let (_, gate_and_later) = release
        .split_once("\n  release-gate:\n")
        .ok_or_else(|| policy_error("release validation gate boundary missing"))?;
    let (gate, _) = gate_and_later
        .split_once("\n  publish:\n")
        .ok_or_else(|| policy_error("release publish job boundary missing"))?;
    for required in [
        "- name: Check out exact candidate",
        "uses: actions/checkout@",
        "ref: ${{ github.sha }}",
        "fetch-depth: 0",
        "persist-credentials: false",
    ] {
        assert!(gate.contains(required), "release validation gate is missing {required}");
    }

    let ordered_steps = [
        "- name: Check out exact candidate",
        "- name: Download current-run native evidence",
        "- name: Validate complete current-run native evidence",
    ];
    let mut previous_position = None;
    for step in ordered_steps {
        let position = gate
            .find(step)
            .ok_or_else(|| policy_error(format!("release validation gate is missing {step}")))?;
        if let Some(previous_position) = previous_position {
            assert!(
                previous_position < position,
                "release validation gate step is out of fail-closed order: {step}"
            );
        }
        previous_position = Some(position);
    }

    Ok(())
}

#[test]
fn hosted_documentation_job_installs_file_tools_without_elevation() -> Result<(), std::io::Error> {
    let workflow = fs::read_to_string(".github/workflows/ci.yml")?;
    let documentation = workflow
        .split_once("\n  documentation:\n")
        .and_then(|(_, remainder)| remainder.split_once("\n  lockfile-release-age:\n"))
        .map(|(job, _)| job)
        .ok_or_else(|| policy_error("CI documentation job boundary missing"))?;
    for required in [
        "npm ci --ignore-scripts",
        "${GITHUB_WORKSPACE}/node_modules/.bin",
        "mktemp -d \"${RUNNER_TEMP}/podman-lens-file-tools.XXXXXXXX\"",
        "bash scripts/install-host-tools.sh \"${file_tool_bin}\"",
        "printf '%s\\n' \"${file_tool_bin}\" >> \"${GITHUB_PATH}\"",
        "bash scripts/check-files.sh --check",
    ] {
        assert!(
            documentation.contains(required),
            "CI documentation job is missing {required}"
        );
    }
    assert!(
        !documentation.contains("sudo "),
        "PR-controlled documentation steps must run without sudo"
    );
    Ok(())
}

#[test]
fn hosted_file_tools_delegate_to_the_pinned_shared_inventory() -> Result<(), std::io::Error> {
    let installer = fs::read_to_string("scripts/install-host-tools.sh")?;
    assert!(installer.contains("set -Eeuo pipefail"));
    assert!(installer.contains("shared-ci.py\" tools"));
    assert!(
        installer.contains(
            "--destination \"${1:-/usr/local/bin}\" --tools tombi shfmt shellcheck hadolint actionlint lychee"
        )
    );
    assert!(!installer.contains("_version="));
    assert!(!installer.contains("_checksum="));
    let legacy = fs::read_to_string("scripts/install-file-tools.sh")?;
    assert!(legacy.contains("install-host-tools.sh"));
    assert!(!legacy.contains("_version="));
    assert!(!legacy.contains("_checksum="));
    let container = fs::read_to_string(".devcontainer/Dockerfile")?;
    assert!(!container.contains("install-file-tools.sh"));
    assert!(container.contains("scripts/install-host-tools.sh"));
    assert!(container.contains("COPY scripts/shared-ci.py"));
    assert!(container.contains("bash /opt/podman-lens-tooling/scripts/install-host-tools.sh /usr/local/bin"));
    assert!(container.contains("--destination /usr/local/bin --python-tools"));
    for name in ["ci.yml", "native-podman-conformance.yml"] {
        let workflow = fs::read_to_string(format!(".github/workflows/{name}"))?;
        if name == "ci.yml" {
            assert!(workflow.contains("scripts/install-host-tools.sh"));
        }
        assert!(!workflow.contains("scripts/install-file-tools.sh"));
    }
    Ok(())
}

#[test]
fn public_api_compatibility_uses_the_repository_owned_release_policy() -> Result<(), std::io::Error> {
    let workflow = fs::read_to_string(".github/workflows/ci.yml")?;
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    for required in ["fetch-depth: 0", "shared-rust-semver.yml@", "api_purpose:"] {
        assert!(workflow.contains(required), "CI API job is missing {required}");
        assert!(release.contains(required), "release API job is missing {required}");
    }

    assert!(workflow.contains("git -C source log -1 --format=%s"));
    assert!(!workflow.contains("scripts/check-public-api.sh --release-type-only"));
    let script = fs::read_to_string("scripts/check-public-api.sh")?;
    for required in ["--task api-development", "--task api-published"] {
        assert!(script.contains(required), "public API script is missing {required}");
    }
    for obsolete in ["--release-type-only", "git tag", "git log", "BREAKING CHANGE:"] {
        assert!(
            !script.contains(obsolete),
            "public API wrapper retains obsolete {obsolete}"
        );
    }
    assert!(!release.contains("api-release-type:"));
    assert!(!release.contains("release_type:"));
    assert!(
        !script.contains("mapfile"),
        "public API script must support the macOS Bash version"
    );
    Ok(())
}

#[test]
fn renovate_keeps_base_image_releases_and_digests_together() -> Result<(), std::io::Error> {
    let configuration: Value = serde_json::from_str(&fs::read_to_string(".github/renovate.json")?)
        .map_err(|error| policy_error(format!("invalid Renovate JSON: {error}")))?;
    let managers = configuration["customManagers"]
        .as_array()
        .ok_or_else(|| policy_error("missing custom image owners"))?;
    let owners: Vec<_> = managers
        .iter()
        .filter(|manager| {
            manager["description"] == serde_json::json!(["Update the Dev Container base release and digest together"])
        })
        .collect();
    assert_eq!(owners.len(), 1, "image declarations need a single custom owner");
    assert_eq!(owners[0]["versioningTemplate"], "docker");
    assert_eq!(
        owners[0]["autoReplaceStringTemplate"],
        "# renovate: datasource={{{datasource}}} depName={{{depName}}}\n# release: {{{newValue}}}\nFROM {{{depName}}}:{{{newValue}}}@{{{newDigest}}}"
    );
    let dockerfile = fs::read_to_string(".devcontainer/Dockerfile")?;
    let lines: Vec<_> = dockerfile.lines().collect();
    let mut images = 0;
    for block in lines.windows(3) {
        if let Some(name) = block[0].strip_prefix("# renovate: datasource=docker depName=") {
            let tag = block[1]
                .strip_prefix("# release: ")
                .ok_or_else(|| policy_error("missing image release annotation"))?;
            assert!(!tag.is_empty());
            let prefix = format!("FROM {name}:{tag}@sha256:");
            let reference = block[2]
                .strip_prefix(&prefix)
                .ok_or_else(|| policy_error("retained image digest must match its annotated owner"))?;
            let digest = reference
                .split_ascii_whitespace()
                .next()
                .ok_or_else(|| policy_error("missing image digest"))?;
            assert_eq!(digest.len(), 64);
            assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
            images += 1;
        }
    }
    assert_eq!(images, 2);
    assert_eq!(images, lines.iter().filter(|line| line.starts_with("FROM ")).count());
    Ok(())
}

#[test]
fn shared_validation_preserves_explicit_repository_contracts() -> Result<(), Box<dyn std::error::Error>> {
    let workflow = fs::read_to_string(".github/workflows/ci.yml")?;
    let policy: Value = serde_json::from_str(&fs::read_to_string("scripts/validation-policy.json")?)?;
    assert_eq!(policy["schema"], 2);
    assert_eq!(policy["repository"], "Strukturpiloten/podman-lens");
    for required in [
        "merge_group:",
        "name: Required CI",
        "if: always()",
        "--tested-sha",
        "--policy-sha",
        "boxferry_ci gate",
        "shared-dependency-wait.yml@",
        "shared-devcontainer-validation.yml@",
        "--devcontainer-validation",
        "statuses: read",
    ] {
        assert!(workflow.contains(required), "shared CI contract omits {required}");
    }
    let python = fs::read_to_string("scripts/test-automation-contract.py")?;
    for required in [
        "test_image_qualification_preserves_original_jobs_and_local_tasks",
        "test_actual_maintenance_prose_reaches_pr_queue_and_local_narrow_plans",
        "test_real_narrow_gate_rejects_unexpected_outcomes_skips_and_candidate",
        "test_effective_renovate",
        "test_unique_native",
        "test_trusted_service",
        "test_all_shared_references",
    ] {
        assert!(python.contains(required), "consumer regression omits {required}");
    }
    assert!(fs::read_to_string("scripts/check-all.sh")?.contains("scripts/shared-ci.py run --task python-quality"));
    Ok(())
}

fn policy_error(message: impl Into<String>) -> std::io::Error {
    std::io::Error::other(message.into())
}

#[test]
fn compiler_policy_has_one_support_floor_and_metadata_driven_consumers() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = fs::read_to_string("Cargo.toml")?;
    let msrv = manifest
        .lines()
        .find_map(|line| {
            line.strip_prefix("rust-version = \"")
                .and_then(|value| value.strip_suffix('"'))
        })
        .ok_or_else(|| policy_error("workspace rust-version must be an explicit compiler version"))?;
    let minimum = Version::parse(msrv)?;
    assert!(minimum.pre.is_empty() && minimum.build.is_empty());
    assert!(manifest.contains("rust-version.workspace = true"));
    assert_eq!(fs::read_to_string("clippy.toml")?, format!("msrv = \"{msrv}\"\n"));
    assert!(fs::read_to_string("README.md")?.contains(&format!("requires Rust {msrv} or newer")));

    let toolchain = fs::read_to_string("rust-toolchain.toml")?;
    let compiler = toolchain
        .lines()
        .find_map(|line| {
            line.strip_prefix("channel = \"")
                .and_then(|value| value.strip_suffix('"'))
        })
        .ok_or_else(|| policy_error("development toolchain must pin an explicit compiler version"))?;
    let development = Version::parse(compiler)?;
    assert!(development.pre.is_empty() && development.build.is_empty());
    assert!(
        development >= minimum,
        "development compiler must support the declared floor"
    );

    let local = fs::read_to_string("scripts/check-all.sh")?;
    assert!(local.contains("cargo metadata --locked --no-deps --format-version 1"));
    assert!(local.contains("[.packages[].rust_version] | unique"));
    for alias in ["ci-check", "ci-policy"] {
        assert!(local.contains(&format!("cargo \"+${{msrv}}\" {alias}")));
    }
    assert!(fs::read_to_string(".github/workflows/release.yml")?.contains("uses: ./.github/workflows/ci.yml"));

    let renovate: Value = serde_json::from_str(&fs::read_to_string(".github/renovate.json")?)?;
    let presets = renovate["extends"]
        .as_array()
        .ok_or_else(|| policy_error("shared presets missing"))?;
    assert_eq!(presets.len(), 2);
    assert!(presets[1].as_str().is_some_and(|preset| preset.contains("/rust#")));
    assert!(renovate["customManagers"].as_array().is_some_and(|managers| {
        managers.iter().all(|manager| {
            !manager["matchStrings"].to_string().contains("rust-version")
                && !manager["matchStrings"].to_string().contains("rust-toolchain")
                && !manager["matchStrings"].to_string().contains("msrv")
        })
    }));
    Ok(())
}

#[test]
fn schema_test_dependencies_have_no_obsolete_compiler_restriction() -> Result<(), std::io::Error> {
    let manifest = fs::read_to_string("Cargo.toml")?;
    let renovate: Value = serde_json::from_str(&fs::read_to_string(".github/renovate.json")?)?;

    assert!(
        !manifest.lines().any(|line| line.starts_with("yoke-derive =")),
        "schema tests must use the ordinary transitive yoke-derive resolution"
    );
    let package_rules = renovate["packageRules"]
        .as_array()
        .ok_or_else(|| policy_error("Renovate packageRules must be an array"))?;
    assert!(
        !package_rules
            .iter()
            .any(|rule| rule["matchPackageNames"] == serde_json::json!(["yoke-derive"])),
        "Renovate must not retain the obsolete yoke-derive compiler restriction"
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
        "# Workspace defaults; keep primary-session overrides aligned with Sol/xhigh.",
        "model = \"gpt-6-sol\"",
        "model_reasoning_effort = \"xhigh\"",
        "This limit excludes the primary manager: eight subagents means nine agents in total.",
        "max_concurrent_threads_per_session = 8",
        "default_subagent_model = \"gpt-6.1-sol\"",
        "default_subagent_reasoning_effort = \"medium\"",
    ] {
        assert!(config.contains(required), "missing agent default: {required}");
    }
    for (role, model, effort, sandbox) in [
        ("implementation-worker", "gpt-6.1-sol", "high", "workspace-write"),
        ("specification-researcher", "gpt-6.1-sol", "high", "read-only"),
        ("reviewer", "gpt-6.1-sol", "high", "read-only"),
        ("presentation-worker", "gpt-6.1-terra", "medium", "workspace-write"),
        ("verifier", "gpt-6-luna", "high", "workspace-write"),
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
    let presentation = fs::read_to_string(root.join(".codex/agents/presentation-worker.toml"))?;
    for required in [
        "explicit file ownership",
        "Escalate architecture, native-format semantics, security, migrations",
        "Browser and application inspection is read-only",
        "Git or GitHub writes",
        "A Sol reviewer independently checks the original requirements",
    ] {
        assert!(
            presentation.contains(required),
            "missing presentation boundary: {required}"
        );
    }
    let reviewer = fs::read_to_string(root.join(".codex/agents/reviewer.toml"))?;
    assert!(reviewer.contains("original user requirements"));
    assert!(reviewer.contains("independent expected results"));
    let verifier = fs::read_to_string(root.join(".codex/agents/verifier.toml"))?;
    assert!(verifier.contains("./scripts/check-all.sh --check"));
    assert!(verifier.contains("never run the default formatting gate"));
    assert!(verifier.contains("Escalate difficult failure diagnosis to a gpt-6.1-sol agent"));
    let instructions = fs::read_to_string(root.join("AGENTS.md"))?;
    assert!(instructions.contains("`gpt-6-sol` with `xhigh` reasoning"));
    let policy = instructions.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "Default subagents use `gpt-6.1-sol` with `medium` reasoning",
        "review use `gpt-6.1-sol` with `high` reasoning",
        "check-only verification uses `gpt-6-luna` with `high` reasoning",
        "Task-specific subagents may use `gpt-6.1-sol`, `gpt-6.1-terra`, or `gpt-6-luna` with `medium`, `high`, or `xhigh` reasoning",
        "Use `gpt-6-astra` only with `xhigh` reasoning for particularly difficult architectural questions",
        "up to eight concurrent subagents plus the primary manager (nine agents in total)",
        "Eight is a ceiling, not a target or eight distinct roles",
        "subagents may use the same role for independent tasks",
        "Do not create nested agents to evade the limit",
        "`agents.max_concurrent_threads_per_session` excludes the primary manager",
        "at most one complete gate or heavy runtime suite",
    ] {
        assert!(policy.contains(required), "missing agent policy: {required}");
    }
    let testing = fs::read_to_string(root.join("docs/testing.md"))?;
    assert!(testing.contains("authority is in [`AGENTS.md`](../AGENTS.md)"));
    assert!(testing.contains("the primary owns final validation"));
    assert!(testing.contains("`./scripts/check-all.sh --check`"));
    Ok(())
}

#[test]
fn standing_github_authorization_stays_scoped_and_guarded() -> Result<(), Box<dyn std::error::Error>> {
    let instructions = fs::read_to_string("AGENTS.md")?;
    let (_, authorization) = instructions
        .split_once("## Workspace scope and standing GitHub authorization")
        .ok_or("missing standing authorization section")?;
    let (authorization, _) = authorization
        .split_once("## Agent roles and verification")
        .ok_or("missing agent roles section")?;
    let repositories = authorization
        .lines()
        .filter(|line| line.starts_with("- "))
        .collect::<Vec<_>>();
    assert_eq!(
        repositories,
        [
            "- `Strukturpiloten/boxferry`",
            "- `Strukturpiloten/compose-lens`",
            "- `Strukturpiloten/podman-lens`",
            "- `Strukturpiloten/quadlet-lens`",
            "- `Strukturpiloten/boxferry-website`",
            "- `Strukturpiloten/docker-lens`",
        ]
    );

    let policy = authorization.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "Do not work on or modify any repository outside this explicit allowlist",
        "A newly discovered checkout is not implicitly in scope",
        "For user-requested work within this scope",
        "merge verified task-related pull requests without asking for renewed approval",
        "does not authorize unrelated backlog work",
        "discussion-only proposals",
        "expansion of the requested product scope",
        "A later user instruction may narrow or revoke this permission",
        "read back the exact head commit",
        "ready, mergeable, independently reviewed",
        "every required check successful",
        "exact-head safeguard",
        "never bypass branch protection or use an administrator override",
        "does not authorize releases, publication, deployment operations",
        "merging release/publication/deployment pull requests",
        "Subagents remain within their assigned task and checkout and must not perform those writes",
    ] {
        assert!(policy.contains(required), "missing authorization boundary: {required}");
    }
    assert!(!instructions.contains("does not authorize a merge"));
    assert!(!instructions.contains("Merge only when the user explicitly authorizes"));
    assert!(!instructions.contains("default stopping point"));
    Ok(())
}

// The full shell gate targets the Linux Dev Container; macOS client compatibility is unverified.
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
