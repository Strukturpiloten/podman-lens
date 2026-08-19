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
