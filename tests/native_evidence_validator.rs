//! Executable regression contracts for current-run native Release evidence.

use std::{fs, path::Path, process::Command};

use serde_json::Value;
use sha2::{Digest, Sha256};

fn active_pin(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let pins = fs::read_to_string("scripts/native-runtime-pins.sh")?;
    pins.lines()
        .find_map(|line| line.strip_prefix(&format!("readonly {name}=")))
        .map(str::to_owned)
        .ok_or_else(|| format!("missing active native pin {name}").into())
}

fn invalid_evidence_cases() -> Vec<(&'static str, Value)> {
    vec![
        ("candidate", Value::String("b".repeat(40))),
        ("run_id", Value::String("previous-run".into())),
        ("run_attempt", Value::String("1".into())),
        ("cell", Value::String("podman-6.1-rootless".into())),
        ("root_mode", Value::String("rootless".into())),
        ("expected_version", Value::String("6.1.0".into())),
        ("api_version", Value::String("not-a-version".into())),
        ("image_id", Value::String("not-an-image-id".into())),
        ("base_image", Value::String("an-unreviewed-base".into())),
        ("rpm_sha256", Value::String("a".repeat(64))),
        ("source_revision", Value::String("a".repeat(40))),
        ("source_rpm_sha256", Value::String("a".repeat(64))),
        ("source_archive_sha256", Value::String("a".repeat(64))),
        ("repomd_sha256", Value::String("a".repeat(64))),
        ("primary_sha256", Value::String("a".repeat(64))),
        ("closure_sha256", Value::String("not-a-digest".into())),
        ("closure_sha256", Value::String("b".repeat(64))),
        ("peak_state_kib", Value::String("8388609".into())),
        ("peak_state_kib", Value::String("0".into())),
        ("build_peak_state_kib", Value::String("0".into())),
        ("runtime_peak_state_kib", Value::String("8388609".into())),
        ("peak_state_kib", Value::String("1048576".into())),
        ("packages", serde_json::json!(["unreviewed 0:1-1.x86_64"])),
        ("build_outcome", Value::String("failure".into())),
        ("outcome", Value::String("failure".into())),
    ]
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn package_closure_sha(packages: &[String]) -> String {
    // The evidence producer hashes one exact package line followed by LF per entry.
    lowercase_hex(&Sha256::digest(format!("{}\n", packages.join("\n")).as_bytes()))
}

fn assert_missing_attempt_message(
    root: &Path,
    candidate: &str,
    run: &str,
    attempt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("bash")
        .arg("scripts/validate-native-evidence.sh")
        .arg(root)
        .arg(candidate)
        .arg(run)
        .arg(attempt)
        .output()?;
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(message.contains("fresh full Release validation run"));
    assert!(message.contains("Rerunning failed jobs alone"));
    Ok(())
}

fn assert_malformed_packages(
    root: &Path,
    run_identity: (&str, &str, &str),
    version: &str,
    rpm_release: &str,
    evidence: &impl Fn(&str, &str) -> Value,
    write_evidence: &impl Fn(&str, &str, &Value) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (candidate, run, attempt) = run_identity;
    let exact_podman = format!("podman 5:{version}-{rpm_release}.x86_64");
    for (case, malformed) in [
        ("caret without snapshot", "passt 0:0^-2.fc45.x86_64"),
        ("non-pubkey none architecture", "passt 0:0-2.fc45.(none)"),
        ("pubkey suffix", "gpg-pubkey 0:4f50a611-6888bc98.(none)extra"),
        ("parenthesized version", "passt 0:0(foo)-2.fc45.x86_64"),
        ("leading whitespace", " passt 0:0-2.fc45.x86_64"),
        ("double separator", "passt  0:0-2.fc45.x86_64"),
        ("embedded tab", "passt 0:0-2.fc45.\tx86_64"),
        ("embedded newline", "passt 0:0-2.fc45.\nx86_64"),
        ("trailing newline", "passt 0:0-2.fc45.x86_64\n"),
        ("control byte", "passt 0:0-2.fc45.\u{0000}x86_64"),
    ] {
        let packages = vec![exact_podman.clone(), malformed.to_owned()];
        let mut invalid = evidence(attempt, "podman-6.1-rootful");
        invalid["closure_sha256"] = Value::String(package_closure_sha(&packages));
        invalid["packages"] = serde_json::json!(packages);
        write_evidence(attempt, "podman-6.1-rootful", &invalid)?;
        let output = Command::new("bash")
            .arg("scripts/validate-native-evidence.sh")
            .arg(root)
            .arg(candidate)
            .arg(run)
            .arg(attempt)
            .output()?;
        assert!(!output.status.success(), "malformed package case {case} must fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Invalid current-attempt native evidence"));
        assert!(!stderr.contains(malformed), "do not echo package text for {case}");
        write_evidence(attempt, "podman-6.1-rootful", &evidence(attempt, "podman-6.1-rootful"))?;
    }
    Ok(())
}

fn assert_stale_retry_isolated(
    root: &Path,
    attempt: &str,
    evidence: &impl Fn(&str, &str) -> Value,
    write_evidence: &impl Fn(&str, &str, &Value) -> Result<(), Box<dyn std::error::Error>>,
    validate: &impl Fn() -> Result<std::process::ExitStatus, std::io::Error>,
) -> Result<(), Box<dyn std::error::Error>> {
    // A stale directory cannot satisfy the current target, and a retry replacement must itself
    // be successful rather than inheriting an earlier successful artifact.
    fs::create_dir_all(root.join("podman-lens-native-api-old-sha-old-run-podman-6.1-rootful"))?;
    assert!(
        validate()?.success(),
        "stale retry evidence must not affect the current target"
    );
    let mut replaced = evidence(attempt, "podman-6.1-rootless");
    replaced["outcome"] = Value::String("cancelled".into());
    write_evidence(attempt, "podman-6.1-rootless", &replaced)?;
    assert!(
        !validate()?.success(),
        "a failed replacement of current retry evidence must fail"
    );
    Ok(())
}

#[test]
fn validator_rejects_missing_stale_and_failed_retry_evidence() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("podman-lens-native-evidence-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root)?;
    let candidate = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let run = "12345";
    let attempt = "2";
    let version_tag = active_pin("PODMAN_NATIVE_VERSION")?;
    let version = version_tag.trim_start_matches('v');
    let base_image = active_pin("PODMAN_NATIVE_BASE_IMAGE")?;
    let rpm_sha = active_pin("PODMAN_NATIVE_RPM_SHA256")?;
    let source_revision = active_pin("PODMAN_NATIVE_SOURCE_REVISION")?;
    let source_rpm_sha = active_pin("PODMAN_NATIVE_SOURCE_RPM_SHA256")?;
    let source_archive_sha = active_pin("PODMAN_NATIVE_SOURCE_ARCHIVE_SHA256")?;
    let repomd_sha = active_pin("PODMAN_NATIVE_REPOMD_SHA256")?;
    let primary_sha = active_pin("PODMAN_NATIVE_PRIMARY_SHA256")?;
    let rpm_release = active_pin("PODMAN_NATIVE_RPM_RELEASE")?;
    let write_evidence =
        |evidence_attempt: &str, cell: &str, value: &Value| -> Result<(), Box<dyn std::error::Error>> {
            let directory = root.join(format!(
                "podman-lens-native-api-{candidate}-{run}-{evidence_attempt}-{cell}"
            ));
            fs::create_dir_all(&directory)?;
            fs::write(
                directory.join("native-podman-evidence.json"),
                serde_json::to_vec(value)?,
            )?;
            Ok(())
        };
    let evidence = |evidence_attempt: &str, cell: &str| {
        let root_mode = if cell.ends_with("rootful") {
            "rootful"
        } else {
            "rootless"
        };
        let state_key = if root_mode == "rootful" { "rf" } else { "rl" };
        let packages = vec![
            format!("podman 5:{version}-{rpm_release}.x86_64"),
            "passt 0:0^20260728.gf8df3f1-2.fc45.x86_64".to_owned(),
            "gpg-pubkey 0:4f50a6114cd5c6976a7f1179655a4b02f577861e-6888bc98.(none)".to_owned(),
        ];
        let closure_sha = package_closure_sha(&packages);
        serde_json::json!({
            "candidate": candidate, "run_id": run, "run_attempt": evidence_attempt, "task": "native-api", "cell": cell,
            "root_mode": root_mode, "expected_version": version, "api_version": "6.1.0",
            "image": format!("localhost/podman-lens-native:{version_tag}-{state_key}"),
            "image_id": format!("sha256:{}", "a".repeat(64)),
            "base_image": base_image, "rpm_sha256": rpm_sha, "source_revision": source_revision,
            "source_rpm_sha256": source_rpm_sha, "source_archive_sha256": source_archive_sha,
            "repomd_sha256": repomd_sha,
            "primary_sha256": primary_sha, "closure_sha256": closure_sha, "packages": packages,
            "build_peak_state_kib": "1048576", "runtime_peak_state_kib": "2097152", "peak_state_kib": "2097152",
            "build_outcome": "success", "outcome": "success", "socket_scope": "isolated-disposable-service"
        })
    };
    let validate = || -> Result<std::process::ExitStatus, std::io::Error> {
        Command::new("bash")
            .arg("scripts/validate-native-evidence.sh")
            .arg(&root)
            .arg(candidate)
            .arg(run)
            .arg(attempt)
            .status()
    };

    assert!(!validate()?.success(), "missing evidence must fail");
    for cell in ["podman-6.1-rootful", "podman-6.1-rootless"] {
        write_evidence("1", cell, &evidence("1", cell))?;
    }
    assert!(
        !validate()?.success(),
        "evidence from an earlier attempt must not satisfy the current attempt"
    );
    assert_missing_attempt_message(&root, candidate, run, attempt)?;
    for cell in ["podman-6.1-rootful", "podman-6.1-rootless"] {
        write_evidence(attempt, cell, &evidence(attempt, cell))?;
    }
    assert!(validate()?.success(), "complete current evidence must pass");
    for (field, replacement) in invalid_evidence_cases() {
        let mut invalid = evidence(attempt, "podman-6.1-rootful");
        invalid[field] = replacement;
        write_evidence(attempt, "podman-6.1-rootful", &invalid)?;
        assert!(!validate()?.success(), "a stale or invalid {field} must fail");
        write_evidence(attempt, "podman-6.1-rootful", &evidence(attempt, "podman-6.1-rootful"))?;
    }
    assert_malformed_packages(
        &root,
        (candidate, run, attempt),
        version,
        &rpm_release,
        &evidence,
        &write_evidence,
    )?;
    assert_stale_retry_isolated(&root, attempt, &evidence, &write_evidence, &validate)?;
    fs::remove_dir_all(root)?;
    Ok(())
}
