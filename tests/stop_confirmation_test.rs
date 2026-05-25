//! Integration tests for `git paw stop`'s new confirmation prompt
//! behaviour. We do NOT exercise the interactive prompt path here —
//! `assert_cmd` invocations don't allocate a PTY, so stdin is always
//! non-TTY and the prompt is bypassed for v0.4 back-compat. The two
//! tests confirm the bypass branches both exit 0 and don't hang.

use std::fs;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("stop-{tag}-{pid}-{n}")
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

fn kill_tmux_session(name: &str) {
    let _ = StdCommand::new("tmux")
        .args(["kill-session", "-t", name])
        .status();
}

fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> std::path::PathBuf {
    let original = tr.path().to_path_buf();
    let parent = original.parent().expect("repo has parent").to_path_buf();
    let renamed = parent.join(new_basename);
    fs::rename(&original, &renamed).expect("rename repo dir");
    renamed
}

fn canonical_repo_root(repo: &std::path::Path) -> std::path::PathBuf {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse");
    assert!(out.status.success());
    std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

fn sessions_dir_for(fake_home: &std::path::Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        fake_home.join("Library/Application Support/git-paw/sessions")
    } else {
        fake_home.join(".local/share/git-paw/sessions")
    }
}

fn setup_active_session(
    tag: &str,
) -> (
    TestRepo,
    TempDir,
    std::path::PathBuf,
    String,
    std::path::PathBuf,
) {
    let tr = setup_test_repo();
    let project = unique_project_name(tag);
    let repo = rename_repo_basename(&tr, &project);
    let canonical = canonical_repo_root(&repo);

    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");

    // Spawn a real tmux session so stop has something to kill.
    let st = StdCommand::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &session_name,
            "-c",
            repo.to_str().unwrap(),
        ])
        .status()
        .expect("tmux new-session");
    assert!(st.success());

    let json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "active",
        "worktrees": [],
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&json).expect("serialize"),
    )
    .expect("write session json");

    (tr, fake_home, repo, session_name, sessions_dir)
}

// ---------------------------------------------------------------------------
// 10.5 stop_force_skips_prompt
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn stop_force_skips_prompt() {
    if skip_if_no_tmux() {
        return;
    }

    let (_tr, fake_home, repo, session_name, _sessions_dir) = setup_active_session("force");

    let out = cmd()
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .timeout(Duration::from_secs(10))
        .args(["stop", "--force"])
        .output()
        .expect("run stop --force");

    let alive_after = StdCommand::new("tmux")
        .args(["has-session", "-t", &session_name])
        .status()
        .is_ok_and(|s| s.success());
    kill_tmux_session(&session_name);

    assert!(
        out.status.success(),
        "stop --force should exit 0. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!alive_after, "tmux should be killed after stop --force");
}

// ---------------------------------------------------------------------------
// 10.6 stop_non_tty_skips_prompt (v0.4 back-compat: no --force needed
// when stdin is not a TTY — assert_cmd's spawned child has a piped
// stdin so this matches the non-TTY branch).
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn stop_non_tty_skips_prompt() {
    if skip_if_no_tmux() {
        return;
    }

    let (_tr, fake_home, repo, session_name, _sessions_dir) = setup_active_session("notty");

    let out = cmd()
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .timeout(Duration::from_secs(10))
        // No --force: stdin is non-TTY (assert_cmd pipes it), so the
        // prompt is bypassed.
        .args(["stop"])
        .output()
        .expect("run stop");

    let alive_after = StdCommand::new("tmux")
        .args(["has-session", "-t", &session_name])
        .status()
        .is_ok_and(|s| s.success());
    kill_tmux_session(&session_name);

    assert!(
        out.status.success(),
        "stop (non-TTY, no --force) should exit 0. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!alive_after, "tmux should be killed");
}
