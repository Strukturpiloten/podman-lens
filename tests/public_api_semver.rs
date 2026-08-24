//! Behavioral contract tests for the repository-owned public API command.

#![cfg(unix)]

use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct TemporaryRepository {
    path: PathBuf,
}

impl TemporaryRepository {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let unique = format!(
            "podman-lens-public-api-semver-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        );
        let path = env::temp_dir().join(unique);
        fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for TemporaryRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run_git(repository: &Path, arguments: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new("git").args(arguments).current_dir(repository).status()?;
    assert!(status.success(), "git {} failed", arguments.join(" "));
    Ok(())
}

fn write_manifest(path: &Path, version: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(
        path.join("Cargo.toml"),
        format!("[workspace]\nresolver = \"3\"\n\n[workspace.package]\nversion = \"{version}\"\n"),
    )?;
    Ok(())
}

fn recorded_arguments(message: &str, current_version: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let repository = TemporaryRepository::new()?;
    let scripts = repository.path.join("scripts");
    fs::create_dir_all(&scripts)?;
    fs::copy("scripts/check-public-api.sh", scripts.join("check-public-api.sh"))?;
    write_manifest(&repository.path, "0.1.1")?;
    fs::write(repository.path.join("README.md"), "baseline\n")?;
    run_git(&repository.path, &["init", "--quiet"])?;
    run_git(&repository.path, &["config", "user.email", "tests@example.invalid"])?;
    run_git(&repository.path, &["config", "user.name", "PodmanLens tests"])?;
    run_git(&repository.path, &["add", "."])?;
    run_git(&repository.path, &["commit", "--quiet", "-m", "chore: baseline"])?;
    run_git(&repository.path, &["tag", "v0.1.1"])?;

    write_manifest(&repository.path, current_version)?;
    fs::write(repository.path.join("README.md"), "current\n")?;
    run_git(&repository.path, &["add", "."])?;
    run_git(&repository.path, &["commit", "--quiet", "-m", message])?;

    let capture = repository.path.join("arguments.txt");
    let cargo = repository.path.join("fake-cargo.sh");
    fs::write(
        &cargo,
        "#!/usr/bin/env bash\nprintf '%s\\n' \"$@\" > \"${PODMAN_LENS_SEMVER_CAPTURE:?}\"\n",
    )?;
    let mut permissions = fs::metadata(&cargo)?.permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&cargo, permissions)?;

    let status = Command::new("bash")
        .arg(scripts.join("check-public-api.sh"))
        .current_dir(&repository.path)
        .env("CARGO", cargo)
        .env("PODMAN_LENS_SEMVER_CAPTURE", &capture)
        .status()?;
    assert!(status.success(), "public API command failed");
    Ok(fs::read_to_string(capture)?.lines().map(str::to_owned).collect())
}

#[test]
fn breaking_markers_only_force_pre_one_minor_semantics_without_a_manifest_bump()
-> Result<(), Box<dyn std::error::Error>> {
    let breaking = recorded_arguments("feat!: replace the public selector", "0.1.1")?;
    assert_eq!(
        breaking,
        [
            "semver-checks",
            "check-release",
            "--package",
            "podman-lens",
            "--release-type",
            "major"
        ]
    );

    let bumped = recorded_arguments("feat!: replace the public selector", "0.2.0")?;
    assert_eq!(
        bumped,
        [
            "semver-checks",
            "check-release",
            "--package",
            "podman-lens",
            "--release-type",
            "major"
        ]
    );

    let compatible = recorded_arguments("feat: add an accessor", "0.1.1")?;
    assert_eq!(
        compatible,
        ["semver-checks", "check-release", "--package", "podman-lens"]
    );
    Ok(())
}
