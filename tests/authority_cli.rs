//! `broker-agent-helper` "Root-side helper verbs carry the authority
//! credential": the hidden `git paw __authority` subcommand.
//!
//! Each test points `HOME` at a temp dir so the binary's user-data sessions
//! directory lands inside it, and writes a real session file with the
//! production serializer.

use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::SystemTime;

use assert_cmd::Command;
use git_paw::session::{Session, SessionMode, SessionStatus, save_session_in};
use tempfile::TempDir;

/// XDG data-dir layout used by the binary, mirroring `dirs::data_dir()`.
fn sessions_dir_under_home(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

/// Runs `git args` in `dir`, panicking on failure.
fn git(dir: &Path, args: &[&str]) {
    let out = StdCommand::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A repository with one commit, returned with its canonical root.
fn repo() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().expect("repo tempdir");
    git(tmp.path(), &["init", "-q", "-b", "main"]);
    git(tmp.path(), &["config", "user.email", "t@example.com"]);
    git(tmp.path(), &["config", "user.name", "t"]);
    std::fs::write(tmp.path().join("f.txt"), "x").unwrap();
    git(tmp.path(), &["add", "f.txt"]);
    git(tmp.path(), &["commit", "-q", "-m", "init"]);
    let root = git_paw::git::validate_repo(tmp.path()).expect("validate repo");
    (tmp, root)
}

/// Saves a broker session for `repo_root` carrying `credential`.
fn save_session_with(home: &Path, repo_root: &Path, credential: Option<&str>) {
    let session = Session {
        session_name: "paw-authority".to_string(),
        repo_path: repo_root.to_path_buf(),
        project_name: "authority".to_string(),
        created_at: SystemTime::UNIX_EPOCH,
        status: SessionStatus::Active,
        worktrees: Vec::new(),
        broker_port: Some(9119),
        broker_bind: Some("127.0.0.1".to_string()),
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
        layout: None,
        broker_authority: credential.map(str::to_string),
    };
    save_session_in(&session, &sessions_dir_under_home(home)).expect("save session");
}

/// `git paw __authority` run in `cwd` with `HOME` at `home`.
fn authority_cmd(cwd: &Path, home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("git-paw").expect("binary");
    cmd.arg("__authority")
        .current_dir(cwd)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME");
    cmd
}

/// Scenario: __authority prints the credential at the repo root.
#[test]
fn authority_prints_the_credential_at_the_repo_root() {
    let (_tmp, root) = repo();
    let home = TempDir::new().expect("home");
    let credential = "ab".repeat(32);
    save_session_with(home.path(), &root, Some(&credential));

    let out = authority_cmd(&root, home.path()).output().expect("run");
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("{credential}\n")
    );
}

/// Scenario: __authority refuses from a linked worktree — non-zero, empty
/// stdout — even though the repository's session has a credential.
#[test]
fn authority_refuses_from_a_linked_worktree() {
    let (_tmp, root) = repo();
    let wt_parent = TempDir::new().expect("worktree parent");
    let wt = wt_parent.path().join("wt");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feat/x",
            wt.to_str().unwrap(),
        ],
    );
    let home = TempDir::new().expect("home");
    // A session recorded against the worktree's own root: the exact-match
    // session lookup would find it, so only the linked-worktree check can
    // refuse here.
    let wt_root = git_paw::git::validate_repo(&wt).expect("validate worktree");
    save_session_with(home.path(), &wt_root, Some(&"cd".repeat(32)));

    let out = authority_cmd(&wt, home.path()).output().expect("run");
    assert!(!out.status.success(), "must refuse from a linked worktree");
    assert!(out.stdout.is_empty(), "stdout must be empty: {out:?}");
}

/// Scenario: __authority without a session is actionable.
#[test]
fn authority_without_a_session_names_git_paw_start() {
    let (_tmp, root) = repo();
    let home = TempDir::new().expect("home");

    let out = authority_cmd(&root, home.path()).output().expect("run");
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("git paw start"), "stderr: {stderr}");
}

/// A session without a credential (no broker, or saved by an earlier binary
/// and never recovered) is reported the same way.
#[test]
fn authority_for_a_session_without_a_credential_names_git_paw_start() {
    let (_tmp, root) = repo();
    let home = TempDir::new().expect("home");
    save_session_with(home.path(), &root, None);

    let out = authority_cmd(&root, home.path()).output().expect("run");
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("git paw start"));
}

/// Scenario: __authority is hidden from help.
#[test]
fn authority_is_hidden_from_help() {
    let out = Command::cargo_bin("git-paw")
        .expect("binary")
        .arg("--help")
        .output()
        .expect("run");
    assert!(!String::from_utf8_lossy(&out.stdout).contains("__authority"));
}
