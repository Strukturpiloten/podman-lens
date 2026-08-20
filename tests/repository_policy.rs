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
        "docs/api-stability.md",
        "docs/dependency-policy.md",
        "docs/development-environment.md",
        "docs/releasing.md",
        "docs/testing.md",
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
        "cargo-semver-checks-action",
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
fn renovate_keeps_base_image_releases_and_digests_together() -> Result<(), std::io::Error> {
    let configuration = fs::read_to_string(".github/renovate.json")?;
    assert!(configuration.contains("currentDigest"));
    assert!(configuration.contains("Dev Container base release and digest together"));
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
