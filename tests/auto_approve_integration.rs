//! Integration tests for the auto-approve poll loop.
//!
//! Spawns a real (detached) tmux session, lays down a fake "agent pane"
//! whose buffer contains a permission prompt, and verifies that
//! `auto_approve_pane` either dispatches the option-index keystrokes (the
//! option digit followed by `Enter`) or no-ops depending on whether the
//! captured command is classified safe and live.
//!
//! tmux is a hard dependency of git-paw, so these tests run normally and
//! are not gated behind `#[ignore]`.
//!
//! tmux socket isolation: every test sets `TMUX_TMPDIR` on the current
//! process via `helpers::tmux_test_env().apply_to_process()` so the
//! in-process library calls (e.g. `capture_pane`, `auto_approve_pane`)
//! and the direct `Command::new("tmux")` invocations all share a
//! test-owned socket. The tests are `#[serial]` because they mutate
//! the process env. see openspec/changes/test-tmux-isolation

use std::process::Command;
use std::time::Duration;

use serial_test::serial;

use git_paw::supervisor::approve::{ApprovalRequest, TmuxKeyDispatcher, auto_approve_pane};
use git_paw::supervisor::permission_prompt::{PermissionType, capture_pane};
use git_paw::supervisor::poll::TmuxPaneInspector;

mod helpers;

/// A throwaway repository root for the exclusive pane claim each approval
/// takes, so these serial tests never inherit a claim from one another and
/// never write into the real repository's `.git-paw/tmp/`.
fn claim_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp repo root")
}

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn unique_session_name(tag: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("paw-aa-{tag}-{nanos}")
}

fn kill_session(name: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output();
}

/// Creates a detached tmux session running a long-lived shell so we can
/// drive `send-keys` against it.
fn create_detached_session(name: &str) {
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            name,
            "-x",
            "200",
            "-y",
            "50",
            "sh",
        ])
        .status()
        .expect("tmux new-session");
    assert!(status.success(), "tmux new-session failed");
    // Give the shell a moment to settle so capture-pane sees the prompt.
    std::thread::sleep(Duration::from_millis(150));
}

/// Splits window 0 of `name` `count` times so panes `1..=count` exist, each
/// running a long-lived shell. Used to place the "agent pane" at a non-zero
/// index — the approval-send gate refuses pane 0 (the supervisor's pane).
fn add_panes(name: &str, count: usize) {
    for _ in 0..count {
        let target = format!("{name}:0");
        let status = Command::new("tmux")
            .args(["split-window", "-t", &target, "sh"])
            .status()
            .expect("tmux split-window");
        assert!(status.success(), "tmux split-window failed");
    }
    std::thread::sleep(Duration::from_millis(150));
}

#[test]
#[serial]
fn safe_prompt_dispatches_keystrokes_against_real_tmux() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tmux_env = helpers::tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    let session = unique_session_name("safe");
    create_detached_session(&session);
    // Place the agent pane at index 2 (a coding-agent pane); the gate refuses
    // pane 0 (the supervisor's own pane).
    add_panes(&session, 2);

    // Print a synthetic prompt into the pane that mimics what an agent CLI
    // would surface. We use `printf` so the pane buffer contains both the
    // approval marker and the matched command text.
    let printf_cmd = "printf 'cargo test --workspace\\n[y/N] requires approval '";
    let target = format!("{session}:0.2");
    let status = Command::new("tmux")
        .args(["send-keys", "-t", &target, printf_cmd, "Enter"])
        .status()
        .expect("send printf");
    assert!(status.success());
    std::thread::sleep(Duration::from_millis(200));

    // Sanity: capture-pane should see our marker.
    let captured = capture_pane(&session, 2).expect("capture pane");
    assert!(
        captured.contains("requires approval"),
        "pane should contain marker, got: {captured}"
    );
    assert!(captured.contains("cargo test"));

    // Drive the auto-approver. The Git class is safe and live, so the gate
    // re-confirms the prompt via a fresh capture and dispatches the option
    // digit + Enter via send-keys.
    let capturer = TmuxPaneInspector;
    let mut dispatcher = TmuxKeyDispatcher;
    let repo = claim_root();
    let req = ApprovalRequest {
        enabled: true,
        session: &session,
        repo_root: repo.path(),
        pane_index: 2,
        agent_id: "feat-test",
        kind: PermissionType::Git,
        matched_entry: Some("cargo test"),
        live_prompt: true,
        option_index: 1,
        broker_url: None,
    };
    let fired = auto_approve_pane(&capturer, &mut dispatcher, req).expect("auto_approve_pane");
    assert!(fired, "safe prompt must dispatch keystrokes");
    assert!(
        !git_paw::supervisor::claim::claim_path(repo.path(), 2).exists(),
        "the pane claim must be released once the approval returns"
    );

    // After the Enter, the shell should still be alive — capture again.
    std::thread::sleep(Duration::from_millis(150));
    let _post_capture = capture_pane(&session, 2);

    kill_session(&session);
}

#[test]
#[serial]
fn unsafe_prompt_is_noop_against_real_tmux() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tmux_env = helpers::tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    let session = unique_session_name("unsafe");
    create_detached_session(&session);

    let target = format!("{session}:0.0");
    // Unsafe command (`rm -rf`) — auto-approver must NOT fire since the
    // class is Unknown.
    let printf_cmd = "printf 'rm -rf /tmp/foo\\n[y/N] requires approval '";
    let status = Command::new("tmux")
        .args(["send-keys", "-t", &target, printf_cmd, "Enter"])
        .status()
        .expect("send printf");
    assert!(status.success());
    std::thread::sleep(Duration::from_millis(200));

    let capturer = TmuxPaneInspector;
    let mut dispatcher = TmuxKeyDispatcher;
    let repo = claim_root();
    let req = ApprovalRequest {
        enabled: true,
        session: &session,
        repo_root: repo.path(),
        pane_index: 2,
        agent_id: "feat-test",
        kind: PermissionType::Unknown,
        matched_entry: None,
        live_prompt: true,
        option_index: 1,
        broker_url: None,
    };
    let fired = auto_approve_pane(&capturer, &mut dispatcher, req).expect("auto_approve_pane");
    assert!(!fired, "Unknown class must not dispatch keystrokes (no-op)");

    kill_session(&session);
}

#[test]
#[serial]
fn disabled_config_is_noop_against_real_tmux() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tmux_env = helpers::tmux_test_env();
    let _proc_env = tmux_env.apply_to_process();

    let session = unique_session_name("disabled");
    create_detached_session(&session);

    let capturer = TmuxPaneInspector;
    let mut dispatcher = TmuxKeyDispatcher;
    // Even with a safe class, enabled=false must short-circuit before
    // touching tmux.
    let repo = claim_root();
    let req = ApprovalRequest {
        enabled: false,
        session: &session,
        repo_root: repo.path(),
        pane_index: 2,
        agent_id: "feat-test",
        kind: PermissionType::Git,
        matched_entry: Some("cargo test"),
        live_prompt: true,
        option_index: 1,
        broker_url: None,
    };
    let fired = auto_approve_pane(&capturer, &mut dispatcher, req).expect("auto_approve_pane");
    assert!(!fired, "enabled=false must be a no-op");

    kill_session(&session);
}
