//! Message routing, cursor-based polling, and log flush.
//!
//! Contains the core delivery logic for the broker: publishing messages
//! to agent inboxes, polling with cursor-based pagination, snapshot
//! queries for the dashboard, and the background log flush thread.

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
        | BrokerMessage::Question { agent_id, .. }
        | BrokerMessage::Intent { agent_id, .. } => agent_id,
        BrokerMessage::Verified { payload, .. } => &payload.verified_by,
        BrokerMessage::Feedback { payload, .. } => &payload.from,
        // The sender identity lives in the payload's `from` (typically
        // "supervisor"), like Verified/Feedback.
        BrokerMessage::AdvancedMain { payload } => &payload.from,
        // The learning's sender lives in the payload (no envelope id).
        BrokerMessage::Learning { payload } => &payload.agent_id,
        // Broker-emitted; the branch it nudges is the closest identity.
        BrokerMessage::VerifyNow { branch_id } => branch_id,
    }
}
/// Whether a message updates the agent roster (`/status` `agents[]`).
///
/// Only messages whose sender is the real top-level `agent_id` —
/// `Status`, `Artifact`, `Blocked`, `Intent` — may mint or mutate a roster
/// row. `Feedback`, `Question`, `Verified`, and `AdvancedMain` carry the
/// sender's identity in *payload* fields (`from` / `verified_by`); harvesting
/// those into the roster mints phantom rows (W15-16: a `from:"human"` feedback
/// created a `"human"` agent that never heartbeats). `VerifyNow` is
/// broker-internal. All excluded variants are still routed and stored by
/// [`route_message`]; they just never touch the roster.
fn upserts_roster(msg: &BrokerMessage) -> bool {
    matches!(
        msg,
        BrokerMessage::Status { .. }
            | BrokerMessage::Artifact { .. }
            | BrokerMessage::Blocked { .. }
            | BrokerMessage::Intent { .. }
    )
}

/// Updates (or creates) the agent record and inbox for the message sender.
fn update_agent_record(inner: &mut BrokerStateInner, msg: &BrokerMessage) {
    // Roster is populated only from status-publishing senders (W15-16). The
    // excluded variants — feedback/question/verified (payload identity
    // fields) and the broker-internal verify-now nudge — are routed and
    // stored by the caller but must not create or mutate a roster row.
    if !upserts_roster(msg) {
        return;
    }

    let ttl = inner.republish_working_ttl;
    let agent_id = sender_id(msg).to_string();
    let status = msg.status_label().to_string();
    let now = std::time::Instant::now();

    // Bug 8: an `agent.artifact status: "committed"` event stamps the record
    // so the watcher can gate post-commit `working` re-entry against the TTL.
    let is_committed_artifact =
        matches!(msg, BrokerMessage::Artifact { payload, .. } if payload.status == "committed");

    let record = inner
        .agents
        .entry(agent_id.clone())
        .or_insert_with(|| AgentRecord {
            agent_id: agent_id.clone(),
            status: String::new(),
            last_seen: now,
            last_message: None,
            last_committed_at: None,
        });

    // Make terminal states sticky: only update status if the new status is also terminal
    // or if the current status is not terminal.
    let is_terminal_status = |s: &str| matches!(s, "done" | "verified" | "blocked" | "committed");

    // Bug 8: `committed` is transient — a `working` status observed within the
    // configured TTL after the committed event re-enters the working state.
    // With TTL == 0 the v0.5.0 "committed is terminal" behaviour is preserved.
    let committed_reentry = record.status == "committed"
        && status == "working"
        && !ttl.is_zero()
        && record.last_committed_at.is_some_and(|t| t.elapsed() <= ttl);

    if committed_reentry || !is_terminal_status(&record.status) || is_terminal_status(&status) {
        record.status = status;
    }

    if is_committed_artifact {
        record.last_committed_at = Some(now);
    }

    record.last_seen = now;
    record.last_message = Some(msg.clone());

    // CLI is pre-filled authoritatively at launch — the supervisor from
    // `[supervisor].cli`/`default_cli` via `with_seeded_cli`, and each coding
    // agent from its `WatchTarget` (per-repo session JSON). The bundled skills
    // do NOT make agents self-report their CLI (git-paw knows it; an agent
    // would only be guessing). This block is a defensive fallback: if some
    // `agent.status` ever does carry a `cli`, fill the map only when empty so
    // it can never clobber the authoritative prefill.
    if let BrokerMessage::Status { payload, .. } = msg
        && let Some(cli) = payload.cli.as_ref()
        && !cli.is_empty()
    {
        inner
            .agent_clis
            .entry(agent_id.clone())
            .or_insert_with(|| cli.clone());
    }

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
    {
        let mut inner = state.write();

        update_agent_record(&mut inner, msg);

        // Append to the in-memory message log
        inner
            .message_log
            .push((seq, SystemTime::now(), msg.clone()));

        // Route based on message type
        route_message(&mut inner, msg, seq);
    }

    // Forward to the learnings aggregator after delivery completes. The
    // aggregator only writes to a markdown file — it does NOT publish back
    // into the broker.
    if let Some(agg) = state.learnings.as_ref()
        && let Ok(mut a) = agg.lock()
    {
        a.observe(msg);
    }

    // Per-commit verification nudge: when an agent publishes a `committed`
    // artifact and the feature is enabled, emit a `supervisor.verify-now`
    // nudge so the supervisor verifies this commit on an explicit event
    // rather than waiting to notice it during a sweep — and never batches it
    // with another agent's commit. The nudge carries the committing branch
    // verbatim. Emitted as a separate message (recursive publish) so it gets
    // its own sequence number, lands in the log, and routes to the supervisor
    // inbox; `update_agent_record` skips it so it does not perturb the
    // committing agent's record.
    if state.verify_on_commit_nudge
        && let BrokerMessage::Artifact { agent_id, payload } = msg
        && payload.status == "committed"
    {
        let nudge = BrokerMessage::VerifyNow {
            branch_id: agent_id.clone(),
        };
        publish_message(state, &nudge);
    }

    // opsx role-gating guard: when a `committed` artifact arrives and a
    // role-gating context is attached, classify the commit and publish
    // feedback/learning if a non-supervisor agent performed archive activity
    // under the OpenSpec engine. Runs after the write lock is released (it
    // publishes its own messages, mirroring the verify-now nudge above). The
    // guard itself re-checks activation, so this only filters on the cheap
    // message shape here.
    if let Some(ctx) = state.role_gating.as_ref()
        && let BrokerMessage::Artifact { agent_id, payload } = msg
        && payload.status == "committed"
    {
        crate::opsx::role_guard::run_guard(state, agent_id, payload, ctx);
    }
}

fn route_message(inner: &mut BrokerStateInner, msg: &BrokerMessage, seq: u64) {
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
        BrokerMessage::Question { .. } | BrokerMessage::VerifyNow { .. } => {
            // Route to the supervisor inbox, creating it if absent.
            // Do NOT enqueue in sender's or any other agent's inbox.
            // `VerifyNow` is broker-emitted; like a question it is a
            // supervisor-directed signal.
            let inbox = inner.queues.entry("supervisor".to_string()).or_default();
            inbox.push((seq, msg.clone()));
        }
        BrokerMessage::Learning { payload } => {
            // A branch-scoped learning lands in that branch's inbox so it is
            // retrievable via `messages/<branch_id>`; a cross-cutting learning
            // (no `branch_id`) goes to the supervisor inbox. Either inbox is
            // created if absent. The record is always retained in the message
            // log (by the caller) regardless of routing.
            let target = payload.branch_id.as_deref().unwrap_or("supervisor");
            let inbox = inner.queues.entry(target.to_string()).or_default();
            inbox.push((seq, msg.clone()));
        }
        BrokerMessage::Intent { agent_id, .. } => {
            // Broadcast to every other registered agent's inbox, skipping
            // the sender. Agents without an existing inbox are silently
            // skipped — same pattern as Artifact / Verified.
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
        BrokerMessage::AdvancedMain { payload } => {
            // Broadcast to every other registered agent's inbox, skipping the
            // publisher (`payload.from`). Every dependent agent learns the
            // base moved on its next `/messages/<id>` poll without a special
            // subscription — same broadcast pattern as Artifact / Intent.
            let sender = payload.from.clone();
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
    // A roster row appears only for an agent that has actually published
    // (the `agents` map) — consistent with the status-publisher-only rule
    // (W15-16), so an unstarted or aborted pane never shows a phantom row
    // (supervisor included). The CLI column is filled from the authoritative
    // `agent_clis` seed (supervisor from config, agents from their
    // `WatchTarget`), so when a row does appear its CLI is the launcher-known
    // value — never a self-reported guess and never blank for a known pane.
    let mut out: Vec<AgentStatusEntry> = inner
        .agents
        .values()
        .map(|r| {
            let cli = inner
                .agent_clis
                .get(&r.agent_id)
                .cloned()
                .unwrap_or_default();
            // Prefer the most-recent `payload.phase` (if any) so the dashboard
            // can show it over the message-type-derived status label.
            let phase = if let Some(BrokerMessage::Status { payload, .. }) = r.last_message.as_ref()
            {
                payload.phase.clone()
            } else {
                None
            };
            AgentStatusEntry {
                agent_id: r.agent_id.clone(),
                cli,
                status: r.status.clone(),
                last_seen_seconds: r.last_seen.elapsed().as_secs(),
                last_seen: r.last_seen,
                phase,
            }
        })
        .collect();
    // Sort by agent_id so the dashboard rows stay in a stable order across
    // ticks — otherwise HashMap iteration order makes rows jitter on every
    // redraw.
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
        ArtifactPayload, BlockedPayload, FeedbackPayload, IntentPayload, QuestionPayload,
        StatusPayload, VerifiedPayload,
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
                ..Default::default()
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

    fn make_intent(agent_id: &str, files: &[&str], summary: &str, ttl: u64) -> BrokerMessage {
        BrokerMessage::Intent {
            agent_id: agent_id.to_string(),
            payload: IntentPayload {
                files: files
                    .iter()
                    .map(|s| crate::broker::messages::FileIntent::from(*s))
                    .collect(),
                summary: summary.to_string(),
                valid_for_seconds: ttl,
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
    fn verified_does_not_mutate_verifier_record() {
        // W15-16: a `Verified` carries the verifier in `payload.verified_by`,
        // not in a top-level agent_id. Harvesting that identity into the
        // roster wrongly flips the verifier's own row to "verified". The
        // roster is now status-publisher-only, so the verifier's record keeps
        // its real status.
        let state = fresh_state();
        publish_message(&state, &make_status("supervisor", "working"));

        publish_message(&state, &make_verified("feat-errors", "supervisor", None));

        let inner = state.read();
        let record = inner
            .agents
            .get("supervisor")
            .expect("supervisor record exists");
        assert_eq!(
            record.status, "working",
            "a Verified message must not mutate the verifier's roster row",
        );
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
    fn feedback_does_not_mutate_sender_record() {
        // W15-16: a `Feedback` carries its sender in `payload.from`, not a
        // top-level agent_id. The roster is status-publisher-only, so a
        // supervisor-originated feedback must not flip the supervisor's row to
        // "feedback".
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
        assert_eq!(
            record.status, "working",
            "a Feedback message must not mutate the sender's roster row",
        );
    }

    #[test]
    fn feedback_from_non_agent_creates_no_phantom_row() {
        // W15-16: the headline phantom — a `from:"human"` feedback minted a
        // "human" agent that never heartbeats. The roster must contain only
        // the real status publishers.
        let state = fresh_state();
        publish_message(&state, &make_status("feat-errors", "working"));
        publish_message(&state, &make_status("supervisor", "working"));

        publish_message(
            &state,
            &make_feedback("feat-errors", "human", &["fix the flaky test"]),
        );

        let inner = state.read();
        assert!(
            !inner.agents.contains_key("human"),
            "a feedback's `from` identity must never mint a roster row",
        );
        assert_eq!(
            inner.agents.len(),
            2,
            "roster holds exactly the two status publishers",
        );
        // ...and the feedback is still delivered to its target.
        drop(inner);
        let (msgs, _) = poll_messages(&state, "feat-errors", 0);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].status_label(), "feedback");
    }

    #[test]
    fn fresh_broker_state_builds_roster_only_from_status_publishers() {
        // W15-16: the roster is in-memory; a restart (a brand-new
        // `BrokerState`, as every `git paw start` produces) carries no
        // pre-existing phantom. After re-registration the roster holds only
        // the fresh status publishers.
        let state = fresh_state();
        {
            let inner = state.read();
            assert!(inner.agents.is_empty(), "a fresh broker has no roster rows");
        }
        publish_message(&state, &make_status("feat-a", "working"));
        publish_message(&state, &make_status("supervisor", "working"));
        // Identity-bearing traffic that would have minted phantoms.
        publish_message(&state, &make_feedback("feat-a", "human", &["nudge"]));

        let inner = state.read();
        let mut ids: Vec<&str> = inner.agents.keys().map(String::as_str).collect();
        ids.sort_unstable();
        assert_eq!(ids, ["feat-a", "supervisor"]);
    }

    #[test]
    fn question_and_verified_identities_create_no_phantom_rows() {
        // W15-16 scenario 2: identities embedded in question/verified
        // messages never appear as roster rows unless they independently
        // publish agent.status.
        let state = fresh_state();
        publish_message(&state, &make_status("supervisor", "working"));

        // A question/verified naming identities that never published status.
        publish_message(&state, &make_verified("feat-x", "reviewer-bot", None));
        publish_message(&state, &make_question("feat-x", "proceed?"));

        let inner = state.read();
        assert!(!inner.agents.contains_key("reviewer-bot"));
        assert!(!inner.agents.contains_key("feat-x"));
        assert_eq!(
            inner.agents.len(),
            1,
            "only the status-publishing supervisor is a roster row",
        );
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

    // === Intent broadcast (forward-coordination) ===

    #[test]
    fn intent_broadcast_reaches_all_peers() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-auth", "working"));
        publish_message(&state, &make_status("feat-detect", "working"));
        publish_message(&state, &make_status("supervisor", "working"));

        publish_message(
            &state,
            &make_intent("feat-auth", &["src/a.rs"], "wire AuthClient", 600),
        );

        let (detect_msgs, _) = poll_messages(&state, "feat-detect", 0);
        let (sup_msgs, _) = poll_messages(&state, "supervisor", 0);
        assert!(
            detect_msgs
                .iter()
                .any(|m| matches!(m, BrokerMessage::Intent { .. }))
        );
        assert!(
            sup_msgs
                .iter()
                .any(|m| matches!(m, BrokerMessage::Intent { .. }))
        );
    }

    #[test]
    fn intent_broadcast_skips_sender() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-auth", "working"));
        publish_message(&state, &make_status("feat-detect", "working"));

        publish_message(
            &state,
            &make_intent("feat-auth", &["src/a.rs"], "wire AuthClient", 600),
        );

        let (own_msgs, _) = poll_messages(&state, "feat-auth", 0);
        assert!(
            !own_msgs
                .iter()
                .any(|m| matches!(m, BrokerMessage::Intent { .. }))
        );
    }

    #[test]
    fn intent_broadcast_skips_unregistered_agents() {
        let state = fresh_state();
        publish_message(&state, &make_status("feat-auth", "working"));

        publish_message(
            &state,
            &make_intent("feat-auth", &["src/a.rs"], "wire AuthClient", 600),
        );

        let inner = state.read();
        assert!(!inner.queues.contains_key("feat-detect"));
    }

    #[test]
    fn intent_updates_sender_record_status_to_intent() {
        let state = fresh_state();
        publish_message(
            &state,
            &make_intent("feat-auth", &["src/a.rs"], "wire AuthClient", 600),
        );
        let inner = state.read();
        let record = inner.agents.get("feat-auth").expect("record exists");
        assert_eq!(record.status, "intent");
    }

    // === Question coverage (v040-hardening) ===

    #[test]
    fn question_does_not_create_sender_roster_row() {
        // W15-16: agent.question does not mint a roster row. A lone question
        // from an agent that never published status leaves the roster empty
        // (the question is still routed to the supervisor inbox).
        let state = fresh_state();
        publish_message(&state, &make_question("feat-x", "Should I rebase?"));

        let inner = state.read();
        assert!(
            !inner.agents.contains_key("feat-x"),
            "a question must not create a roster row for its sender",
        );
    }

    #[test]
    fn question_leaves_existing_sender_row_unchanged() {
        // When the asker is already a roster row (it published status), a
        // subsequent question does not overwrite that row's status with
        // "question".
        let state = fresh_state();
        publish_message(&state, &make_status("feat-x", "working"));
        publish_message(&state, &make_question("feat-x", "Should I rebase?"));

        let inner = state.read();
        let record = inner
            .agents
            .get("feat-x")
            .expect("sender record exists from its status publish");
        assert_eq!(
            record.status, "working",
            "a question must not flip an existing row to status \"question\"",
        );
    }

    #[test]
    fn question_vs_blocked_inbox_creation_differs() {
        // The spec calls out that `Question` creates the supervisor inbox if
        // it is missing, whereas `Blocked` silently drops when its target
        // inbox is missing. This test pins both behaviours in one place so
        // any regression on either side is loud.
        let state = fresh_state();

        // Blocked with a non-existent target: nothing should be enqueued, and
        // no inbox should be created for the missing target.
        publish_message(
            &state,
            &make_blocked("feat-x", "needs types", "feat-missing"),
        );
        {
            let inner = state.read();
            assert!(
                !inner.queues.contains_key("feat-missing"),
                "Blocked must not create the target inbox when it is missing"
            );
        }

        // Question without any pre-existing supervisor inbox: the inbox must
        // be created and the message must be enqueued there.
        publish_message(&state, &make_question("feat-x", "anything?"));
        let inner = state.read();
        assert!(
            inner.queues.contains_key("supervisor"),
            "Question must create the supervisor inbox when it is missing"
        );
        let (msgs, _) = poll_messages(&state, "supervisor", 0);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].status_label(), "question");
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

    // === supervisor-as-pane-followups: cli + phase plumbing ===

    #[test]
    fn snapshot_carries_phase_from_most_recent_status_message() {
        let state = fresh_state();
        let msg = BrokerMessage::Status {
            agent_id: "supervisor".to_string(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: vec![],
                message: None,
                cli: Some("claude".to_string()),
                phase: Some("merging".to_string()),
                detail: None,
            },
        };
        publish_message(&state, &msg);

        let snap = agent_status_snapshot(&state);
        let entry = snap.iter().find(|e| e.agent_id == "supervisor").unwrap();
        assert_eq!(entry.phase.as_deref(), Some("merging"));
        assert_eq!(entry.cli, "claude");
    }

    #[test]
    fn snapshot_phase_is_none_when_last_message_is_not_status() {
        let state = fresh_state();
        publish_message(&state, &make_status("supervisor", "working"));
        publish_message(
            &state,
            &make_feedback("feat-x", "supervisor", &["bad test"]),
        );

        let snap = agent_status_snapshot(&state);
        let entry = snap.iter().find(|e| e.agent_id == "supervisor").unwrap();
        assert_eq!(
            entry.phase, None,
            "Feedback as last_message must not carry over a phase"
        );
    }

    #[test]
    fn supervisor_cli_lands_in_agent_clis_via_status_payload() {
        let state = fresh_state();
        let msg = BrokerMessage::Status {
            agent_id: "supervisor".to_string(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: vec![],
                message: None,
                cli: Some("claude".to_string()),
                phase: Some("baseline".to_string()),
                detail: None,
            },
        };
        publish_message(&state, &msg);

        let inner = state.read();
        assert_eq!(
            inner.agent_clis.get("supervisor").map(String::as_str),
            Some("claude"),
            "supervisor's cli must be upserted into agent_clis from the status payload",
        );
    }

    #[test]
    fn coding_agent_status_cli_appears_in_snapshot() {
        // W15-15: a coding agent's boot `agent.status` carries its CLI, and
        // the broker surfaces it in the `/status` snapshot row — not just for
        // the supervisor.
        let state = fresh_state();
        let msg = BrokerMessage::Status {
            agent_id: "feat-roster".to_string(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: vec![],
                message: None,
                cli: Some("claude-oss".to_string()),
                phase: None,
                detail: None,
            },
        };
        publish_message(&state, &msg);

        let snap = agent_status_snapshot(&state);
        let entry = snap.iter().find(|e| e.agent_id == "feat-roster").unwrap();
        assert_eq!(
            entry.cli, "claude-oss",
            "a coding agent's cli must populate its snapshot row",
        );
    }

    #[test]
    fn seeded_cli_appears_only_after_the_pane_publishes() {
        // A roster row appears only once the pane publishes (W15-16
        // publisher-only rule) — a seeded-but-unpublished pane shows NO
        // phantom row. When it does publish, its row carries the
        // authoritatively-seeded CLI (not a self-reported value).
        let state = Arc::new(BrokerState::new(None).with_seeded_cli("supervisor", "claude-oss"));

        // Seeded but not yet published → no row.
        let snap = agent_status_snapshot(&state);
        assert!(
            snap.iter().all(|e| e.agent_id != "supervisor"),
            "a seeded-but-unpublished pane must not show a phantom row",
        );

        // After publishing (no cli in the payload), the row appears with the
        // seeded cli.
        publish_message(&state, &make_status("supervisor", "working"));
        let snap = agent_status_snapshot(&state);
        let entry = snap
            .iter()
            .find(|e| e.agent_id == "supervisor")
            .expect("supervisor row appears once it publishes");
        assert_eq!(
            entry.cli, "claude-oss",
            "the published row carries the authoritatively-seeded cli",
        );
    }

    #[test]
    fn seeded_cli_wins_over_a_wrong_self_report() {
        // The launcher-authoritative seed (config/watch-target) must not be
        // clobbered by an agent's self-reported guess. The supervisor reported
        // `claude` while actually running `claude-oss`; the seeded value wins.
        let state = Arc::new(BrokerState::new(None).with_seeded_cli("supervisor", "claude-oss"));
        let msg = BrokerMessage::Status {
            agent_id: "supervisor".to_string(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: vec![],
                message: None,
                cli: Some("claude".to_string()),
                phase: None,
                detail: None,
            },
        };
        publish_message(&state, &msg);

        let snap = agent_status_snapshot(&state);
        let entry = snap.iter().find(|e| e.agent_id == "supervisor").unwrap();
        assert_eq!(
            entry.cli, "claude-oss",
            "the authoritative seed must win over a wrong self-reported cli",
        );
    }

    #[test]
    fn with_seeded_cli_ignores_blank_value() {
        // A missing config value must never clobber the map or mint a blank
        // row.
        let state = Arc::new(BrokerState::new(None).with_seeded_cli("supervisor", ""));
        let snap = agent_status_snapshot(&state);
        assert!(
            snap.iter().all(|e| e.agent_id != "supervisor"),
            "a blank seed must not create a supervisor row",
        );
    }

    #[test]
    fn snapshot_resolves_cli_from_seeded_map_when_status_omits_it() {
        // W15-15 fallback: when an agent's status payload has no `cli`, the
        // broker still shows the CLI seeded from the per-repo session JSON
        // (modelled here by the `agent_clis` map the watch-target seeding
        // fills at broker start).
        let state = fresh_state();
        {
            let mut inner = state.write();
            inner
                .agent_clis
                .insert("feat-seeded".to_string(), "claude-oss".to_string());
        }
        // A cli-less status (e.g. the filesystem watcher's auto-publish).
        publish_message(&state, &make_status("feat-seeded", "working"));

        let snap = agent_status_snapshot(&state);
        let entry = snap.iter().find(|e| e.agent_id == "feat-seeded").unwrap();
        assert_eq!(
            entry.cli, "claude-oss",
            "snapshot must fall back to the seeded cli when status omits it",
        );
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
            ..Default::default()
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
            ..Default::default()
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
        // `committed` is intentionally excluded: bug 8 makes it transient — a
        // `working` status within the re-entry TTL re-enters working. The
        // committed-specific transitions are covered by the dedicated tests
        // below. `done` / `verified` / `blocked` remain fully terminal.
        let terminal_states = ["done", "verified", "blocked"];

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

    // === Bug 8: committed -> working re-entry within TTL ===

    #[test]
    fn committed_artifact_stamps_last_committed_at() {
        let state = fresh_state();
        publish_message(&state, &make_artifact("feat-x", "committed", &[]));
        let inner = state.read();
        let rec = inner.agents.get("feat-x").expect("record exists");
        assert_eq!(rec.status, "committed");
        assert!(
            rec.last_committed_at.is_some(),
            "committed artifact must stamp last_committed_at"
        );
    }

    #[test]
    fn committed_reenters_working_within_ttl() {
        // Default fresh_state TTL is 60s, so an immediate working re-enters.
        let state = fresh_state();
        publish_message(&state, &make_artifact("feat-x", "committed", &[]));
        assert_eq!(state.read().agents["feat-x"].status, "committed");

        publish_message(&state, &make_status("feat-x", "working"));
        assert_eq!(
            state.read().agents["feat-x"].status,
            "working",
            "a working status within the TTL must re-enter the working state"
        );
    }

    #[test]
    fn committed_stays_terminal_when_ttl_zero() {
        // TTL == 0 restores v0.5.0: committed is terminal against working.
        let state = fresh_state();
        state.set_republish_working_ttl(Duration::ZERO);
        publish_message(&state, &make_artifact("feat-x", "committed", &[]));
        publish_message(&state, &make_status("feat-x", "working"));
        assert_eq!(
            state.read().agents["feat-x"].status,
            "committed",
            "with TTL=0, committed must stay terminal (v0.5.0 model)"
        );
    }

    #[test]
    fn committed_does_not_reenter_after_ttl_window() {
        // A short TTL plus a back-dated committed timestamp simulates a write
        // that arrives after the window has elapsed.
        let state = fresh_state();
        state.set_republish_working_ttl(Duration::from_secs(5));
        publish_message(&state, &make_artifact("feat-x", "committed", &[]));
        {
            let mut inner = state.write();
            let rec = inner.agents.get_mut("feat-x").unwrap();
            rec.last_committed_at = Some(
                std::time::Instant::now()
                    .checked_sub(Duration::from_secs(90))
                    .unwrap(),
            );
        }
        publish_message(&state, &make_status("feat-x", "working"));
        assert_eq!(
            state.read().agents["feat-x"].status,
            "committed",
            "a working status past the TTL window must not re-enter working"
        );
    }

    // Maps to scenario `Question creates supervisor inbox when absent` from
    // v040-hardening. (test-coverage-v0-5-0 task 8.1)
    #[test]
    fn question_creates_supervisor_inbox_when_absent() {
        let state = fresh_state();
        // Register feat-x but no supervisor inbox.
        publish_message(&state, &make_status("feat-x", "working"));
        {
            let inner = state.read();
            assert!(
                !inner.queues.contains_key("supervisor"),
                "supervisor inbox must be absent before publishing the question"
            );
        }

        publish_message(&state, &make_question("feat-x", "How should I proceed?"));

        {
            let inner = state.read();
            assert!(
                inner.queues.contains_key("supervisor"),
                "publishing an agent.question must create the supervisor inbox; got queues: {:?}",
                inner.queues.keys().collect::<Vec<_>>()
            );
        }

        let (messages, last_seq) = poll_messages(&state, "supervisor", 0);
        assert_eq!(
            messages.len(),
            1,
            "supervisor inbox should contain the published question"
        );
        assert!(
            matches!(&messages[0], BrokerMessage::Question { agent_id, payload }
                if agent_id == "feat-x" && payload.question == "How should I proceed?"),
            "supervisor inbox should hold the original question; got: {:?}",
            messages[0]
        );
        assert!(
            last_seq > 0,
            "poll_messages should return a non-zero cursor"
        );
    }

    // === supervisor.verify-now nudge (per-commit-verification-v0-6-x) ===

    fn nudge_state() -> Arc<BrokerState> {
        Arc::new(BrokerState::new(None).with_verify_on_commit_nudge(true))
    }

    fn supervisor_verify_now_branches(state: &Arc<BrokerState>) -> Vec<String> {
        let (msgs, _) = poll_messages(state, "supervisor", 0);
        msgs.into_iter()
            .filter_map(|m| match m {
                BrokerMessage::VerifyNow { branch_id } => Some(branch_id),
                _ => None,
            })
            .collect()
    }

    // Scenario: Nudge published on committed event.
    #[test]
    fn committed_artifact_publishes_verify_now_to_supervisor() {
        let state = nudge_state();
        publish_message(&state, &make_artifact("feat-foo", "committed", &[]));

        assert_eq!(
            supervisor_verify_now_branches(&state),
            vec!["feat-foo".to_string()],
            "a committed artifact must publish exactly one verify-now nudge carrying the branch"
        );
    }

    // Scenario: Default config enables the nudge — a default SupervisorConfig
    // resolves verify_on_commit_nudge to true, so a committed artifact through
    // a broker wired from that default publishes the nudge.
    #[test]
    fn default_config_enables_the_nudge() {
        let enabled = crate::config::SupervisorConfig::default().verify_on_commit_nudge_enabled();
        let state = Arc::new(BrokerState::new(None).with_verify_on_commit_nudge(enabled));
        publish_message(&state, &make_artifact("feat-foo", "committed", &[]));

        assert_eq!(
            supervisor_verify_now_branches(&state),
            vec!["feat-foo".to_string()],
            "the default config must enable the verify-now nudge"
        );
    }

    // Scenario: the nudge carries the committing branch verbatim, including the
    // slashed `feat/foo` form the post-commit hook may use.
    #[test]
    fn verify_now_carries_committing_branch_verbatim() {
        let state = nudge_state();
        publish_message(&state, &make_artifact("feat/foo", "committed", &[]));

        assert_eq!(
            supervisor_verify_now_branches(&state),
            vec!["feat/foo".to_string()]
        );
    }

    // Scenario: Nudge suppressed when disabled.
    #[test]
    fn committed_artifact_suppresses_nudge_when_disabled() {
        // `fresh_state()` leaves verify_on_commit_nudge at its `false` default.
        let state = fresh_state();
        publish_message(&state, &make_artifact("feat-foo", "committed", &[]));

        assert!(
            supervisor_verify_now_branches(&state).is_empty(),
            "no verify-now nudge may be published when the feature is disabled"
        );
    }

    // A non-committed terminal status (e.g. `done`) must NOT trigger the nudge —
    // the trigger is specifically the committed artifact event.
    #[test]
    fn done_artifact_does_not_trigger_nudge() {
        let state = nudge_state();
        publish_message(&state, &make_artifact("feat-foo", "done", &[]));

        assert!(
            supervisor_verify_now_branches(&state).is_empty(),
            "only `committed` artifacts trigger the verify-now nudge, not `done`"
        );
    }

    // The nudge must not perturb the committing agent's record: its sticky
    // `committed` status and its own `last_message` are preserved.
    #[test]
    fn nudge_does_not_overwrite_committing_agent_record() {
        let state = nudge_state();
        publish_message(&state, &make_artifact("feat-foo", "committed", &["api_fn"]));

        let inner = state.read();
        let record = inner
            .agents
            .get("feat-foo")
            .expect("committing record exists");
        assert_eq!(record.status, "committed", "status must remain committed");
        assert!(
            matches!(record.last_message, Some(BrokerMessage::Artifact { .. })),
            "last_message must remain the committing artifact, not the nudge"
        );
    }

    // The nudge is logged with its own sequence number after the artifact.
    #[test]
    fn nudge_appears_in_message_log_after_artifact() {
        let state = nudge_state();
        publish_message(&state, &make_artifact("feat-foo", "committed", &[]));

        let inner = state.read();
        assert_eq!(inner.message_log.len(), 2, "artifact + nudge");
        assert!(matches!(
            inner.message_log[0].2,
            BrokerMessage::Artifact { .. }
        ));
        assert!(matches!(
            inner.message_log[1].2,
            BrokerMessage::VerifyNow { .. }
        ));
        assert!(
            inner.message_log[0].0 < inner.message_log[1].0,
            "nudge sequence number must follow the artifact's"
        );
    }
}
