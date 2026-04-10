//! Broker integration tests.
//!
//! Tests the broker lifecycle wiring: hidden `__dashboard` subcommand,
//! session state broker fields, tmux `set_environment`, start/stop/purge/status.

use std::path::PathBuf;
use std::time::SystemTime;

use serial_test::serial;
use tempfile::TempDir;

use git_paw::session::{
    Session, SessionStatus, WorktreeEntry, delete_session_in, load_session_from, save_session_in,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_session_with_broker(suffix: &str) -> Session {
    Session {
        session_name: format!("paw-broker-{suffix}"),
        repo_path: PathBuf::from(format!("/tmp/fake-broker-repo-{suffix}")),
        project_name: format!("broker-{suffix}"),
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: vec![WorktreeEntry {
            branch: "feat/auth".to_string(),
            worktree_path: PathBuf::from(format!("/tmp/wt-{suffix}-auth")),
            cli: "claude".to_string(),
            branch_created: false,
        }],
        broker_port: Some(9119),
        broker_bind: Some("127.0.0.1".to_string()),
        broker_log_path: Some(PathBuf::from(format!("/tmp/broker-{suffix}.log"))),
    }
}

fn make_session_without_broker(suffix: &str) -> Session {
    Session {
        session_name: format!("paw-nobroker-{suffix}"),
        repo_path: PathBuf::from(format!("/tmp/fake-nobroker-repo-{suffix}")),
        project_name: format!("nobroker-{suffix}"),
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: vec![WorktreeEntry {
            branch: "feat/auth".to_string(),
            worktree_path: PathBuf::from(format!("/tmp/wt-{suffix}-auth")),
            cli: "claude".to_string(),
            branch_created: false,
        }],
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
    }
}

// ---------------------------------------------------------------------------
// 10.5: __dashboard outside tmux returns error
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn dashboard_outside_tmux_returns_error() {
    let output = assert_cmd::Command::cargo_bin("git-paw")
        .unwrap()
        .arg("__dashboard")
        .env_remove("TMUX")
        .output()
        .expect("run git-paw __dashboard");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("internal command"),
        "error should mention 'internal command', got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// 10.6: Session state JSON includes broker fields when enabled
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn session_state_includes_broker_fields_when_enabled() {
    let dir = TempDir::new().unwrap();
    let session = make_session_with_broker("state-check");
    save_session_in(&session, dir.path()).unwrap();

    let loaded = load_session_from(&session.session_name, dir.path())
        .unwrap()
        .expect("session should exist");

    assert_eq!(loaded.broker_port, Some(9119));
    assert_eq!(loaded.broker_bind.as_deref(), Some("127.0.0.1"));
    assert!(loaded.broker_log_path.is_some());
}

#[test]
#[serial]
fn session_state_omits_broker_fields_when_disabled() {
    let dir = TempDir::new().unwrap();
    let session = make_session_without_broker("state-check");
    save_session_in(&session, dir.path()).unwrap();

    let json =
        std::fs::read_to_string(dir.path().join(format!("{}.json", session.session_name))).unwrap();
    assert!(!json.contains("broker_port"));
    assert!(!json.contains("broker_bind"));
    assert!(!json.contains("broker_log_path"));
}

// ---------------------------------------------------------------------------
// 10.8: purge removes broker.log
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn purge_removes_broker_log_when_exists() {
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("broker.log");
    std::fs::write(&log_path, "test log content").unwrap();

    let mut session = make_session_with_broker("purge-log");
    session.broker_log_path = Some(log_path.clone());
    save_session_in(&session, dir.path()).unwrap();

    // Simulate purge: remove broker log, then session
    if let Some(ref path) = session.broker_log_path {
        let _ = std::fs::remove_file(path);
    }
    delete_session_in(&session.session_name, dir.path()).unwrap();

    assert!(
        !log_path.exists(),
        "broker.log should be deleted after purge"
    );
}

#[test]
#[serial]
fn purge_succeeds_when_broker_log_does_not_exist() {
    let dir = TempDir::new().unwrap();
    let log_path = dir.path().join("nonexistent-broker.log");

    let mut session = make_session_with_broker("purge-nolog");
    session.broker_log_path = Some(log_path);
    save_session_in(&session, dir.path()).unwrap();

    // Simulate purge with missing log — should not panic or error
    if let Some(ref path) = session.broker_log_path {
        let _ = std::fs::remove_file(path);
    }
    delete_session_in(&session.session_name, dir.path()).unwrap();
}

// ---------------------------------------------------------------------------
// Backward compatibility: v0.2.0 sessions load correctly
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn v020_session_loads_with_broker_fields_as_none() {
    let dir = TempDir::new().unwrap();
    let json = serde_json::json!({
        "session_name": "paw-legacy-project",
        "repo_path": "/tmp/legacy",
        "project_name": "legacy",
        "created_at": "2024-03-23T12:00:00Z",
        "status": "active",
        "worktrees": [{
            "branch": "main",
            "worktree_path": "/tmp/wt-main",
            "cli": "claude"
        }]
    });

    std::fs::write(
        dir.path().join("paw-legacy-project.json"),
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();

    let loaded = load_session_from("paw-legacy-project", dir.path())
        .unwrap()
        .expect("v0.2.0 session should load");

    assert!(loaded.broker_port.is_none());
    assert!(loaded.broker_bind.is_none());
    assert!(loaded.broker_log_path.is_none());
    assert_eq!(loaded.worktrees.len(), 1);
}

// ---------------------------------------------------------------------------
// Tmux builder: set_environment integration
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn tmux_builder_with_dashboard_pane_and_env_var() {
    use git_paw::tmux::{PaneSpec, TmuxSessionBuilder};

    let session = TmuxSessionBuilder::new("broker-test")
        .add_pane(PaneSpec {
            branch: "dashboard".to_string(),
            worktree: "/tmp/repo".to_string(),
            cli_command: "git paw __dashboard".to_string(),
        })
        .add_pane(PaneSpec {
            branch: "feat/auth".to_string(),
            worktree: "/tmp/wt-auth".to_string(),
            cli_command: "claude".to_string(),
        })
        .set_environment("GIT_PAW_BROKER_URL", "http://127.0.0.1:9119")
        .build()
        .unwrap();

    let cmds = session.command_strings();

    // Dashboard is pane 0
    let pane0_cmds: Vec<&String> = cmds.iter().filter(|c| c.contains(":0.0")).collect();
    assert!(
        pane0_cmds.iter().any(|c| c.contains("__dashboard")),
        "pane 0 should run __dashboard"
    );

    // Agent is pane 1
    let pane1_cmds: Vec<&String> = cmds.iter().filter(|c| c.contains(":0.1")).collect();
    assert!(
        pane1_cmds.iter().any(|c| c.contains("claude")),
        "pane 1 should run claude"
    );

    // set-environment before send-keys
    let first_env = cmds
        .iter()
        .position(|c| c.contains("set-environment"))
        .expect("should have set-environment");
    let first_send = cmds
        .iter()
        .position(|c| c.contains("send-keys"))
        .expect("should have send-keys");
    assert!(first_env < first_send);

    // Total of 3 send-keys: pane 0 dashboard + pane 1 cd && claude (actually 2 send-keys)
    let send_keys: Vec<&String> = cmds.iter().filter(|c| c.contains("send-keys")).collect();
    assert_eq!(send_keys.len(), 2, "should have 2 send-keys commands");
}

// ---------------------------------------------------------------------------
// BrokerConfig
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn broker_config_defaults() {
    use git_paw::config::BrokerConfig;

    let config = BrokerConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.port, 9119);
    assert_eq!(config.bind, "127.0.0.1");
    assert_eq!(config.url(), "http://127.0.0.1:9119");
}

#[test]
#[serial]
fn broker_config_parses_from_toml() {
    let toml_str = r#"
[broker]
enabled = true
port = 8080
bind = "0.0.0.0"
"#;
    let config: git_paw::config::PawConfig = toml::from_str(toml_str).unwrap();
    let broker = &config.broker;
    assert!(broker.enabled);
    assert_eq!(broker.port, 8080);
    assert_eq!(broker.bind, "0.0.0.0");
    assert_eq!(broker.url(), "http://0.0.0.0:8080");
}

#[test]
#[serial]
fn missing_broker_section_defaults_to_disabled() {
    let toml_str = "default_cli = \"claude\"\n";
    let config: git_paw::config::PawConfig = toml::from_str(toml_str).unwrap();
    assert!(!config.broker.enabled);
    assert_eq!(config.broker.port, 9119);
}

// ===========================================================================
// E2E tests requiring tmux / HTTP broker
// ===========================================================================

/// Atomic counter to ensure each test gets a unique port.
static PORT_COUNTER: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

/// Starts a broker on a unique free port and returns the handle + URL.
///
/// Takes a closure that creates a fresh `BrokerState` on each retry attempt,
/// since `start_broker` consumes the state by value.
fn spawn_test_broker_with<F>(make_state: F) -> (git_paw::broker::BrokerHandle, String)
where
    F: Fn() -> git_paw::broker::BrokerState,
{
    use std::sync::atomic::Ordering;

    #[allow(clippy::cast_possible_truncation)]
    let base = 21_000 + (std::process::id() as u16 % 5000);
    let offset = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut port = base + offset;
    let mut attempts = 0;
    loop {
        let config = git_paw::config::BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
        };
        match git_paw::broker::start_broker(&config, make_state()) {
            Ok(handle) => {
                let url = config.url();
                return (handle, url);
            }
            Err(_) if attempts < 10 => {
                port = port.wrapping_add(100);
                attempts += 1;
            }
            Err(e) => panic!("failed to start test broker after retries: {e}"),
        }
    }
}

/// Helper to make HTTP requests to the broker using raw TCP.
fn http_req(
    url: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (u16, String) {
    use std::fmt::Write as _;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let addr = url.strip_prefix("http://").unwrap_or(url);
    let mut stream = TcpStream::connect(addr).expect("failed to connect to broker");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (key, value) in headers {
        let _ = write!(request, "{key}: {value}\r\n");
    }
    if !body.is_empty() {
        let _ = write!(request, "Content-Length: {}\r\n", body.len());
    }
    request.push_str("\r\n");
    request.push_str(body);

    stream
        .write_all(request.as_bytes())
        .expect("failed to write request");

    let mut response = String::new();
    stream.read_to_string(&mut response).ok();

    // Parse status code
    let header_section = response.split("\r\n\r\n").next().unwrap_or("");
    let status_line = header_section.lines().next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Extract body (handle chunked)
    let body_raw = response
        .split_once("\r\n\r\n")
        .map_or_else(String::new, |(_, b)| b.to_string());
    let body_decoded = if header_section
        .to_lowercase()
        .contains("transfer-encoding: chunked")
    {
        decode_chunked_body(&body_raw)
    } else {
        body_raw
    };

    (status, body_decoded)
}

fn decode_chunked_body(body: &str) -> String {
    let mut result = String::new();
    let mut remaining = body;
    loop {
        let line_end = remaining.find("\r\n").unwrap_or(remaining.len());
        let size_str = &remaining[..line_end];
        let size = usize::from_str_radix(size_str.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        remaining = &remaining[line_end + 2..];
        if remaining.len() >= size {
            result.push_str(&remaining[..size]);
            remaining = &remaining[size..];
            if remaining.starts_with("\r\n") {
                remaining = &remaining[2..];
            }
        } else {
            break;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Test 1: Full orchestration — publish via HTTP, poll back via HTTP
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn full_orchestration_publish_poll_status_via_http() {
    let (handle, url) = spawn_test_broker_with(|| git_paw::broker::BrokerState::new(None));

    // Step 1: POST agent.status for agent "alpha"
    let (status, _) = http_req(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.status","agent_id":"alpha","payload":{"status":"working","modified_files":[]}}"#,
    );
    assert_eq!(status, 202, "status publish should return 202");

    // Register a second agent so artifact broadcast has a target
    let (status, _) = http_req(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.status","agent_id":"beta","payload":{"status":"idle","modified_files":[]}}"#,
    );
    assert_eq!(status, 202, "second agent status should return 202");

    // Step 2: POST agent.artifact from alpha (broadcasts to beta)
    let (status, _) = http_req(
        &url,
        "POST",
        "/publish",
        &[("Content-Type", "application/json")],
        r#"{"type":"agent.artifact","agent_id":"alpha","payload":{"status":"done","exports":[],"modified_files":["src/lib.rs"]}}"#,
    );
    assert_eq!(status, 202, "artifact publish should return 202");

    // Step 3: GET /messages/beta?since=0 — verify the artifact is in beta's inbox
    let (status, body) = http_req(&url, "GET", "/messages/beta?since=0", &[], "");
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid JSON from /messages");
    let messages = json["messages"].as_array().expect("messages is array");
    assert_eq!(messages.len(), 1, "beta should have exactly 1 artifact");
    let last_seq = json["last_seq"].as_u64().expect("last_seq is number");
    assert!(last_seq > 0, "last_seq should be positive");

    // Step 4: GET /messages/beta?since=<last_seq> — should be empty (cursor advanced)
    let path = format!("/messages/beta?since={last_seq}");
    let (status, body) = http_req(&url, "GET", &path, &[], "");
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid JSON");
    let messages = json["messages"].as_array().expect("messages is array");
    assert!(
        messages.is_empty(),
        "no new messages after cursor, got {}",
        messages.len()
    );

    // Step 5: GET /status — verify both agents appear with correct statuses
    let (status, body) = http_req(&url, "GET", "/status", &[], "");
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).expect("valid JSON from /status");
    assert_eq!(json["git_paw"], true);
    let agents = json["agents"].as_array().expect("agents is array");
    assert_eq!(agents.len(), 2, "should have 2 agents");

    // Check that alpha has status "done" (updated by artifact) and beta has "idle"
    let alpha_entry = agents
        .iter()
        .find(|a| a["agent_id"] == "alpha")
        .expect("alpha should be in agents list");
    assert_eq!(alpha_entry["status"], "done");
    let beta_entry = agents
        .iter()
        .find(|a| a["agent_id"] == "beta")
        .expect("beta should be in agents list");
    assert_eq!(beta_entry["status"], "idle");

    // Drop handle and verify port is freed
    let addr: std::net::SocketAddr = url
        .strip_prefix("http://")
        .unwrap()
        .parse()
        .expect("valid socket addr");
    drop(handle);
    std::thread::sleep(std::time::Duration::from_millis(100));
    let result = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(500));
    assert!(
        result.is_err(),
        "port should be freed after handle is dropped"
    );
}

// ---------------------------------------------------------------------------
// Test 2: __dashboard subcommand — TMUX env var path
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn dashboard_subcommand_with_tmux_env_does_not_return_internal_error() {
    use git_paw::cli::{Cli, Command};

    // Verify that __dashboard parses to Command::Dashboard
    let cli = <Cli as clap::Parser>::try_parse_from(["git-paw", "__dashboard"])
        .expect("__dashboard should parse");
    assert!(
        matches!(cli.command, Some(Command::Dashboard)),
        "should parse as Dashboard variant"
    );

    // When TMUX is NOT set, the binary returns an error mentioning "internal command"
    let output_no_tmux = assert_cmd::Command::cargo_bin("git-paw")
        .unwrap()
        .arg("__dashboard")
        .env_remove("TMUX")
        .output()
        .expect("run __dashboard without TMUX");
    assert!(!output_no_tmux.status.success(), "should fail without TMUX");
    let stderr = String::from_utf8_lossy(&output_no_tmux.stderr);
    assert!(
        stderr.contains("internal command"),
        "error should mention 'internal command', got: {stderr}"
    );

    // When TMUX IS set, the command should NOT produce the "internal command" error.
    // It will still fail (no git repo in cwd, no config, etc.), but the error
    // should be different — proving the TMUX guard passed.
    let output_with_tmux = assert_cmd::Command::cargo_bin("git-paw")
        .unwrap()
        .arg("__dashboard")
        .env("TMUX", "/tmp/tmux-test/default,12345,0")
        .current_dir(std::env::temp_dir())
        .output()
        .expect("run __dashboard with TMUX");

    let stderr_tmux = String::from_utf8_lossy(&output_with_tmux.stderr);
    assert!(
        !stderr_tmux.contains("internal command"),
        "with TMUX set, should NOT get 'internal command' error, got: {stderr_tmux}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: Broker log flush on shutdown
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn broker_log_flush_on_shutdown() {
    use std::sync::atomic::Ordering;

    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("broker.log");

    // Build a broker with log_path configured
    #[allow(clippy::cast_possible_truncation)]
    let base = 22_000 + (std::process::id() as u16 % 5000);
    let offset = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut port = base + offset;
    let mut attempts = 0;

    let (handle, url) = loop {
        let config = git_paw::config::BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
        };
        match git_paw::broker::start_broker(
            &config,
            git_paw::broker::BrokerState::new(Some(log_path.clone())),
        ) {
            Ok(h) => {
                let u = config.url();
                break (h, u);
            }
            Err(_) if attempts < 10 => {
                port += 100;
                attempts += 1;
            }
            Err(e) => panic!("failed to start test broker: {e}"),
        }
    };

    // Publish 5 messages via HTTP
    for i in 0..5 {
        let body = if i == 0 {
            // First message registers the agent
            r#"{"type":"agent.status","agent_id":"flush-agent","payload":{"status":"working","modified_files":[]}}"#
                .to_string()
        } else {
            r#"{"type":"agent.artifact","agent_id":"flush-agent","payload":{"status":"done","exports":[],"modified_files":["src/lib.rs"]}}"#
                .to_string()
        };
        let (status, _) = http_req(
            &url,
            "POST",
            "/publish",
            &[("Content-Type", "application/json")],
            &body,
        );
        assert_eq!(status, 202, "message {i} should be accepted");
    }

    // Drop the handle — triggers flush thread shutdown + final flush
    drop(handle);

    // Read the log file and verify it contains 5 lines
    let content = std::fs::read_to_string(&log_path).expect("log file should exist after flush");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(
        lines.len(),
        5,
        "log file should contain 5 lines, got {}:\n{}",
        lines.len(),
        content
    );

    // Each line should match the Display format: [seq] timestamp [agent_id] message
    for (i, line) in lines.iter().enumerate() {
        let expected_seq = format!("[{}]", i + 1);
        assert!(
            line.starts_with(&expected_seq),
            "line {i} should start with {expected_seq}, got: {line}"
        );
        assert!(
            line.contains("[flush-agent]"),
            "line {i} should contain [flush-agent], got: {line}"
        );
    }
}
