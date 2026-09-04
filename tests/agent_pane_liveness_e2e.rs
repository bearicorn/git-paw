//! Behavioral coverage for GP-03c: `git paw status` and `git paw doctor`
//! report a session whose agent panes have all reverted to bare shells as
//! not active — even though the tmux session object itself still exists
//! (the full-auto boot-exit failure mode, where every pane's CLI process
//! exits within seconds of launch and the pane reverts to its underlying
//! interactive shell rather than closing).
//!
//! Unlike `session_stale_hygiene.rs` (which uses session names guaranteed
//! absent from tmux), this test creates a REAL tmux session, so it uses the
//! live-session guard.

mod helpers;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{Duration, SystemTime};

use assert_cmd::Command;
use git_paw::session::{Session, SessionMode, SessionStatus};
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// The sessions directory the binary resolves for a given fake `HOME`,
/// mirroring `git_paw::dirs::data_dir()`.
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

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A receipt for a `Bare`-mode, broker-disabled session — `agent_pane_offset`
/// resolves to `0`, so every pane in the window (including pane 0, tmux's
/// default interactive shell) is a coding-agent pane for the liveness probe.
/// `created_at` is set well past the launch grace period so the probe's
/// agent-pane check is actually exercised rather than the launch-window
/// tolerance.
fn dead_panes_receipt(session_name: &str, repo_path: &Path) -> Session {
    Session {
        session_name: session_name.to_string(),
        repo_path: repo_path.to_path_buf(),
        project_name: "gp03c".to_string(),
        created_at: SystemTime::now() - Duration::from_secs(120),
        status: SessionStatus::Active,
        worktrees: vec![],
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
        mode: SessionMode::Bare,
        dashboard_pane: None,
    }
}

#[test]
fn status_and_doctor_report_a_session_whose_panes_reverted_to_shells_as_not_active() {
    helpers::guard_against_live_session();
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let session_name = "paw-gp03c-dead-panes-e2e-test";
    let _ = StdCommand::new("tmux")
        .args(["kill-session", "-t", session_name])
        .output();

    // A bare tmux session whose only pane runs the default interactive
    // shell — never a coding-agent CLI. `tmux has-session` reports this
    // session alive, exactly like a real session whose full-auto pane
    // exited at boot and reverted to a bare shell.
    let created = StdCommand::new("tmux")
        .args(["new-session", "-d", "-s", session_name])
        .status()
        .expect("tmux new-session");
    assert!(created.success(), "failed to create the test tmux session");

    let home = TempDir::new().expect("home");
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let repo_path = canon(repo.path());

    let sdir = sessions_dir_for_home(home.path());
    fs::create_dir_all(&sdir).expect("create sessions dir");
    git_paw::session::save_session_in(&dead_panes_receipt(session_name, &repo_path), &sdir)
        .expect("save receipt");

    let json_out = cmd()
        .current_dir(&repo_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["status", "--json"])
        .output()
        .expect("status --json");
    let stdout = String::from_utf8_lossy(&json_out.stdout);
    assert!(
        stdout.contains("\"status\":\"stale\"") || stdout.contains("\"status\": \"stale\""),
        "a session with no live agent-pane CLI must not report active/healthy via status; \
         got:\n{stdout}"
    );

    let doctor_out = cmd()
        .current_dir(&repo_path)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["doctor", "--json"])
        .output()
        .expect("doctor --json");
    let doctor_stdout = String::from_utf8_lossy(&doctor_out.stdout);
    let document: serde_json::Value =
        serde_json::from_str(&doctor_stdout).expect("doctor --json parses");
    let checks = document["checks"].as_array().expect("checks array present");
    let session_state = checks
        .iter()
        .find(|c| c["name"] == "session state")
        .expect("'session state' check present");
    assert_eq!(
        session_state["status"], "warn",
        "doctor must flag the dead-paned session as stale, got:\n{doctor_stdout}"
    );
    assert!(
        session_state["detail"]
            .as_str()
            .is_some_and(|d| d.contains(session_name)),
        "the stale-session detail must name the dead-paned session, got:\n{doctor_stdout}"
    );

    let _ = StdCommand::new("tmux")
        .args(["kill-session", "-t", session_name])
        .output();
}
