//! Offline, executable admission checks for the hosted native conformance worker.

use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command, time::SystemTime};

const CANDIDATE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!("podman-lens-admission-{}-{nonce}", std::process::id()));
        fs::create_dir(&root)?;
        let git = root.join("git");
        fs::write(
            &git,
            "#!/bin/sh\ncase \"$1 $2\" in\n  'check-ref-format --branch') exit 0 ;;\n  'rev-parse HEAD') printf '%s\\n' \"$FAKE_CHECKOUT_SHA\" ;;\n  'ls-remote --exit-code')\n    [ \"${FAKE_REF_MISSING:-0}\" = 0 ] || exit 2\n    for last do :; done\n    printf '%s\\t%s\\n' \"$FAKE_REMOTE_SHA\" \"$last\" ;;\n  *) exit 99 ;;\nesac\n",
        )?;
        let gh = root.join("gh");
        fs::write(
            &gh,
            "#!/bin/sh\n[ \"${FAKE_GH_FAIL:-0}\" = 0 ] || exit 1\nfor last do :; done\ncase \"$last\" in\n  */original/permission) printf '{\"permission\":\"%s\",\"role_name\":\"%s\"}\\n' \"$FAKE_ORIGINAL_PERMISSION\" \"$FAKE_ORIGINAL_ROLE_NAME\" ;;\n  */rerunner/permission) printf '{\"permission\":\"%s\",\"role_name\":\"%s\"}\\n' \"$FAKE_RERUN_PERMISSION\" \"$FAKE_RERUN_ROLE_NAME\" ;;\n  *) exit 99 ;;\nesac\n",
        )?;
        for executable in [&git, &gh] {
            fs::set_permissions(executable, fs::Permissions::from_mode(0o755))?;
        }
        Ok(Self { root })
    }

    fn run(&self, overrides: &[(&str, &str)]) -> Result<bool, Box<dyn std::error::Error>> {
        let output = self.root.join("github-output");
        fs::write(&output, "")?;
        let mut command = Command::new("bash");
        command.arg("scripts/admit-native-candidate.sh");
        command.env("PATH", format!("{}:/usr/bin:/bin", self.root.display()));
        command.env("GITHUB_REPOSITORY", "Strukturpiloten/podman-lens");
        command.env("GITHUB_EVENT_NAME", "workflow_dispatch");
        command.env("GITHUB_REF", "refs/heads/TheRealBecks/issue98");
        command.env("GITHUB_SHA", CANDIDATE);
        command.env("GITHUB_OUTPUT", &output);
        command.env("NATIVE_DEFAULT_BRANCH", "main");
        command.env("NATIVE_RELEASE_VALIDATION", "false");
        command.env("NATIVE_CANDIDATE_SHA", CANDIDATE);
        command.env("ORIGINAL_ACTOR", "original");
        command.env("RERUN_ACTOR", "rerunner");
        command.env("FAKE_CHECKOUT_SHA", CANDIDATE);
        command.env("FAKE_REMOTE_SHA", CANDIDATE);
        command.env("FAKE_ORIGINAL_PERMISSION", "write");
        command.env("FAKE_ORIGINAL_ROLE_NAME", "write");
        command.env("FAKE_RERUN_PERMISSION", "admin");
        command.env("FAKE_RERUN_ROLE_NAME", "admin");
        for (name, value) in overrides {
            command.env(name, value);
        }
        let result = command.output()?;
        let admitted = result.status.success();
        assert_eq!(
            fs::read_to_string(output)?.is_empty(),
            !admitted,
            "only admitted candidates may publish a SHA"
        );
        Ok(admitted)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn manual_admission_requires_exact_current_reviewed_branch_and_both_actors() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    assert!(fixture.run(&[])?);
    for overrides in [
        vec![("NATIVE_CANDIDATE_SHA", OTHER_SHA)],
        vec![("NATIVE_CANDIDATE_SHA", "short")],
        vec![("FAKE_CHECKOUT_SHA", OTHER_SHA)],
        vec![("FAKE_REMOTE_SHA", OTHER_SHA)],
        vec![("FAKE_REF_MISSING", "1")],
        vec![("GITHUB_REF", "refs/tags/v6.1.2")],
        vec![("GITHUB_REF", "refs/heads/main")],
        vec![("GITHUB_REF", "refs/heads/unreviewed")],
        vec![("GITHUB_REPOSITORY", "someone/podman-lens")],
        vec![("GITHUB_EVENT_NAME", "pull_request_target")],
        vec![
            ("FAKE_ORIGINAL_PERMISSION", "read"),
            ("FAKE_ORIGINAL_ROLE_NAME", "read"),
        ],
        vec![("FAKE_RERUN_PERMISSION", "triage"), ("FAKE_RERUN_ROLE_NAME", "triage")],
        vec![("FAKE_GH_FAIL", "1")],
    ] {
        assert!(!fixture.run(&overrides)?, "must reject {overrides:?}");
    }
    assert!(fixture.run(&[
        ("FAKE_ORIGINAL_PERMISSION", "admin"),
        ("FAKE_RERUN_PERMISSION", "write")
    ])?);
    assert!(fixture.run(&[
        ("FAKE_ORIGINAL_PERMISSION", "read"),
        ("FAKE_ORIGINAL_ROLE_NAME", "maintain")
    ])?);
    Ok(())
}

#[test]
fn release_call_requires_current_default_branch_even_for_dispatch_event() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let release = [("NATIVE_RELEASE_VALIDATION", "true"), ("GITHUB_REF", "refs/heads/main")];
    assert!(fixture.run(&release)?);
    assert!(!fixture.run(&[("NATIVE_RELEASE_VALIDATION", "true")])?);
    assert!(!fixture.run(&[
        ("NATIVE_RELEASE_VALIDATION", "true"),
        ("GITHUB_REF", "refs/tags/v6.1.2")
    ])?);
    assert!(!fixture.run(&[
        ("NATIVE_RELEASE_VALIDATION", "true"),
        ("GITHUB_REF", "refs/heads/main"),
        ("FAKE_REMOTE_SHA", OTHER_SHA)
    ])?);
    assert!(!fixture.run(&[
        ("NATIVE_RELEASE_VALIDATION", "unknown"),
        ("GITHUB_REF", "refs/heads/main")
    ])?);
    Ok(())
}
