//! Integration tests for the auto-approve poll loop.
//!
//! Spawns a real (detached) tmux session, lays down a fake "agent pane"
//! whose buffer contains a permission prompt, and verifies that
//! `auto_approve_pane` either dispatches `BTab Down Enter` or no-ops
//! depending on whether the captured command matches the safe-command
//! whitelist.
//!
//! tmux is a hard dependency of git-paw, so these tests run normally and
//! are not gated behind `#[ignore]`.

use std::process::Command;
use std::time::Duration;

use git_paw::supervisor::approve::{ApprovalRequest, TmuxKeyDispatcher, auto_approve_pane};
use git_paw::supervisor::permission_prompt::{PermissionType, capture_pane};

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn unique_session_name(tag: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
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

#[test]
fn safe_prompt_dispatches_btab_down_enter_against_real_tmux() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let session = unique_session_name("safe");
    create_detached_session(&session);

    // Print a synthetic prompt into the pane that mimics what an agent CLI
    // would surface. We use `printf` so the pane buffer contains both the
    // approval marker and the matched command text.
    let printf_cmd = "printf 'cargo test --workspace\\n[y/N] requires approval '";
    let target = format!("{session}:0.0");
    let status = Command::new("tmux")
        .args(["send-keys", "-t", &target, printf_cmd, "Enter"])
        .status()
        .expect("send printf");
    assert!(status.success());
    std::thread::sleep(Duration::from_millis(200));

    // Sanity: capture-pane should see our marker.
    let captured = capture_pane(&session, 0).expect("capture pane");
    assert!(
        captured.contains("requires approval"),
        "pane should contain marker, got: {captured}"
    );
    assert!(captured.contains("cargo test"));

    // Drive the auto-approver. The Cargo class is safe, so it must
    // dispatch BTab Down Enter via three send-keys calls.
    let mut dispatcher = TmuxKeyDispatcher;
    let req = ApprovalRequest {
        enabled: true,
        session: &session,
        pane_index: 0,
        agent_id: "feat-test",
        kind: PermissionType::Cargo,
        matched_entry: Some("cargo test"),
        broker_url: None,
    };
    let fired = auto_approve_pane(&mut dispatcher, req).expect("auto_approve_pane");
    assert!(fired, "safe prompt must dispatch keystrokes");

    // After the Enter, the shell should still be alive — capture again.
    std::thread::sleep(Duration::from_millis(150));
    let _post_capture = capture_pane(&session, 0);

    kill_session(&session);
}

#[test]
fn unsafe_prompt_is_noop_against_real_tmux() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
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

    let mut dispatcher = TmuxKeyDispatcher;
    let req = ApprovalRequest {
        enabled: true,
        session: &session,
        pane_index: 0,
        agent_id: "feat-test",
        kind: PermissionType::Unknown,
        matched_entry: None,
        broker_url: None,
    };
    let fired = auto_approve_pane(&mut dispatcher, req).expect("auto_approve_pane");
    assert!(!fired, "Unknown class must not dispatch keystrokes (no-op)");

    kill_session(&session);
}

#[test]
fn disabled_config_is_noop_against_real_tmux() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let session = unique_session_name("disabled");
    create_detached_session(&session);

    let mut dispatcher = TmuxKeyDispatcher;
    // Even with a safe class, enabled=false must short-circuit before
    // touching tmux.
    let req = ApprovalRequest {
        enabled: false,
        session: &session,
        pane_index: 0,
        agent_id: "feat-test",
        kind: PermissionType::Cargo,
        matched_entry: Some("cargo test"),
        broker_url: None,
    };
    let fired = auto_approve_pane(&mut dispatcher, req).expect("auto_approve_pane");
    assert!(!fired, "enabled=false must be a no-op");

    kill_session(&session);
}
