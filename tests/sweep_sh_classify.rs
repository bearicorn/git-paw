//! Fixture tests for the `classify` subcommand of the bundled
//! `<repo>/.git-paw/scripts/sweep.sh`, verifying parity with the Rust
//! auto-approve classifier (`src/supervisor/auto_approve.rs`).
//!
//! Each test runs `git paw init` (which writes the helper), then pipes a
//! scripted pane capture into `sweep.sh classify` and asserts the printed
//! decision. Coverage mirrors the four §8.2 cases: a danger pattern escalates,
//! the rm -rf scratch exception approves, a worktree-confined `git commit`
//! pre-approves, and a non-live capture is a no-op.
//!
//! Maps to openspec/changes/auto-approve-classifier/tasks.md §8.

use std::fs;
use std::io::Write;
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn init_git_repo(dir: &std::path::Path) {
    for args in [
        &["init", "-b", "main"][..],
        &["config", "user.email", "test@test.com"][..],
        &["config", "user.name", "Test"][..],
    ] {
        let st = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("git");
        assert!(st.success());
    }
    fs::write(dir.join("README.md"), "# test").expect("write readme");
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["add", "."])
        .status();
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["commit", "-m", "initial"])
        .status();
}

struct Fixture {
    _tmp: TempDir,
    sweep: std::path::PathBuf,
    root: std::path::PathBuf,
}

fn setup() -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    let init_out = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("git paw init");
    assert!(init_out.status.success(), "git paw init must succeed");
    let sweep = tmp.path().join(".git-paw/scripts/sweep.sh");
    assert!(sweep.exists(), "init must write sweep.sh");
    let root = tmp.path().to_path_buf();
    Fixture {
        _tmp: tmp,
        sweep,
        root,
    }
}

fn classify(fx: &Fixture, capture: &str, root_arg: Option<&str>) -> String {
    let mut c = StdCommand::new("bash");
    c.arg(&fx.sweep).arg("classify");
    if let Some(r) = root_arg {
        c.arg(r);
    }
    let mut child = c
        .current_dir(&fx.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sweep.sh classify");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(capture.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
#[serial]
fn danger_pattern_escalates() {
    let fx = setup();
    let out = classify(
        &fx,
        "Bash command\n  git push --force origin main\nDo you want to proceed?\nEsc to cancel",
        None,
    );
    assert!(
        out.contains("escalate") && out.contains("danger"),
        "force-push must escalate, got: {out}"
    );
}

#[test]
#[serial]
fn scratch_rm_approves() {
    let fx = setup();
    let out = classify(
        &fx,
        "Bash command\n  rm -rf /tmp/paw-build-1\nDo you want to proceed?\nEsc to cancel",
        None,
    );
    assert!(
        out.contains("approve") && out.contains("scratch-rm"),
        "scratch delete must approve, got: {out}"
    );
}

#[test]
#[serial]
fn worktree_commit_approves() {
    let fx = setup();
    let root = fx.root.to_string_lossy().to_string();
    let out = classify(
        &fx,
        "Bash command\n  git commit -m \"feat: x\"\nDo you want to proceed?\nEsc to cancel",
        Some(&root),
    );
    assert!(
        out.contains("approve") && out.contains("worktree-git"),
        "worktree-confined commit must approve, got: {out}"
    );
}

#[test]
#[serial]
fn non_live_capture_is_noop() {
    let fx = setup();
    let out = classify(&fx, "I might run cargo test later\njust narration", None);
    assert!(
        out.contains("no-op") && out.contains("not live"),
        "non-live capture must be a no-op, got: {out}"
    );
}
