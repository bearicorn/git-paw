//! Behaviour tests for the root-side `answer` and `advanced-main` verbs in
//! `<repo>/.git-paw/scripts/sweep.sh`. A localhost stub records every
//! `POST /publish` body so the tests assert the published message shape, and
//! each body is also decoded as a `BrokerMessage` so the helper and the
//! broker wire format cannot drift apart.
//!
//! Maps to
//! openspec/changes/orchestrator-skill-export/specs/broker-agent-helper/spec.md
//! "sweep.sh publishes the orchestrator's answers and main-advance events":
//! - "answer publishes an agent.answer to the recipient"
//! - "advanced-main resolves base and SHA itself"
//! - "A missing argument publishes nothing"
//! - "Neither verb is granted by setup-cli"

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use assert_cmd::Command;
use git_paw::broker::messages::BrokerMessage;
use git_paw::cli_setup::facts::{SWEEP_GRANTED, SWEEP_NEVER_GRANTED, planned_allow_entries};
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

fn run_sweep(fx: &Fixture, args: &[&str]) -> std::process::Output {
    StdCommand::new("bash")
        .arg(&fx.sweep)
        .args(args)
        .current_dir(&fx.root)
        .output()
        .expect("run sweep.sh")
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

fn git_stdout(root: &std::path::Path, args: &[&str]) -> String {
    let out = StdCommand::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
#[serial]
fn answer_publishes_an_agent_answer_to_the_recipient() {
    let fx = setup();
    let out = run_sweep(&fx, &["answer", "feat-a", "use RS256", "q-auth"]);
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        out.status.success(),
        "answer should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one publish: {bodies:?}");
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("json");
    assert_eq!(v["type"], "agent.answer");
    assert_eq!(v["agent_id"], "feat-a", "agent_id is the recipient");
    assert_eq!(v["payload"]["from"], "supervisor");
    assert_eq!(v["payload"]["answer"], "use RS256");
    assert_eq!(v["payload"]["re"], "q-auth");

    let msg =
        BrokerMessage::from_json(&bodies[0]).expect("decodes and validates as a BrokerMessage");
    assert!(
        matches!(msg, BrokerMessage::Answer { ref agent_id, .. } if agent_id == "feat-a"),
        "decodes as an agent.answer to the recipient: {msg:?}"
    );
}

#[test]
#[serial]
fn answer_without_re_omits_the_field() {
    let fx = setup();
    let out = run_sweep(&fx, &["answer", "feat-a", "carry on"]);
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(out.status.success());
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1);
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("json");
    assert!(
        v["payload"].get("re").is_none(),
        "no re when not given: {v}"
    );
}

#[test]
#[serial]
fn advanced_main_resolves_base_and_sha_itself() {
    let fx = setup();
    // A default branch that is not literally `main` proves `base` is
    // resolved from the checkout rather than hard-coded.
    git_stdout(&fx.root, &["branch", "-m", "trunk"]);
    let expected_sha = git_stdout(&fx.root, &["rev-parse", "--short=12", "HEAD"]);

    let out = run_sweep(&fx, &["advanced-main", "feat/auth", "auth landed"]);
    wait_for_bodies(&fx, 1, Duration::from_secs(5));
    fx.stop.store(true, Ordering::SeqCst);

    assert!(
        out.status.success(),
        "advanced-main should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let bodies = fx.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one publish: {bodies:?}");
    let v: serde_json::Value = serde_json::from_str(&bodies[0]).expect("json");
    assert_eq!(v["type"], "agent.advanced-main");
    assert_eq!(v["from"], "supervisor");
    assert_eq!(v["merged_branch"], "feat/auth");
    assert_eq!(v["base"], "trunk");
    assert_eq!(v["new_main_sha"], expected_sha.as_str());
    assert_eq!(expected_sha.len(), 12);
    assert_eq!(v["summary"], "auth landed");

    let msg =
        BrokerMessage::from_json(&bodies[0]).expect("decodes and validates as a BrokerMessage");
    match msg {
        BrokerMessage::AdvancedMain { payload } => {
            assert_eq!(payload.base, "trunk");
            assert!(
                (chrono::Utc::now() - payload.merged_at).num_minutes().abs() < 5,
                "merged_at is the current UTC time"
            );
        }
        other => panic!("expected AdvancedMain, got {other:?}"),
    }
}

#[test]
#[serial]
fn a_missing_argument_publishes_nothing() {
    let fx = setup();
    let cases: [&[&str]; 4] = [
        &["answer", "feat-a"],
        &["answer", "", "text"],
        &["advanced-main", "feat/auth"],
        &["advanced-main", "", "summary"],
    ];
    for args in cases {
        let out = run_sweep(&fx, args);
        assert!(!out.status.success(), "{args:?} should exit non-zero");
        let stderr = String::from_utf8_lossy(&out.stderr);
        // The verb's own diagnostic, not the unknown-subcommand usage dump.
        let expected = if args[0] == "answer" {
            "answer <agent-id> <text> [re]"
        } else {
            "advanced-main <merged-branch> <summary>"
        };
        assert!(
            stderr.contains("usage: ") && stderr.contains(expected),
            "{args:?} should print its own diagnostic; stderr: {stderr}"
        );
        assert!(
            !stderr.contains("unknown subcommand"),
            "{args:?} must be a known verb; stderr: {stderr}"
        );
    }
    thread::sleep(Duration::from_millis(300));
    fx.stop.store(true, Ordering::SeqCst);
    assert!(
        fx.bodies.lock().unwrap().is_empty(),
        "nothing may be published on a missing argument"
    );
}

#[test]
fn neither_verb_is_granted_by_setup_cli() {
    for verb in ["answer", "advanced-main"] {
        assert!(
            SWEEP_NEVER_GRANTED.contains(&verb),
            "{verb} must be listed as never granted"
        );
        assert!(!SWEEP_GRANTED.contains(&verb), "{verb} must not be granted");
        for entry in planned_allow_entries() {
            assert!(
                !entry.contains(&format!("sweep.sh {verb}")),
                "{entry} grants {verb}"
            );
        }
    }
}
