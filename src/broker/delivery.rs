//! Message routing, cursor-based polling, and log flush.
//!
//! Contains the core delivery logic for the broker: publishing messages
//! to agent inboxes, polling with cursor-based pagination, snapshot
//! queries for the dashboard, and the background log flush thread.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use super::messages::BrokerMessage;
use super::{AgentRecord, AgentStatusEntry, BrokerState, BrokerStateInner};

/// Returns the sender's `agent_id` for a message.
///
/// For most variants this is the top-level `agent_id` field. For `Verified`
/// and `Feedback`, the `agent_id` identifies the target, so the sender lives
/// inside the payload (`verified_by` / `from`).
fn sender_id(msg: &BrokerMessage) -> &str {
    match msg {
        BrokerMessage::Status { agent_id, .. }
        | BrokerMessage::Artifact { agent_id, .. }
        | BrokerMessage::Blocked { agent_id, .. }
        | BrokerMessage::Question { agent_id, .. } => agent_id,
        BrokerMessage::Verified { payload, .. } => &payload.verified_by,
        BrokerMessage::Feedback { payload, .. } => &payload.from,
    }
}
/// Updates (or creates) the agent record and inbox for the message sender.
fn update_agent_record(inner: &mut BrokerStateInner, msg: &BrokerMessage) {
    let agent_id = sender_id(msg).to_string();
    let status = msg.status_label().to_string();

    let record = inner
        .agents
        .entry(agent_id.clone())
        .or_insert_with(|| AgentRecord {
            agent_id: agent_id.clone(),
            status: String::new(),
            last_seen: std::time::Instant::now(),
            last_message: None,
        });

    // Make terminal states sticky: only update status if the new status is also terminal
    // or if the current status is not terminal
    let is_terminal_status = |s: &str| matches!(s, "done" | "verified" | "blocked" | "committed");

    if !is_terminal_status(&record.status) || is_terminal_status(&status) {
        record.status = status;
    }

    record.last_seen = std::time::Instant::now();
    record.last_message = Some(msg.clone());

    // Ensure inbox exists for the sender
    inner.queues.entry(agent_id).or_default();
}

/// Publishes a message through the broker.
///
/// - Updates the sender's [`AgentRecord`]
/// - Assigns a global sequence number
/// - Routes the message to the appropriate inboxes:
///   - `Status` -- record update only, no inbox routing
///   - `Artifact` -- broadcast to every other registered agent's inbox
///   - `Blocked` -- delivered to `payload.from`'s inbox (if registered)
/// - Appends the message to the in-memory log
pub fn publish_message(state: &Arc<BrokerState>, msg: &BrokerMessage) {
    let seq = state.next_seq();
    let mut inner = state.write();

    update_agent_record(&mut inner, msg);

    // Append to the in-memory message log
    inner
        .message_log
        .push((seq, SystemTime::now(), msg.clone()));

    // Route based on message type
    match msg {
        BrokerMessage::Status { .. } => {
            // Status messages are informational only -- not routed to inboxes
        }
        BrokerMessage::Artifact { agent_id, .. } => {
            // Broadcast to every other agent's inbox
            let targets: Vec<String> = inner
                .queues
                .keys()
                .filter(|id| id.as_str() != agent_id)
                .cloned()
                .collect();
            for target in targets {
                if let Some(inbox) = inner.queues.get_mut(&target) {
                    inbox.push((seq, msg.clone()));
                }
            }
        }
        BrokerMessage::Blocked { payload, .. } => {
            // Deliver to the target agent's inbox if it exists
            if let Some(inbox) = inner.queues.get_mut(&payload.from) {
                inbox.push((seq, msg.clone()));
            }
            // Silently drop if target has no inbox (not yet registered)
        }
        BrokerMessage::Verified { payload, .. } => {
            // Broadcast to every other agent's inbox, skipping the verifier
            let sender = payload.verified_by.clone();
            let targets: Vec<String> = inner
                .queues
                .keys()
                .filter(|id| id.as_str() != sender.as_str())
                .cloned()
                .collect();
            for target in targets {
                if let Some(inbox) = inner.queues.get_mut(&target) {
                    inbox.push((seq, msg.clone()));
                }
            }
        }
        BrokerMessage::Feedback { agent_id, .. } => {
            // Deliver to the target agent's inbox if it exists
            if let Some(inbox) = inner.queues.get_mut(agent_id) {
                inbox.push((seq, msg.clone()));
            }
            // Silently drop if target has no inbox (not yet registered)
        }
        BrokerMessage::Question { .. } => {
            // Route to the supervisor inbox, creating it if absent.
            // Do NOT enqueue in sender's or any other agent's inbox.
            let inbox = inner.queues.entry("supervisor".to_string()).or_default();
            inbox.push((seq, msg.clone()));
        }
    }
}

/// Polls an agent's inbox for messages newer than `since`.
///
/// Returns `(messages, last_seq)` where `last_seq` is the highest
/// sequence number in the result, or `0` if no messages match.
/// This is a non-destructive read -- messages remain in the inbox.
///
/// Uses a read lock only.
pub fn poll_messages(
    state: &Arc<BrokerState>,
    agent_id: &str,
    since: u64,
) -> (Vec<BrokerMessage>, u64) {
    let inner = state.read();

    let Some(inbox) = inner.queues.get(agent_id) else {
        return (Vec::new(), 0);
    };

    let mut messages = Vec::new();
    let mut last_seq: u64 = 0;

    for (seq, msg) in inbox {
        if *seq > since {
            messages.push(msg.clone());
            if *seq > last_seq {
                last_seq = *seq;
            }
        }
    }

    (messages, last_seq)
}

/// Returns the most recent broker messages for display in the dashboard.
///
/// Returns messages in reverse chronological order (newest first), limited
/// to the specified number of messages. Takes a read lock only during
/// the data copy operation.
pub fn recent_messages(
    state: &Arc<BrokerState>,
    limit: usize,
) -> Vec<(u64, std::time::SystemTime, BrokerMessage)> {
    let inner = state.read();
    inner
        .message_log
        .iter()
        .rev()
        .take(limit)
        .cloned()
        .collect()
}

/// Returns the broker's full message log, in chronological order (oldest
/// first), filtered to messages with `seq > since`. `since == 0` returns
/// every message.
///
/// Used by `cmd_supervisor` to reconstruct broker state from outside the
/// dashboard process: the supervisor runs in a different process from the
/// broker, so it cannot read [`BrokerState`] directly. Fetching the full
/// log over HTTP lets the supervisor rebuild a state-equivalent view for
/// merge-order decisions and the session-summary write.
///
/// Takes a read lock only during the data copy.
pub fn full_log(
    state: &Arc<BrokerState>,
    since: u64,
) -> Vec<(u64, std::time::SystemTime, BrokerMessage)> {
    let inner = state.read();
    inner
        .message_log
        .iter()
        .filter(|(seq, _, _)| *seq > since)
        .cloned()
        .collect()
}

/// Returns a snapshot of all known agents' status.
///
/// Takes a read lock, clones each record into an [`AgentStatusEntry`],
/// and releases the lock. The returned value is fully owned and can be
/// used for rendering or serialization without holding any lock.
pub fn agent_status_snapshot(state: &Arc<BrokerState>) -> Vec<AgentStatusEntry> {
    let inner = state.read();
    // Start from the watched-target CLI map so every known pane shows up
    // even before it has published a status message, then overlay any
    // agents that have actually published with their live status.
    let mut entries: HashMap<String, AgentStatusEntry> = inner
        .agent_clis
        .iter()
        .map(|(agent_id, cli)| {
            (
                agent_id.clone(),
                AgentStatusEntry {
                    agent_id: agent_id.clone(),
                    cli: cli.clone(),
                    status: "idle".to_string(),
                    last_seen_seconds: 0,
                    summary: String::new(),
                    last_seen: std::time::Instant::now(),
                },
            )
        })
        .collect();
    for r in inner.agents.values() {
        let cli = inner
            .agent_clis
            .get(&r.agent_id)
            .cloned()
            .unwrap_or_default();
        entries.insert(
            r.agent_id.clone(),
            AgentStatusEntry {
                agent_id: r.agent_id.clone(),
                cli,
                status: r.status.clone(),
                last_seen_seconds: r.last_seen.elapsed().as_secs(),
                summary: String::new(),
                last_seen: r.last_seen,
            },
        );
    }
    // Sort by agent_id so the dashboard rows stay in a stable order across
    // ticks — otherwise HashMap iteration order makes rows jitter on every
    // redraw.
    let mut out: Vec<AgentStatusEntry> = entries.into_values().collect();
    out.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
    out
}

/// Background loop that periodically flushes new log entries to disk.
///
/// Runs every ~5 seconds, reading new entries under a read lock and
/// writing them outside the lock. Performs a final flush when the
/// stop flag is set. Sleeps in small increments to enable prompt
/// shutdown (exits within ~100ms of the stop signal).
pub fn flush_loop(state: &Arc<BrokerState>, stop: &Arc<AtomicBool>) {
    let log_path = match &state.log_path {
        Some(p) => p.clone(),
        None => return,
    };

    let mut last_flushed_seq: u64 = 0;
    let flush_interval = Duration::from_secs(5);
    let check_interval = Duration::from_millis(100);

    loop {
        // Sleep in small increments, checking the stop flag
        let mut elapsed = Duration::ZERO;
        while elapsed < flush_interval {
            if stop.load(Ordering::Acquire) {
                flush_entries(state, &log_path, &mut last_flushed_seq);
                return;
            }
            std::thread::sleep(check_interval);
            elapsed += check_interval;
        }

        flush_entries(state, &log_path, &mut last_flushed_seq);
    }
}

/// Flushes log entries with `seq > last_flushed_seq` to the given path.
///
/// Updates `last_flushed_seq` to the highest flushed sequence number.
/// Disk write failures are silently ignored (best-effort).
fn flush_entries(state: &Arc<BrokerState>, log_path: &std::path::Path, last_flushed_seq: &mut u64) {
    let entries: Vec<(u64, SystemTime, BrokerMessage)> = {
        let inner = state.read();
        inner
            .message_log
            .iter()
            .filter(|(seq, _, _)| *seq > *last_flushed_seq)
            .cloned()
            .collect()
    };

    if entries.is_empty() {
        return;
    }

    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) else {
        return; // Best-effort: silently ignore disk write failures
    };

    for (seq, timestamp, msg) in &entries {
        let ts = timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or_else(|_| "0".to_string(), |d| d.as_secs().to_string());

        let line = format!("[{seq}] {ts} [{}] {msg}\n", msg.agent_id());
        let _ = file.write_all(line.as_bytes());
    }

    if let Some((max_seq, _, _)) = entries.last() {
        *last_flushed_seq = *max_seq;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::messages::{
        ArtifactPayload, BlockedPayload, FeedbackPayload, QuestionPayload, StatusPayload,
        VerifiedPayload,
    };
    use crate::broker::start_broker;
    use crate::config::BrokerConfig;

    fn make_status(agent_id: &str, status: &str) -> BrokerMessage {
        BrokerMessage::Status {
            agent_id: agent_id.to_string(),
            payload: StatusPayload {
                status: status.to_string(),
                modified_files: vec![],
                message: None,
            },
        }
    }

    fn make_artifact(agent_id: &str, status: &str, exports: &[&str]) -> BrokerMessage {
        BrokerMessage::Artifact {
            agent_id: agent_id.to_string(),
            payload: ArtifactPayload {
                status: status.to_string(),
                exports: exports.iter().map(|s| (*s).to_string()).collect(),
                modified_files: vec!["src/main.rs".to_string()],
            },
        }
    }

    fn make_blocked(agent_id: &str, needs: &str, from: &str) -> BrokerMessage {
        BrokerMessage::Blocked {
            agent_id: agent_id.to_string(),
            payload: BlockedPayload {
                needs: needs.to_string(),
                from: from.to_string(),
            },
        }
    }

    fn make_verified(agent_id: &str, verified_by: &str, message: Option<&str>) -> BrokerMessage {
        BrokerMessage::Verified {
            agent_id: agent_id.to_string(),
            payload: VerifiedPayload {
                verified_by: verified_by.to_string(),
                message: message.map(str::to_string),
            },
        }
    }

    fn make_feedback(agent_id: &str, from: &str, errors: &[&str]) -> BrokerMessage {
        BrokerMessage::Feedback {
            agent_id: agent_id.to_string(),
            payload: FeedbackPayload {
                from: from.to_string(),
                errors: errors.iter().map(|s| (*s).to_string()).collect(),
            },
        }
    }

    fn make_question(agent_id: &str, question: &str) -> BrokerMessage {
        BrokerMessage::Question {
            agent_id: agent_id.to_string(),
            payload: QuestionPayload {
                question: question.to_string(),
            },
        }
    }

    fn fresh_state() -> Arc<BrokerState> {
        Arc::new(BrokerState::new(None))
    }

    // === Task 3: Message log accumulation ===

    #[test]
    fn message_log_accumulates_three_entries() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_artifact("b", "done", &[]));
        publish_message(&state, &make_blocked("c", "reason", "a"));

        let inner = state.read();
        assert_eq!(inner.message_log.len(), 3);
        assert_eq!(inner.message_log[0].0, 1);
        assert_eq!(inner.message_log[1].0, 2);
        assert_eq!(inner.message_log[2].0, 3);
    }

    #[test]
    fn message_log_includes_all_types() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_artifact("a", "done", &[]));
        publish_message(&state, &make_blocked("b", "reason", "a"));

        let inner = state.read();
        assert_eq!(inner.message_log.len(), 3);
    }

    // === Task 4: Inbox storage with sequence numbers ===

    #[test]
    fn inbox_stores_correct_sequence_number() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working")); // seq 1
        publish_message(&state, &make_status("b", "working")); // seq 2
        publish_message(&state, &make_artifact("a", "done", &[])); // seq 3

        let inner = state.read();
        let b_inbox = &inner.queues["b"];
        assert_eq!(b_inbox.len(), 1);
        assert_eq!(b_inbox[0].0, 3);
    }

    // === Task 5: publish_message routing ===

    #[test]
    fn first_publish_creates_record_and_inbox() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));

        let inner = state.read();
        assert!(inner.agents.contains_key("feat-errors"));
        assert_eq!(inner.agents["feat-errors"].status, "working");
        assert!(inner.queues.contains_key("feat-errors"));
    }

    #[test]
    fn status_not_routed_to_any_inbox() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_status("feat-detect", "working"));
        publish_message(&state, &make_status("feat-errors", "idle"));

        let (detect_msgs, _) = poll_messages(&state, "feat-detect", 0);
        let (errors_msgs, _) = poll_messages(&state, "feat-errors", 0);
        assert!(detect_msgs.is_empty());
        assert!(errors_msgs.is_empty());
    }

    #[test]
    fn artifact_broadcast_to_all_peers() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_status("feat-detect", "working"));
        publish_message(&state, &make_status("feat-config", "working"));

        publish_message(&state, &make_artifact("feat-errors", "done", &[]));

        let (detect_msgs, _) = poll_messages(&state, "feat-detect", 0);
        let (config_msgs, _) = poll_messages(&state, "feat-config", 0);
        assert_eq!(detect_msgs.len(), 1);
        assert_eq!(config_msgs.len(), 1);
    }

    #[test]
    fn artifact_broadcast_skips_sender() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_status("feat-detect", "working"));

        publish_message(&state, &make_artifact("feat-errors", "done", &[]));

        let (errors_msgs, _) = poll_messages(&state, "feat-errors", 0);
        assert!(errors_msgs.is_empty());
    }

    #[test]
    fn artifact_broadcast_skips_unregistered_agents() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));

        publish_message(&state, &make_artifact("feat-errors", "done", &[]));

        let inner = state.read();
        assert!(!inner.queues.contains_key("feat-detect"));
    }

    #[test]
    fn blocked_delivered_to_target() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-config", "working"));
        publish_message(&state, &make_status("feat-errors", "working"));

        publish_message(
            &state,
            &make_blocked("feat-config", "error types", "feat-errors"),
        );

        let (errors_msgs, _) = poll_messages(&state, "feat-errors", 0);
        assert_eq!(errors_msgs.len(), 1);
        assert_eq!(errors_msgs[0].agent_id(), "feat-config");
    }

    #[test]
    fn blocked_not_delivered_to_other_agents() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-config", "working"));
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_status("feat-detect", "working"));

        publish_message(
            &state,
            &make_blocked("feat-config", "error types", "feat-errors"),
        );

        let (detect_msgs, _) = poll_messages(&state, "feat-detect", 0);
        assert!(detect_msgs.is_empty());
    }

    #[test]
    fn blocked_to_unregistered_target_silently_dropped() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-config", "working"));

        publish_message(
            &state,
            &make_blocked("feat-config", "error types", "feat-errors"),
        );

        let inner = state.read();
        assert!(!inner.queues.contains_key("feat-errors"));
    }

    // === Supervisor messages: verified and feedback ===

    #[test]
    fn verified_broadcast_reaches_all_peers() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_status("feat-detect", "working"));
        publish_message(&state, &make_status("supervisor", "working"));

        publish_message(&state, &make_verified("feat-errors", "supervisor", None));

        let (errors_msgs, _) = poll_messages(&state, "feat-errors", 0);
        let (detect_msgs, _) = poll_messages(&state, "feat-detect", 0);
        assert_eq!(errors_msgs.len(), 1);
        assert_eq!(detect_msgs.len(), 1);
    }

    #[test]
    fn verified_broadcast_skips_sender() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_status("supervisor", "working"));

        publish_message(&state, &make_verified("feat-errors", "supervisor", None));

        let (sup_msgs, _) = poll_messages(&state, "supervisor", 0);
        assert!(sup_msgs.is_empty());
    }

    #[test]
    fn verified_updates_sender_record() {
        let state = fresh_state();
        publish_message(&state, &make_status("supervisor", "working"));

        publish_message(&state, &make_verified("feat-errors", "supervisor", None));

        let inner = state.read();
        let record = inner
            .agents
            .get("supervisor")
            .expect("supervisor record exists");
        assert_eq!(record.status, "verified");
    }

    #[test]
    fn feedback_delivered_to_target_agent() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_status("supervisor", "working"));

        publish_message(
            &state,
            &make_feedback("feat-errors", "supervisor", &["test failed"]),
        );

        let (errors_msgs, _) = poll_messages(&state, "feat-errors", 0);
        assert_eq!(errors_msgs.len(), 1);
        assert_eq!(errors_msgs[0].status_label(), "feedback");
    }

    #[test]
    fn feedback_not_delivered_to_other_agents() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_status("feat-detect", "working"));
        publish_message(&state, &make_status("supervisor", "working"));

        publish_message(
            &state,
            &make_feedback("feat-errors", "supervisor", &["test failed"]),
        );

        let (detect_msgs, _) = poll_messages(&state, "feat-detect", 0);
        assert!(detect_msgs.is_empty());
    }

    #[test]
    fn feedback_updates_sender_record() {
        let state = fresh_state();
        publish_message(&state, &make_status("supervisor", "working"));

        publish_message(
            &state,
            &make_feedback("feat-errors", "supervisor", &["test failed"]),
        );

        let inner = state.read();
        let record = inner
            .agents
            .get("supervisor")
            .expect("supervisor record exists");
        assert_eq!(record.status, "feedback");
    }

    // === Question routing ===

    #[test]
    fn question_routed_to_supervisor_inbox() {
        let state = fresh_state();
        publish_message(
            &state,
            &make_question("feat-config", "Should I skip tests?"),
        );

        let (msgs, _) = poll_messages(&state, "supervisor", 0);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].agent_id(), "feat-config");
        assert_eq!(msgs[0].status_label(), "question");
    }

    #[test]
    fn question_creates_supervisor_inbox_if_absent() {
        let state = fresh_state();
        {
            let inner = state.read();
            assert!(!inner.queues.contains_key("supervisor"));
        }

        publish_message(&state, &make_question("feat-config", "anything?"));

        let inner = state.read();
        assert!(inner.queues.contains_key("supervisor"));
    }

    #[test]
    fn question_not_in_sender_inbox() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-config", "working"));
        publish_message(&state, &make_question("feat-config", "anything?"));

        let (msgs, _) = poll_messages(&state, "feat-config", 0);
        assert!(msgs.is_empty());
    }

    #[test]
    fn question_not_delivered_to_other_agents() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-config", "working"));
        publish_message(&state, &make_status("feat-detect", "working"));
        publish_message(&state, &make_question("feat-config", "anything?"));

        let (msgs, _) = poll_messages(&state, "feat-detect", 0);
        assert!(msgs.is_empty());
    }

    #[test]
    fn question_appears_in_message_log() {
        let state = fresh_state();
        publish_message(&state, &make_question("feat-config", "anything?"));

        let inner = state.read();
        assert_eq!(inner.message_log.len(), 1);
        assert_eq!(inner.message_log[0].2.status_label(), "question");
    }

    // === Task 6: poll_messages ===

    #[test]
    fn poll_since_zero_returns_all() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working")); // seq 1
        publish_message(&state, &make_status("b", "working")); // seq 2
        publish_message(&state, &make_artifact("b", "done", &[])); // seq 3
        publish_message(&state, &make_artifact("b", "done", &[])); // seq 4
        publish_message(&state, &make_artifact("b", "done", &[])); // seq 5

        let (msgs, last_seq) = poll_messages(&state, "a", 0);
        assert_eq!(msgs.len(), 3);
        assert_eq!(last_seq, 5);
    }

    #[test]
    fn poll_since_filters_older_messages() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working")); // seq 1
        publish_message(&state, &make_status("b", "working")); // seq 2
        for _ in 0..5 {
            publish_message(&state, &make_artifact("b", "done", &[]));
        } // seqs 3..7, all go to a's inbox

        let (msgs, last_seq) = poll_messages(&state, "a", 5);
        assert_eq!(msgs.len(), 2); // seqs 6, 7
        assert_eq!(last_seq, 7);
    }

    #[test]
    fn poll_since_latest_returns_empty() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_status("b", "working"));
        publish_message(&state, &make_artifact("b", "done", &[]));

        let (_, first_seq) = poll_messages(&state, "a", 0);

        let (msgs, last_seq) = poll_messages(&state, "a", first_seq);
        assert!(msgs.is_empty());
        assert_eq!(last_seq, 0);
    }

    #[test]
    fn poll_is_nondestructive() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_status("b", "working"));
        publish_message(&state, &make_artifact("b", "done", &[]));

        let (msgs1, seq1) = poll_messages(&state, "a", 0);
        let (msgs2, seq2) = poll_messages(&state, "a", 0);
        assert_eq!(msgs1.len(), msgs2.len());
        assert_eq!(seq1, seq2);
    }

    #[test]
    fn poll_unknown_agent_returns_empty() {
        let state = fresh_state();
        let (msgs, last_seq) = poll_messages(&state, "feat-unknown", 0);
        assert!(msgs.is_empty());
        assert_eq!(last_seq, 0);
    }

    // === Task 7: agent_status_snapshot ===

    #[test]
    fn snapshot_contains_all_registered_agents() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_status("b", "idle"));
        publish_message(&state, &make_status("c", "done"));

        let snap = agent_status_snapshot(&state);
        assert_eq!(snap.len(), 3);
    }

    #[test]
    fn snapshot_reflects_latest_status() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_artifact("feat-errors", "done", &[]));

        let snap = agent_status_snapshot(&state);
        let entry = snap.iter().find(|e| e.agent_id == "feat-errors").unwrap();
        assert_eq!(entry.status, "done");
    }

    #[test]
    fn snapshot_empty_on_fresh_state() {
        let state = fresh_state();
        let snap = agent_status_snapshot(&state);
        assert!(snap.is_empty());
    }

    // === Task 8: Log flush thread ===

    #[test]
    fn flush_writes_messages_to_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("broker.log");
        let state = Arc::new(BrokerState::new(Some(log_path.clone())));

        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_status("b", "working"));
        publish_message(&state, &make_artifact("a", "done", &[]));

        let mut last_flushed = 0u64;
        flush_entries(&state, &log_path, &mut last_flushed);

        let content = std::fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("[1]"));
        assert!(lines[2].starts_with("[3]"));
        assert_eq!(last_flushed, 3);
    }

    #[test]
    fn flush_only_writes_new_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("broker.log");
        let state = Arc::new(BrokerState::new(Some(log_path.clone())));

        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_status("b", "working"));
        publish_message(&state, &make_artifact("a", "done", &[]));

        let mut last_flushed = 0u64;
        flush_entries(&state, &log_path, &mut last_flushed);
        assert_eq!(last_flushed, 3);

        publish_message(&state, &make_artifact("b", "done", &[]));
        publish_message(&state, &make_artifact("a", "done", &[]));

        flush_entries(&state, &log_path, &mut last_flushed);
        assert_eq!(last_flushed, 5);

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert_eq!(content.lines().count(), 5);
    }

    #[test]
    fn final_flush_on_handle_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("broker.log");
        let config = BrokerConfig {
            enabled: true,
            #[allow(clippy::cast_possible_truncation)]
            port: 19_300 + (std::process::id() as u16 % 100),
            bind: "127.0.0.1".to_string(),
        };
        let handle = start_broker(
            &config,
            BrokerState::new(Some(log_path.clone())),
            Vec::new(),
        );
        if let Ok(handle) = handle {
            publish_message(&handle.state, &make_status("a", "working"));
            publish_message(&handle.state, &make_artifact("a", "done", &[]));
            drop(handle);
            let content = std::fs::read_to_string(&log_path).unwrap();
            assert_eq!(content.lines().count(), 2);
        }
    }

    #[test]
    fn no_flush_thread_when_no_log_path() {
        let config = BrokerConfig {
            enabled: true,
            #[allow(clippy::cast_possible_truncation)]
            port: 19_400 + (std::process::id() as u16 % 100),
            bind: "127.0.0.1".to_string(),
        };
        if let Ok(handle) = start_broker(&config, BrokerState::new(None), Vec::new()) {
            assert!(handle.flush_thread.is_none());
            publish_message(&handle.state, &make_status("a", "working"));
            let inner = handle.state.read();
            assert!(inner.agents.contains_key("a"));
        }
    }

    #[test]
    fn disk_failure_does_not_affect_state() {
        let bad_path = std::path::PathBuf::from("/nonexistent/path/broker.log");
        let state = Arc::new(BrokerState::new(Some(bad_path.clone())));

        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_artifact("a", "done", &[]));

        let mut last_flushed = 0u64;
        flush_entries(&state, &bad_path, &mut last_flushed);

        // In-memory state is unaffected
        let inner = state.read();
        assert_eq!(inner.message_log.len(), 2);
        assert!(inner.agents.contains_key("a"));
    }

    // === recent_messages function ===

    #[test]
    fn recent_messages_returns_empty_when_no_messages() {
        let state = fresh_state();
        let messages = recent_messages(&state, 10);
        assert!(messages.is_empty());
    }

    #[test]
    fn recent_messages_returns_messages_in_reverse_order() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working")); // seq 1
        publish_message(&state, &make_status("b", "working")); // seq 2
        publish_message(&state, &make_artifact("a", "done", &[])); // seq 3

        let messages = recent_messages(&state, 10);
        assert_eq!(messages.len(), 3);
        // Should be in reverse order (newest first)
        assert_eq!(messages[0].0, 3); // seq 3
        assert_eq!(messages[1].0, 2); // seq 2
        assert_eq!(messages[2].0, 1); // seq 1
    }

    #[test]
    fn recent_messages_respects_limit() {
        let state = fresh_state();
        for i in 0..5 {
            publish_message(&state, &make_status(&format!("agent-{i}"), "working"));
        }

        let messages = recent_messages(&state, 3);
        assert_eq!(messages.len(), 3);
        // Should get the 3 most recent (seqs 5, 4, 3)
        assert_eq!(messages[0].0, 5);
        assert_eq!(messages[1].0, 4);
        assert_eq!(messages[2].0, 3);
    }

    #[test]
    fn recent_messages_includes_all_types() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_artifact("b", "done", &[]));
        publish_message(&state, &make_blocked("c", "types", "b"));
        publish_message(&state, &make_verified("d", "supervisor", None));
        publish_message(&state, &make_feedback("e", "supervisor", &["error"]));
        publish_message(&state, &make_question("f", "question?"));

        let messages = recent_messages(&state, 10);
        assert_eq!(messages.len(), 6);
        // Verify all types are present by checking message variants
        let has_status = messages
            .iter()
            .any(|(_, _, msg)| matches!(msg, BrokerMessage::Status { .. }));
        let has_artifact = messages
            .iter()
            .any(|(_, _, msg)| matches!(msg, BrokerMessage::Artifact { .. }));
        let has_blocked = messages
            .iter()
            .any(|(_, _, msg)| matches!(msg, BrokerMessage::Blocked { .. }));
        let has_verified = messages
            .iter()
            .any(|(_, _, msg)| matches!(msg, BrokerMessage::Verified { .. }));
        let has_feedback = messages
            .iter()
            .any(|(_, _, msg)| matches!(msg, BrokerMessage::Feedback { .. }));
        let has_question = messages
            .iter()
            .any(|(_, _, msg)| matches!(msg, BrokerMessage::Question { .. }));

        assert!(has_status, "Should contain Status message");
        assert!(has_artifact, "Should contain Artifact message");
        assert!(has_blocked, "Should contain Blocked message");
        assert!(has_verified, "Should contain Verified message");
        assert!(has_feedback, "Should contain Feedback message");
        assert!(has_question, "Should contain Question message");
    }

    // === Sequence number correctness ===

    #[test]
    fn first_message_gets_sequence_one() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working"));
        publish_message(&state, &make_status("b", "working"));
        publish_message(&state, &make_artifact("a", "done", &[])); // seq 3

        let inner = state.read();
        assert_eq!(inner.message_log[0].0, 1);
    }

    #[test]
    fn sequence_numbers_are_globally_monotonic() {
        let state = fresh_state();
        publish_message(&state, &make_status("a", "working")); // seq 1
        publish_message(&state, &make_status("b", "working")); // seq 2
        publish_message(&state, &make_artifact("a", "done", &[])); // seq 3 -> b's inbox
        publish_message(&state, &make_artifact("b", "done", &[])); // seq 4 -> a's inbox

        let inner = state.read();
        let b_inbox_seq = inner.queues["b"][0].0; // should be 3
        let a_inbox_seq = inner.queues["a"][0].0; // should be 4
        assert!(b_inbox_seq < a_inbox_seq);
    }

    // === Terminal Status Protection Tests ===

    #[test]
    fn terminal_state_not_overwritten_by_non_terminal() {
        let state = fresh_state();
        // Set agent to terminal state "done"
        publish_message(&state, &make_artifact("feat-errors", "done", &[]));

        // Verify status is "done"
        assert_eq!(state.read().agents["feat-errors"].status, "done");

        // Try to overwrite with non-terminal state "working"
        publish_message(&state, &make_status("feat-errors", "working"));

        // Verify status remains "done" (protected)
        assert_eq!(state.read().agents["feat-errors"].status, "done");
    }

    #[test]
    fn terminal_state_not_overwritten_by_non_terminal_simple() {
        // Simplified version of the hanging test
        let state = fresh_state();

        // Set agent to terminal state "done"
        publish_message(&state, &make_artifact("feat-simple", "done", &[]));

        // Verify status is "done"
        assert_eq!(state.read().agents["feat-simple"].status, "done");

        // Try to overwrite with non-terminal state "working"
        publish_message(&state, &make_status("feat-simple", "working"));

        // Verify status remains "done" (protected)
        assert_eq!(state.read().agents["feat-simple"].status, "done");
    }

    #[test]
    fn terminal_state_can_be_overwritten_by_other_terminal() {
        let state = fresh_state();
        // Set agent to terminal state "done"
        publish_message(&state, &make_artifact("feat-errors", "done", &[]));

        // Overwrite with another terminal state "verified"
        publish_message(&state, &make_artifact("feat-errors", "verified", &[]));

        // Verify status changed to "verified"
        let inner = state.read();
        assert_eq!(inner.agents["feat-errors"].status, "verified");
    }

    #[test]
    fn non_terminal_state_can_be_overwritten_by_terminal() {
        let state = fresh_state();
        // Set agent to non-terminal state "working"
        publish_message(&state, &make_status("feat-errors", "working"));

        // Overwrite with terminal state "done"
        publish_message(&state, &make_artifact("feat-errors", "done", &[]));

        // Verify status changed to "done"
        let inner = state.read();
        assert_eq!(inner.agents["feat-errors"].status, "done");
    }

    #[test]
    fn all_terminal_states_are_protected() {
        let terminal_states = ["done", "verified", "blocked", "committed"];

        for &terminal_state in &terminal_states {
            // Create a unique agent for each terminal state
            let agent_id = format!("feat-{terminal_state}");
            let state = fresh_state(); // Use fresh_state() helper instead of Arc::new directly

            // Set agent to terminal state
            publish_message(&state, &make_artifact(&agent_id, terminal_state, &[]));

            // Try to overwrite with non-terminal state "working"
            publish_message(&state, &make_status(&agent_id, "working"));

            // Verify status remains protected
            let inner = state.read();
            assert_eq!(
                inner.agents[&agent_id].status, terminal_state,
                "Terminal state {terminal_state} should be protected from non-terminal overwrite"
            );
        }
    }

    #[test]
    fn terminal_status_protection_with_artifact_messages() {
        let state = fresh_state();
        // Set agent to terminal state via artifact
        publish_message(
            &state,
            &make_artifact("feat-config", "done", &["ConfigType"]),
        );

        // Try to overwrite with non-terminal status message
        publish_message(&state, &make_status("feat-config", "working"));

        // Verify status remains "done" (protected)
        let inner = state.read();
        assert_eq!(inner.agents["feat-config"].status, "done");
    }

    #[test]
    fn terminal_status_protection_with_blocked_messages() {
        let state = fresh_state();
        // Set agent to terminal state "blocked"
        publish_message(&state, &make_artifact("feat-ui", "blocked", &[]));

        // Try to overwrite with non-terminal status
        publish_message(&state, &make_status("feat-ui", "idle"));

        // Verify status remains "blocked" (protected)
        let inner = state.read();
        assert_eq!(inner.agents["feat-ui"].status, "blocked");
    }
}
