//! Prompt-inbox Enter integration test.
//!
//! Drives the production `send_reply_to_pane` seam — the same function the
//! dashboard's Enter handler calls when the operator submits a reply — and
//! verifies that the targeted tmux pane's buffer contains the reply text.
//!
//! `send_reply_to_pane` builds a target string of the form
//! `<session>:0.<pane_index>` so tmux routes the reply to the specific pane
//! inside window 0. The test starts a single-window session with two panes
//! and asserts that pane 1 — and not pane 0 — receives the reply.

use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serial_test::serial;

// ---------------------------------------------------------------------------
// tmux helpers — local to this test file to keep it self-contained.
// ---------------------------------------------------------------------------

/// Atomic counter so each test gets a unique tmux session name.
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_session_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("paw-test-{tag}-{pid}-{n}")
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

/// Creates a detached tmux session with one window containing two panes,
/// each running an interactive `sh`. Pane 0 = the "dashboard" pane (untargeted
/// in this test), pane 1 = the agent pane that should receive the reply.
fn start_two_pane_session(name: &str) {
    // Start the session detached. `-d` = detached, `-s` = session name,
    // `-x/-y` = give the pseudo-terminal a sane size so capture-pane returns
    // useful output.
    let status = StdCommand::new("tmux")
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
    assert!(status.success(), "tmux new-session must succeed");

    // Split window 0 to create pane 1 running sh.
    let status = StdCommand::new("tmux")
        .args(["split-window", "-t", &format!("{name}:0"), "sh"])
        .status()
        .expect("tmux split-window");
    assert!(status.success(), "tmux split-window must succeed");
}

fn kill_session(name: &str) {
    let _ = StdCommand::new("tmux")
        .args(["kill-session", "-t", name])
        .status();
}

fn capture_pane(session: &str, pane: usize) -> String {
    let target = format!("{session}:0.{pane}");
    let output = StdCommand::new("tmux")
        .args(["capture-pane", "-t", &target, "-p"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&output.stdout).to_string()
}

// ---------------------------------------------------------------------------
// C15: send_reply_to_pane delivers reply text to the focused pane
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn enter_sends_reply_text_to_focused_pane() {
    if skip_if_no_tmux() {
        return;
    }

    let session = unique_session_name("inbox");
    start_two_pane_session(&session);

    // Drive the same code path the dashboard's Enter handler uses, targeting
    // pane 1 in window 0 (the "agent" pane that the prompt is mapped to).
    let reply = "answer text";
    git_paw::dashboard::send_reply_to_pane(&session, 1, reply).expect("send_reply_to_pane");

    // Poll the targeted buffer until the reply text shows up. The shell
    // echoes the line back as it is typed, then runs it (giving "command not
    // found") — both shapes contain the literal text.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut target_buffer = String::new();
    while Instant::now() < deadline {
        target_buffer = capture_pane(&session, 1);
        if target_buffer.contains(reply) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let untargeted_buffer = capture_pane(&session, 0);
    let target_has_reply = target_buffer.contains(reply);
    let untargeted_has_reply = untargeted_buffer.contains(reply);
    kill_session(&session);

    assert!(
        target_has_reply,
        "target buffer should contain reply text {reply:?}; got:\n{target_buffer}"
    );

    // Sanity: the unfocused pane must NOT contain the reply — routing must
    // hit exactly the pane the operator focused on.
    assert!(
        !untargeted_has_reply,
        "unfocused pane must not contain reply text; got:\n{untargeted_buffer}"
    );
}
