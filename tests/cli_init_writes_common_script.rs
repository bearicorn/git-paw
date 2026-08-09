//! Asserts `git paw init` installs `<repo>/.git-paw/scripts/_paw_common.sh` —
//! the shared preamble the bundled helpers source — and marks it executable.
//! Mirrors `cli_init_writes_broker_script` for the preamble (`core-init` /
//! "Init installs the shared shell preamble").

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

/// Scenario `Init installs _paw_common.sh in a fresh repo`: a repo with no
/// `.git-paw/` gets the preamble, opening with a bash shebang and carrying the
/// execute bits on Unix.
#[test]
#[serial]
fn init_writes_executable_common_script() {
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
    // Init reports creation of _paw_common.sh, like the other bundled scripts.
    assert!(
        stdout.contains("Created .git-paw/scripts/_paw_common.sh"),
        "init should report _paw_common.sh creation; stdout:\n{stdout}"
    );

    let common = tmp.path().join(".git-paw/scripts/_paw_common.sh");
    assert!(
        common.is_file(),
        "_paw_common.sh should exist at {common:?}"
    );

    // The first line is the shebang.
    let content = fs::read_to_string(&common).expect("read _paw_common.sh");
    let first = content.lines().next().unwrap_or("");
    assert!(
        first == "#!/usr/bin/env bash" || first == "#!/bin/bash",
        "_paw_common.sh first line should be a bash shebang; got: {first:?}"
    );

    // Executable bit is on (Unix only).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&common)
            .expect("stat _paw_common.sh")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "_paw_common.sh mode {mode:o} should have user/group/other execute bits"
        );
    }
}

/// Scenario `Init overwrites a stale _paw_common.sh`: a hand-edited local copy
/// is replaced by the bundled content and reported as updated.
#[test]
#[serial]
fn init_overwrites_existing_common_script() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    // Pre-populate _paw_common.sh with a marker that init MUST overwrite.
    let scripts_dir = tmp.path().join(".git-paw/scripts");
    fs::create_dir_all(&scripts_dir).expect("mkdir scripts");
    fs::write(
        scripts_dir.join("_paw_common.sh"),
        "# stale local content\n",
    )
    .expect("write stub");

    let output = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run git paw init");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success());
    assert!(
        stdout.contains("Updated .git-paw/scripts/_paw_common.sh"),
        "init should report _paw_common.sh was updated; stdout:\n{stdout}"
    );

    let content = fs::read_to_string(tmp.path().join(".git-paw/scripts/_paw_common.sh"))
        .expect("read _paw_common.sh");
    assert!(
        !content.contains("# stale local content"),
        "init should have overwritten the local stub"
    );
    let bundled = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/_paw_common.sh"),
    )
    .expect("read bundled _paw_common.sh");
    assert_eq!(
        content, bundled,
        "the deployed preamble should be the bundled content verbatim"
    );
}
