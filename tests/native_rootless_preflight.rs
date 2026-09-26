//! Offline privacy and output-bound checks for diagnostic-only inner rootless preflight.

use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::SystemTime};

#[test]
fn rootless_image_preserves_only_required_mapping_helper_capabilities() -> Result<(), Box<dyn std::error::Error>> {
    let containerfile = fs::read_to_string("containers/native-podman/Containerfile")?;
    let rootless_stage = containerfile
        .split_once("FROM runtime AS rootless\n")
        .map(|(_, stage)| stage)
        .ok_or("rootless image stage missing")?;
    for exact_capability in [
        "setcap cap_setuid=ep /usr/bin/newuidmap",
        "setcap cap_setgid=ep /usr/bin/newgidmap",
        "getcap -n -- /usr/bin/newuidmap | grep -Fx '/usr/bin/newuidmap cap_setuid=ep'",
        "getcap -n -- /usr/bin/newgidmap | grep -Fx '/usr/bin/newgidmap cap_setgid=ep'",
    ] {
        assert!(
            rootless_stage.contains(exact_capability),
            "rootless image lost exact helper check: {exact_capability}"
        );
    }
    assert!(rootless_stage.contains("USER 1000"));

    let builder = fs::read_to_string("scripts/build-native-runtime.sh")?;
    assert!(builder.contains("if [[ \"${root_mode}\" == rootless ]]; then"));
    assert!(builder.contains("[[ \"$(id -u)\" == 1000 ]]"));
    for exact_capability in [
        "getcap -n -- /usr/bin/newuidmap | grep -Fx \"/usr/bin/newuidmap cap_setuid=ep\"",
        "getcap -n -- /usr/bin/newgidmap | grep -Fx \"/usr/bin/newgidmap cap_setgid=ep\"",
    ] {
        assert!(
            builder.contains(exact_capability),
            "runtime image lost exact helper check: {exact_capability}"
        );
    }
    Ok(())
}

#[test]
fn rootless_preflight_reports_only_bounded_nonsecret_facts() -> Result<(), Box<dyn std::error::Error>> {
    let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("podman-lens-rootless-preflight-{}-{nonce}", std::process::id()));
    let proc_root = root.join("proc");
    let etc_root = root.join("etc");
    fs::create_dir_all(&proc_root)?;
    fs::create_dir_all(&etc_root)?;
    fs::write(proc_root.join("uid_map"), "0 1000 1\n1 100000 65536\n")?;
    fs::write(proc_root.join("gid_map"), "0 1000 1\n1 200000 65536\n")?;
    fs::write(
        proc_root.join("status"),
        "Name:\tprivate-user-name\nNoNewPrivs:\t1\nCapEff:\t0000000000000000\nCapBnd:\t00000000a80425fb\nSeccomp:\t2\nEnvironment:\tsecret-value\n",
    )?;
    let uid = String::from_utf8(Command::new("id").arg("-u").output()?.stdout)?;
    fs::write(
        etc_root.join("subuid"),
        format!("someone-else:900000:65536\n{}:100000:65536\n", uid.trim()),
    )?;
    fs::write(etc_root.join("subgid"), format!("{}:200000:65536\n", uid.trim()))?;
    let uid_helper = root.join("newuidmap");
    fs::write(&uid_helper, "")?;
    fs::set_permissions(&uid_helper, fs::Permissions::from_mode(0o755))?;
    let gid_helper = root.join("newgidmap");
    fs::write(&gid_helper, "")?;
    fs::set_permissions(&gid_helper, fs::Permissions::from_mode(0o755))?;
    let getcap = root.join("getcap");
    fs::write(
        &getcap,
        "#!/bin/sh\n[ \"$1\" = -n ] && [ \"$2\" = -- ] || exit 64\ncase \"$3\" in\n  */newuidmap) printf '%s cap_setuid=ep\\n' \"$3\" ;;\n  */newgidmap) printf '%s cap_setgid=ep\\n' \"$3\" ;;\n  *) exit 64 ;;\nesac\n",
    )?;
    fs::set_permissions(&getcap, fs::Permissions::from_mode(0o755))?;
    let output = Command::new("sh")
        .arg("scripts/native-rootless-preflight.sh")
        .env("NATIVE_PREFLIGHT_PROC_ROOT", proc_root)
        .env("NATIVE_PREFLIGHT_ETC_ROOT", etc_root)
        .env("NATIVE_PREFLIGHT_NEWUIDMAP", uid_helper)
        .env("NATIVE_PREFLIGHT_NEWGIDMAP", gid_helper)
        .env("NATIVE_PREFLIGHT_GETCAP", getcap)
        .output()?;
    fs::remove_dir_all(root)?;
    assert!(output.status.success());
    let report = String::from_utf8(output.stdout)?;
    assert!(report.len() < 2048, "preflight must not print unbounded input");
    for required in [
        "uid_map inside=0 outside=low-id-redacted length=1",
        "uid_map inside=1 outside=100000 length=65536",
        "gid_map inside=1 outside=200000 length=65536",
        "NoNewPrivs: 1",
        "CapEff: 0000000000000000",
        "CapBnd: 00000000a80425fb",
        "Seccomp: 2",
        "newuidmap owner-group-mode=",
        "newuidmap file-capability=expected",
        "newgidmap owner-group-mode=",
        "newgidmap file-capability=expected",
        "subuid start=100000 length=65536",
        "subgid start=200000 length=65536",
    ] {
        assert!(report.contains(required), "preflight omitted {required}");
    }
    for secret in ["private-user-name", "secret-value", "someone-else", "900000"] {
        assert!(!report.contains(secret), "preflight leaked unrelated input");
    }
    Ok(())
}

#[test]
fn unavailable_preflight_files_remain_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "podman-lens-rootless-preflight-missing-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root)?;
    let missing = root.join("missing");
    let output = Command::new("sh")
        .arg("scripts/native-rootless-preflight.sh")
        .env("NATIVE_PREFLIGHT_PROC_ROOT", &missing)
        .env("NATIVE_PREFLIGHT_ETC_ROOT", &missing)
        .env("NATIVE_PREFLIGHT_NEWUIDMAP", &missing)
        .env("NATIVE_PREFLIGHT_NEWGIDMAP", &missing)
        .env("NATIVE_PREFLIGHT_GETCAP", &missing)
        .output()?;
    fs::remove_dir_all(root)?;
    assert!(
        output.status.success(),
        "missing diagnostics must not decide compatibility"
    );
    let report = String::from_utf8(output.stdout)?;
    for unavailable in [
        "uid_map unavailable",
        "gid_map unavailable",
        "status unavailable",
        "newuidmap unavailable",
        "newgidmap unavailable",
        "subuid unavailable",
        "subgid unavailable",
    ] {
        assert!(report.contains(unavailable), "preflight omitted {unavailable}");
    }
    Ok(())
}

#[test]
fn helper_capability_facts_are_classified_without_raw_xattrs() -> Result<(), Box<dyn std::error::Error>> {
    let nonce = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!(
        "podman-lens-helper-capabilities-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root)?;
    let uidmap = root.join("newuidmap");
    let gidmap = root.join("newgidmap");
    fs::write(&uidmap, "")?;
    fs::write(&gidmap, "")?;
    let getcap = root.join("getcap");
    fs::write(
        &getcap,
        "#!/bin/sh\ncase \"$PREFLIGHT_TEST_CAP_MODE\" in\n none) exit 0 ;;\n other) printf '%s cap_secret_private=ep\\n' \"$3\" ;;\n fail) exit 1 ;;\nesac\n",
    )?;
    fs::set_permissions(&getcap, fs::Permissions::from_mode(0o755))?;
    for (mode, expected) in [("none", "none"), ("other", "other-redacted"), ("fail", "unavailable")] {
        let output = Command::new("sh")
            .arg("scripts/native-rootless-preflight.sh")
            .env("NATIVE_PREFLIGHT_PROC_ROOT", root.join("missing"))
            .env("NATIVE_PREFLIGHT_ETC_ROOT", root.join("missing"))
            .env("NATIVE_PREFLIGHT_NEWUIDMAP", &uidmap)
            .env("NATIVE_PREFLIGHT_NEWGIDMAP", &gidmap)
            .env("NATIVE_PREFLIGHT_GETCAP", &getcap)
            .env("PREFLIGHT_TEST_CAP_MODE", mode)
            .output()?;
        assert!(
            output.status.success(),
            "diagnostics must never decide launch privilege"
        );
        let report = String::from_utf8(output.stdout)?;
        assert!(report.len() < 2048);
        for label in ["newuidmap", "newgidmap"] {
            assert!(report.contains(&format!("{label} file-capability={expected}")));
        }
        assert!(!report.contains("cap_secret_private"));
        assert!(!report.contains(&root.to_string_lossy().to_string()));
    }
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn workflow_collects_rootless_diagnostics_before_the_failing_probe() -> Result<(), Box<dyn std::error::Error>> {
    let workflow = fs::read_to_string(".github/workflows/native-podman-conformance.yml")?;
    let scoped_payload = workflow
        .split_once("if [[ \"${root_mode}\" == rootless ]]; then\n")
        .and_then(|(_, after)| after.split_once("\n          fi\n"))
        .map(|(body, _)| body)
        .ok_or("rootless-only preflight payload guard is missing")?;
    for required in [
        "[[ -f scripts/native-rootless-preflight.sh && ! -L scripts/native-rootless-preflight.sh ]]",
        "[[ \"$(stat --format=%s -- scripts/native-rootless-preflight.sh)\" -le 8192 ]]",
        "preflight_script=\"$(< scripts/native-rootless-preflight.sh)\"",
        "[[ -n \"${preflight_script}\" ]]",
    ] {
        assert!(scoped_payload.contains(required), "rootless payload lacks {required}");
    }
    assert!(
        workflow.contains("preflight_script=''"),
        "rootful must not load the diagnostic payload"
    );
    let rootless_only = workflow
        .find("if test \"$2\" = rootless; then")
        .ok_or("rootless preflight guard is missing")?;
    let collection = workflow
        .find("/bin/sh -c \"$4\" >&2 || printf")
        .ok_or("diagnostic-only inner preflight call is missing")?;
    let probe = workflow
        .find("actual_rootless=\"$(podman info")
        .ok_or("native root-mode probe is missing")?;
    assert!(rootless_only < collection && collection < probe);
    assert!(workflow.contains("\"${fixture_image}\" \"${preflight_script}\""));
    Ok(())
}
