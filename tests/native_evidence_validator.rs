//! Executable regression contracts for current-run native Release evidence.

use std::{fs, process::Command};

use serde_json::Value;

#[test]
fn validator_rejects_missing_stale_and_failed_retry_evidence() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("podman-lens-native-evidence-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root)?;
    let candidate = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let run = "12345";
    let attempt = "2";
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
        serde_json::json!({
            "candidate": candidate, "run_id": run, "run_attempt": evidence_attempt, "task": "native-api", "cell": cell,
            "root_mode": root_mode, "expected_version": "6.1.0",
            "image": format!("ghcr.io/strukturpiloten/{cell}:v6.1.0@sha256:{}", "a".repeat(64)),
            "outcome": "success", "socket_scope": "isolated-disposable-service"
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
    for cell in ["podman-6.1-rootful", "podman-6.1-rootless"] {
        write_evidence(attempt, cell, &evidence(attempt, cell))?;
    }
    assert!(validate()?.success(), "complete current evidence must pass");
    for (field, replacement) in [
        ("candidate", Value::String("b".repeat(40))),
        ("run_id", Value::String("previous-run".into())),
        ("run_attempt", Value::String("1".into())),
        ("cell", Value::String("podman-6.1-rootless".into())),
        ("root_mode", Value::String("rootless".into())),
        ("outcome", Value::String("failure".into())),
    ] {
        let mut invalid = evidence(attempt, "podman-6.1-rootful");
        invalid[field] = replacement;
        write_evidence(attempt, "podman-6.1-rootful", &invalid)?;
        assert!(!validate()?.success(), "a stale or invalid {field} must fail");
        write_evidence(attempt, "podman-6.1-rootful", &evidence(attempt, "podman-6.1-rootful"))?;
    }
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
    fs::remove_dir_all(root)?;
    Ok(())
}
