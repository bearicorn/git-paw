//! Asserts `git paw init` installs `<repo>/.git-paw/scripts/sweep.sh` and
//! marks it executable. Maps to `agent-skills` /
//! "`git paw init` writes the sweep helper" scenarios (task 3.8).

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
fn init_writes_executable_sweep_script() {
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

    let sweep = tmp.path().join(".git-paw/scripts/sweep.sh");
    assert!(sweep.is_file(), "sweep.sh should exist at {sweep:?}");

    // The first line is the shebang.
    let content = fs::read_to_string(&sweep).expect("read sweep.sh");
    let first = content.lines().next().unwrap_or("");
    assert!(
        first == "#!/usr/bin/env bash" || first == "#!/bin/bash",
        "sweep.sh first line should be a bash shebang; got: {first:?}"
    );

    // Executable bit is on (Unix only).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&sweep)
            .expect("stat sweep.sh")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "sweep.sh mode {mode:o} should have user/group/other execute bits"
        );
    }
}

#[test]
#[serial]
fn init_creates_and_gitignores_repo_local_tmp() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    let output = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run git paw init");
    assert!(output.status.success(), "git paw init should succeed");

    // The repo-local scratch dir exists...
    assert!(
        tmp.path().join(".git-paw/tmp").is_dir(),
        "init should create the repo-local .git-paw/tmp/ scratch dir"
    );
    // ...and is gitignored so verify worktrees / self-test sessions are
    // never committed in the consuming repo.
    let gitignore = fs::read_to_string(tmp.path().join(".gitignore")).expect("read .gitignore");
    assert!(
        gitignore.contains(".git-paw/tmp/"),
        "init should add .git-paw/tmp/ to .gitignore; got:\n{gitignore}"
    );
}

#[test]
#[serial]
fn init_overwrites_existing_sweep_script() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    // Pre-populate sweep.sh with a marker that init MUST overwrite.
    let scripts_dir = tmp.path().join(".git-paw/scripts");
    fs::create_dir_all(&scripts_dir).expect("mkdir scripts");
    fs::write(scripts_dir.join("sweep.sh"), "# stale local content\n").expect("write stub");

    let output = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("run git paw init");
    assert!(output.status.success());

    let content =
        fs::read_to_string(tmp.path().join(".git-paw/scripts/sweep.sh")).expect("read sweep.sh");
    assert!(
        !content.contains("# stale local content"),
        "init should have overwritten the local stub"
    );
    assert!(
        content.lines().next().is_some_and(|l| l.starts_with("#!")),
        "sweep.sh first line should be a shebang"
    );
}
