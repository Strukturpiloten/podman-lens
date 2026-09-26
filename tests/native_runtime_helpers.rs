//! Offline tests for the exact-store native runtime watchdog and evidence helpers.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime},
};

fn normalize(raw: &str) -> Result<std::process::Output, std::io::Error> {
    let helper = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/native-runtime-helpers.sh");
    Command::new("bash")
        .arg("-c")
        .arg("source \"$1\"; normalize_native_image_id \"$2\"")
        .arg("bash")
        .arg(helper)
        .arg(raw)
        .output()
}

fn budget(available: &str, state: &str, minimum: &str, maximum: &str) -> Result<std::process::Output, std::io::Error> {
    let helper = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/native-runtime-helpers.sh");
    Command::new("bash")
        .arg("-c")
        .arg("source \"$1\"; native_storage_budget_allows \"$2\" \"$3\" \"$4\" \"$5\"")
        .arg("bash")
        .arg(helper)
        .args([available, state, minimum, maximum])
        .output()
}

#[test]
fn native_image_id_accepts_bare_podman_id_and_canonical_digest() -> Result<(), std::io::Error> {
    let bare = "a".repeat(64);
    for input in [bare.clone(), format!("sha256:{bare}")] {
        let output = normalize(&input)?;
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8_lossy(&output.stdout), format!("sha256:{bare}\n"));
    }
    Ok(())
}

#[test]
fn native_image_id_rejects_untrusted_or_malformed_values() -> Result<(), std::io::Error> {
    for input in [
        String::new(),
        "a".repeat(63),
        "A".repeat(64),
        format!("sha256:sha256:{}", "a".repeat(64)),
        format!("{}\nextra", "a".repeat(64)),
        format!("{}; touch /tmp/never-execute", "a".repeat(64)),
    ] {
        let output = normalize(&input)?;
        assert!(!output.status.success(), "accepted {input:?}");
        assert!(output.stdout.is_empty(), "invalid IDs must never enter evidence");
    }
    Ok(())
}

#[test]
fn native_storage_budget_rejects_both_boundaries_and_malformed_measurements() -> Result<(), std::io::Error> {
    assert!(budget("100", "20", "100", "20")?.status.success());
    for values in [
        ["99", "20", "100", "20"],
        ["100", "21", "100", "20"],
        ["unknown", "20", "100", "20"],
        ["100", "-1", "100", "20"],
    ] {
        assert!(!budget(values[0], values[1], values[2], values[3])?.status.success());
    }
    Ok(())
}

#[test]
fn native_build_preflight_fails_before_creating_a_store() -> Result<(), Box<dyn std::error::Error>> {
    let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
    let temporary = std::env::temp_dir().join(format!("podman-lens-budget-{}-{nonce}", std::process::id()));
    fs::create_dir(&temporary)?;
    let fake_df = temporary.join("df");
    fs::write(
        &fake_df,
        "#!/bin/sh\nprintf 'Filesystem 1024-blocks Used Available Capacity Mounted on\\nfake 100000 99999 1 99%% /tmp\\n'\n",
    )?;
    fs::set_permissions(&fake_df, fs::Permissions::from_mode(0o755))?;
    let output_file = temporary.join("github-output");
    fs::write(&output_file, "")?;
    let run_id = format!("98{}", std::process::id());
    let state = PathBuf::from(format!("/tmp/pl-{run_id}-1-rf"));
    assert!(!state.exists(), "test must not touch another task's native store");
    let path = format!("{}:{}", temporary.display(), std::env::var("PATH")?);
    let output = Command::new("bash")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/build-native-runtime-budgeted.sh"))
        .args(["rf", "rootful"])
        .env("PATH", path)
        .env("GITHUB_RUN_ID", &run_id)
        .env("GITHUB_RUN_ATTEMPT", "1")
        .env("GITHUB_OUTPUT", &output_file)
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Native preflight requires"));
    assert!(fs::read_to_string(&output_file)?.contains("peak_state_kib=0"));
    assert!(!state.exists(), "preflight failure must not create a Podman store");
    fs::remove_dir_all(temporary)?;
    Ok(())
}

struct FakeBuildResult {
    succeeded: bool,
    child_running: bool,
    state_remained: bool,
    stderr: String,
    output: String,
}

fn fake_build_failure(cancel: bool) -> Result<FakeBuildResult, Box<dyn std::error::Error>> {
    let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
    let temporary = std::env::temp_dir().join(format!("podman-lens-budget-phase-{}-{nonce}", std::process::id()));
    fs::create_dir(&temporary)?;
    let run_id = format!("98{}{}", std::process::id(), nonce % 10_000);
    let state = PathBuf::from(format!("/tmp/pl-{run_id}-1-rf"));
    assert!(!state.exists(), "test must not touch another task's native store");
    let fake_df = temporary.join("df");
    fs::write(
        &fake_df,
        "#!/bin/sh\ncount=$(cat \"$PODMAN_LENS_TEST_COUNTER\")\ncount=$((count + 1))\nprintf '%s\\n' \"$count\" > \"$PODMAN_LENS_TEST_COUNTER\"\navailable=20000000\nif [ \"$PODMAN_LENS_TEST_MODE\" = breach ] && [ \"$count\" -ge 3 ]; then available=1; fi\nprintf 'Filesystem 1024-blocks Used Available Capacity Mounted on\\nfake 30000000 10000000 %s 50%% /tmp\\n' \"$available\"\n",
    )?;
    let fake_sudo = temporary.join("sudo");
    fs::write(
        &fake_sudo,
        "#!/bin/sh\noperation=$1\nshift\nif [ \"$operation\" = env ]; then [ \"$1\" = LC_ALL=C ] || exit 64; shift; operation=$1; shift; fi\ncase \"$operation\" in\n  du) exec /usr/bin/du \"$@\" ;;\n  rm) for last do :; done; [ \"$last\" = \"$PODMAN_LENS_TEST_STATE\" ] || exit 64; exec /usr/bin/rm \"$@\" ;;\n  *) exit 64 ;;\nesac\n",
    )?;
    let command_script = temporary.join("fake-build");
    fs::write(
        &command_script,
        "#!/bin/sh\nmkdir -m 700 \"$PODMAN_LENS_TEST_STATE\"\nprintf '%s\\n' \"$$\" > \"$PODMAN_LENS_TEST_CHILD_PID\"\nexec sleep 30\n",
    )?;
    for script in [&fake_df, &fake_sudo, &command_script] {
        fs::set_permissions(script, fs::Permissions::from_mode(0o755))?;
    }
    let counter = temporary.join("df-count");
    fs::write(&counter, "0\n")?;
    let child_pid_file = temporary.join("child-pid");
    let github_output = temporary.join("github-output");
    fs::write(&github_output, "")?;
    let stderr_path = temporary.join("stderr");
    let stderr = fs::File::create(&stderr_path)?;
    let path = format!("{}:{}", temporary.display(), std::env::var("PATH")?);
    let mut command = Command::new("bash")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/run-native-runtime-budgeted.sh"))
        .args(["rf", "build", "--", "sh"])
        .arg(&command_script)
        .env("PATH", path)
        .env("GITHUB_RUN_ID", &run_id)
        .env("GITHUB_RUN_ATTEMPT", "1")
        .env("GITHUB_OUTPUT", &github_output)
        .env("PODMAN_LENS_TEST_MODE", if cancel { "cancel" } else { "breach" })
        .env("PODMAN_LENS_TEST_COUNTER", &counter)
        .env("PODMAN_LENS_TEST_STATE", &state)
        .env("PODMAN_LENS_TEST_CHILD_PID", &child_pid_file)
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()?;
    if cancel {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !child_pid_file.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(child_pid_file.exists(), "fake child did not start before cancellation");
        Command::new("kill")
            .arg("-TERM")
            .arg(command.id().to_string())
            .status()?;
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = command.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            command.kill()?;
            let _ = command.wait()?;
            return Err("budget watchdog did not terminate in fifteen seconds".into());
        }
        thread::sleep(Duration::from_millis(50));
    };
    let child_pid = fs::read_to_string(&child_pid_file)?;
    let child_state = Command::new("ps")
        .args(["-o", "stat=", "-p", child_pid.trim()])
        .output()?;
    let child_running =
        child_state.status.success() && !child_state.stdout.is_empty() && !child_state.stdout.starts_with(b"Z");
    if child_running {
        let _ = Command::new("kill").arg("-TERM").arg(child_pid.trim()).status();
    }
    let state_remained = state.exists();
    let stderr = fs::read_to_string(stderr_path)?;
    let output = fs::read_to_string(github_output)?;
    if state_remained {
        fs::remove_dir_all(&state)?;
    }
    fs::remove_dir_all(temporary)?;
    Ok(FakeBuildResult {
        succeeded: status.success(),
        child_running,
        state_remained,
        stderr,
        output,
    })
}

#[test]
fn mid_build_budget_breach_terminates_child_and_cleans_exact_store() -> Result<(), Box<dyn std::error::Error>> {
    let result = fake_build_failure(false)?;
    assert!(!result.succeeded);
    assert!(
        !result.child_running,
        "budget breach must terminate the child process group"
    );
    assert!(
        !result.state_remained,
        "budget breach must remove the exact test-owned store"
    );
    assert!(result.stderr.contains("Native build storage budget exceeded"));
    assert!(result.output.contains("peak_state_kib="));
    Ok(())
}

#[test]
fn cancellation_terminates_child_and_cleans_exact_store() -> Result<(), Box<dyn std::error::Error>> {
    let result = fake_build_failure(true)?;
    assert!(!result.succeeded);
    assert!(
        !result.child_running,
        "cancellation must terminate the child process group"
    );
    assert!(
        !result.state_remained,
        "cancellation must remove the exact test-owned store"
    );
    assert!(result.output.contains("peak_state_kib="));
    Ok(())
}
