//! Shared PTY-driver harness for interactive-prompt e2e tests (`cli-interaction-e2e`).
//!
//! `dialoguer` prompts need a real TTY, so we drive the real binary inside a
//! detached tmux pane and assert on observable outcomes (written config /
//! `session.json` / panes). Socket isolation is the caller's responsibility via
//! [`super::TmuxTestEnv::apply_to_process`]; callers MUST be `#[serial]` and gate
//! on [`tmux_available`]. Synchronisation is poll-until-rendered ([`wait_for_pane`]
//! / [`wait_for_file`]), never a fixed sleep as the primary gate.

#![allow(dead_code)]

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// True when a usable `tmux` is on PATH.
pub fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A process-unique tmux session name under `prefix` (e.g. `"paw-init-specs"`).
pub fn unique_session_name(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{prefix}-{nanos}")
}

/// Best-effort teardown of `name`.
pub fn kill_session(name: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .output();
}

/// Detached tmux session running a long-lived shell we can `send-keys` into.
pub fn create_detached_session(name: &str) {
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
    std::thread::sleep(Duration::from_millis(150));
}

/// Captures the visible buffer of pane 0 of `session`.
pub fn capture(session: &str) -> String {
    let out = Command::new("tmux")
        .args(["capture-pane", "-t", &format!("{session}:0.0"), "-p"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Sends `keys` (literal text or named keys like `Down`/`Enter`) to pane 0.
pub fn send_keys(session: &str, keys: &[&str]) {
    let target = format!("{session}:0.0");
    let mut args = vec!["send-keys", "-t", &target];
    args.extend_from_slice(keys);
    let status = Command::new("tmux")
        .args(&args)
        .status()
        .expect("tmux send-keys");
    assert!(status.success(), "tmux send-keys failed");
}

/// Polls the pane until `needle` appears, or panics after `timeout`.
pub fn wait_for_pane(session: &str, needle: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        let buf = capture(session);
        if buf.contains(needle) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {needle:?} in pane; last capture:\n{buf}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Polls until `path` exists, or panics after `timeout`.
pub fn wait_for_file(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {} to be written",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
