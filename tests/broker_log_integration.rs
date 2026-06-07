//! End-to-end tests for the dashboard's Broker log panel against a real
//! [`BrokerState`].
//!
//! The dashboard feeds its Broker log ring buffer by polling the broker's
//! in-process message log each tick (`full_log(state, cursor)` →
//! `BrokerLog::ingest`). These tests drive that exact path — publish real
//! messages into a `BrokerState`, ingest, and assert on the panel's visible
//! rows, filter behaviour, details overlay, and restart resilience — without
//! standing up a TUI.

use std::sync::Arc;

use crossterm::event::KeyCode;

use git_paw::broker::BrokerState;
use git_paw::broker::delivery::{full_log, publish_message};
use git_paw::broker::messages::{
    ArtifactPayload, BlockedPayload, BrokerMessage, FileIntent, IntentPayload, QuestionPayload,
    StatusPayload,
};
use git_paw::dashboard::broker_log::{
    BIT_INTENT, BIT_STATUS, BrokerLog, LogKeyAction, handle_key, pretty_json,
};

fn fresh_state() -> Arc<BrokerState> {
    Arc::new(BrokerState::new(None))
}

fn status(agent: &str, message: &str) -> BrokerMessage {
    BrokerMessage::Status {
        agent_id: agent.to_string(),
        payload: StatusPayload {
            status: "working".to_string(),
            modified_files: vec![],
            message: Some(message.to_string()),
            ..Default::default()
        },
    }
}

fn intent(agent: &str, summary: &str) -> BrokerMessage {
    BrokerMessage::Intent {
        agent_id: agent.to_string(),
        payload: IntentPayload {
            files: vec![FileIntent::from("src/x.rs")],
            summary: summary.to_string(),
            valid_for_seconds: 60,
        },
    }
}

/// The dashboard's per-tick feed: pull everything newer than the cursor and
/// push it onto the panel. Mirrors `run_dashboard_with_panes`.
fn tick(log: &mut BrokerLog, state: &Arc<BrokerState>) {
    log.ingest(full_log(state, log.last_seq()));
}

// 3.4: a published broker message reaches the BrokerLog on the next feed.
#[test]
fn published_message_reaches_broker_log() {
    let state = fresh_state();
    let mut log = BrokerLog::new(500, true);

    publish_message(&state, &status("feat-auth", "rebasing"));
    tick(&mut log, &state);

    assert_eq!(log.len(), 1);
    let entry = log.iter_visible().next().expect("one visible row");
    assert_eq!(entry.2.agent_id(), "feat-auth");
}

// 10.1: messages of multiple types all land in the buffer, newest first.
#[test]
fn multiple_types_land_newest_first() {
    let state = fresh_state();
    let mut log = BrokerLog::new(500, true);

    publish_message(&state, &status("a", "one")); // seq 1
    publish_message(
        &state,
        &BrokerMessage::Artifact {
            agent_id: "b".to_string(),
            payload: ArtifactPayload {
                status: "done".to_string(),
                exports: vec![],
                modified_files: vec!["src/lib.rs".to_string()],
            },
        },
    ); // seq 2
    publish_message(&state, &intent("c", "wire client")); // seq 3
    tick(&mut log, &state);

    let seqs: Vec<u64> = log.iter_visible().map(|e| e.0).collect();
    assert_eq!(seqs, vec![3, 2, 1], "newest message at the top");
    assert_eq!(log.visible_count(), 3);
}

// 10.3: bitmask filter combinations show/hide the expected rows; the buffer
// keeps everything regardless.
#[test]
fn filter_combinations_show_and_hide_expected_rows() {
    let state = fresh_state();
    let mut log = BrokerLog::new(500, true);

    publish_message(&state, &status("a", "one"));
    publish_message(&state, &intent("b", "wire client"));
    publish_message(
        &state,
        &BrokerMessage::Blocked {
            agent_id: "c".to_string(),
            payload: BlockedPayload {
                needs: "types".to_string(),
                from: "a".to_string(),
            },
        },
    );
    tick(&mut log, &state);
    assert_eq!(log.visible_count(), 3, "All shows everything");

    // Narrow to status only.
    handle_key(&mut log, KeyCode::Char('1'));
    assert_eq!(log.visible_count(), 1);
    assert!(log.filter().is_chip_active(BIT_STATUS));

    // Add intent: status + intent visible, blocked hidden.
    handle_key(&mut log, KeyCode::Char('7'));
    assert_eq!(log.visible_count(), 2);
    assert!(log.filter().is_chip_active(BIT_INTENT));

    // Buffer still holds all three regardless of the active filter.
    assert_eq!(log.len(), 3);

    // Reset to All.
    handle_key(&mut log, KeyCode::Char('a'));
    assert!(log.filter().is_all());
    assert_eq!(log.visible_count(), 3);
}

// 10.4: Enter opens the details overlay on the highlighted row; Esc closes
// it; the overlay content matches the highlighted message.
#[test]
fn enter_esc_cycle_through_details_overlay() {
    let state = fresh_state();
    let mut log = BrokerLog::new(500, true);

    publish_message(
        &state,
        &BrokerMessage::Question {
            agent_id: "feat-x".to_string(),
            payload: QuestionPayload {
                question: "rs256 or hs256?".to_string(),
            },
        },
    );
    tick(&mut log, &state);

    assert!(!log.overlay_open());
    assert_eq!(handle_key(&mut log, KeyCode::Enter), LogKeyAction::Handled);
    assert!(log.overlay_open());

    // The overlay renders the highlighted message's pretty JSON.
    let selected = log.selected_entry().expect("a highlighted row");
    let json = pretty_json(&selected.2);
    assert!(json.contains("agent.question"));
    assert!(json.contains("rs256 or hs256?"));

    assert_eq!(handle_key(&mut log, KeyCode::Esc), LogKeyAction::Handled);
    assert!(!log.overlay_open());
}

// 8.2: the buffer survives a transient broker-watcher restart — historical
// messages remain and new ones resume appending.
//
// In this architecture the BrokerState persists in-process across a watcher
// bounce; the panel owns its buffer and reads via a monotonic seq cursor, so
// a restart is transparent. We model the restart as: ingest some messages,
// then (after the "restart") ingest the full log again plus new messages.
// The cursor must keep history intact, avoid duplicating already-seen
// entries, and append the post-restart messages.
#[test]
fn buffer_survives_transient_watcher_restart() {
    let state = fresh_state();
    let mut log = BrokerLog::new(500, true);

    publish_message(&state, &status("a", "before-1"));
    publish_message(&state, &status("a", "before-2"));
    tick(&mut log, &state);
    assert_eq!(log.len(), 2, "two pre-restart messages buffered");
    let cursor_before = log.last_seq();

    // --- watcher restarts here; the buffer is NOT cleared ---

    // A fresh feed after restart re-fetches from the cursor. Even if the
    // watcher replayed the whole log, the cursor dedups already-seen seqs.
    log.ingest(full_log(&state, 0)); // worst case: replay everything
    assert_eq!(log.len(), 2, "replayed messages must not duplicate history");
    assert_eq!(log.last_seq(), cursor_before);

    // New messages after the restart resume appending at the top.
    publish_message(&state, &status("a", "after-1"));
    tick(&mut log, &state);
    assert_eq!(log.len(), 3, "post-restart message appended");

    let newest = log.iter_visible().next().unwrap();
    match &newest.2 {
        BrokerMessage::Status { payload, .. } => {
            assert_eq!(payload.message.as_deref(), Some("after-1"));
        }
        other => panic!("expected the post-restart status at the top, got {other:?}"),
    }
    // Pre-restart history is still present at the bottom.
    let oldest = log.iter_visible().last().unwrap();
    match &oldest.2 {
        BrokerMessage::Status { payload, .. } => {
            assert_eq!(payload.message.as_deref(), Some("before-1"));
        }
        other => panic!("expected pre-restart history retained, got {other:?}"),
    }
}

// Configured cap is respected end-to-end: with a small cap, only the most
// recent N messages survive (bounded ring buffer requirement).
#[test]
fn configured_cap_retains_only_most_recent() {
    let state = fresh_state();
    let mut log = BrokerLog::new(3, true);

    for i in 0..10 {
        publish_message(&state, &status("a", &format!("msg-{i}")));
    }
    tick(&mut log, &state);

    assert_eq!(log.len(), 3, "buffer caps at the configured max");
    let messages: Vec<String> = log
        .iter_visible()
        .map(|e| match &e.2 {
            BrokerMessage::Status { payload, .. } => payload.message.clone().unwrap(),
            _ => unreachable!(),
        })
        .collect();
    // Newest first: msg-9, msg-8, msg-7 retained; earlier ones dropped.
    assert_eq!(messages, vec!["msg-9", "msg-8", "msg-7"]);
}
