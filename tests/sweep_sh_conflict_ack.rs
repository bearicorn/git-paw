//! Behaviour tests for the `conflict-ack` verb in
//! `<repo>/.git-paw/scripts/sweep.sh` (conflict-escalation-persistence task
//! 4.1). A localhost stub records every `POST /publish` body so the tests
//! assert the shaped `agent.feedback` payload:
//!
//! - both agents receive the acknowledgement, tagged with the detector's
//!   `[conflict-ack]` token and naming the pair and the file;
//! - a free-text note rides as a SECOND error entry, leaving the tag entry
//!   parseable (the file path runs to the end of its string);
//! - a missing argument is rejected and nothing is published.
//!
//! Maps to
//! openspec/changes/conflict-escalation-persistence/specs/broker-conflict-detection/spec.md
//! "The detector SHALL accept an acknowledgement for a pair and file" — this
//! is the orchestrator-reachable path to that acceptance.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use assert_cmd::Command;
use git_paw::broker::conflict::{CONFLICT_ACK_TAG, parse_conflict_ack};
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
                            bodies_c.lock().unwrap().push(req[idx + 4..].to_string());
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

fn run_conflict_ack(fx: &Fixture, args: &[&str]) -> std::process::Output {
    StdCommand::new("bash")
        .arg(&fx.sweep)
        .arg("conflict-ack")
        .args(args)
        .current_dir(&fx.root)
        .output()
        .expect("run sweep.sh conflict-ack")
}

/// Polls until the recorder has captured at least `min` publishes, or the
/// timeout elapses.
fn wait_for_bodies(fx: &Fixture, min: usize, timeout: Duration) {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if fx.bodies.lock().unwrap().len() >= min {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn parsed(bodies: &[String]) -> Vec<serde_json::Value> {
    bodies
        .iter()
        .map(|b| serde_json::from_str(b).expect("valid json"))
        .collect()
}

#[test]
#[serial]
fn conflict_ack_publishes_a_tagged_feedback_to_both_agents() {
    let fx = setup();
    let out = run_conflict_ack(&fx, &["feat-x", "feat-y", "src/a.rs"]);
    wait_for_bodies(&fx, 2, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        out.status.success(),
        "conflict-ack should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(
        bodies.len(),
        2,
        "expected one publish per agent: {bodies:?}"
    );
    let msgs = parsed(&bodies);
    let mut targets: Vec<String> = Vec::new();
    for v in &msgs {
        assert_eq!(v["type"], "agent.feedback");
        assert_eq!(
            v["payload"]["from"], "supervisor",
            "only supervisor-sourced feedback acknowledges a conflict"
        );
        let errors = v["payload"]["errors"].as_array().expect("errors array");
        assert_eq!(
            errors.len(),
            2,
            "the tag entry plus a default human-readable note: {v}"
        );
        let text = errors[0].as_str().expect("error text");
        assert!(
            text.starts_with(CONFLICT_ACK_TAG),
            "error must carry the acknowledgement tag: {text}"
        );
        assert_eq!(
            parse_conflict_ack(text),
            Some((
                "feat-x".to_string(),
                "feat-y".to_string(),
                "src/a.rs".to_string()
            )),
            "the detector must be able to parse what the helper publishes"
        );
        // Agents are taught that `errors` are problems to fix, so a bare tag
        // line would read as a cryptic error. The default note says there is
        // nothing to fix, in words.
        let note = errors[1].as_str().expect("default note");
        assert!(
            note.contains("src/a.rs") && note.contains("nothing for you to fix"),
            "default note must name the file and say there is nothing to fix: {note}"
        );
        assert!(
            parse_conflict_ack(note).is_none(),
            "the note must not itself parse as an acknowledgement: {note}"
        );
        targets.push(v["agent_id"].as_str().expect("agent_id").to_string());
    }
    targets.sort();
    assert_eq!(targets, vec!["feat-x".to_string(), "feat-y".to_string()]);
}

#[test]
#[serial]
fn conflict_ack_note_rides_as_a_separate_error_entry() {
    let fx = setup();
    let out = run_conflict_ack(
        &fx,
        &[
            "feat-x",
            "feat-y",
            "src/a.rs",
            "feat-x owns this file until it merges",
        ],
    );
    wait_for_bodies(&fx, 2, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        out.status.success(),
        "conflict-ack with a note should exit 0"
    );
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(
        bodies.len(),
        2,
        "expected one publish per agent: {bodies:?}"
    );
    for v in parsed(&bodies) {
        let errors = v["payload"]["errors"].as_array().expect("errors array");
        assert_eq!(errors.len(), 2, "tag and note are separate entries: {v}");
        assert!(
            parse_conflict_ack(errors[0].as_str().expect("tag text")).is_some(),
            "the tag entry stays parseable alongside a note: {v}"
        );
        assert_eq!(errors[1], "feat-x owns this file until it merges");
    }
}

#[test]
#[serial]
fn conflict_ack_without_a_file_is_rejected_and_publishes_nothing() {
    let fx = setup();
    let out = run_conflict_ack(&fx, &["feat-x", "feat-y"]);
    thread::sleep(Duration::from_millis(250));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        !out.status.success(),
        "a conflict-ack missing the file must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("conflict-ack"),
        "stderr must carry a usage diagnostic; got: {stderr}"
    );
    assert!(
        fx.bodies.lock().unwrap().is_empty(),
        "a rejected conflict-ack must publish nothing"
    );
}
