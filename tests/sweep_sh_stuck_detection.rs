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
    }
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
