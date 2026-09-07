//! Repository-policy regression tests.

use std::{collections::BTreeSet, fmt::Write as _, fs, path::Path};

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
    for consumer in [
        "scripts/check-all.sh",
        ".github/workflows/ci.yml",
        ".github/workflows/release.yml",
    ] {
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
    for required in [
        "cargo llvm-cov",
        "cargo-deny-action@",
        "bash scripts/check-public-api.sh",
        "lycheeverse/lychee-action@",
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
    for required in [
        "Automerge tested non-major dependency updates",
        "Do not delay BoxFerry and Lens releases",
        r#""minimumReleaseAge": "0 days""#,
        r#""platformAutomerge": false"#,
        r#""boxferry-model""#,
        r#""compose-lens""#,
        r#""podman-lens""#,
        r#""quadlet-lens""#,
    ] {
        assert!(
            configuration.contains(required),
            "Renovate configuration is missing {required}"
        );
    }
    assert_eq!(
        configuration.matches(r#""automerge": false"#).count(),
        2,
        "Dev Container features and checksum-pinned tools must remain manual"
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
