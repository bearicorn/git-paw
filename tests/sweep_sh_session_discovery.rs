//! Asserts that `<repo>/.git-paw/scripts/sweep.sh` discovers the session
//! name from `<repo>/.git-paw/sessions/*.json` instead of hardcoding
//! `paw-git-paw`. Spawns a tiny localhost HTTP stub on a free port,
//! configures the script via the broker URL discovery path, and verifies
//! the script's `status` subcommand renders the agent ids reported by the
//! stub. (task 3.9)

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command as StdCommand;
use std::sync::Arc;
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

/// Spawn a one-shot HTTP server that responds to GET /status with the given
/// JSON body and ignores everything else. Returns the bound port + a shutdown
/// flag.
fn spawn_status_stub(body: &str) -> (u16, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(false).ok();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
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
                    let mut buf = [0u8; 1024];
                    let _ = socket.read(&mut buf);
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
fn sweep_sh_discovers_session_name_and_broker_port() {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());

    // git paw init installs sweep.sh.
    let init_out = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("git paw init");
    assert!(init_out.status.success());

    // Seed a session JSON with a non-default session name.
    let sessions_dir = tmp.path().join(".git-paw/sessions");
    fs::create_dir_all(&sessions_dir).expect("mkdir sessions");
    let session_json = serde_json::json!({
        "session_name": "paw-myproject",
        "repo_path": tmp.path().to_string_lossy(),
        "project_name": "myproject",
        "created_at": "2026-01-01T00:00:00Z",
        "status": "stopped",
        "mode": "bare",
        "worktrees": [],
    });
    fs::write(
        sessions_dir.join("paw-myproject.json"),
        serde_json::to_string_pretty(&session_json).unwrap(),
    )
    .expect("write session.json");

    // Spawn the broker stub on a free port and rewrite config.toml so the
    // script discovers the port we control.
    let body = r#"{"agents":[{"agent_id":"feat-x","status":"working","last_seen_seconds":1},{"agent_id":"a","status":"working","last_seen_seconds":1}]}"#;
    let (port, stop) = spawn_status_stub(body);
    let config = format!("[broker]\nenabled = true\nport = {port}\n");
    fs::write(tmp.path().join(".git-paw/config.toml"), config).expect("write config.toml");

    // Run `.git-paw/scripts/sweep.sh status` and inspect stdout.
    let sweep = tmp.path().join(".git-paw/scripts/sweep.sh");
    let output = StdCommand::new("bash")
        .arg(&sweep)
        .arg("status")
        .current_dir(tmp.path())
        .env_remove("PAW_SESSION")
        .env_remove("PAW_BROKER")
        .output()
        .expect("run sweep.sh status");
    stop.store(true, Ordering::SeqCst);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "sweep.sh status should succeed; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("feat-x"),
        "sweep.sh status should list feat-x; stdout:\n{stdout}"
    );
    assert!(
        !stdout.lines().any(|l| l.starts_with("a ")),
        "phantom `a` should be filtered out of the table; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("phantoms (use --all to show):"),
        "phantoms summary line should mention the filter; stdout:\n{stdout}"
    );
}
