//! End-to-end regression test for GP-12: a repository with local commits and
//! no `origin/HEAD` (no remote at all) must not abort `git paw start` when an
//! existing branch triggers the rebase-onto-default step.
//!
//! Before the fix, `default_branch()` required `refs/remotes/origin/HEAD` and
//! aborted otherwise; `create_worktree` only reaches that resolver when the
//! target branch already exists locally (the "resume" / re-`start` case), so
//! a first launch of a brand-new branch worked while every subsequent launch
//! against an existing branch failed. This test pre-creates the branch (as a
//! real resume would find it) in a repo with no remote configured at all, so
//! the rebase path is exercised on the very first `git paw start` call.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("default-branch-fallback-{tag}-{pid}-{n}")
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

fn kill_session(name: &str) {
    let _ = StdCommand::new("tmux")
        .args(["kill-session", "-t", name])
        .status();
}

fn run_git(dir: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run git command");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A local-only repository (no remote, so no `refs/remotes/origin/HEAD`)
/// with a commit on `main` and a pre-existing `feat/example` branch — the
/// state a resumed session would find.
struct LocalOnlySandbox {
    _sandbox: TempDir,
    repo: PathBuf,
}

fn setup_local_only_sandbox(project: &str) -> LocalOnlySandbox {
    let sandbox = TempDir::new().expect("tempdir");
    let repo = sandbox.path().join(project);
    fs::create_dir_all(&repo).unwrap();

    run_git(&repo, &["init", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "test@test.com"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("a.txt"), "one\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);
    run_git(&repo, &["branch", "feat/example"]);

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).unwrap();
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"sh\"\n\n[clis.sh]\ncommand = \"sh\"\ndisplay_name = \"Shell\"\n",
    )
    .unwrap();

    LocalOnlySandbox {
        _sandbox: sandbox,
        repo,
    }
}

#[test]
#[serial]
fn start_rebases_an_existing_branch_without_origin_head() {
    if skip_if_no_tmux() {
        return;
    }

    let project = unique_project_name("existing");
    let sandbox = setup_local_only_sandbox(&project);

    let session_name = format!("paw-{project}");
    kill_session(&session_name);

    // `feat/example` already exists locally (no remote in this repo at all),
    // so `create_worktree` takes the rebase-onto-default path on this very
    // first `git paw start` call. Pre-fix this aborted with "git
    // symbolic-ref failed"; post-fix it falls back to the local `main`.
    let output = cmd()
        .current_dir(&sandbox.repo)
        .args(["start", "--cli", "sh", "--branches", "feat/example"])
        .output()
        .expect("run git paw start");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("git symbolic-ref failed") && !stderr.contains("BranchError"),
        "start must not abort resolving the default branch without origin/HEAD; stderr:\n{stderr}"
    );

    let parent = sandbox.repo.parent().unwrap();
    let wt_path = parent.join(format!("{project}-feat-example"));
    assert!(
        wt_path.exists(),
        "worktree must have been created at {}",
        wt_path.display()
    );

    kill_session(&session_name);
}
