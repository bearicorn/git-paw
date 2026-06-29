//! Fixture tests for the bug-4 stuck-on-prompt detection in
//! `<repo>/.git-paw/scripts/sweep.sh`.
//!
//! Drives the `stuck-eval` subcommand (the per-agent decision factored out of
//! `detect-stuck` so it can run without tmux): a scripted pane capture is fed
//! on stdin together with an agent id and a `last_seen_seconds` value. A
//! localhost stub records every `POST /publish` body so the tests can assert:
//!
//! - a permission prompt with a stale heartbeat publishes `agent.status` with
//!   `phase: "stuck-on-prompt"` and `detail.captured_prompt`;
//! - a permission prompt with a fresh heartbeat does NOT publish;
//! - a paste-buffer (`Pasted text #N`) capture publishes with the
//!   paste-buffer variant annotation;
//! - a repeated detection of the same (agent, prompt-shape) is deduped.
//!
//! Maps to openspec/changes/auto-approve-scope-v0-6-x/specs/stuck-prompt-detection/spec.md

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn init_git_repo(dir: &std::path::Path) {
    for args in [
        &["init", "-b", "main"][..],
        &["config", "user.email", "test@test.com"][..],
        &["config", "user.name", "Test"][..],
    ] {
        let st = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("git");
        assert!(st.success());
    }
    fs::write(dir.join("README.md"), "# test").expect("write readme");
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["add", "."])
        .status();
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["commit", "-m", "initial"])
        .status();
}

/// Stub broker that records every `POST /publish` body. Replies 202 to POST
/// and a small JSON to anything else. Returns (port, recorded bodies, stop).
fn spawn_publish_recorder() -> (u16, Arc<Mutex<Vec<String>>>, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().unwrap().port();
    let bodies: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let bodies_c = Arc::clone(&bodies);
    let stop_c = Arc::clone(&stop);
    thread::spawn(move || {
        listener.set_nonblocking(true).expect("nonblocking");
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !stop_c.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    socket
                        .set_read_timeout(Some(Duration::from_millis(500)))
                        .ok();
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 2048];
                    // Read until the socket stalls (best-effort full request).
                    loop {
                        match socket.read(&mut chunk) {
                            Ok(n) if n > 0 => {
                                buf.extend_from_slice(&chunk[..n]);
                                if n < chunk.len() {
                                    break;
                                }
                            }
                            _ => break,
                        }
                    }
                    let req = String::from_utf8_lossy(&buf).to_string();
                    if req.starts_with("POST") {
                        if let Some(idx) = req.find("\r\n\r\n") {
                            let body = req[idx + 4..].to_string();
                            bodies_c.lock().unwrap().push(body);
                        }
                        let _ =
                            socket.write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n");
                    } else {
                        let body = r#"{"agents":[]}"#;
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = socket.write_all(resp.as_bytes());
                    }
                    let _ = socket.flush();
                }
                Err(_) => thread::sleep(Duration::from_millis(25)),
            }
        }
    });
    (port, bodies, stop)
}

struct Fixture {
    _tmp: TempDir,
    sweep: std::path::PathBuf,
    root: std::path::PathBuf,
    bodies: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    port: u16,
}

fn setup() -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    let init_out = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("git paw init");
    assert!(init_out.status.success());

    let (port, bodies, stop) = spawn_publish_recorder();
    fs::write(
        tmp.path().join(".git-paw/config.toml"),
        format!("[broker]\nenabled = true\nport = {port}\n"),
    )
    .expect("write config");

    let sweep = tmp.path().join(".git-paw/scripts/sweep.sh");
    let root = tmp.path().to_path_buf();
    Fixture {
        _tmp: tmp,
        sweep,
        root,
        bodies,
        stop,
        port,
    }
}

/// Rewrites the fixture's `config.toml` to add a `[supervisor]` block with the
/// given body (e.g. `"context_bloat_threshold_k = 200\n"`), preserving the
/// recorder broker port so the detector still publishes to the stub.
fn set_supervisor_config(fx: &Fixture, supervisor_body: &str) {
    fs::write(
        fx.root.join(".git-paw/config.toml"),
        format!(
            "[broker]\nenabled = true\nport = {}\n[supervisor]\n{supervisor_body}",
            fx.port
        ),
    )
    .expect("rewrite config with [supervisor]");
}

/// Seeds the no-progress snapshot file `.git-paw/.sweep-progress` with a single
/// `agent<TAB>checkbox<TAB>commit<TAB>timestamp` line so a test can pin the
/// prior snapshot (and its age) without waiting real wall-clock time.
fn seed_progress(fx: &Fixture, agent: &str, checkbox: &str, commit: &str, ts: u64) {
    fs::write(
        fx.root.join(".git-paw/.sweep-progress"),
        format!("{agent}\t{checkbox}\t{commit}\t{ts}\n"),
    )
    .expect("seed .sweep-progress");
}

/// Runs `stuck-eval` with the full positional arg set (`checkbox_count`,
/// `commit_count`, `blocked_age_seconds`); pass "" for any the branch under
/// test does not use.
#[allow(clippy::too_many_arguments)]
fn run_stuck_eval_full(
    fx: &Fixture,
    agent: &str,
    last_seen: u64,
    checkbox: &str,
    commit: &str,
    blocked_age: &str,
    capture: &str,
) -> String {
    let mut child = StdCommand::new("bash")
        .arg(&fx.sweep)
        .arg("stuck-eval")
        .arg(agent)
        .arg(last_seen.to_string())
        .arg(checkbox)
        .arg(commit)
        .arg(blocked_age)
        .current_dir(&fx.root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn sweep.sh");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(capture.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait");
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn run_stuck_eval(fx: &Fixture, agent: &str, last_seen: u64, capture: &str) -> String {
    let mut child = StdCommand::new("bash")
        .arg(&fx.sweep)
        .arg("stuck-eval")
        .arg(agent)
        .arg(last_seen.to_string())
        .current_dir(&fx.root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn sweep.sh");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(capture.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Polls until the recorder has captured at least `min` publishes, or the
/// timeout elapses. The curl POST completes before `run_stuck_eval` returns,
/// but the recorder thread records the body asynchronously — a fixed sleep
/// raced under parallel load (W2-9 flake). Polling closes that window.
fn wait_for_bodies(fx: &Fixture, min: usize, timeout: Duration) {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if fx.bodies.lock().unwrap().len() >= min {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[serial]
fn permission_prompt_with_stale_heartbeat_publishes_stuck() {
    let fx = setup();
    let out = run_stuck_eval(
        &fx,
        "feat-x",
        45,
        "tool output\nDo you want to proceed?\n  1. Yes\n  2. No\n",
    );
    // Wait (bounded) for the recorder to capture the publish.
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(out.contains("stuck"), "expected stuck output, got: {out}");
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(
        bodies.len(),
        1,
        "expected exactly one publish, got {bodies:?}"
    );
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("valid json");
    assert_eq!(v["type"], "agent.status");
    assert_eq!(v["agent_id"], "feat-x");
    assert_eq!(v["payload"]["phase"], "stuck-on-prompt");
    assert!(
        v["payload"]["detail"]["captured_prompt"]
            .as_str()
            .unwrap()
            .contains("Do you want to proceed?"),
        "captured_prompt must carry the prompt text: {}",
        bodies[0]
    );
}

#[test]
#[serial]
fn permission_prompt_with_fresh_heartbeat_does_not_publish() {
    let fx = setup();
    let out = run_stuck_eval(&fx, "feat-y", 5, "Do you want to proceed?\n");
    thread::sleep(Duration::from_millis(200));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(
        out.contains("not-stuck"),
        "fresh heartbeat must be not-stuck, got: {out}"
    );
    assert!(
        fx.bodies.lock().unwrap().is_empty(),
        "fresh heartbeat must not publish"
    );
}

#[test]
#[serial]
fn paste_buffer_capture_publishes_paste_variant() {
    let fx = setup();
    run_stuck_eval(&fx, "feat-z", 60, "Pasted text #3 (240 lines)\n");
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one publish, got {bodies:?}");
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).unwrap();
    assert_eq!(v["payload"]["phase"], "stuck-on-prompt");
    assert_eq!(
        v["payload"]["detail"]["variant"], "paste-buffer",
        "paste-buffer capture must annotate the variant: {}",
        bodies[0]
    );
}

#[test]
#[serial]
fn repeated_detection_is_deduped() {
    let fx = setup();
    let cap = "Do you want to proceed?\n";
    run_stuck_eval(&fx, "feat-x", 45, cap);
    let second = run_stuck_eval(&fx, "feat-x", 50, cap);
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(
        second.contains("deduped"),
        "second identical detection must be deduped, got: {second}"
    );
    assert_eq!(
        fx.bodies.lock().unwrap().len(),
        1,
        "a persistently stuck agent publishes exactly once per window"
    );
}

// ---------------------------------------------------------------------------
// supervisor-stuck-bloat-detection: the four new stuck shapes.
// ---------------------------------------------------------------------------

/// 4.1 — a stream-timeout / transport-error marker in a pane publishes
/// `phase: "stuck-stream-timeout"` with the captured prompt.
#[test]
#[serial]
fn stream_timeout_marker_publishes_stuck_stream_timeout() {
    let fx = setup();
    let out = run_stuck_eval_full(
        &fx,
        "feat-st",
        60,
        "",
        "",
        "",
        "compiling the workspace\nRequest timed out\n",
    );
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(out.contains("stuck"), "expected stuck output, got: {out}");
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one publish, got {bodies:?}");
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("valid json");
    assert_eq!(v["type"], "agent.status");
    assert_eq!(v["agent_id"], "feat-st");
    assert_eq!(v["payload"]["phase"], "stuck-stream-timeout");
    assert!(
        v["payload"]["detail"]["captured_prompt"]
            .as_str()
            .unwrap()
            .contains("Request timed out"),
        "captured_prompt must carry the marker text: {}",
        bodies[0]
    );
}

/// 4.2a — a `/clear to save Nk tokens` hint at/over the threshold publishes
/// `phase: "context-bloat"` with the parsed token figure.
#[test]
#[serial]
fn context_bloat_over_threshold_publishes() {
    let fx = setup();
    set_supervisor_config(&fx, "context_bloat_threshold_k = 200\n");
    let out = run_stuck_eval_full(
        &fx,
        "feat-cb",
        60,
        "",
        "",
        "",
        "still working\n/clear to save 250k tokens\n",
    );
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(out.contains("stuck"), "expected stuck output, got: {out}");
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one publish, got {bodies:?}");
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("valid json");
    assert_eq!(v["payload"]["phase"], "context-bloat");
    assert_eq!(
        v["payload"]["detail"]["tokens_k"], 250,
        "detail must carry the parsed token figure: {}",
        bodies[0]
    );
}

/// 4.2b — a clear hint below the threshold is NOT flagged.
#[test]
#[serial]
fn context_bloat_below_threshold_does_not_publish() {
    let fx = setup();
    set_supervisor_config(&fx, "context_bloat_threshold_k = 200\n");
    let out = run_stuck_eval_full(
        &fx,
        "feat-cb2",
        60,
        "",
        "",
        "",
        "still working\n/clear to save 100k tokens\n",
    );
    thread::sleep(Duration::from_millis(200));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(
        out.contains("not-stuck"),
        "below-threshold bloat must be not-stuck, got: {out}"
    );
    assert!(
        fx.bodies.lock().unwrap().is_empty(),
        "below-threshold bloat must not publish"
    );
}

/// 4.3a — both counters unchanged past the window (no pane marker) publishes
/// `phase: "no-progress"`.
#[test]
#[serial]
fn no_progress_unchanged_past_window_publishes() {
    let fx = setup();
    // Prior snapshot at epoch 0 → unchanged_for far exceeds the ~1500s default.
    seed_progress(&fx, "feat-np", "5", "3", 0);
    let out = run_stuck_eval_full(&fx, "feat-np", 60, "5", "3", "", "thinking hard\n");
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(out.contains("stuck"), "expected stuck output, got: {out}");
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one publish, got {bodies:?}");
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("valid json");
    assert_eq!(v["payload"]["phase"], "no-progress");
    assert_eq!(v["payload"]["detail"]["checkbox_count"], "5");
    assert_eq!(v["payload"]["detail"]["commit_count"], "3");
}

/// 4.3b — movement in either counter is NOT no-progress.
#[test]
#[serial]
fn no_progress_counter_movement_does_not_publish() {
    let fx = setup();
    seed_progress(&fx, "feat-np2", "5", "3", 0);
    // Commit count advanced 3 → 4: movement clears the timer.
    let out = run_stuck_eval_full(&fx, "feat-np2", 60, "5", "4", "", "thinking hard\n");
    thread::sleep(Duration::from_millis(200));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(
        out.contains("not-stuck"),
        "counter movement must be not-stuck, got: {out}"
    );
    assert!(
        fx.bodies.lock().unwrap().is_empty(),
        "counter movement must not publish"
    );
}

/// 4.3c — the first observation of an agent only records state; it never flags.
#[test]
#[serial]
fn no_progress_first_observation_only_records() {
    let fx = setup();
    // No prior snapshot on file.
    let out = run_stuck_eval_full(&fx, "feat-np3", 60, "5", "3", "", "thinking hard\n");
    thread::sleep(Duration::from_millis(200));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(
        out.contains("not-stuck"),
        "first observation must be not-stuck, got: {out}"
    );
    assert!(
        fx.bodies.lock().unwrap().is_empty(),
        "first observation must not publish"
    );
    let progress = fs::read_to_string(fx.root.join(".git-paw/.sweep-progress"))
        .expect("first observation must record a snapshot");
    assert!(
        progress.contains("feat-np3\t5\t3\t"),
        "snapshot must record the current counts: {progress}"
    );
}

/// 4.4 — read-pane rule: a pane showing a permission marker is stuck-on-prompt,
/// NOT no-progress, even when the checkbox/commit counters are unchanged past
/// the window.
#[test]
#[serial]
fn permission_marker_is_stuck_on_prompt_not_no_progress() {
    let fx = setup();
    // Counters unchanged past the window would otherwise trip no-progress...
    seed_progress(&fx, "feat-pp", "5", "3", 0);
    // ...but the live pane shows a permission prompt, so the read-pane rule wins.
    run_stuck_eval_full(
        &fx,
        "feat-pp",
        60,
        "5",
        "3",
        "",
        "running the build\nDo you want to proceed?\n  1. Yes\n  2. No\n",
    );
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one publish, got {bodies:?}");
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("valid json");
    assert_eq!(
        v["payload"]["phase"], "stuck-on-prompt",
        "a prompt-blocked agent must be stuck-on-prompt, never no-progress: {}",
        bodies[0]
    );
}

/// 4.5a — a supervisor-targeted block unanswered past the window publishes
/// `phase: "blocked-on-supervisor"`.
#[test]
#[serial]
fn blocked_on_supervisor_past_window_publishes() {
    let fx = setup();
    // blocked_age 1200s > default 900s window.
    let out = run_stuck_eval_full(
        &fx,
        "feat-b",
        60,
        "",
        "",
        "1200",
        "waiting for the supervisor\n",
    );
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(out.contains("stuck"), "expected stuck output, got: {out}");
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one publish, got {bodies:?}");
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("valid json");
    assert_eq!(v["payload"]["phase"], "blocked-on-supervisor");
    assert_eq!(v["payload"]["detail"]["unanswered_for_seconds"], 1200);
}

/// 4.5b — a freshly-blocked agent (within the window) is NOT flagged.
#[test]
#[serial]
fn blocked_on_supervisor_fresh_does_not_publish() {
    let fx = setup();
    // blocked_age 30s < default 900s window.
    let out = run_stuck_eval_full(
        &fx,
        "feat-b2",
        60,
        "",
        "",
        "30",
        "waiting for the supervisor\n",
    );
    thread::sleep(Duration::from_millis(200));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(
        out.contains("not-stuck"),
        "a fresh block must be not-stuck, got: {out}"
    );
    assert!(
        fx.bodies.lock().unwrap().is_empty(),
        "a fresh block must not publish"
    );
}

/// 4.6 — dedup: each shape publishes once per window per `(agent_id, shape)`
/// across repeated sweeps. Exercised for stream-timeout and no-progress.
#[test]
#[serial]
fn each_shape_dedups_per_agent_and_shape() {
    let fx = setup();

    // Stream-timeout: two identical sweeps → one publish, second deduped.
    let cap_st = "compiling\nRequest timed out\n";
    run_stuck_eval_full(&fx, "feat-d", 60, "", "", "", cap_st);
    let second_st = run_stuck_eval_full(&fx, "feat-d", 60, "", "", "", cap_st);
    assert!(
        second_st.contains("deduped"),
        "second stream-timeout detection must be deduped, got: {second_st}"
    );

    // No-progress: two identical sweeps (prior snapshot pinned old) → one
    // publish, second deduped. Different agent so the shape keys are isolated.
    seed_progress(&fx, "feat-d2", "7", "2", 0);
    run_stuck_eval_full(&fx, "feat-d2", 60, "7", "2", "", "thinking\n");
    let second_np = run_stuck_eval_full(&fx, "feat-d2", 60, "7", "2", "", "thinking\n");
    assert!(
        second_np.contains("deduped"),
        "second no-progress detection must be deduped, got: {second_np}"
    );

    wait_for_bodies(&fx, 2, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);
    assert_eq!(
        fx.bodies.lock().unwrap().len(),
        2,
        "each shape publishes exactly once per window: one stream-timeout + one no-progress"
    );
}
