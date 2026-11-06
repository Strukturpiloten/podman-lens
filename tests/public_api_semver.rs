//! Adapter delegation only; canonical helper tests own baseline and breaking-API semantics.
#![cfg(unix)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
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

struct AdapterRun {
    output: Output,
    arguments: Option<Vec<String>>,
}

fn recorded_delegation(
    message: &str,
    current_version: &str,
    arguments: &[&str],
    helper_status: i32,
) -> Result<AdapterRun, Box<dyn std::error::Error>> {
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

    // This stub proves adapter argv/cwd/status, not canonical engine execution or API success.
    fs::write(
        scripts.join("shared-ci.py"),
        "import json, os, pathlib, sys\n\
         pathlib.Path(os.environ['PODMAN_LENS_ADAPTER_CAPTURE']).write_text(\n\
         json.dumps({'argv': sys.argv[1:], 'cwd': os.getcwd()}))\n\
         raise SystemExit(int(os.environ['PODMAN_LENS_ADAPTER_STATUS']))\n",
    )?;
    let capture = repository.path.join("delegation.json");
    let output = Command::new("bash")
        .arg(scripts.join("check-public-api.sh"))
        .args(arguments)
        .current_dir(env::temp_dir())
        .env("CARGO", repository.path.join("unavailable-cargo"))
        .env("PODMAN_LENS_SEMVER_CHECK", "0")
        .env("BOXFERRY_SEMVER_RELEASE_TYPE", "major")
        .env("PODMAN_LENS_ADAPTER_CAPTURE", &capture)
        .env("PODMAN_LENS_ADAPTER_STATUS", helper_status.to_string())
        .output()?;
    let recorded = if capture.exists() {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(capture)?)?;
        assert_eq!(value["cwd"].as_str(), repository.path.to_str());
        Some(serde_json::from_value(value["argv"].clone())?)
    } else {
        None
    };
    Ok(AdapterRun {
        output,
        arguments: recorded,
    })
}

#[test]
fn historical_breaking_markers_and_manifest_bumps_do_not_select_adapter_allowances()
-> Result<(), Box<dyn std::error::Error>> {
    for (message, version) in [
        ("feat!: replace public selector", "0.1.1"),
        ("feat!: replace public selector", "0.2.0"),
        ("feat: add accessor", "0.1.1"),
    ] {
        let AdapterRun {
            output,
            arguments: recorded,
        } = recorded_delegation(message, version, &[], 0)?;
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(
            recorded,
            Some(vec!["run".into(), "--task".into(), "api-development".into()])
        );
        assert!(
            output.stdout.is_empty(),
            "adapter fixture must not manufacture API receipts"
        );
    }
    Ok(())
}

#[test]
fn explicit_purposes_delegate_and_preserve_helper_failures() -> Result<(), Box<dyn std::error::Error>> {
    for (arguments, task) in [
        (vec!["--development"], "api-development"),
        (vec!["--published"], "api-published"),
    ] {
        for status in [0, 23] {
            let AdapterRun {
                output,
                arguments: recorded,
            } = recorded_delegation("feat!: historical API break", "0.2.0", &arguments, status)?;
            assert_eq!(output.status.code(), Some(status));
            assert_eq!(recorded, Some(vec!["run".into(), "--task".into(), task.into()]));
            assert!(
                output.stdout.is_empty(),
                "adapter fixture must not manufacture API receipts"
            );
        }
    }
    Ok(())
}

#[test]
fn unsupported_purpose_and_allowance_arguments_stop_before_delegation() -> Result<(), Box<dyn std::error::Error>> {
    for arguments in [
        vec!["--unknown"],
        vec!["--release-type-only"],
        vec!["--development", "extra"],
        vec!["--published", "--release-type", "major"],
    ] {
        let AdapterRun {
            output,
            arguments: recorded,
        } = recorded_delegation("feat!: historical API break", "0.2.0", &arguments, 0)?;
        assert_eq!(output.status.code(), Some(2));
        assert!(
            recorded.is_none(),
            "invalid adapter input must never call the shared helper"
        );
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
    }
    Ok(())
}
