//! Behaviour tests for the widened `status-publish` verb in
//! `<repo>/.git-paw/scripts/sweep.sh` (broker-helper-full-surface tasks
//! 4.1–4.3). A localhost stub records every `POST /publish` body so the tests
//! assert the shaped `agent.status` payload:
//!
//! - the plain form (no flags) publishes `agent_id="supervisor"`,
//!   `status="working"`, the message, `modified_files=[]`, and NO
//!   `phase`/`detail` keys (v0.5.0 wire shape);
//! - the rich form embeds a `phase` label and a structured `detail` object;
//! - a `--detail` that does not parse to a JSON object is rejected (non-zero
//!   exit, stderr diagnostic) and nothing is published.
//!
//! Maps to
//! openspec/changes/broker-helper-full-surface/specs/agent-broker-helper/spec.md
//! scenarios "status-publish plain form preserves the v0.5.0 shape",
//! "status-publish carries a phase and a structured detail", and
//! "status-publish rejects a non-object detail argument".

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

fn run_status_publish(fx: &Fixture, args: &[&str]) -> std::process::Output {
    StdCommand::new("bash")
        .arg(&fx.sweep)
        .arg("status-publish")
        .args(args)
        .current_dir(&fx.root)
        .output()
        .expect("run sweep.sh status-publish")
}

/// Polls until the recorder has captured at least `min` publishes, or the
/// timeout elapses. The curl POST completes before the command returns, but
/// the recorder thread records the body asynchronously — polling closes that
/// window without a fixed sleep that could race under parallel load.
fn wait_for_bodies(fx: &Fixture, min: usize, timeout: Duration) {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if fx.bodies.lock().unwrap().len() >= min {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Task 4.1: the plain `status-publish <msg>` form (no flags) produces an
/// `agent.status` with `agent_id="supervisor"`, `status="working"`, the
/// message, `modified_files=[]`, and NO `phase`/`detail` keys.
#[test]
#[serial]
fn plain_form_publishes_working_status_without_phase_or_detail() {
    let fx = setup();
    let out = run_status_publish(&fx, &["merge orchestration complete"]);
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        out.status.success(),
        "plain status-publish should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(
        bodies.len(),
        1,
        "expected exactly one publish, got {bodies:?}"
    );
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("valid json");
    assert_eq!(v["type"], "agent.status");
    assert_eq!(v["agent_id"], "supervisor");
    assert_eq!(v["payload"]["status"], "working");
    assert_eq!(v["payload"]["message"], "merge orchestration complete");
    assert_eq!(v["payload"]["modified_files"], serde_json::json!([]));
    assert!(
        v["payload"].get("phase").is_none(),
        "plain form must omit the phase key: {}",
        bodies[0]
    );
    assert!(
        v["payload"].get("detail").is_none(),
        "plain form must omit the detail key: {}",
        bodies[0]
    );
}

/// Task 4.2: `status-publish --phase audit --detail '{…}' "<msg>"` produces an
/// `agent.status` with `phase="audit"` and a `detail` object carrying `branch`
/// and `audit_step`, with the message preserved.
#[test]
#[serial]
fn rich_form_embeds_phase_and_structured_detail() {
    let fx = setup();
    let out = run_status_publish(
        &fx,
        &[
            "--phase",
            "audit",
            "--detail",
            r#"{"branch":"feat/auth","audit_step":"tests"}"#,
            "auditing feat/auth",
        ],
    );
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        out.status.success(),
        "rich status-publish should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(
        bodies.len(),
        1,
        "expected exactly one publish, got {bodies:?}"
    );
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("valid json");
    assert_eq!(v["type"], "agent.status");
    assert_eq!(v["agent_id"], "supervisor");
    assert_eq!(v["payload"]["status"], "working");
    assert_eq!(v["payload"]["message"], "auditing feat/auth");
    assert_eq!(v["payload"]["phase"], "audit");
    assert_eq!(
        v["payload"]["detail"]["branch"], "feat/auth",
        "detail.branch must be carried through: {}",
        bodies[0]
    );
    assert_eq!(
        v["payload"]["detail"]["audit_step"], "tests",
        "detail.audit_step must be carried through: {}",
        bodies[0]
    );
}

/// A `--phase` without a `--detail` embeds only the phase; the detail key
/// stays absent. Guards the "phase only" branch of the shaping logic.
#[test]
#[serial]
fn phase_only_form_omits_detail() {
    let fx = setup();
    let out = run_status_publish(&fx, &["--phase", "idle", "waiting for the next event"]);
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        out.status.success(),
        "phase-only status-publish should exit 0"
    );
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(
        bodies.len(),
        1,
        "expected exactly one publish, got {bodies:?}"
    );
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("valid json");
    assert_eq!(v["payload"]["phase"], "idle");
    assert!(
        v["payload"].get("detail").is_none(),
        "phase-only form must omit the detail key: {}",
        bodies[0]
    );
}

/// Task 4.3: `status-publish --detail 'not-json' "msg"` exits non-zero, writes
/// a stderr diagnostic, and publishes nothing.
#[test]
#[serial]
fn non_json_detail_is_rejected_and_publishes_nothing() {
    let fx = setup();
    let out = run_status_publish(&fx, &["--phase", "audit", "--detail", "not-json", "msg"]);
    // No publish should occur; give a stray POST a brief window to surface.
    thread::sleep(Duration::from_millis(250));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        !out.status.success(),
        "a non-JSON --detail must make status-publish exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--detail"),
        "stderr must carry a --detail diagnostic; got: {stderr}"
    );
    assert!(
        fx.bodies.lock().unwrap().is_empty(),
        "a rejected --detail must publish nothing"
    );
}

/// A `--detail` that is valid JSON but not an object (e.g. an array) is also
/// rejected — the broker's `detail: Option<Value>` contract expects an object.
#[test]
#[serial]
fn json_array_detail_is_rejected_and_publishes_nothing() {
    let fx = setup();
    let out = run_status_publish(&fx, &["--detail", "[1,2,3]", "msg"]);
    thread::sleep(Duration::from_millis(250));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        !out.status.success(),
        "a non-object (array) --detail must make status-publish exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("must be a JSON object"),
        "stderr must explain the detail must be a JSON object; got: {stderr}"
    );
    assert!(
        fx.bodies.lock().unwrap().is_empty(),
        "a rejected --detail must publish nothing"
    );
}
