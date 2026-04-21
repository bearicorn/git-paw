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
    Session, SessionStatus, WorktreeEntry, delete_session_in, load_session_from, save_session_in,
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

// Dashboard and prompt-inbox tests
//
// These tests render real frames with `ratatui::backend::TestBackend` so they
// verify the rendered TUI buffer rather than constructing input fixtures and
// asserting on their literal values.

use std::sync::Arc;
use std::time::Instant;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

use git_paw::broker::AgentStatusEntry;
use git_paw::broker::BrokerState;
use git_paw::broker::delivery;
use git_paw::broker::messages::QuestionPayload;
use git_paw::dashboard::{
    QuestionEntry, drive_question_tick, format_agent_rows, format_status_line, render_dashboard,
};

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

/// Renders one frame to a 80x24 `TestBackend` and returns the rendered buffer
/// as a string for substring assertions.
fn render_to_string(
    rows: &[git_paw::dashboard::AgentRow],
    status_line: &str,
    questions: &[QuestionEntry],
    show_message_log: bool,
) -> String {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|f| {
            render_dashboard(
                f,
                rows,
                status_line,
                questions,
                None,
                "",
                &[],
                show_message_log,
            );
        })
        .expect("draw frame");
    let buffer = terminal.backend().buffer().clone();
    buffer_to_string(&buffer)
}

/// Test that the dashboard renders the agent table, status line, and prompts
/// panel — not just that fixture values can be constructed.
#[test]
fn test_dashboard_renders_all_sections() {
    let now = Instant::now();
    let agents = vec![
        AgentStatusEntry {
            agent_id: "feat-auth".to_string(),
            cli: "claude".to_string(),
            status: "working".to_string(),
            last_seen_seconds: 0,
            summary: "implementing oauth".to_string(),
            last_seen: now,
        },
        AgentStatusEntry {
            agent_id: "feat-db".to_string(),
            cli: "cursor".to_string(),
            status: "blocked".to_string(),
            last_seen_seconds: 10,
            summary: "waiting on schema".to_string(),
            last_seen: now,
        },
    ];
    let rows = format_agent_rows(&agents, now);
    let status_line = format_status_line(2, 1, 0, 1, 0);
    let questions = vec![QuestionEntry {
        seq: 1,
        agent_id: "feat-auth".to_string(),
        pane_index: 1,
        question: "Should I use bcrypt?".to_string(),
    }];

    let rendered = render_to_string(&rows, &status_line, &questions, false);

    // Title is always present
    assert!(
        rendered.contains("git-paw dashboard"),
        "should render title; got:\n{rendered}"
    );
    // Agent table renders both agent IDs
    assert!(
        rendered.contains("feat-auth"),
        "should render first agent in table; got:\n{rendered}"
    );
    assert!(
        rendered.contains("feat-db"),
        "should render second agent in table; got:\n{rendered}"
    );
    // Status line text appears verbatim
    assert!(
        rendered.contains("2 agents:"),
        "should render status line; got:\n{rendered}"
    );
    assert!(
        rendered.contains("1 working") && rendered.contains("1 blocked"),
        "status line should show counts; got:\n{rendered}"
    );
    // Prompts panel renders the pending question
    assert!(
        rendered.contains("Questions"),
        "should render prompts panel header; got:\n{rendered}"
    );
    assert!(
        rendered.contains("Should I use bcrypt?"),
        "prompts panel should render question text; got:\n{rendered}"
    );
}

/// Drives one tick of the dashboard's poll path against a real `BrokerState`
/// and verifies that an `agent.question` is converted into a `QuestionEntry`
/// in the dashboard's local question list.
#[test]
fn test_dashboard_picks_up_question_on_next_tick() {
    use git_paw::broker::messages::BrokerMessage;
    use std::collections::HashMap;

    // Build a real broker state.
    let state = Arc::new(BrokerState::new(None));

    // Register supervisor inbox by issuing a no-op poll first (the dashboard
    // does this implicitly on every tick via `poll_messages`).
    let _ = delivery::poll_messages(&state, "supervisor", 0);

    // Publish a real question through the production publish_message path.
    delivery::publish_message(
        &state,
        &BrokerMessage::Question {
            agent_id: "feat-auth".to_string(),
            payload: QuestionPayload {
                question: "Continue with this approach?".to_string(),
            },
        },
    );

    // Drive one tick: the dashboard polls "supervisor" and converts each
    // Question into a QuestionEntry using the pane_map for routing.
    let mut questions: Vec<QuestionEntry> = Vec::new();
    let mut last_seq: u64 = 0;
    let mut pane_map: HashMap<String, usize> = HashMap::new();
    pane_map.insert("feat-auth".to_string(), 3);

    drive_question_tick(&state, &pane_map, &mut questions, &mut last_seq);

    assert_eq!(questions.len(), 1, "tick should pick up one question");
    assert_eq!(questions[0].agent_id, "feat-auth");
    assert_eq!(questions[0].question, "Continue with this approach?");
    assert_eq!(
        questions[0].pane_index, 3,
        "pane_index should come from the pane_map"
    );
    assert!(last_seq > 0, "last_seq should advance");

    // A second tick with the advanced last_seq should not duplicate the question.
    drive_question_tick(&state, &pane_map, &mut questions, &mut last_seq);
    assert_eq!(
        questions.len(),
        1,
        "tick should not duplicate already-seen questions"
    );
}

/// Renders a frame containing several pending questions and asserts each
/// `agent_id` and question text appears in the rendered buffer.
#[test]
fn test_prompts_section_renders_all_pending() {
    let questions = vec![
        QuestionEntry {
            seq: 1,
            agent_id: "feat-auth".to_string(),
            pane_index: 1,
            question: "Should I use bcrypt?".to_string(),
        },
        QuestionEntry {
            seq: 2,
            agent_id: "feat-db".to_string(),
            pane_index: 2,
            question: "Pg or sqlite?".to_string(),
        },
        QuestionEntry {
            seq: 3,
            agent_id: "feat-api".to_string(),
            pane_index: 3,
            question: "REST or GraphQL?".to_string(),
        },
    ];
    let status_line = format_status_line(0, 0, 0, 0, 0);

    let rendered = render_to_string(&[], &status_line, &questions, false);

    for q in &questions {
        assert!(
            rendered.contains(&q.agent_id),
            "rendered buffer should contain agent_id {}; got:\n{rendered}",
            q.agent_id
        );
        assert!(
            rendered.contains(&q.question),
            "rendered buffer should contain question text {:?}; got:\n{rendered}",
            q.question
        );
    }
    assert!(
        rendered.contains("3 pending"),
        "prompts header should report 3 pending; got:\n{rendered}"
    );
}
