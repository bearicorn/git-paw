//! `worker-session-id-capture` — `git paw status --json` exposes each
//! agent's `cli_session_id`; the human-readable table stays byte-identical.
//!
//! Mirrors `session_stale_hygiene.rs`'s pattern: write a session receipt
//! directly to a fake `HOME`'s sessions directory (no tmux involved), then
//! invoke the production binary against it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::SystemTime;

use assert_cmd::Command;
use git_paw::session::{Session, SessionMode, SessionStatus, WorktreeEntry};
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn sessions_dir_for_home(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("write readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

fn receipt(
    session_name: &str,
    repo_path: &Path,
    worktree_path: &Path,
    cli_session_id: Option<&str>,
) -> Session {
    Session {
        session_name: session_name.to_string(),
        repo_path: repo_path.to_path_buf(),
        project_name: "cli-session-id".to_string(),
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: vec![WorktreeEntry {
            branch: "feat/x".to_string(),
            worktree_path: worktree_path.to_path_buf(),
            cli: "claude".to_string(),
            branch_created: false,
            base_branch: None,
            pending_boot_prompt: None,
            runtime_slot: None,
            cli_session_id: cli_session_id.map(String::from),
            spec_id: None,
        }],
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
    }
}

/// A unique-ish, guaranteed-tmux-absent session name (mirrors
/// `session_stale_hygiene.rs`'s `dead_name`) — liveness probing is
/// irrelevant to this test, so a dead session keeps the run socket-isolated.
fn dead_name(tag: &str) -> String {
    format!("paw-cli-session-id-{tag}-doesnotexist")
}

/// Spec scenario "JSON status carries the id".
#[test]
fn status_json_exposes_assigned_cli_session_id() {
    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());

    let sdir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sdir).expect("create sessions dir");
    let name = dead_name("assigned");
    let id = "3f2c1a90-aaaa-4bbb-8ccc-1234567890ab";
    git_paw::session::save_session_in(
        &receipt(
            &name,
            &canon(repo.path()),
            &repo.path().join("wt"),
            Some(id),
        ),
        &sdir,
    )
    .expect("save receipt");

    let out = cmd()
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["status", "--json"])
        .output()
        .expect("status --json");
    assert!(out.status.success(), "status --json should succeed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(&format!("\"cli_session_id\":\"{id}\""))
            || stdout.contains(&format!("\"cli_session_id\": \"{id}\"")),
        "JSON status should carry the assigned cli_session_id; got:\n{stdout}"
    );
}

/// Spec scenario "JSON status shows null when unassigned".
#[test]
fn status_json_shows_null_for_unassigned_cli_session_id() {
    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());

    let sdir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sdir).expect("create sessions dir");
    let name = dead_name("unassigned");
    git_paw::session::save_session_in(
        &receipt(&name, &canon(repo.path()), &repo.path().join("wt"), None),
        &sdir,
    )
    .expect("save receipt");

    let out = cmd()
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["status", "--json"])
        .output()
        .expect("status --json");
    assert!(out.status.success(), "status --json should succeed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"cli_session_id\":null") || stdout.contains("\"cli_session_id\": null"),
        "JSON status should show null for an unassigned id; got:\n{stdout}"
    );
}

/// Spec scenario "Human-readable status is unchanged": an assigned id never
/// surfaces in the human table.
#[test]
fn human_status_output_does_not_mention_cli_session_id() {
    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());

    let sdir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sdir).expect("create sessions dir");
    let name = dead_name("human");
    let id = "3f2c1a90-aaaa-4bbb-8ccc-1234567890ab";
    git_paw::session::save_session_in(
        &receipt(
            &name,
            &canon(repo.path()),
            &repo.path().join("wt"),
            Some(id),
        ),
        &sdir,
    )
    .expect("save receipt");

    let out = cmd()
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .arg("status")
        .output()
        .expect("status");
    assert!(out.status.success(), "status should succeed");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("cli_session_id") && !stdout.contains(id),
        "human status must not mention cli_session_id or its value; got:\n{stdout}"
    );
}
