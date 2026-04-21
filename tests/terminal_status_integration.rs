//! Integration tests for the terminal-status sticky behaviour.
//!
//! Spec scenario `terminal-status-sticky`: once an agent's status is one of
//! the terminal labels (`done`, `verified`, `blocked`, `committed`), the
//! broker MUST NOT downgrade it back to a non-terminal label like
//! `working`. The git-status watcher (`src/broker/watcher.rs`) routinely
//! publishes `working` heartbeats whenever it sees dirty paths in the
//! worktree -- those heartbeats reach the agent record through
//! `delivery::publish_message`, which is the same public seam the watcher
//! uses. These tests exercise that public seam end-to-end so a future
//! refactor that bypasses the sticky check (e.g. by reaching directly into
//! `AgentRecord`) is caught at the integration boundary.
//!
//! Each test follows the pattern:
//!   1. Publish a terminal `agent.artifact` to set the agent's status.
//!   2. Publish an `agent.status` `working` heartbeat the way the watcher
//!      would (same payload shape, same `delivery::publish_message` call).
//!   3. Assert the agent's status is still the terminal label.

use std::sync::Arc;

use git_paw::broker::BrokerState;
use git_paw::broker::delivery::publish_message;
use git_paw::broker::messages::{ArtifactPayload, BrokerMessage, StatusPayload};

/// Builds the same `agent.status working` message shape that
/// `watcher::watch_worktree` produces on every dirty-path tick.
fn watcher_working_status(agent_id: &str, modified_files: &[&str]) -> BrokerMessage {
    BrokerMessage::Status {
        agent_id: agent_id.to_string(),
        payload: StatusPayload {
            status: "working".to_string(),
            modified_files: modified_files.iter().map(|s| (*s).to_string()).collect(),
            message: None,
        },
    }
}

fn terminal_artifact(agent_id: &str, status: &str) -> BrokerMessage {
    BrokerMessage::Artifact {
        agent_id: agent_id.to_string(),
        payload: ArtifactPayload {
            status: status.to_string(),
            exports: Vec::new(),
            modified_files: Vec::new(),
        },
    }
}

#[test]
fn watcher_working_tick_cannot_downgrade_done_status() {
    let state = Arc::new(BrokerState::new(None));

    // Step 1: agent reaches the terminal `done` state via an artifact.
    publish_message(&state, &terminal_artifact("feat-foo", "done"));
    assert_eq!(state.read().agents["feat-foo"].status, "done");

    // Step 2: watcher tick publishes a `working` heartbeat with newly
    // observed dirty paths -- exactly the BrokerMessage shape used by
    // `watch_worktree` in `src/broker/watcher.rs`.
    publish_message(
        &state,
        &watcher_working_status("feat-foo", &["src/lib.rs", "tests/foo.rs"]),
    );

    // Step 3: status MUST remain `done`. A failing assertion here means
    // the watcher path silently downgrades terminal status.
    assert_eq!(
        state.read().agents["feat-foo"].status,
        "done",
        "watcher heartbeat must not downgrade a terminal `done` status"
    );
}

#[test]
fn watcher_working_tick_cannot_downgrade_verified_status() {
    let state = Arc::new(BrokerState::new(None));
    publish_message(&state, &terminal_artifact("feat-bar", "verified"));
    assert_eq!(state.read().agents["feat-bar"].status, "verified");

    publish_message(
        &state,
        &watcher_working_status("feat-bar", &["src/main.rs"]),
    );

    assert_eq!(
        state.read().agents["feat-bar"].status,
        "verified",
        "watcher heartbeat must not downgrade a terminal `verified` status"
    );
}

#[test]
fn watcher_working_tick_cannot_downgrade_blocked_status() {
    let state = Arc::new(BrokerState::new(None));
    publish_message(&state, &terminal_artifact("feat-baz", "blocked"));
    assert_eq!(state.read().agents["feat-baz"].status, "blocked");

    publish_message(
        &state,
        &watcher_working_status("feat-baz", &["src/error.rs"]),
    );

    assert_eq!(
        state.read().agents["feat-baz"].status,
        "blocked",
        "watcher heartbeat must not downgrade a terminal `blocked` status"
    );
}

#[test]
fn watcher_working_tick_cannot_downgrade_committed_status() {
    let state = Arc::new(BrokerState::new(None));
    publish_message(&state, &terminal_artifact("feat-qux", "committed"));
    assert_eq!(state.read().agents["feat-qux"].status, "committed");

    publish_message(&state, &watcher_working_status("feat-qux", &["README.md"]));

    assert_eq!(
        state.read().agents["feat-qux"].status,
        "committed",
        "watcher heartbeat must not downgrade a terminal `committed` status"
    );
}
