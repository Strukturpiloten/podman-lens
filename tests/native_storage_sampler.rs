//! Offline regression for transient overlay mount churn during exact-store sampling.

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command, time::SystemTime};

const LAYER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct SampleResult {
    succeeded: bool,
    stderr: String,
    evidence: String,
    calls: usize,
    state_cleaned_by_runner: bool,
}

fn simulate_build_sample(mode: &str) -> Result<SampleResult, Box<dyn std::error::Error>> {
    let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
    let temporary = std::env::temp_dir().join(format!("podman-lens-sampler-{}-{nonce}", std::process::id()));
    fs::create_dir(&temporary)?;
    let run_id = format!("98{}{}", std::process::id(), nonce % 10_000);
    let state = PathBuf::from(format!("/tmp/pl-{run_id}-1-rf"));
    assert!(!state.exists(), "sampler must not touch another run's store");
    let fake_df = temporary.join("df");
    fs::write(
        &fake_df,
        "#!/bin/sh\nprintf 'Filesystem 1024-blocks Used Available Capacity Mounted on\nfake 30000000 10000000 20000000 50%% /tmp\n'\n",
    )?;
    let fake_sleep = temporary.join("sleep");
    fs::write(&fake_sleep, "#!/bin/sh\nexec /bin/sleep 0.05\n")?;
    let fake_sudo = temporary.join("sudo");
    fs::write(
        &fake_sudo,
        r#"#!/bin/sh
operation=$1; shift
if [ "$operation" = env ]; then [ "$1" = LC_ALL=C ] || exit 64; shift; operation=$1; shift; fi
case "$operation" in
  du)
    [ "$1" = -skx ] || exit 64
    count=$(cat "$SAMPLER_COUNT"); count=$((count + 1)); printf '%s\n' "$count" > "$SAMPLER_COUNT"
    if [ "$count" -gt 1 ]; then
      case "$SAMPLER_MODE" in
        transient) printf '42\t%s\n' "$SAMPLER_STATE"; exit 0 ;;
        transient_overbudget) printf '9000000\t%s\n' "$SAMPLER_STATE"; exit 0 ;;
      esac
    fi
    case "$SAMPLER_MODE" in
      transient|transient_overbudget|persistent) printf 'du: fts_read failed: %s/root/overlay/%s/merged: No such file or directory\n' "$SAMPLER_STATE" "$SAMPLER_LAYER" >&2; printf '42\t%s\n' "$SAMPLER_STATE"; exit 1 ;;
      timeout) exit 124 ;;
      denied) printf 'du: fts_read failed: %s/root/overlay/%s/merged: Permission denied\n' "$SAMPLER_STATE" "$SAMPLER_LAYER" >&2; exit 1 ;;
      outside) printf 'du: fts_read failed: /tmp/unrelated/root/overlay/%s/merged: No such file or directory\n' "$SAMPLER_LAYER" >&2; exit 1 ;;
      malformed) printf 'du: fts_read failed: %s/root/overlay/not-a-layer/merged: No such file or directory\n' "$SAMPLER_STATE" >&2; exit 1 ;;
      *) exit 64 ;;
    esac ;;
  rm) for last do :; done; [ "$last" = "$SAMPLER_STATE" ] || exit 64; exec /usr/bin/rm "$@" ;;
  *) exit 64 ;;
esac
"#,
    )?;
    let fake_build = temporary.join("fake-build");
    fs::write(
        &fake_build,
        "#!/bin/sh\nmkdir -p \"$SAMPLER_STATE/root/overlay/$SAMPLER_LAYER/merged\"\nexec /bin/sleep 1\n",
    )?;
    for executable in [&fake_df, &fake_sleep, &fake_sudo, &fake_build] {
        fs::set_permissions(executable, fs::Permissions::from_mode(0o755))?;
    }
    let count = temporary.join("samples");
    fs::write(&count, "0\n")?;
    let output = temporary.join("github-output");
    fs::write(&output, "")?;
    let result = Command::new("bash")
        .arg("scripts/run-native-runtime-budgeted.sh")
        .args(["rf", "build", "--", "sh"])
        .arg(fake_build)
        .env("PATH", format!("{}:/usr/bin:/bin", temporary.display()))
        .env("GITHUB_RUN_ID", run_id)
        .env("GITHUB_RUN_ATTEMPT", "1")
        .env("GITHUB_OUTPUT", &output)
        .env("SAMPLER_STATE", &state)
        .env("SAMPLER_LAYER", LAYER)
        .env("SAMPLER_COUNT", &count)
        .env("SAMPLER_MODE", mode)
        .output()?;
    let calls = fs::read_to_string(count)?.trim().parse()?;
    let evidence = fs::read_to_string(output)?;
    let state_cleaned_by_runner = !state.exists();
    if !state_cleaned_by_runner {
        fs::remove_dir_all(&state)?;
    }
    fs::remove_dir_all(temporary)?;
    Ok(SampleResult {
        succeeded: result.status.success(),
        stderr: String::from_utf8(result.stderr)?,
        evidence,
        calls,
        state_cleaned_by_runner,
    })
}

#[test]
fn transient_exact_overlay_mount_retries_without_using_partial_total() -> Result<(), Box<dyn std::error::Error>> {
    let result = simulate_build_sample("transient")?;
    assert!(result.succeeded, "{}", result.stderr);
    assert!(result.calls >= 2);
    assert!(result.stderr.contains("retrying exact-store measurement"));
    assert!(result.evidence.contains("peak_state_kib=42\n"));
    assert!(
        !result.state_cleaned_by_runner,
        "successful build retains state for later phases"
    );
    Ok(())
}

#[test]
fn unrelated_or_unreadable_storage_remains_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    for mode in ["denied", "outside", "malformed", "timeout"] {
        let result = simulate_build_sample(mode)?;
        assert!(!result.succeeded, "accepted {mode}");
        assert_eq!(result.calls, 1, "{mode} must not be retried");
        assert!(
            result.state_cleaned_by_runner,
            "{mode} failure must clean its exact owned state"
        );
        assert!(
            result.stderr.contains("sampling failed"),
            "{mode} lacked a failure diagnostic"
        );
    }
    let persistent = simulate_build_sample("persistent")?;
    assert!(
        !persistent.succeeded,
        "repeated overlay churn cannot pass without a complete measurement"
    );
    assert_eq!(persistent.calls, 3, "transient retry count must be bounded");
    assert!(
        persistent.state_cleaned_by_runner,
        "retry exhaustion must clean owned state"
    );
    Ok(())
}

#[test]
fn retry_must_use_complete_over_budget_total() -> Result<(), Box<dyn std::error::Error>> {
    let result = simulate_build_sample("transient_overbudget")?;
    assert!(!result.succeeded, "complete over-budget measurement was accepted");
    assert!(result.calls >= 2, "transient error did not retry");
    assert!(result.stderr.contains("storage budget exceeded"));
    assert!(
        result.state_cleaned_by_runner,
        "budget failure must clean exact owned state"
    );
    Ok(())
}
