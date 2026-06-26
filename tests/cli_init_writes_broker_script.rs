//! Asserts `git paw init` installs `<repo>/.git-paw/scripts/broker.sh` and
//! marks it executable. Mirrors `cli_init_writes_sweep_script` for the
//! agent-side helper (`project-initialization` / "Init installs the
//! agent-broker helper script").

use std::fs;
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn init_git_repo(dir: &std::path::Path) {
    let st = StdCommand::new("git")
        .current_dir(dir)
        .args(["init", "-b", "main"])
        .status()
        .expect("git init");
    assert!(st.success());
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["config", "user.email", "test@test.com"])
        .status();
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["config", "user.name", "Test"])
        .status();
}

#[test]
#[serial]
fn init_writes_executable_broker_script() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    let output = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run git paw init");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success(),
        "git paw init should succeed; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // Init reports creation of broker.sh.
    assert!(
        stdout.contains("Created .git-paw/scripts/broker.sh"),
        "init should report broker.sh creation; stdout:\n{stdout}"
    );

    let broker = tmp.path().join(".git-paw/scripts/broker.sh");
    assert!(broker.is_file(), "broker.sh should exist at {broker:?}");

    // The first line is the shebang.
    let content = fs::read_to_string(&broker).expect("read broker.sh");
    let first = content.lines().next().unwrap_or("");
    assert!(
        first == "#!/usr/bin/env bash" || first == "#!/bin/bash",
        "broker.sh first line should be a bash shebang; got: {first:?}"
    );

    // Executable bit is on (Unix only).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&broker)
            .expect("stat broker.sh")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "broker.sh mode {mode:o} should have user/group/other execute bits"
        );
    }
}

#[test]
#[serial]
fn init_overwrites_existing_broker_script() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    // Pre-populate broker.sh with a marker that init MUST overwrite.
    let scripts_dir = tmp.path().join(".git-paw/scripts");
    fs::create_dir_all(&scripts_dir).expect("mkdir scripts");
    fs::write(scripts_dir.join("broker.sh"), "# stale local content\n").expect("write stub");

    let output = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run git paw init");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success());
    assert!(
        stdout.contains("Updated .git-paw/scripts/broker.sh"),
        "init should report broker.sh was updated; stdout:\n{stdout}"
    );

    let content =
        fs::read_to_string(tmp.path().join(".git-paw/scripts/broker.sh")).expect("read broker.sh");
    assert!(
        !content.contains("# stale local content"),
        "init should have overwritten the local stub"
    );
    assert!(
        content.lines().next().is_some_and(|l| l.starts_with("#!")),
        "broker.sh first line should be a shebang"
    );
}
