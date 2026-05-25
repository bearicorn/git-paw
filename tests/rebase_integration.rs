//! End-to-end rebase-on-start integration tests.
//!
//! Exercises the `--no-rebase` plumbing through the binary: the default
//! `git paw start` invocation rebases existing agent branches onto the
//! repository's default branch before opening their worktrees; passing
//! `--no-rebase` preserves the post-`worktree-resume-fix` v0.5.0 behaviour
//! (no rebase). The unit tests in `src/git.rs::tests` already cover the
//! `create_worktree` function directly; these tests verify that the CLI
//! flag actually reaches the function.

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
    format!("rebase-{tag}-{pid}-{n}")
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

fn capture_git(dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

struct RebaseSandbox {
    _sandbox: TempDir,
    repo: PathBuf,
}

/// Builds a repo with `origin/main` tracking and `feat/example` at the
/// initial commit. Caller advances main as needed.
fn setup_sandbox(project: &str) -> RebaseSandbox {
    let sandbox = TempDir::new().expect("tempdir");
    let bare = sandbox.path().join("bare.git");
    let repo = sandbox.path().join(project);
    fs::create_dir_all(&bare).unwrap();

    run_git(&bare, &["init", "--bare", "-b", "main"]);
    let status = StdCommand::new("git")
        .args([
            "clone",
            bare.to_str().unwrap(),
            repo.to_str().unwrap(),
            "--origin",
            "origin",
        ])
        .status()
        .expect("git clone");
    assert!(status.success());

    run_git(&repo, &["config", "user.email", "test@test.com"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    run_git(&repo, &["checkout", "-b", "main"]);
    fs::write(repo.join("a.txt"), "one\n").unwrap();
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-m", "init"]);
    run_git(&repo, &["push", "-u", "origin", "main"]);
    run_git(&bare, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(&repo, &["remote", "set-head", "origin", "main"]);
    run_git(&repo, &["branch", "feat/example"]);

    RebaseSandbox {
        _sandbox: sandbox,
        repo,
    }
}

fn advance_main(repo: &Path, commits: usize) {
    for i in 0..commits {
        fs::write(repo.join(format!("main-{i}.txt")), format!("v{i}\n")).unwrap();
        run_git(repo, &["add", "."]);
        run_git(repo, &["commit", "-m", &format!("main commit {i}")]);
    }
}

fn write_echo_config(repo: &Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).unwrap();
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"sh\"\n\n[clis.sh]\ncommand = \"sh\"\ndisplay_name = \"Shell\"\n",
    )
    .unwrap();
}

#[test]
#[serial]
fn start_default_rebases_feat_branch_onto_main() {
    if skip_if_no_tmux() {
        return;
    }

    let project = unique_project_name("default");
    let sandbox = setup_sandbox(&project);
    write_echo_config(&sandbox.repo);
    advance_main(&sandbox.repo, 3);

    let session_name = format!("paw-{project}");
    kill_session(&session_name);

    // Exit code is ignored — tmux attach will fail without a TTY, but the
    // worktree-creation (including the rebase) happens before attach.
    let _ = cmd()
        .current_dir(&sandbox.repo)
        .args(["start", "--cli", "sh", "--branches", "feat/example"])
        .output()
        .expect("run git paw start");

    // Worktree lives at sibling-of-repo path.
    let parent = sandbox.repo.parent().unwrap();
    let wt_path = parent.join(format!("{project}-feat-example"));
    assert!(
        wt_path.exists(),
        "worktree must have been created at {}",
        wt_path.display()
    );

    // After the default rebase, feat/example contains main's 3 new commits:
    // `git rev-list --count feat/example..main` must be 0.
    let behind = capture_git(
        &sandbox.repo,
        &["rev-list", "--count", "feat/example..main"],
    );
    assert_eq!(
        behind, "0",
        "feat/example must include all main commits after default rebase"
    );

    kill_session(&session_name);
}

#[test]
#[serial]
fn start_with_no_rebase_preserves_old_baseline() {
    if skip_if_no_tmux() {
        return;
    }

    let project = unique_project_name("no-rebase");
    let sandbox = setup_sandbox(&project);
    write_echo_config(&sandbox.repo);
    advance_main(&sandbox.repo, 3);

    let pre = capture_git(&sandbox.repo, &["rev-parse", "feat/example"]);

    let session_name = format!("paw-{project}");
    kill_session(&session_name);

    let _ = cmd()
        .current_dir(&sandbox.repo)
        .args([
            "start",
            "--no-rebase",
            "--cli",
            "sh",
            "--branches",
            "feat/example",
        ])
        .output()
        .expect("run git paw start --no-rebase");

    let post = capture_git(&sandbox.repo, &["rev-parse", "feat/example"]);
    assert_eq!(
        pre, post,
        "feat/example HEAD must NOT change when --no-rebase is passed"
    );

    // With --no-rebase, feat/example does NOT contain main's 3 new commits.
    let behind = capture_git(
        &sandbox.repo,
        &["rev-list", "--count", "feat/example..main"],
    );
    assert_eq!(
        behind, "3",
        "feat/example must remain 3 commits behind main when --no-rebase is passed"
    );

    kill_session(&session_name);
}
