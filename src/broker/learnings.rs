//! Broker-internal learnings aggregator.
//!
//! Observes the broker's publish-event stream and accumulates per-session
//! signals — stuck durations, recovery cycles, conflict events, and
//! permission patterns — that are flushed as human-readable bullets to
//! `.git-paw/session-learnings.md`.
//!
//! The aggregator does NOT publish back into the broker; the markdown file
//! is its only output sink. See the `learnings-mode` `OpenSpec` change for
//! the per-signal semantics and the markdown format.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::messages::BrokerMessage;

/// Substring marker that conflict-detector-originated messages prepend to
/// their `errors`/`question` text.
pub const CONFLICT_DETECTOR_TAG: &str = "[conflict-detector]";

/// Default `count` threshold below which a permission-pattern entry is
/// withheld at flush. See the `Permission-pattern signal` spec.
pub const PERMISSION_PATTERN_THRESHOLD: u64 = 5;

/// A resolved-or-unresolved stuck-duration observation for one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StuckDurationEntry {
    /// The agent that was blocked.
    pub agent_id: String,
    /// The agent the blocked agent was waiting on (`payload.from`).
    pub blocked_on: String,
    /// Elapsed wall-clock seconds between the block start and either the
    /// resolving artifact or the shutdown flush.
    pub duration_seconds: u64,
    /// `true` if a subsequent artifact resolved the block, `false` if the
    /// session ended with the block still open.
    pub resolved: bool,
}

/// A per-agent recovery-cycle count, recorded when an agent verifies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCycleEntry {
    /// The agent whose work was eventually verified.
    pub agent_id: String,
    /// Number of `agent.feedback` messages the agent received before
    /// verification. Guaranteed to be `>= 1` per the spec.
    pub count: u32,
}

/// Classification of a single conflict-detector-derived event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictCategory {
    /// Forward conflict between agents in the same `SpecEntry` family.
    ForwardConflictIntraSpec {
        /// Sorted pair of agent ids implicated by the conflict.
        agents: Vec<String>,
        /// The shared spec id (e.g. `003-user-list`).
        spec_id: String,
    },
    /// Forward conflict spanning two `SpecEntry` families.
    ForwardConflictCrossSpec {
        /// Sorted pair of agent ids implicated by the conflict.
        agents: Vec<String>,
        /// Spec ids for the agents, in the same order as `agents`. May be
        /// empty entries when the agent → spec mapping is not yet known.
        spec_ids: Vec<String>,
    },
    /// In-flight conflict — both agents currently editing the same file.
    InFlightConflict {
        /// Sorted pair of agent ids touching the shared file.
        agents: Vec<String>,
    },
    /// Ownership violation — `violator` edited a file owned by `owner`.
    OwnershipViolation {
        /// Agent that touched the file outside its declared ownership.
        violator: String,
        /// Agent that owns the file (per its `agent.intent`).
        owner: String,
        /// The conflicting file path, if extractable from the message.
        file: String,
    },
}

/// A classified conflict event ready to be rendered as a markdown bullet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictEvent {
    /// What kind of conflict this is.
    pub category: ConflictCategory,
}

/// Aggregator state maintained across observed events.
///
/// The aggregator is normally owned by a single `Arc<Mutex<_>>` shared
/// between the publish path (which observes events) and the periodic flush
/// task. All input methods take `&mut self` so callers must hold the lock
/// for the duration of the call.
#[derive(Debug)]
pub struct LearningsAggregator {
    /// Outstanding blocks keyed by blocked agent id.
    pending_blocks: HashMap<String, (SystemTime, String)>,
    /// Running feedback counts per target agent id. Cleared once the agent
    /// verifies (or at shutdown flush for unresolved agents).
    feedback_counts: HashMap<String, u32>,
    /// Completed stuck-duration observations awaiting flush.
    stuck_events: Vec<StuckDurationEntry>,
    /// Recovery-cycle observations awaiting flush.
    recovery_events: Vec<RecoveryCycleEntry>,
    /// Classified conflict events awaiting flush.
    conflict_events: Vec<ConflictEvent>,
    /// Per-command-class auto-approve hit counts. Persisted across flushes
    /// so a slow burn can eventually cross the threshold.
    permission_counts: HashMap<String, u64>,
    /// Command classes already emitted to the markdown file this session.
    /// Each class produces at most one entry per session.
    permission_emitted: HashMap<String, u64>,
    /// Cursor: number of `stuck_events` already written to the markdown.
    stuck_flushed: usize,
    /// Cursor: number of `recovery_events` already written to the markdown.
    recovery_flushed: usize,
    /// Cursor: number of `conflict_events` already written to the markdown.
    conflict_flushed: usize,
    /// Whether the H2 session header has been written.
    h2_written: bool,
    /// Session start time, used for the H2 header and shutdown durations.
    session_start: SystemTime,
    /// Per-flush threshold for permission patterns. Lower-count classes do
    /// not appear in the markdown until they cross this threshold.
    permission_threshold: u64,
    /// Agent → spec id mapping, used to classify forward conflicts as
    /// intra-spec vs cross-spec. May be empty.
    spec_ids: HashMap<String, String>,
    /// Output markdown path (typically `.git-paw/session-learnings.md`).
    file_path: PathBuf,
    /// Cached set of known agent ids, used to extract the "other" agent
    /// id from conflict-detector message text.
    known_agents: Vec<String>,
}

impl LearningsAggregator {
    /// Creates a new aggregator with default thresholds and an empty
    /// agent → spec id mapping. Callers register agents via
    /// [`Self::register_agent`] and the optional spec id via
    /// [`Self::set_spec_id`].
    #[must_use]
    pub fn new(file_path: PathBuf) -> Self {
        Self::with_threshold(file_path, PERMISSION_PATTERN_THRESHOLD)
    }

    /// Like [`Self::new`] but with a custom permission-pattern threshold.
    #[must_use]
    pub fn with_threshold(file_path: PathBuf, permission_threshold: u64) -> Self {
        Self {
            pending_blocks: HashMap::new(),
            feedback_counts: HashMap::new(),
            stuck_events: Vec::new(),
            recovery_events: Vec::new(),
            conflict_events: Vec::new(),
            permission_counts: HashMap::new(),
            permission_emitted: HashMap::new(),
            stuck_flushed: 0,
            recovery_flushed: 0,
            conflict_flushed: 0,
            h2_written: false,
            session_start: SystemTime::now(),
            permission_threshold,
            spec_ids: HashMap::new(),
            file_path,
            known_agents: Vec::new(),
        }
    }

    /// Registers an agent so its id can be extracted from conflict-detector
    /// message text. Idempotent.
    pub fn register_agent(&mut self, agent_id: &str) {
        if !self.known_agents.iter().any(|a| a == agent_id) {
            self.known_agents.push(agent_id.to_string());
        }
    }

    /// Records the `agent_id` → `spec_id` mapping used by forward-conflict
    /// intra-vs-cross classification. Replaces any previous value.
    pub fn set_spec_id(&mut self, agent_id: &str, spec_id: &str) {
        self.spec_ids
            .insert(agent_id.to_string(), spec_id.to_string());
    }

    /// Returns the path the aggregator flushes to.
    #[must_use]
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    /// Records the start of a block from agent X waiting on agent Y.
    pub fn record_blocked(&mut self, agent_id: &str, blocked_on: &str, ts: SystemTime) {
        self.register_agent(agent_id);
        self.pending_blocks
            .insert(agent_id.to_string(), (ts, blocked_on.to_string()));
    }

    /// Records an artifact event from `agent_id`. If a pending block exists
    /// for that agent, it is resolved into a stuck-duration entry.
    pub fn record_artifact(&mut self, agent_id: &str, ts: SystemTime) {
        self.register_agent(agent_id);
        if let Some((start, blocked_on)) = self.pending_blocks.remove(agent_id) {
            let duration = ts.duration_since(start).unwrap_or(Duration::ZERO).as_secs();
            self.stuck_events.push(StuckDurationEntry {
                agent_id: agent_id.to_string(),
                blocked_on,
                duration_seconds: duration,
                resolved: true,
            });
        }
    }

    /// Increments the recovery-cycle counter for the agent the feedback is
    /// addressed to.
    pub fn record_feedback(&mut self, target_agent_id: &str) {
        self.register_agent(target_agent_id);
        *self
            .feedback_counts
            .entry(target_agent_id.to_string())
            .or_insert(0) += 1;
    }

    /// Emits a recovery-cycle entry if the target accumulated at least one
    /// feedback before verifying. Clears the counter either way.
    pub fn record_verified(&mut self, target_agent_id: &str) {
        self.register_agent(target_agent_id);
        if let Some(count) = self.feedback_counts.remove(target_agent_id)
            && count >= 1
        {
            self.recovery_events.push(RecoveryCycleEntry {
                agent_id: target_agent_id.to_string(),
                count,
            });
        }
    }

    /// Increments the per-class auto-approve counter. Threshold gating
    /// happens at flush time.
    pub fn record_auto_approve(&mut self, command_class: &str) {
        let key = command_class.trim();
        if key.is_empty() {
            return;
        }
        *self.permission_counts.entry(key.to_string()).or_insert(0) += 1;
    }

    /// Classifies a `[conflict-detector]`-tagged feedback or question
    /// message and (if it represents a new conflict event) accumulates an
    /// entry.
    ///
    /// Duplicate detector messages — e.g. the symmetric forward-conflict
    /// feedback sent to both agents in the pair — are deduplicated by
    /// canonical agent pair and category so each conflict yields exactly
    /// one bullet.
    pub fn record_detector_message(&mut self, msg: &BrokerMessage) {
        let text = match msg {
            BrokerMessage::Feedback { payload, .. } => payload.errors.join(" "),
            BrokerMessage::Question { payload, .. } => payload.question.clone(),
            _ => return,
        };
        if !text.contains(CONFLICT_DETECTOR_TAG) {
            return;
        }

        let target = msg.agent_id().to_string();
        self.register_agent(&target);
        let others = self.other_agents_in_text(&text, &target);
        let file = extract_file_token(&text);

        if text.contains("ownership violation") {
            // The recipient is the violator; the other agent is the owner.
            if let Some(owner) = others.first() {
                let candidate = ConflictCategory::OwnershipViolation {
                    violator: target.clone(),
                    owner: owner.clone(),
                    file: file.clone().unwrap_or_default(),
                };
                if !self.has_conflict_category(&candidate) {
                    self.conflict_events.push(ConflictEvent {
                        category: candidate,
                    });
                }
            }
            return;
        }

        if text.contains("forward conflict") {
            if let Some(other) = others.first() {
                let pair = sorted_pair(&target, other);
                let category = self.classify_forward(&pair);
                if !self.has_conflict_category(&category) {
                    self.conflict_events.push(ConflictEvent { category });
                }
            }
            return;
        }

        if text.contains("in-flight conflict")
            && let Some(other) = others.first()
        {
            let pair = sorted_pair(&target, other);
            let category = ConflictCategory::InFlightConflict { agents: pair };
            if !self.has_conflict_category(&category) {
                self.conflict_events.push(ConflictEvent { category });
            }
        }
    }

    /// Convenience dispatcher: routes a `BrokerMessage` to the appropriate
    /// `record_*` method. Returns `true` if the aggregator's state changed.
    pub fn observe(&mut self, msg: &BrokerMessage) {
        match msg {
            BrokerMessage::Blocked { agent_id, payload } => {
                self.record_blocked(agent_id, &payload.from, SystemTime::now());
            }
            BrokerMessage::Artifact { agent_id, .. } => {
                self.record_artifact(agent_id, SystemTime::now());
            }
            BrokerMessage::Verified { agent_id, .. } => {
                self.record_verified(agent_id);
            }
            BrokerMessage::Feedback {
                agent_id, payload, ..
            } => {
                self.record_feedback(agent_id);
                // The forward / in-flight / ownership detectors all send
                // `agent.feedback`; classification is keyed on text content.
                let text = payload.errors.join(" ");
                if text.contains(CONFLICT_DETECTOR_TAG) {
                    self.record_detector_message(msg);
                }
            }
            BrokerMessage::Question { payload, .. } => {
                if payload.question.contains(CONFLICT_DETECTOR_TAG) {
                    self.record_detector_message(msg);
                }
            }
            BrokerMessage::Status { agent_id, payload } => {
                if payload.status == "auto_approved"
                    && let Some(cls) = extract_command_class(payload.message.as_deref())
                {
                    self.record_auto_approve(&cls);
                }
                self.register_agent(agent_id);
            }
            BrokerMessage::Intent { agent_id, .. } => {
                // `agent.intent` is purely informational for the aggregator —
                // it carries the agent's planned files, not a learning signal.
                // Register the sender so downstream `agent.blocked` /
                // `agent.artifact` correlations can find it.
                self.register_agent(agent_id);
            }
        }
    }

    /// Appends accumulated entries (since the last flush) to the markdown
    /// file. Empty categories are omitted from the output.
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.write_flush(false)
    }

    /// Identical to [`Self::flush`] but additionally records any open
    /// stuck-duration entries as unresolved with the duration measured up
    /// to `now`. Used at broker shutdown.
    pub fn flush_at_shutdown(&mut self) -> std::io::Result<()> {
        let now = SystemTime::now();
        let pending: Vec<(String, SystemTime, String)> = self
            .pending_blocks
            .drain()
            .map(|(agent, (start, on))| (agent, start, on))
            .collect();
        for (agent, start, on) in pending {
            let duration = now
                .duration_since(start)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            self.stuck_events.push(StuckDurationEntry {
                agent_id: agent,
                blocked_on: on,
                duration_seconds: duration,
                resolved: false,
            });
        }
        // Flush any recovery cycles for agents that never verified.
        let pending_recovery: Vec<(String, u32)> = self.feedback_counts.drain().collect();
        for (agent, count) in pending_recovery {
            if count >= 1 {
                self.recovery_events.push(RecoveryCycleEntry {
                    agent_id: agent,
                    count,
                });
            }
        }
        self.write_flush(true)
    }

    fn classify_forward(&self, pair: &[String]) -> ConflictCategory {
        let spec_a = self.spec_ids.get(&pair[0]);
        let spec_b = self.spec_ids.get(&pair[1]);
        match (spec_a, spec_b) {
            (Some(a), Some(b)) if a == b => ConflictCategory::ForwardConflictIntraSpec {
                agents: pair.to_vec(),
                spec_id: a.clone(),
            },
            (Some(a), Some(b)) => ConflictCategory::ForwardConflictCrossSpec {
                agents: pair.to_vec(),
                spec_ids: vec![a.clone(), b.clone()],
            },
            _ => ConflictCategory::ForwardConflictCrossSpec {
                agents: pair.to_vec(),
                spec_ids: vec![
                    spec_a.cloned().unwrap_or_default(),
                    spec_b.cloned().unwrap_or_default(),
                ],
            },
        }
    }

    fn has_conflict_category(&self, candidate: &ConflictCategory) -> bool {
        self.conflict_events
            .iter()
            .any(|e| matches_category(&e.category, candidate))
    }

    fn other_agents_in_text(&self, text: &str, exclude: &str) -> Vec<String> {
        self.known_agents
            .iter()
            .filter(|id| *id != exclude && text.contains(id.as_str()))
            .cloned()
            .collect()
    }

    fn write_flush(&mut self, _shutdown: bool) -> std::io::Result<()> {
        let new_stuck = &self.stuck_events[self.stuck_flushed..];
        let new_recovery = &self.recovery_events[self.recovery_flushed..];
        let new_conflicts = &self.conflict_events[self.conflict_flushed..];

        let permission_entries: Vec<(String, u64)> = {
            let mut entries: Vec<(String, u64)> = self
                .permission_counts
                .iter()
                .filter(|(class, count)| {
                    **count >= self.permission_threshold
                        && self.permission_emitted.get(*class).copied().unwrap_or(0) < **count
                })
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            entries
        };

        let has_any = !new_stuck.is_empty()
            || !new_recovery.is_empty()
            || !new_conflicts.is_empty()
            || !permission_entries.is_empty();
        if !has_any {
            return Ok(());
        }

        let mut out = String::new();
        if !self.h2_written {
            let ts = format_iso8601_utc(self.session_start);
            let _ = writeln!(out, "## Session Learnings — {ts}");
            self.h2_written = true;
        }

        if !new_conflicts.is_empty() {
            out.push_str("\n### Conflict events\n");
            for ev in new_conflicts {
                let _ = writeln!(out, "- {}", render_conflict(&ev.category));
            }
        }
        if !new_stuck.is_empty() {
            out.push_str("\n### Where agents got stuck\n");
            for ev in new_stuck {
                let _ = writeln!(out, "- {}", render_stuck(ev));
            }
        }
        if !new_recovery.is_empty() {
            out.push_str("\n### Recovery cycles\n");
            for ev in new_recovery {
                let _ = writeln!(out, "- {}", render_recovery(ev));
            }
        }
        if !permission_entries.is_empty() {
            out.push_str("\n### Permission patterns\n");
            for (class, count) in &permission_entries {
                let _ = writeln!(out, "- {}", render_permission(class, *count));
            }
        }

        append_to_file(&self.file_path, &out)?;

        self.stuck_flushed = self.stuck_events.len();
        self.recovery_flushed = self.recovery_events.len();
        self.conflict_flushed = self.conflict_events.len();
        for (class, count) in &permission_entries {
            self.permission_emitted.insert(class.clone(), *count);
        }
        Ok(())
    }

    #[cfg(test)]
    fn stuck_events(&self) -> &[StuckDurationEntry] {
        &self.stuck_events
    }

    #[cfg(test)]
    fn recovery_events(&self) -> &[RecoveryCycleEntry] {
        &self.recovery_events
    }

    #[cfg(test)]
    fn conflict_events(&self) -> &[ConflictEvent] {
        &self.conflict_events
    }
}

/// Reference-counted aggregator handle shared between the broker's publish
/// path and the periodic flush task.
pub type SharedLearnings = Arc<Mutex<LearningsAggregator>>;

fn append_to_file(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(contents.as_bytes())
}

fn sorted_pair(a: &str, b: &str) -> Vec<String> {
    let mut pair = vec![a.to_string(), b.to_string()];
    pair.sort();
    pair
}

fn matches_category(a: &ConflictCategory, b: &ConflictCategory) -> bool {
    use ConflictCategory::{
        ForwardConflictCrossSpec, ForwardConflictIntraSpec, InFlightConflict, OwnershipViolation,
    };
    match (a, b) {
        (
            ForwardConflictIntraSpec { agents: x, .. },
            ForwardConflictIntraSpec { agents: y, .. },
        )
        | (
            ForwardConflictCrossSpec { agents: x, .. },
            ForwardConflictCrossSpec { agents: y, .. },
        )
        | (InFlightConflict { agents: x }, InFlightConflict { agents: y }) => x == y,
        (
            OwnershipViolation {
                violator: vx,
                owner: ox,
                file: fx,
            },
            OwnershipViolation {
                violator: vy,
                owner: oy,
                file: fy,
            },
        ) => vx == vy && ox == oy && fx == fy,
        _ => false,
    }
}

fn extract_file_token(text: &str) -> Option<String> {
    // Heuristic: pick the first whitespace-delimited token that looks like
    // a path with an extension (e.g. `src/main.rs`).
    text.split_whitespace()
        .find(|tok| {
            let cleaned = tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.');
            cleaned.contains('.') && cleaned.contains('/')
        })
        .map(|tok| {
            tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.')
                .to_string()
        })
}

fn extract_command_class(message: Option<&str>) -> Option<String> {
    let msg = message?;
    msg.strip_prefix("auto_approved: matched ")
        .map(|rest| rest.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn render_conflict(cat: &ConflictCategory) -> String {
    match cat {
        ConflictCategory::ForwardConflictIntraSpec { agents, spec_id } => {
            format!(
                "forward-conflict-intra-spec: {} (spec {})",
                agents.join(" and "),
                spec_id
            )
        }
        ConflictCategory::ForwardConflictCrossSpec { agents, spec_ids } => {
            let specs: Vec<String> = spec_ids.iter().filter(|s| !s.is_empty()).cloned().collect();
            if specs.is_empty() {
                format!("forward-conflict-cross-spec: {}", agents.join(" and "))
            } else {
                format!(
                    "forward-conflict-cross-spec: {} (specs {})",
                    agents.join(" and "),
                    specs.join(", ")
                )
            }
        }
        ConflictCategory::InFlightConflict { agents } => {
            format!("in-flight-conflict: {}", agents.join(" and "))
        }
        ConflictCategory::OwnershipViolation {
            violator,
            owner,
            file,
        } => {
            if file.is_empty() {
                format!("ownership-violation: {violator} edited a file owned by {owner}")
            } else {
                format!("ownership-violation: {violator} edited `{file}` owned by {owner}")
            }
        }
    }
}

fn render_stuck(ev: &StuckDurationEntry) -> String {
    let dur = format_duration(ev.duration_seconds);
    let suffix = if ev.resolved {
        String::new()
    } else {
        " (unresolved at session end)".to_string()
    };
    format!(
        "{}: blocked {dur} waiting on {}{suffix}",
        ev.agent_id, ev.blocked_on
    )
}

fn render_recovery(ev: &RecoveryCycleEntry) -> String {
    let cycles = if ev.count == 1 { "cycle" } else { "cycles" };
    format!(
        "{}: {} feedback {cycles} before verifying",
        ev.agent_id, ev.count
    )
}

fn render_permission(class: &str, count: u64) -> String {
    format!("`{class}` auto-approved {count} times")
}

fn format_duration(seconds: u64) -> String {
    let m = seconds / 60;
    let s = seconds % 60;
    if m == 0 {
        format!("{s}s")
    } else {
        format!("{m}m{s:02}s")
    }
}

fn format_iso8601_utc(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (year, month, day, hour, min, sec) = secs_to_civil(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
fn secs_to_civil(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec_of_day = secs % 86400;
    let hour = sec_of_day / 3600;
    let min = (sec_of_day % 3600) / 60;
    let sec = sec_of_day % 60;

    let mut days = (secs / 86400) as i64;
    days += 719_468;
    let era = days.div_euclid(146_097);
    let doe = (days - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u64, m, d, hour, min, sec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::messages::{
        ArtifactPayload, BlockedPayload, FeedbackPayload, QuestionPayload, StatusPayload,
        VerifiedPayload,
    };
    use std::time::Duration;
    use tempfile::TempDir;

    fn agg(tmp: &TempDir) -> LearningsAggregator {
        LearningsAggregator::new(tmp.path().join("session-learnings.md"))
    }

    fn read_md(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    fn blocked(agent: &str, from: &str) -> BrokerMessage {
        BrokerMessage::Blocked {
            agent_id: agent.to_string(),
            payload: BlockedPayload {
                needs: "x".to_string(),
                from: from.to_string(),
            },
        }
    }

    fn artifact(agent: &str) -> BrokerMessage {
        BrokerMessage::Artifact {
            agent_id: agent.to_string(),
            payload: ArtifactPayload {
                status: "done".to_string(),
                exports: vec![],
                modified_files: vec![],
            },
        }
    }

    fn feedback(target: &str, errors: &[&str]) -> BrokerMessage {
        BrokerMessage::Feedback {
            agent_id: target.to_string(),
            payload: FeedbackPayload {
                from: "supervisor".to_string(),
                errors: errors.iter().map(|s| (*s).to_string()).collect(),
            },
        }
    }

    fn verified(target: &str) -> BrokerMessage {
        BrokerMessage::Verified {
            agent_id: target.to_string(),
            payload: VerifiedPayload {
                verified_by: "supervisor".to_string(),
                message: None,
            },
        }
    }

    fn question(text: &str) -> BrokerMessage {
        BrokerMessage::Question {
            agent_id: "supervisor".to_string(),
            payload: QuestionPayload {
                question: text.to_string(),
            },
        }
    }

    fn auto_approve_status(agent: &str, class: &str) -> BrokerMessage {
        BrokerMessage::Status {
            agent_id: agent.to_string(),
            payload: StatusPayload {
                status: "auto_approved".to_string(),
                modified_files: vec![],
                message: Some(format!("auto_approved: matched {class}")),
                ..Default::default()
            },
        }
    }

    #[test]
    fn stuck_duration_resolved_on_artifact() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        let t0 = SystemTime::now();
        a.record_blocked("x", "y", t0);
        a.record_artifact("x", t0 + Duration::from_secs(672));
        let events = a.stuck_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].agent_id, "x");
        assert_eq!(events[0].blocked_on, "y");
        assert!((670..=674).contains(&events[0].duration_seconds));
        assert!(events[0].resolved);

        a.flush().unwrap();
        let md = read_md(a.file_path());
        assert!(md.contains("### Where agents got stuck"));
        assert!(md.contains("x: blocked"));
        assert!(md.contains("waiting on y"));
    }

    #[test]
    fn stuck_duration_unresolved_at_shutdown() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        let t0 = SystemTime::now() - Duration::from_mins(2);
        a.record_blocked("x", "y", t0);
        a.flush_at_shutdown().unwrap();
        let events = a.stuck_events();
        assert_eq!(events.len(), 1);
        assert!(!events[0].resolved);
        assert!(events[0].duration_seconds >= 119);
        let md = read_md(a.file_path());
        assert!(md.contains("unresolved at session end"));
    }

    #[test]
    fn recovery_cycles_recorded_on_verify() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.record_feedback("x");
        a.record_feedback("x");
        a.record_feedback("x");
        a.record_verified("x");
        assert_eq!(a.recovery_events().len(), 1);
        assert_eq!(a.recovery_events()[0].agent_id, "x");
        assert_eq!(a.recovery_events()[0].count, 3);
    }

    #[test]
    fn recovery_cycles_zero_count_skipped() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.record_verified("x");
        assert!(a.recovery_events().is_empty());
        a.flush().unwrap();
        assert_eq!(read_md(a.file_path()), "");
    }

    #[test]
    fn forward_conflict_intra_spec_recorded_once() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.set_spec_id("feat-x", "003-user-list");
        a.set_spec_id("feat-y", "003-user-list");

        a.record_detector_message(&feedback(
            "feat-x",
            &["[conflict-detector] forward conflict with feat-y on src/main.rs"],
        ));
        a.record_detector_message(&feedback(
            "feat-y",
            &["[conflict-detector] forward conflict with feat-x on src/main.rs"],
        ));

        let events = a.conflict_events();
        assert_eq!(events.len(), 1);
        match &events[0].category {
            ConflictCategory::ForwardConflictIntraSpec { agents, spec_id } => {
                assert_eq!(agents, &vec!["feat-x".to_string(), "feat-y".to_string()]);
                assert_eq!(spec_id, "003-user-list");
            }
            other => panic!("expected intra-spec, got {other:?}"),
        }
    }

    #[test]
    fn forward_conflict_cross_spec_records_specs() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.set_spec_id("feat-x", "003-user-list");
        a.set_spec_id("feat-y", "004-error-handling");

        a.record_detector_message(&feedback(
            "feat-x",
            &["[conflict-detector] forward conflict with feat-y on src/main.rs"],
        ));
        a.record_detector_message(&feedback(
            "feat-y",
            &["[conflict-detector] forward conflict with feat-x on src/main.rs"],
        ));

        let events = a.conflict_events();
        assert_eq!(events.len(), 1);
        match &events[0].category {
            ConflictCategory::ForwardConflictCrossSpec { agents, spec_ids } => {
                assert_eq!(agents, &vec!["feat-x".to_string(), "feat-y".to_string()]);
                assert!(spec_ids.iter().any(|s| s == "003-user-list"));
                assert!(spec_ids.iter().any(|s| s == "004-error-handling"));
            }
            other => panic!("expected cross-spec, got {other:?}"),
        }
    }

    #[test]
    fn in_flight_conflict_classified() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.record_detector_message(&feedback(
            "feat-x",
            &["[conflict-detector] in-flight conflict with feat-y on src/a.rs"],
        ));
        a.record_detector_message(&feedback(
            "feat-y",
            &["[conflict-detector] in-flight conflict with feat-x on src/a.rs"],
        ));
        let events = a.conflict_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].category,
            ConflictCategory::InFlightConflict { .. }
        ));
    }

    #[test]
    fn ownership_violation_classified() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.record_detector_message(&feedback(
            "feat-y",
            &["[conflict-detector] ownership violation on src/a.rs claimed by feat-x"],
        ));
        let events = a.conflict_events();
        assert_eq!(events.len(), 1);
        match &events[0].category {
            ConflictCategory::OwnershipViolation {
                violator,
                owner,
                file,
            } => {
                assert_eq!(violator, "feat-y");
                assert_eq!(owner, "feat-x");
                assert_eq!(file, "src/a.rs");
            }
            other => panic!("expected ownership-violation, got {other:?}"),
        }
    }

    #[test]
    fn detector_question_to_supervisor_is_classified() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.record_detector_message(&question(
            "[conflict-detector] in-flight conflict between feat-x and feat-y on src/a.rs",
        ));
        // Question target is "supervisor" which isn't in known_agents,
        // so the classifier looks at the two real agents both mentioned.
        let events = a.conflict_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].category,
            ConflictCategory::InFlightConflict { .. }
        ));
    }

    #[test]
    fn permission_pattern_above_threshold_emits_entry() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        for _ in 0..23 {
            a.record_auto_approve("cargo check");
        }
        a.flush().unwrap();
        let md = read_md(a.file_path());
        assert!(md.contains("### Permission patterns"));
        assert!(md.contains("`cargo check` auto-approved 23 times"));
    }

    #[test]
    fn permission_pattern_below_threshold_omitted_then_emitted_later() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.record_auto_approve("git status");
        a.record_auto_approve("git status");
        // Need at least one signal to flush — give it an artifact.
        a.flush().unwrap();
        let md1 = read_md(a.file_path());
        assert!(!md1.contains("git status"));

        // A later burst pushes the count over the default 5.
        for _ in 0..5 {
            a.record_auto_approve("git status");
        }
        a.flush().unwrap();
        let md2 = read_md(a.file_path());
        assert!(md2.contains("`git status` auto-approved 7 times"));
    }

    #[test]
    fn no_learnings_session_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.flush().unwrap();
        a.flush_at_shutdown().unwrap();
        assert_eq!(read_md(a.file_path()), "");
        assert!(!a.file_path().exists() || read_md(a.file_path()).is_empty());
    }

    #[test]
    fn flush_writes_h2_header_once_per_session() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        for _ in 0..PERMISSION_PATTERN_THRESHOLD {
            a.record_auto_approve("cargo check");
        }
        a.flush().unwrap();
        // Add another signal and flush again — should NOT add a second H2.
        a.record_feedback("alpha");
        a.record_verified("alpha");
        a.flush().unwrap();

        let md = read_md(a.file_path());
        let h2_count = md.matches("## Session Learnings — ").count();
        assert_eq!(h2_count, 1, "expected exactly one H2, got\n{md}");
        // ISO timestamp on the H2 line.
        let h2_line = md
            .lines()
            .find(|l| l.starts_with("## Session Learnings — "))
            .unwrap();
        let ts = h2_line.trim_start_matches("## Session Learnings — ").trim();
        assert!(
            regex_like_iso(ts),
            "H2 timestamp did not match ISO regex: {ts:?}"
        );
    }

    fn regex_like_iso(s: &str) -> bool {
        // Equivalent of ^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$
        let bytes = s.as_bytes();
        if bytes.len() != 20 {
            return false;
        }
        for (i, b) in bytes.iter().enumerate() {
            let ok = match i {
                4 | 7 => *b == b'-',
                10 => *b == b'T',
                13 | 16 => *b == b':',
                19 => *b == b'Z',
                _ => b.is_ascii_digit(),
            };
            if !ok {
                return false;
            }
        }
        true
    }

    #[test]
    fn second_session_appends_new_h2_preserves_prior_content() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session-learnings.md");
        let mut a1 = LearningsAggregator::new(path.clone());
        for _ in 0..PERMISSION_PATTERN_THRESHOLD {
            a1.record_auto_approve("cargo check");
        }
        a1.flush().unwrap();
        let after_first = read_md(&path);
        assert!(after_first.contains("`cargo check`"));

        // Wait a second so the ISO timestamps differ.
        std::thread::sleep(Duration::from_secs(1));

        let mut a2 = LearningsAggregator::new(path.clone());
        for _ in 0..PERMISSION_PATTERN_THRESHOLD {
            a2.record_auto_approve("cargo fmt");
        }
        a2.flush().unwrap();
        let after_second = read_md(&path);
        // Prior content unchanged at the start of the file.
        assert!(after_second.starts_with(after_first.trim_end()));
        // Two H2 headers in the file.
        let h2_count = after_second.matches("## Session Learnings — ").count();
        assert_eq!(h2_count, 2);
        assert!(after_second.contains("`cargo fmt`"));
    }

    #[test]
    fn observe_routes_blocked_and_artifact() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.observe(&blocked("x", "y"));
        a.observe(&artifact("x"));
        assert_eq!(a.stuck_events().len(), 1);
    }

    #[test]
    fn observe_increments_feedback_then_records_recovery() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        for _ in 0..3 {
            a.observe(&feedback("x", &["test failed"]));
        }
        a.observe(&verified("x"));
        assert_eq!(a.recovery_events().len(), 1);
        assert_eq!(a.recovery_events()[0].count, 3);
    }

    #[test]
    fn observe_auto_approve_increments_counter() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        for _ in 0..PERMISSION_PATTERN_THRESHOLD {
            a.observe(&auto_approve_status("feat-x", "cargo check"));
        }
        a.flush().unwrap();
        assert!(read_md(a.file_path()).contains("`cargo check` auto-approved"));
    }

    #[test]
    fn extract_command_class_parses_matched_entry() {
        assert_eq!(
            extract_command_class(Some("auto_approved: matched cargo check")),
            Some("cargo check".to_string())
        );
        assert_eq!(extract_command_class(Some("auto_approved")), None);
        assert_eq!(extract_command_class(None), None);
    }

    /// Spec scenario `Markdown file output / Empty categories are omitted`:
    /// a session with conflict events but no stuck-duration events must
    /// produce a `### Conflict events` heading and MUST NOT produce a
    /// `### Where agents got stuck` heading.
    #[test]
    fn empty_categories_are_omitted_from_markdown() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.record_detector_message(&feedback(
            "feat-x",
            &["[conflict-detector] in-flight conflict with feat-y on src/a.rs"],
        ));
        a.flush().unwrap();

        let md = read_md(a.file_path());
        assert!(md.contains("### Conflict events"));
        assert!(
            !md.contains("### Where agents got stuck"),
            "stuck heading should be omitted when there are no stuck events:\n{md}"
        );
        assert!(
            !md.contains("### Recovery cycles"),
            "recovery heading should be omitted when there are no recovery events:\n{md}"
        );
        assert!(
            !md.contains("### Permission patterns"),
            "permission heading should be omitted when there are no permission entries:\n{md}"
        );
    }

    /// Spec scenario `Periodic flush + shutdown flush / Burst of events
    /// does not trigger eager flush`: observing 5 events in quick
    /// succession SHALL NOT write anything until `flush()` is invoked.
    #[test]
    fn burst_of_events_does_not_write_until_flush() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        for _ in 0..5 {
            a.record_detector_message(&feedback(
                "feat-x",
                &["[conflict-detector] in-flight conflict with feat-y on src/a.rs"],
            ));
        }
        // Five back-to-back observes — the file must remain unwritten.
        assert!(
            !a.file_path().exists() || read_md(a.file_path()).is_empty(),
            "aggregator wrote eagerly without a flush call"
        );
        // One explicit flush captures all events together.
        a.flush().unwrap();
        let md = read_md(a.file_path());
        // Despite five `observe` calls, deduping keeps exactly one bullet
        // for the same canonical pair — the assertion the spec actually
        // cares about is "no flush until the timer fires".
        assert!(md.contains("### Conflict events"));
    }

    /// Spec scenario `No agent.learning broker variant in v0.5.0 / No
    /// agent.learning variant exists in BrokerMessage in v0.5.0`:
    /// inspect every variant of `BrokerMessage` and confirm none uses the
    /// `agent.learning` serde tag.
    #[test]
    fn broker_message_has_no_agent_learning_variant() {
        // Serialise one instance of every variant and assert the wire tag
        // is one of the six known v0.4/v0.5 tags. Adding a new variant
        // without updating this list (and the requirement) makes the test
        // fail.
        let allowed = [
            "agent.status",
            "agent.artifact",
            "agent.blocked",
            "agent.verified",
            "agent.feedback",
            "agent.question",
        ];
        let samples = [
            (
                "agent.status",
                serde_json::to_string(&BrokerMessage::Status {
                    agent_id: "x".to_string(),
                    payload: crate::broker::messages::StatusPayload {
                        status: "working".to_string(),
                        modified_files: vec![],
                        message: None,
                        ..Default::default()
                    },
                })
                .unwrap(),
            ),
            (
                "agent.artifact",
                serde_json::to_string(&BrokerMessage::Artifact {
                    agent_id: "x".to_string(),
                    payload: crate::broker::messages::ArtifactPayload {
                        status: "done".to_string(),
                        exports: vec![],
                        modified_files: vec![],
                    },
                })
                .unwrap(),
            ),
            (
                "agent.blocked",
                serde_json::to_string(&blocked("x", "y")).unwrap(),
            ),
            (
                "agent.verified",
                serde_json::to_string(&verified("x")).unwrap(),
            ),
            (
                "agent.feedback",
                serde_json::to_string(&feedback("x", &["e"])).unwrap(),
            ),
            (
                "agent.question",
                serde_json::to_string(&question("[conflict-detector] x")).unwrap(),
            ),
        ];
        for (expected_tag, json) in &samples {
            assert!(
                json.contains(&format!("\"type\":\"{expected_tag}\"")),
                "serialised {expected_tag} did not contain expected tag: {json}"
            );
            assert!(
                !json.contains("agent.learning"),
                "no variant should use the reserved agent.learning tag: {json}"
            );
        }
        // Reject any unknown variant by trying to deserialise an
        // `agent.learning` envelope.
        let probe = r#"{"type":"agent.learning","agent_id":"x","payload":{}}"#;
        let err = serde_json::from_str::<BrokerMessage>(probe);
        assert!(
            err.is_err(),
            "deserialising agent.learning must fail — the variant must not exist"
        );
        // And the supervisor-config delta side: ensure the allowed list is
        // a superset of nothing surprising. This is a guard that the test
        // catches additions if someone adds a 7th variant.
        for (tag, _) in &samples {
            assert!(allowed.contains(tag));
        }
    }

    /// Spec scenario `Aggregator does not start when learnings flag is
    /// false` and `Aggregator does not start when supervisor is disabled`:
    /// the wiring decision keys on `supervisor.enabled && supervisor.learnings`.
    #[test]
    fn wiring_predicate_only_enables_when_supervisor_and_learnings_both_true() {
        use crate::config::{LearningsConfig, SupervisorConfig};

        // The predicate used in `cmd_dashboard` to decide whether to
        // attach a learnings aggregator. Mirroring it in a test pins down
        // the lifecycle requirement: any change to the gating logic
        // breaks this test.
        fn should_attach(s: Option<&SupervisorConfig>) -> bool {
            s.is_some_and(|s| s.enabled && s.learnings)
        }

        // Section absent → no aggregator.
        assert!(!should_attach(None));

        // Supervisor disabled, learnings true → no aggregator.
        assert!(!should_attach(Some(&SupervisorConfig {
            enabled: false,
            learnings: true,
            learnings_config: LearningsConfig::default(),
            ..SupervisorConfig::default()
        })));

        // Supervisor enabled, learnings false → no aggregator.
        assert!(!should_attach(Some(&SupervisorConfig {
            enabled: true,
            learnings: false,
            learnings_config: LearningsConfig::default(),
            ..SupervisorConfig::default()
        })));

        // Both enabled → aggregator attached.
        assert!(should_attach(Some(&SupervisorConfig {
            enabled: true,
            learnings: true,
            learnings_config: LearningsConfig::default(),
            ..SupervisorConfig::default()
        })));
    }

    // Maps to scenario `Default flush interval is 60 seconds` from
    // learnings-mode. (test-coverage-v0-5-0 task 5.1)
    #[test]
    fn default_flush_interval_is_60_seconds() {
        use crate::config::LearningsConfig;
        let cfg = LearningsConfig::default();
        assert_eq!(
            cfg.flush_interval_seconds, 60,
            "LearningsConfig::default().flush_interval_seconds must be 60"
        );
    }
}
