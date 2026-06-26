//! Asserts the installed `<repo>/.git-paw/scripts/broker.sh` discovers the
//! broker URL from `<repo>/.git-paw/config.toml [broker]` (non-default port)
//! rather than hardcoding `http://127.0.0.1:9119`. Spawns a tiny localhost
//! HTTP stub on a free port, configures the script via the broker-URL
//! discovery path, and verifies the script's `status` subcommand POSTs the
//! expected `agent.status` payload to the configured URL.
//!
//! Mirrors `sweep_sh_session_discovery` for the agent-side helper
//! (`agent-broker-helper` / "Helper discovers the broker URL from config").

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn init_git_repo(dir: &std::path::Path) {
    let st = StdCommand::new("git")
        .current_dir(dir)
        .args(["init", "-b", "main"])
        .status()
        .expect("git init");
    assert!(st.success());
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["config", "user.email", "test@test.com"])
        .status();
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["config", "user.name", "Test"])
        .status();
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

/// Spawn a one-shot HTTP server that captures the first request's
/// method+path+body, replies 200, and returns the captured request line via
/// the shared slot. Returns the bound port + a shutdown flag.
fn spawn_capture_stub(captured: Arc<Mutex<String>>) -> (u16, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let body = "{}";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("non-blocking listener");
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !stop_clone.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
                    let mut buf = [0u8; 4096];
                    let n = socket.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    if let Ok(mut slot) = captured.lock()
                        && slot.is_empty()
                    {
                        *slot = req;
                    }
                    let _ = socket.write_all(response.as_bytes());
                    let _ = socket.flush();
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    });
    (port, stop)
}

#[test]
#[serial]
fn broker_sh_targets_configured_broker_port() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    // git paw init installs broker.sh.
    let init_out = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("git paw init");
    assert!(init_out.status.success());

    // Spawn the broker stub on a free port and rewrite config.toml so the
    // script discovers the port we control (non-default).
    let captured = Arc::new(Mutex::new(String::new()));
    let (port, stop) = spawn_capture_stub(captured.clone());
    assert_ne!(port, 9119, "ephemeral port must differ from the default");
    let config = format!("[broker]\nenabled = true\nport = {port}\n");
    fs::write(tmp.path().join(".git-paw/config.toml"), config).expect("write config.toml");

    // Run `.git-paw/scripts/broker.sh --agent feat-x status booting`.
    let broker = tmp.path().join(".git-paw/scripts/broker.sh");
    let output = StdCommand::new("bash")
        .arg(&broker)
        .args(["--agent", "feat-x", "status", "booting"])
        .current_dir(tmp.path())
        .output()
        .expect("run broker.sh status");
    stop.store(true, Ordering::SeqCst);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "broker.sh status should succeed; stderr:\n{stderr}"
    );

    let req = captured.lock().unwrap().clone();
    assert!(
        req.starts_with("POST /publish"),
        "broker.sh should POST to /publish on the configured port; captured request:\n{req}"
    );
    assert!(
        req.contains("\"type\": \"agent.status\"") || req.contains("\"type\":\"agent.status\""),
        "broker.sh status should publish an agent.status payload; captured:\n{req}"
    );
    assert!(
        req.contains("feat-x"),
        "payload should carry the --agent id; captured:\n{req}"
    );
    assert!(
        req.contains("booting"),
        "payload should carry the status message; captured:\n{req}"
    );
}
