//! Asserts `git paw init` installs `<repo>/.git-paw/scripts/docs-fetch.sh` and
//! marks it executable. Mirrors `cli_init_writes_broker_script` for the
//! docs-fetch helper (`docs-fetch-skill` / "init installs and path-allowlists
//! the helper").

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
fn init_writes_executable_docs_fetch_script() {
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
    // Init reports creation of docs-fetch.sh.
    assert!(
        stdout.contains("Created .git-paw/scripts/docs-fetch.sh"),
        "init should report docs-fetch.sh creation; stdout:\n{stdout}"
    );

    let helper = tmp.path().join(".git-paw/scripts/docs-fetch.sh");
    assert!(helper.is_file(), "docs-fetch.sh should exist at {helper:?}");

    // The first line is the shebang.
    let content = fs::read_to_string(&helper).expect("read docs-fetch.sh");
    let first = content.lines().next().unwrap_or("");
    assert!(
        first == "#!/usr/bin/env bash" || first == "#!/bin/bash",
        "docs-fetch.sh first line should be a bash shebang; got: {first:?}"
    );

    // Executable bit is on (Unix only).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&helper)
            .expect("stat docs-fetch.sh")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "docs-fetch.sh mode {mode:o} should have user/group/other execute bits"
        );
    }
}

#[test]
#[serial]
fn init_overwrites_existing_docs_fetch_script() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    // Pre-populate docs-fetch.sh with a marker that init MUST overwrite.
    let scripts_dir = tmp.path().join(".git-paw/scripts");
    fs::create_dir_all(&scripts_dir).expect("mkdir scripts");
    fs::write(scripts_dir.join("docs-fetch.sh"), "# stale local content\n").expect("write stub");

    let output = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run git paw init");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success());
    assert!(
        stdout.contains("Updated .git-paw/scripts/docs-fetch.sh"),
        "init should report docs-fetch.sh was updated; stdout:\n{stdout}"
    );

    let content = fs::read_to_string(tmp.path().join(".git-paw/scripts/docs-fetch.sh"))
        .expect("read docs-fetch.sh");
    assert!(
        !content.contains("# stale local content"),
        "init should have overwritten the local stub"
    );
    assert!(
        content.lines().next().is_some_and(|l| l.starts_with("#!")),
        "docs-fetch.sh first line should be a shebang"
    );
}
