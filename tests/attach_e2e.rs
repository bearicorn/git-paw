//! Integration tests for `git paw attach`.
//!
//! Maps to the `cli-parsing` spec scenarios "Attach reattaches to the
//! repository's running session" and "Attach with no running session errors
//! actionably".
//!
//! `tmux attach-session` replaces the client's controlling terminal, which a
//! headless test process does not have — a real attach is not something
//! these tests can drive end to end. Instead, the "resolves the repo's
//! session name" scenario is verified by proving *which* session `attach`
//! tried to reach: with no session it must fail with the "no session"
//! message; with a live session for the repo it must fail with tmux's own
//! "not a terminal" error naming that exact session, never the "no session"
//! message. That distinguishes "found and attempted the right session" from
//! "found nothing".
//!
//! Uses [`helpers::tmux_test_env`] throughout so these tests run against a
//! private tmux socket, never the live dogfood session's default socket.

use std::fs;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("attach-{tag}-{pid}-{n}")
}

fn canonical_repo_root(repo: &std::path::Path) -> std::path::PathBuf {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse");
    assert!(out.status.success(), "git rev-parse must succeed");
    std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

fn sessions_dir_for(fake_home: &std::path::Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        fake_home.join("Library/Application Support/git-paw/sessions")
    } else {
        fake_home.join(".local/share/git-paw/sessions")
    }
}

/// Writes a minimal Session JSON to the simulated sessions dir — only the
/// fields `attach`'s resolution path reads (session name, repo path, status,
/// an empty worktree roster).
fn write_session_json(
    sessions_dir: &std::path::Path,
    session_name: &str,
    repo_canonical: &std::path::Path,
    project: &str,
) {
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": repo_canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "active",
        "worktrees": [],
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");
}

// ---------------------------------------------------------------------------
// Scenario: Attach with no running session errors actionably
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn attach_with_no_session_errors_actionably() {
    let tr = setup_test_repo();
    let fake_home = TempDir::new().expect("create temp HOME");
    let tmux_env = tmux_test_env();

    let mut attach = cmd();
    tmux_env.apply_assert(&mut attach);
    let out = attach
        .current_dir(tr.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["attach"])
        .output()
        .expect("run attach");

    assert!(
        !out.status.success(),
        "attach with no session should exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        stderr.contains("no session is running"),
        "stderr should name the problem; got: {stderr}"
    );
    assert!(
        stderr.contains("git paw start"),
        "stderr should point at 'git paw start'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: Attach reattaches to the repository's running session
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn attach_resolves_the_repos_running_session() {
    let tr = setup_test_repo();
    let project = unique_project_name("resolve");
    let canonical = canonical_repo_root(tr.path());

    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let tmux_env = tmux_test_env();
    let session_name = format!("paw-{project}");

    let mut new_session = StdCommand::new("tmux");
    tmux_env.apply(&mut new_session);
    let st = new_session
        .args([
            "new-session",
            "-d",
            "-s",
            &session_name,
            "-x",
            "80",
            "-y",
            "24",
        ])
        .status()
        .expect("tmux new-session");
    assert!(st.success(), "tmux new-session for {session_name}");

    write_session_json(&sessions_dir, &session_name, &canonical, &project);

    let mut attach = cmd();
    tmux_env.apply_assert(&mut attach);
    let out = attach
        .current_dir(tr.path())
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["attach"])
        .output()
        .expect("run attach");

    let mut kill = StdCommand::new("tmux");
    tmux_env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session_name]).status();

    // A headless test process has no controlling terminal, so the real
    // `tmux attach-session` call fails — but the failure must name THIS
    // session, proving resolution reached it rather than reporting "no
    // session is running" (the no-session error path exercised by the
    // sibling test).
    assert!(
        !out.status.success(),
        "headless attach-session cannot succeed without a controlling terminal"
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        !stderr.contains("no session is running"),
        "attach should have found the repo's session, not reported none running; got: {stderr}"
    );
    assert!(
        stderr.contains(&session_name),
        "attach's failure should name the resolved session '{session_name}'; got: {stderr}"
    );
}
