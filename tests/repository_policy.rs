//! Repository-policy regression tests.

use std::{fs, path::Path};

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
