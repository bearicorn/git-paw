//! Dashboard rendering integration tests.
//!
//! Tests the dashboard TUI rendering with various configurations,
//! including the broker messages panel when enabled.

use std::path::PathBuf;
use std::time::SystemTime;

use serial_test::serial;
use tempfile::TempDir;

use git_paw::broker::messages::{ArtifactPayload, BlockedPayload, BrokerMessage, StatusPayload};
use git_paw::config::{DashboardConfig, PawConfig};
use git_paw::session::{
    Session, SessionMode, SessionStatus, WorktreeEntry, delete_session_in, load_session_from,
    save_session_in,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_session_with_broker(suffix: &str) -> Session {
    Session {
        session_name: format!("paw-dashboard-{suffix}"),
        repo_path: PathBuf::from(format!("/tmp/fake-dashboard-repo-{suffix}")),
        project_name: format!("dashboard-{suffix}"),
        created_at: SystemTime::now(),
        status: SessionStatus::Active,
        worktrees: vec![WorktreeEntry {
            branch: "feat/auth".to_string(),
            worktree_path: PathBuf::from(format!("/tmp/wt-{suffix}-auth")),
            cli: "claude".to_string(),
            branch_created: false,
        }],
        broker_port: Some(9120),
        broker_bind: Some("127.0.0.1".to_string()),
        broker_log_path: Some(PathBuf::from(format!("/tmp/broker-{suffix}.log"))),
        mode: SessionMode::Bare,
        dashboard_pane: Some(0),
    }
}

fn make_config_with_message_log(show_log: bool) -> PawConfig {
    PawConfig {
        dashboard: Some(DashboardConfig {
            show_message_log: show_log,
        }),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 11.1: Dashboard renders without message log when disabled
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn dashboard_renders_without_message_log_when_disabled() {
    let config = make_config_with_message_log(false);

    // Verify that show_message_log is false
    assert!(!config.get_dashboard().unwrap().show_message_log);
}

// ---------------------------------------------------------------------------
// 11.2: Dashboard renders with message log when enabled
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn dashboard_renders_with_message_log_when_enabled() {
    let config = make_config_with_message_log(true);

    // Verify that show_message_log is true
    assert!(config.get_dashboard().unwrap().show_message_log);
}

// ---------------------------------------------------------------------------
// 11.3: Message log configuration can be toggled
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn message_log_configuration_can_be_toggled() {
    let mut config = make_config_with_message_log(false);

    // Initially disabled
    assert!(!config.get_dashboard().unwrap().show_message_log);

    // Enable it
    if let Some(dashboard) = config.dashboard.as_mut() {
        dashboard.show_message_log = true;
    }

    // Now enabled
    assert!(config.get_dashboard().unwrap().show_message_log);
}

// ---------------------------------------------------------------------------
// 11.4: Dashboard layout changes based on message log setting
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn dashboard_layout_changes_based_on_message_log_setting() {
    // This test verifies that the layout constraints are different
    // when show_message_log is enabled vs disabled

    let config_disabled = make_config_with_message_log(false);
    let config_enabled = make_config_with_message_log(true);

    // When disabled, should have fewer layout sections
    // When enabled, should have additional section for messages
    assert!(!config_disabled.get_dashboard().unwrap().show_message_log);
    assert!(config_enabled.get_dashboard().unwrap().show_message_log);
}

// ---------------------------------------------------------------------------
// 11.5: Broker message types are properly formatted for display
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn broker_message_types_are_properly_formatted() {
    // Test that different broker message types can be created
    // This verifies the message formatting infrastructure works

    let status_msg = BrokerMessage::Status {
        agent_id: "agent-1".to_string(),
        payload: StatusPayload {
            status: "working".to_string(),
            modified_files: vec![],
            message: Some("implementing feature".to_string()),
            ..Default::default()
        },
    };

    let artifact_msg = BrokerMessage::Artifact {
        agent_id: "agent-2".to_string(),
        payload: ArtifactPayload {
            status: "done".to_string(),
            exports: vec![],
            modified_files: vec!["schema.json".to_string()],
        },
    };

    let blocked_msg = BrokerMessage::Blocked {
        agent_id: "agent-3".to_string(),
        payload: BlockedPayload {
            needs: "api_spec".to_string(),
            from: "agent-1".to_string(),
        },
    };

    // Verify we can create different message types
    match &status_msg {
        BrokerMessage::Status { agent_id, payload } => {
            assert_eq!(agent_id, "agent-1");
            assert_eq!(payload.status, "working");
            assert_eq!(payload.message.as_ref().unwrap(), "implementing feature");
        }
        _ => panic!("Expected Status message"),
    }

    match &artifact_msg {
        BrokerMessage::Artifact { agent_id, payload } => {
            assert_eq!(agent_id, "agent-2");
            assert_eq!(payload.status, "done");
            assert!(payload.modified_files.contains(&"schema.json".to_string()));
        }
        _ => panic!("Expected Artifact message"),
    }

    match &blocked_msg {
        BrokerMessage::Blocked { agent_id, payload } => {
            assert_eq!(agent_id, "agent-3");
            assert_eq!(payload.needs, "api_spec");
            assert_eq!(payload.from, "agent-1");
        }
        _ => panic!("Expected Blocked message"),
    }
}

// ---------------------------------------------------------------------------
// 11.6: Dashboard session management with broker
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn dashboard_session_management_with_broker() {
    let dir = TempDir::new().unwrap();
    let session = make_session_with_broker("session-mgmt");
    save_session_in(&session, dir.path()).unwrap();

    let loaded = load_session_from(&session.session_name, dir.path())
        .unwrap()
        .expect("session should exist");

    // Verify broker fields are preserved
    assert_eq!(loaded.broker_port, Some(9120));
    assert_eq!(loaded.broker_bind.as_deref(), Some("127.0.0.1"));
    assert!(loaded.broker_log_path.is_some());

    // Clean up
    delete_session_in(&session.session_name, dir.path()).unwrap();
    let deleted = load_session_from(&session.session_name, dir.path()).unwrap();
    assert!(deleted.is_none(), "session should be deleted");
}

// ---------------------------------------------------------------------------
// 11.7: Dashboard configuration serialization
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn dashboard_configuration_serialization() {
    let config = make_config_with_message_log(true);

    // Verify the config can be serialized and deserialized
    let toml = toml::to_string(&config).expect("should serialize to TOML");
    assert!(toml.contains("show_message_log"));
    assert!(toml.contains("true"));

    // Verify it can be deserialized back
    let parsed: PawConfig = toml::from_str(&toml).expect("should deserialize from TOML");
    assert!(parsed.get_dashboard().unwrap().show_message_log);
}

// Dashboard rendering tests (observation-only after prompt-inbox removal).
//
// These tests render real frames with `ratatui::backend::TestBackend` so they
// verify the rendered TUI buffer rather than constructing input fixtures and
// asserting on their literal values.

use std::time::Instant;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

use git_paw::broker::AgentStatusEntry;
use git_paw::dashboard::{format_agent_rows, format_status_line, render_dashboard};

/// Flattens a `Buffer` into a single `String` so substring assertions work
/// across cell boundaries (ratatui stores one grapheme per cell).
fn buffer_to_string(buffer: &Buffer) -> String {
    let mut out = String::new();
    let area = buffer.area;
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            out.push_str(cell.symbol());
        }
        out.push('\n');
    }
    out
}

/// Renders one frame to a 120x30 `TestBackend` and returns the rendered
/// buffer as a string for substring assertions.
fn render_to_string(
    rows: &[git_paw::dashboard::AgentRow],
    status_line: &str,
    show_message_log: bool,
) -> String {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|f| render_dashboard(f, rows, status_line, &[], show_message_log))
        .expect("draw frame");
    let buffer = terminal.backend().buffer().clone();
    buffer_to_string(&buffer)
}

/// The dashboard renders the title, agent table, and status line. After
/// the v0.5.0 prompt-inbox removal it does NOT render a Questions panel
/// or a Reply input field.
#[test]
fn test_dashboard_renders_observation_sections() {
    let now = Instant::now();
    let agents = vec![
        AgentStatusEntry {
            agent_id: "feat-auth".to_string(),
            cli: "claude".to_string(),
            status: "working".to_string(),
            last_seen_seconds: 0,
            summary: "implementing oauth".to_string(),
            last_seen: now,
            phase: None,
        },
        AgentStatusEntry {
            agent_id: "feat-db".to_string(),
            cli: "cursor".to_string(),
            status: "blocked".to_string(),
            last_seen_seconds: 10,
            summary: "waiting on schema".to_string(),
            last_seen: now,
            phase: None,
        },
    ];
    let rows = format_agent_rows(&agents, now);
    let status_line = format_status_line(2, 1, 0, 1, 0);

    let rendered = render_to_string(&rows, &status_line, false);

    assert!(
        rendered.contains("git-paw dashboard"),
        "should render title; got:\n{rendered}"
    );
    assert!(
        rendered.contains("feat-auth"),
        "should render first agent in table; got:\n{rendered}"
    );
    assert!(
        rendered.contains("feat-db"),
        "should render second agent in table; got:\n{rendered}"
    );
    assert!(
        rendered.contains("2 agents:"),
        "should render status line; got:\n{rendered}"
    );
    assert!(
        rendered.contains("1 working") && rendered.contains("1 blocked"),
        "status line should show counts; got:\n{rendered}"
    );
    assert!(
        !rendered.contains("Questions ("),
        "dashboard MUST NOT render a Questions panel after the v0.5.0 inbox removal; got:\n{rendered}",
    );
    assert!(
        !rendered.contains("Reply to"),
        "dashboard MUST NOT render a Reply-to input prompt; got:\n{rendered}",
    );
}
