//! End-to-end integration tests for the supervisor session-summary writer.
//!
//! Spec scenario `session-summary`: when the supervisor finishes,
//! `<repo_root>/.git-paw/session-summary.md` MUST exist on disk and
//! contain the totals/test-results sections plus per-agent test outcomes.
//!
//! These tests construct a `MergeResults`-shaped value (the supervisor's
//! output) and call `git_paw::summary::write_supervisor_summary` -- the
//! same public seam invoked by the supervisor handler in `src/main.rs` --
//! and assert the file exists with the spec'd section headers and agent
//! test rows. They live in `tests/` (not as unit tests) so the full
//! file-system write path, directory creation, and path resolution are
//! exercised.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use git_paw::broker::AgentRecord;
use git_paw::broker::BrokerState;
use git_paw::session::{Session, SessionStatus, WorktreeEntry};
use git_paw::summary::{TestResult, write_supervisor_summary};
use tempfile::TempDir;

fn sample_session() -> Session {
    Session {
        session_name: "paw-demo".to_string(),
        repo_path: PathBuf::from("/tmp/demo"),
        project_name: "demo".to_string(),
        // Fixed start time so total-duration formatting is deterministic.
        created_at: UNIX_EPOCH + Duration::from_secs(1_711_200_000),
        status: SessionStatus::Active,
        worktrees: vec![
            WorktreeEntry {
                branch: "feat/config".to_string(),
                worktree_path: PathBuf::from("/tmp/demo-feat-config"),
                cli: "claude".to_string(),
                branch_created: true,
            },
            WorktreeEntry {
                branch: "feat/errors".to_string(),
                worktree_path: PathBuf::from("/tmp/demo-feat-errors"),
                cli: "gemini".to_string(),
                branch_created: true,
            },
        ],
        broker_port: None,
        broker_bind: None,
        broker_log_path: None,
    }
}

fn populate_agent(state: &BrokerState, agent_id: &str, status: &str) {
    let mut inner = state.write();
    inner.agents.insert(
        agent_id.to_string(),
        AgentRecord {
            agent_id: agent_id.to_string(),
            status: status.to_string(),
            last_seen: std::time::Instant::now(),
            last_message: None,
        },
    );
}

#[test]
fn write_supervisor_summary_writes_file_with_totals_and_test_results() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path();

    // The supervisor reaches this point with two known agents and a
    // `MergeResults` containing per-branch test outcomes. Mirror that
    // shape: BrokerState records the agents; the supervisor passes the
    // merge_order vector and test_results map directly into
    // write_supervisor_summary (the public summary entry point).
    let state = BrokerState::new(None);
    populate_agent(&state, "feat-config", "verified");
    populate_agent(&state, "feat-errors", "verified");

    let merge_order = vec!["feat-config".to_string(), "feat-errors".to_string()];
    let mut test_results: HashMap<String, TestResult> = HashMap::new();
    test_results.insert(
        "feat-config".to_string(),
        TestResult {
            success: true,
            output: "all 42 tests passed".to_string(),
        },
    );
    test_results.insert(
        "feat-errors".to_string(),
        TestResult {
            success: false,
            output: "thread 'main' panicked: oh no".to_string(),
        },
    );

    let session = sample_session();

    let summary_path =
        write_supervisor_summary(&state, &session, &merge_order, &test_results, repo_root)
            .expect("write_supervisor_summary should succeed under a writable temp root");

    // Spec scenario: file exists under the canonical timestamped path
    // `<repo>/.git-paw/sessions/<UTC-timestamp>.md`.
    assert!(
        summary_path.starts_with(repo_root.join(".git-paw").join("sessions")),
        "summary path {} must live under .git-paw/sessions/",
        summary_path.display()
    );
    assert_eq!(
        summary_path.extension().and_then(|s| s.to_str()),
        Some("md")
    );
    assert!(
        summary_path.exists(),
        "expected supervisor summary at {}",
        summary_path.display()
    );
    let content = fs::read_to_string(&summary_path).expect("read summary");

    // Required section headers.
    assert!(
        content.contains("# Session Summary"),
        "missing top-level header in summary; got:\n{content}"
    );
    assert!(
        content.contains("## Totals"),
        "missing Totals section in summary; got:\n{content}"
    );
    assert!(
        content.contains("## Test Results"),
        "missing Test Results section in summary; got:\n{content}"
    );

    // Per-agent test-result rows: each branch must appear with the
    // correct pass/fail marker, and the captured output must be embedded
    // verbatim (the operator depends on this when triaging failures).
    assert!(
        content.contains("**feat-config**"),
        "feat-config should appear as a test-result row; got:\n{content}"
    );
    assert!(
        content.contains("**feat-errors**"),
        "feat-errors should appear as a test-result row; got:\n{content}"
    );
    assert!(
        content.contains("PASS"),
        "successful agent must show a PASS marker; got:\n{content}"
    );
    assert!(
        content.contains("FAIL"),
        "failing agent must show a FAIL marker; got:\n{content}"
    );
    assert!(
        content.contains("all 42 tests passed"),
        "test stdout for feat-config must be embedded; got:\n{content}"
    );
    assert!(
        content.contains("thread 'main' panicked: oh no"),
        "test stdout for feat-errors must be embedded; got:\n{content}"
    );

    // The Totals section reports agent count.
    assert!(
        content.contains("Total agents: 2"),
        "Totals must report `Total agents: 2`; got:\n{content}"
    );
}

#[test]
fn write_supervisor_summary_creates_sessions_dir_when_absent() {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path();
    assert!(
        !repo_root.join(".git-paw").exists(),
        "precondition: .git-paw must not exist yet"
    );

    let state = BrokerState::new(None);
    let session = sample_session();

    let written = write_supervisor_summary(
        &state,
        &session,
        &[],
        &HashMap::<String, TestResult>::new(),
        repo_root,
    )
    .expect("write_supervisor_summary creates .git-paw/sessions/ on demand");

    assert!(
        repo_root.join(".git-paw").is_dir(),
        ".git-paw directory must be created if absent"
    );
    assert!(
        repo_root.join(".git-paw").join("sessions").is_dir(),
        ".git-paw/sessions directory must be created if absent"
    );
    assert!(
        written.is_file(),
        "summary file at {} must exist",
        written.display()
    );
    assert!(
        written.starts_with(repo_root.join(".git-paw").join("sessions")),
        "summary file must live under .git-paw/sessions/"
    );
}
