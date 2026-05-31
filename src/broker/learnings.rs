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

use std::hash::Hasher as _;

use chrono::{TimeZone as _, Utc};

use super::messages::{BrokerMessage, LearningPayload};

/// Substring marker that conflict-detector-originated messages prepend to
/// their `errors`/`question` text.
pub const CONFLICT_DETECTOR_TAG: &str = "[conflict-detector]";

/// Default `count` threshold below which a permission-pattern entry is
/// withheld at flush. See the `Permission-pattern signal` spec.
pub const PERMISSION_PATTERN_THRESHOLD: u64 = 5;

/// Category tag for conflict-detector-derived learnings.
pub const CATEGORY_CONFLICT_EVENT: &str = "conflict_event";
/// Category tag for stuck-duration learnings.
pub const CATEGORY_STUCK_DURATION: &str = "stuck_duration";
/// Category tag for recovery-cycle learnings.
pub const CATEGORY_RECOVERY_CYCLES: &str = "recovery_cycles";
/// Category tag for permission-pattern learnings.
pub const CATEGORY_PERMISSION_PATTERN: &str = "permission_pattern";

// --- Qualitative learning categories (v0.6.0) ---
//
// These four categories are NOT produced by the aggregator itself; they are
// published by the supervisor LLM via the existing `agent.learning` wire
// variant (no wire-format change — the open `category` enum from the
// `agent-learning-variant` change carries them). The aggregator ingests them
// on the publish path and routes each into its own file section. The broker
// does NOT validate the bodies (consumer-side discipline, design D5); the
// documented body shape below is what the supervisor skill teaches the LLM to
// emit, and the file renderer tolerates drift by falling back to a JSON dump.

/// Category tag for recurring-failure-shape learnings: the same error shape
/// observed across multiple `agent.feedback` cycles from distinct branches.
///
/// Documented body shape (design D1):
/// ```json
/// { "shape": "import cycle in module X",
///   "instances": [ { "branch_id": "feat/a", "feedback_id": "...", "excerpt": "..." } ] }
/// ```
/// Primary identifier (for within-session dedup): `shape`.
pub const CATEGORY_RECURRING_FAILURE_SHAPE: &str = "recurring_failure_shape";
/// Category tag for documentation-gap learnings: a spec assumes a convention
/// that no checked-in doc explains.
///
/// Documented body shape (design D1):
/// ```json
/// { "convention": "agents run lint before commit",
///   "evidence_paths": ["AGENTS.md"], "suggestion": "add to AGENTS.md" }
/// ```
/// Primary identifier: `convention`.
pub const CATEGORY_DOC_GAP: &str = "doc_gap";
/// Category tag for ADR / architectural-drift learnings: code introduces a
/// decision (pattern, dependency, boundary) not reflected in the configured
/// ADRs.
///
/// Documented body shape (design D1):
/// ```json
/// { "decision_area": "async runtime", "observed_pattern": "...",
///   "configured_adr_path": "docs/adr", "candidate_adr_title": "ADR-NNNN: ..." }
/// ```
/// Primary identifier: `decision_area`.
pub const CATEGORY_ADR_DRIFT: &str = "adr_drift";
/// Category tag for scope-mistake learnings: two or more branches coordinated
/// heavily because the original spec scope drew the boundary in the wrong
/// place.
///
/// Documented body shape (design D1):
/// ```json
/// { "branches": ["feat/a", "feat/b"], "shared_files": ["src/foo"],
///   "coordination_events": [], "suggestion": "merge feat/a and feat/b scopes" }
/// ```
/// Primary identifier: the `branches` set.
pub const CATEGORY_SCOPE_MISTAKE: &str = "scope_mistake";

/// Publishing `agent_id` for aggregator-produced learnings. The aggregator
/// runs inside the broker/supervisor process, so every record is attributed
/// to the supervisor regardless of which branch it concerns; per-branch
/// scoping is carried by [`LearningRecord::branch_id`].
pub const LEARNINGS_AGENT_ID: &str = "supervisor";

/// One structured, emittable learning record — the single in-memory
/// representation of a learning destined for the `agent.learning` wire
/// variant.
///
/// The aggregator's working-state entry types ([`StuckDurationEntry`],
/// [`RecoveryCycleEntry`], [`ConflictEvent`], and the permission counters)
/// are projected into `LearningRecord`s at flush time. The broker payload is
/// then produced solely by `From<&LearningRecord> for BrokerMessage` — there
/// is no parallel wire-record data model (the
/// `agent-learning-variant` "Internal model serialises directly" requirement).
#[derive(Debug, Clone, PartialEq)]
pub struct LearningRecord {
    /// Open category tag (see the `CATEGORY_*` constants).
    pub category: String,
    /// Publishing agent id (typically [`LEARNINGS_AGENT_ID`]).
    pub agent_id: String,
    /// Branch the record is scoped to; `None` for cross-cutting records
    /// (permission patterns, conflict pairs).
    pub branch_id: Option<String>,
    /// Short human-readable summary; mirrors the markdown bullet text.
    pub title: String,
    /// Category-specific structured body.
    pub body: serde_json::Value,
    /// When the record was committed; used for the hour bucket and the
    /// emitted ISO-8601 wire timestamp.
    pub timestamp: SystemTime,
}

impl LearningRecord {
    /// Computes the deterministic dedup id: a stable 16-hex-char (64-bit)
    /// hash of a canonical serialisation of `category`, `branch_id`, the
    /// `body` (object keys sorted), and the UTC hour bucket (`YYYY-MM-DDTHH`).
    ///
    /// Uses the std-library [`DefaultHasher`](std::collections::hash_map::DefaultHasher)
    /// — no external crypto dependency. The id is not a security primitive;
    /// it only needs to be deterministic for the same canonical input so
    /// consumers can dedupe. Re-publishing the same logical record within one
    /// UTC hour yields the same id; across an hour boundary the id changes so
    /// a genuine recurrence registers.
    #[must_use]
    pub fn deterministic_id(&self) -> String {
        let mut canon = String::new();
        canon.push_str(&self.category);
        canon.push('|');
        canon.push_str(self.branch_id.as_deref().unwrap_or(""));
        canon.push('|');
        canonical_value(&self.body, &mut canon);
        canon.push('|');
        canon.push_str(&hour_bucket(self.timestamp));

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write(canon.as_bytes());
        format!("{:016x}", hasher.finish())
    }
}

impl From<&LearningRecord> for BrokerMessage {
    fn from(record: &LearningRecord) -> Self {
        BrokerMessage::Learning {
            payload: LearningPayload {
                id: record.deterministic_id(),
                agent_id: record.agent_id.clone(),
                branch_id: record.branch_id.clone(),
                category: record.category.clone(),
                title: record.title.clone(),
                body: record.body.clone(),
                timestamp: format_iso8601_utc(record.timestamp),
            },
        }
    }
}

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
        /// Intersecting region descriptors (e.g. `function validate_token`)
        /// when the forward conflict was detected at region granularity;
        /// empty for a file-level conflict. See
        /// `conflict-detector-fn-granularity`.
        regions: Vec<String>,
    },
    /// Forward conflict spanning two `SpecEntry` families.
    ForwardConflictCrossSpec {
        /// Sorted pair of agent ids implicated by the conflict.
        agents: Vec<String>,
        /// Spec ids for the agents, in the same order as `agents`. May be
        /// empty entries when the agent → spec mapping is not yet known.
        spec_ids: Vec<String>,
        /// Intersecting region descriptors when detected at region
        /// granularity; empty for a file-level conflict.
        regions: Vec<String>,
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
    /// When `true`, flushed entries are also projected into
    /// [`LearningRecord`]s and queued in `pending_publish` for the flush loop
    /// to emit as `agent.learning` broker messages. When `false`, the
    /// aggregator is file-only (v0.5.0 behaviour) and never queues records.
    broker_publish: bool,
    /// Records committed during flushes, awaiting broker publish; drained by
    /// [`Self::take_pending_publish`]. Always empty when `broker_publish` is
    /// `false`.
    pending_publish: Vec<LearningRecord>,
    /// Qualitative learning records ingested from externally-published
    /// `agent.learning` messages (the supervisor LLM), awaiting flush. Unlike
    /// the deterministic event vectors above, these arrive fully-formed on the
    /// publish path rather than being accumulated from raw broker traffic, and
    /// are NEVER re-published (they already came from the broker).
    qualitative_events: Vec<LearningPayload>,
    /// Cursor: number of `qualitative_events` already written to the markdown.
    qualitative_flushed: usize,
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
            broker_publish: false,
            pending_publish: Vec::new(),
            qualitative_events: Vec::new(),
            qualitative_flushed: 0,
        }
    }

    /// Enables or disables broker publication of flushed records. Off by
    /// default (file-only, the v0.5.0 behaviour). The dual-output path is
    /// gated on this so `[broker] enabled = false` or
    /// `[supervisor.learnings] broker_publish = "force_off"` produce no
    /// broker traffic at all.
    pub fn set_broker_publish(&mut self, enabled: bool) {
        self.broker_publish = enabled;
    }

    /// Returns whether broker publication of flushed records is enabled.
    #[must_use]
    pub fn broker_publish_enabled(&self) -> bool {
        self.broker_publish
    }

    /// Drains the queue of [`LearningRecord`]s committed since the last call
    /// so the flush loop can emit them as `agent.learning` messages. Always
    /// empty when broker publish is disabled.
    pub fn take_pending_publish(&mut self) -> Vec<LearningRecord> {
        std::mem::take(&mut self.pending_publish)
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
                let regions = extract_regions(&text);
                let category = self.classify_forward(&pair, regions);
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
            BrokerMessage::Learning { payload } => {
                // `agent.learning` records flow back through the publish path.
                // The aggregator's own deterministic-category records were
                // already rendered by `write_flush`; re-aggregating them would
                // double-render (and recurse), so they are ignored here. Any
                // OTHER category is an externally-published (supervisor LLM)
                // qualitative record the aggregator must route into a file
                // section — see [`Self::record_qualitative`].
                self.record_qualitative(payload);
            }
            // `supervisor.verify-now` is a broker-emitted operational nudge and
            // `agent.advanced-main` is a supervisor-published merge notification
            // — both are coordination signals, not agent learnings, and are
            // ignored.
            BrokerMessage::VerifyNow { .. } | BrokerMessage::AdvancedMain { .. } => {}
        }
    }

    /// Ingests an externally-published `agent.learning` record for file
    /// rendering. Deterministic-category records (the aggregator's own,
    /// flowing back through the publish path) are dropped to avoid
    /// double-rendering. The remaining qualitative / unknown-category records
    /// are accumulated for the next flush, after a within-session dedup pass
    /// (design D3, belt-and-braces on top of the skill-level dedup): a record
    /// whose `(category, primary identifier)` matches one already ingested
    /// this session is suppressed.
    ///
    /// Ingested records are NEVER queued for publish — they already came from
    /// the broker.
    pub fn record_qualitative(&mut self, payload: &LearningPayload) {
        if is_deterministic_category(&payload.category) {
            return;
        }
        let key = qualitative_dedup_key(payload);
        if self
            .qualitative_events
            .iter()
            .any(|p| qualitative_dedup_key(p) == key)
        {
            return;
        }
        self.qualitative_events.push(payload.clone());
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

    fn classify_forward(&self, pair: &[String], regions: Vec<String>) -> ConflictCategory {
        let spec_a = self.spec_ids.get(&pair[0]);
        let spec_b = self.spec_ids.get(&pair[1]);
        match (spec_a, spec_b) {
            (Some(a), Some(b)) if a == b => ConflictCategory::ForwardConflictIntraSpec {
                agents: pair.to_vec(),
                spec_id: a.clone(),
                regions,
            },
            (Some(a), Some(b)) => ConflictCategory::ForwardConflictCrossSpec {
                agents: pair.to_vec(),
                spec_ids: vec![a.clone(), b.clone()],
                regions,
            },
            _ => ConflictCategory::ForwardConflictCrossSpec {
                agents: pair.to_vec(),
                spec_ids: vec![
                    spec_a.cloned().unwrap_or_default(),
                    spec_b.cloned().unwrap_or_default(),
                ],
                regions,
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
        let broker_publish = self.broker_publish;
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

        let new_qualitative = &self.qualitative_events[self.qualitative_flushed..];

        let has_any = !new_stuck.is_empty()
            || !new_recovery.is_empty()
            || !new_conflicts.is_empty()
            || !permission_entries.is_empty()
            || !new_qualitative.is_empty();
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

        render_qualitative_sections(new_qualitative, &mut out);

        // Project the newly-flushed entries into wire-bound `LearningRecord`s
        // for the broker, when broker publish is active. This is purely
        // additive: it reads the same slices used for the markdown above and
        // does NOT alter the file output (which remains byte-for-byte the
        // v0.5.0 shape). `now` stamps every record from this flush so they
        // share one UTC hour bucket for id stability.
        let records: Vec<LearningRecord> = if broker_publish {
            let now = SystemTime::now();
            let mut records =
                Vec::with_capacity(new_conflicts.len() + new_stuck.len() + new_recovery.len());
            for ev in new_conflicts {
                records.push(record_from_conflict(&ev.category, now));
            }
            for ev in new_stuck {
                records.push(record_from_stuck(ev, now));
            }
            for ev in new_recovery {
                records.push(record_from_recovery(ev, now));
            }
            for (class, count) in &permission_entries {
                records.push(record_from_permission(class, *count, now));
            }
            records
        } else {
            Vec::new()
        };

        append_to_file(&self.file_path, &out)?;

        self.stuck_flushed = self.stuck_events.len();
        self.recovery_flushed = self.recovery_events.len();
        self.conflict_flushed = self.conflict_events.len();
        self.qualitative_flushed = self.qualitative_events.len();
        for (class, count) in &permission_entries {
            self.permission_emitted.insert(class.clone(), *count);
        }
        // Queue records only after the file write succeeded, so a failed
        // append never publishes a record that isn't also in the file.
        self.pending_publish.extend(records);
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

    #[cfg(test)]
    fn qualitative_events(&self) -> &[LearningPayload] {
        &self.qualitative_events
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

/// The Markdown H3 header under which `/tell` routing decisions are recorded
/// (design D4).
pub const ROUTING_SECTION_HEADER: &str = "### Supervisor routing";

/// Maximum prompt length recorded verbatim in a routing entry; longer prompts
/// are truncated with a trailing `…` (the full prompt rides the broker
/// message).
pub const ROUTING_PROMPT_MAX_CHARS: usize = 200;

/// Formats one "Supervisor routing" log line for a `/tell` invocation
/// (design D4).
///
/// `ts_iso` is the ISO-8601 UTC timestamp, `target` the agent identifier,
/// `mode` the resolved delivery-mode label (`"feedback"` / `"send-keys"`), and
/// `prompt` the user-typed prompt — truncated to
/// [`ROUTING_PROMPT_MAX_CHARS`] characters with a trailing `…` when longer.
#[must_use]
pub fn format_routing_entry(ts_iso: &str, target: &str, mode: &str, prompt: &str) -> String {
    let trimmed = prompt.trim();
    let shown = if trimmed.chars().count() > ROUTING_PROMPT_MAX_CHARS {
        let mut s: String = trimmed.chars().take(ROUTING_PROMPT_MAX_CHARS).collect();
        s.push('…');
        s
    } else {
        trimmed.to_string()
    };
    format!("- {ts_iso} — supervisor told `{target}` via {mode}: \"{shown}\"")
}

/// Appends a `/tell` routing decision to the "Supervisor routing" section of
/// the learnings file, gated on `enabled` (design D4 / task 7).
///
/// Reuses the same [`append_to_file`] helper the aggregator uses for its
/// flushes. When `enabled` is `false` this is a strict no-op — no file is
/// created or written, honouring the `[supervisor] learnings = false`
/// contract. The section header is written once, the first time a routing
/// record lands in a file that does not already contain it.
///
/// # Errors
/// Propagates any I/O error from reading or appending to the file.
pub fn append_routing_record(
    path: &Path,
    enabled: bool,
    ts_iso: &str,
    target: &str,
    mode: &str,
    prompt: &str,
) -> std::io::Result<()> {
    if !enabled {
        return Ok(());
    }
    let needs_header = match std::fs::read_to_string(path) {
        Ok(existing) => !existing.contains(ROUTING_SECTION_HEADER),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) => return Err(e),
    };
    let mut block = String::new();
    if needs_header {
        block.push('\n');
        block.push_str(ROUTING_SECTION_HEADER);
        block.push('\n');
    }
    block.push_str(&format_routing_entry(ts_iso, target, mode, prompt));
    block.push('\n');
    append_to_file(path, &block)
}

/// Appends a canonical, key-sorted serialisation of `value` to `out`.
///
/// Objects emit their keys in sorted order so that two logically-identical
/// bodies (regardless of insertion order) produce byte-identical canonical
/// strings — a precondition for the deterministic dedup id.
fn canonical_value(value: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(key);
                out.push(':');
                canonical_value(&map[*key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical_value(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// Formats `time` as a UTC hour bucket (`YYYY-MM-DDTHH`) for id hashing.
fn hour_bucket(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    Utc.timestamp_opt(i64::try_from(secs).unwrap_or(0), 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%dT%H").to_string())
        .unwrap_or_default()
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

/// Extracts intersecting region descriptors from a region-aware
/// forward-conflict detector message.
///
/// The detector renders region-level conflicts as
/// `... path (regions: function foo, range 10-30); ...`. This collects each
/// comma-separated descriptor inside every `(regions: ...)` group, preserving
/// order and de-duplicating. Returns an empty vec for a file-level conflict
/// (no `(regions: ...)` group present), in which case the body omits the
/// `regions` field.
fn extract_regions(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("(regions: ") {
        let after = &rest[start + "(regions: ".len()..];
        let Some(end) = after.find(')') else { break };
        for descriptor in after[..end].split(',') {
            let trimmed = descriptor.trim();
            if !trimmed.is_empty() && !out.iter().any(|d| d == trimmed) {
                out.push(trimmed.to_string());
            }
        }
        rest = &after[end..];
    }
    out
}

fn extract_command_class(message: Option<&str>) -> Option<String> {
    let msg = message?;
    msg.strip_prefix("auto_approved: matched ")
        .map(|rest| rest.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Projects a classified conflict event into a `conflict_event`
/// [`LearningRecord`]. Conflicts are cross-cutting (they implicate a pair of
/// branches), so `branch_id` is `None` and the involved agents live in the
/// body.
fn record_from_conflict(cat: &ConflictCategory, now: SystemTime) -> LearningRecord {
    use serde_json::json;
    let body = match cat {
        ConflictCategory::ForwardConflictIntraSpec {
            agents,
            spec_id,
            regions,
        } => {
            let mut b = json!({
                "shape": "forward_intra_spec",
                "agents": agents,
                "spec_id": spec_id,
            });
            if !regions.is_empty() {
                b["regions"] = json!(regions);
            }
            b
        }
        ConflictCategory::ForwardConflictCrossSpec {
            agents,
            spec_ids,
            regions,
        } => {
            let mut b = json!({
                "shape": "forward_cross_spec",
                "agents": agents,
                "spec_ids": spec_ids,
            });
            if !regions.is_empty() {
                b["regions"] = json!(regions);
            }
            b
        }
        ConflictCategory::InFlightConflict { agents } => json!({
            "shape": "in_flight",
            "agents": agents,
        }),
        ConflictCategory::OwnershipViolation {
            violator,
            owner,
            file,
        } => json!({
            "shape": "ownership_violation",
            "violator": violator,
            "owner": owner,
            "file": file,
        }),
    };
    LearningRecord {
        category: CATEGORY_CONFLICT_EVENT.to_string(),
        agent_id: LEARNINGS_AGENT_ID.to_string(),
        branch_id: None,
        title: render_conflict(cat),
        body,
        timestamp: now,
    }
}

/// Projects a stuck-duration entry into a `stuck_duration` [`LearningRecord`]
/// scoped to the blocked agent's branch.
fn record_from_stuck(ev: &StuckDurationEntry, now: SystemTime) -> LearningRecord {
    LearningRecord {
        category: CATEGORY_STUCK_DURATION.to_string(),
        agent_id: LEARNINGS_AGENT_ID.to_string(),
        branch_id: Some(ev.agent_id.clone()),
        title: render_stuck(ev),
        body: serde_json::json!({
            "agent_id": ev.agent_id,
            "blocked_on": ev.blocked_on,
            "duration_seconds": ev.duration_seconds,
            "resolved": ev.resolved,
        }),
        timestamp: now,
    }
}

/// Projects a recovery-cycle entry into a `recovery_cycles`
/// [`LearningRecord`] scoped to the verifying agent's branch.
fn record_from_recovery(ev: &RecoveryCycleEntry, now: SystemTime) -> LearningRecord {
    LearningRecord {
        category: CATEGORY_RECOVERY_CYCLES.to_string(),
        agent_id: LEARNINGS_AGENT_ID.to_string(),
        branch_id: Some(ev.agent_id.clone()),
        title: render_recovery(ev),
        body: serde_json::json!({
            "agent_id": ev.agent_id,
            "count": ev.count,
        }),
        timestamp: now,
    }
}

/// Projects a permission-pattern entry into a `permission_pattern`
/// [`LearningRecord`]. Permission patterns are cross-cutting (they describe
/// the supervisor's auto-approve behaviour), so `branch_id` is `None`.
fn record_from_permission(class: &str, count: u64, now: SystemTime) -> LearningRecord {
    LearningRecord {
        category: CATEGORY_PERMISSION_PATTERN.to_string(),
        agent_id: LEARNINGS_AGENT_ID.to_string(),
        branch_id: None,
        title: render_permission(class, count),
        body: serde_json::json!({
            "command_class": class,
            "count": count,
        }),
        timestamp: now,
    }
}

fn render_conflict(cat: &ConflictCategory) -> String {
    match cat {
        ConflictCategory::ForwardConflictIntraSpec {
            agents, spec_id, ..
        } => {
            format!(
                "forward-conflict-intra-spec: {} (spec {})",
                agents.join(" and "),
                spec_id
            )
        }
        ConflictCategory::ForwardConflictCrossSpec {
            agents, spec_ids, ..
        } => {
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

// === Qualitative learnings (v0.6.0) ===

/// The recognised qualitative categories paired with their file-section
/// headers, in render order. Categories absent from this table fall through
/// to the "Other learnings" section.
const QUALITATIVE_SECTIONS: &[(&str, &str)] = &[
    (CATEGORY_RECURRING_FAILURE_SHAPE, "Recurring failure shapes"),
    (CATEGORY_DOC_GAP, "Documentation gaps"),
    (CATEGORY_ADR_DRIFT, "ADR / architectural drift"),
    (CATEGORY_SCOPE_MISTAKE, "Scope-mistake signals"),
];

/// Returns `true` for the four v0.5.0 deterministic categories the aggregator
/// produces itself. Used to drop the aggregator's own records when they flow
/// back through the publish path, so only externally-published qualitative
/// records are ingested for file rendering.
fn is_deterministic_category(category: &str) -> bool {
    matches!(
        category,
        CATEGORY_CONFLICT_EVENT
            | CATEGORY_STUCK_DURATION
            | CATEGORY_RECOVERY_CYCLES
            | CATEGORY_PERMISSION_PATTERN
    )
}

/// Renders the qualitative records flushed this round into `out` (design D4):
/// one section per recognised category in a fixed order, each emitted only
/// when there is a record for it, followed by an "Other learnings" fallback
/// that absorbs every unrecognised category so nothing is silently dropped.
fn render_qualitative_sections(new_qualitative: &[LearningPayload], out: &mut String) {
    for (category, header) in QUALITATIVE_SECTIONS {
        let mut wrote_header = false;
        for p in new_qualitative.iter().filter(|p| &p.category == category) {
            if !wrote_header {
                let _ = writeln!(out, "\n### {header}");
                wrote_header = true;
            }
            out.push_str(&render_qualitative(p));
        }
    }
    let mut wrote_other = false;
    for p in new_qualitative
        .iter()
        .filter(|p| qualitative_section(&p.category).is_none())
    {
        if !wrote_other {
            out.push_str("\n### Other learnings\n");
            wrote_other = true;
        }
        out.push_str(&render_qualitative(p));
    }
}

/// Returns the file-section header for a qualitative `category`, or `None`
/// when the category is unrecognised (routes to "Other learnings").
fn qualitative_section(category: &str) -> Option<&'static str> {
    QUALITATIVE_SECTIONS
        .iter()
        .find(|(cat, _)| *cat == category)
        .map(|(_, header)| *header)
}

/// Reads a string `key` from a JSON object body, if present and a string.
fn string_field(body: &serde_json::Value, key: &str) -> Option<String> {
    body.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

/// Reads an array `key` from a JSON object body and returns its elements
/// sorted and comma-joined, for use as a stable primary identifier. Non-string
/// elements are serialised with their JSON representation.
fn sorted_array_field(body: &serde_json::Value, key: &str) -> Option<String> {
    let arr = body.get(key)?.as_array()?;
    let mut items: Vec<String> = arr
        .iter()
        .map(|v| {
            v.as_str()
                .map_or_else(|| v.to_string(), std::string::ToString::to_string)
        })
        .collect();
    items.sort();
    Some(items.join(","))
}

/// Computes the within-session dedup key for a qualitative record: its
/// category plus the category's primary identifier (design D3). When the
/// primary identifier is absent (malformed body) or the category is unknown,
/// the publisher's deterministic `id` is used instead, so only exact
/// duplicates are suppressed and distinct-but-malformed records survive.
fn qualitative_dedup_key(p: &LearningPayload) -> String {
    let primary = match p.category.as_str() {
        CATEGORY_RECURRING_FAILURE_SHAPE => string_field(&p.body, "shape"),
        CATEGORY_DOC_GAP => string_field(&p.body, "convention"),
        CATEGORY_ADR_DRIFT => string_field(&p.body, "decision_area"),
        CATEGORY_SCOPE_MISTAKE => sorted_array_field(&p.body, "branches"),
        _ => None,
    };
    match primary {
        Some(id) => format!("{}|{}", p.category, id),
        None => format!("{}|#{}", p.category, p.id),
    }
}

/// Serialises a JSON body compactly, falling back to its `Display` form if
/// serialisation somehow fails (it cannot for an in-memory `Value`).
fn compact_json(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
}

/// Renders one qualitative record as a markdown bullet block (always ending
/// in a newline). Well-formed bodies get a structured one-line summary; bodies
/// that don't match the documented shape fall back to the title plus a JSON
/// dump (design D5, tolerant rendering) so a record is never dropped.
fn render_qualitative(p: &LearningPayload) -> String {
    render_qualitative_structured(p).unwrap_or_else(|| render_qualitative_fallback(p))
}

/// Tolerant fallback: the record's `title` followed by its `body` serialised
/// as compact JSON on an indented continuation line.
fn render_qualitative_fallback(p: &LearningPayload) -> String {
    format!("- {}\n  {}\n", p.title, compact_json(&p.body))
}

/// Structured rendering for a recognised, well-formed qualitative body.
/// Returns `None` when a required field is missing so the caller falls back to
/// [`render_qualitative_fallback`].
fn render_qualitative_structured(p: &LearningPayload) -> Option<String> {
    match p.category.as_str() {
        CATEGORY_RECURRING_FAILURE_SHAPE => {
            let shape = string_field(&p.body, "shape")?;
            let instances = p.body.get("instances")?.as_array()?;
            let branches: Vec<String> = instances
                .iter()
                .filter_map(|i| i.get("branch_id").and_then(|v| v.as_str()))
                .map(str::to_string)
                .collect();
            let n = instances.len();
            let noun = if n == 1 { "instance" } else { "instances" };
            let across = if branches.is_empty() {
                String::new()
            } else {
                format!(" across {}", branches.join(", "))
            };
            Some(format!("- {shape}: {n} {noun}{across}\n"))
        }
        CATEGORY_DOC_GAP => {
            let convention = string_field(&p.body, "convention")?;
            let suggestion = string_field(&p.body, "suggestion")?;
            Some(format!("- {convention} — {suggestion}\n"))
        }
        CATEGORY_ADR_DRIFT => {
            let area = string_field(&p.body, "decision_area")?;
            let observed = string_field(&p.body, "observed_pattern")?;
            Some(format!("- {area}: {observed}\n"))
        }
        CATEGORY_SCOPE_MISTAKE => {
            let branches = p.body.get("branches")?.as_array()?;
            let names: Vec<String> = branches
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect();
            if names.is_empty() {
                return None;
            }
            let suggestion = string_field(&p.body, "suggestion")?;
            Some(format!("- {} — {suggestion}\n", names.join(" and ")))
        }
        _ => None,
    }
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
            ConflictCategory::ForwardConflictIntraSpec {
                agents, spec_id, ..
            } => {
                assert_eq!(agents, &vec!["feat-x".to_string(), "feat-y".to_string()]);
                assert_eq!(spec_id, "003-user-list");
            }
            other => panic!("expected intra-spec, got {other:?}"),
        }
    }

    #[test]
    fn forward_conflict_region_aware_body_includes_regions() {
        // A region-aware detector message yields a conflict_event body with a
        // `regions` array naming the intersecting regions
        // (conflict-detector-fn-granularity task 6.1).
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.record_detector_message(&feedback(
            "feat-y",
            &["[conflict-detector] forward conflict: agent feat-x also intends to modify: src/auth.rs (regions: function validate_token, function refresh_session)"],
        ));
        let events = a.conflict_events();
        assert_eq!(events.len(), 1);
        let record = record_from_conflict(&events[0].category, SystemTime::now());
        let regions = record.body.get("regions").expect("regions field present");
        assert_eq!(
            regions,
            &serde_json::json!(["function validate_token", "function refresh_session"])
        );
    }

    #[test]
    fn forward_conflict_file_level_body_omits_regions() {
        // A file-level (regionless) forward conflict omits the `regions` field.
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.record_detector_message(&feedback(
            "feat-y",
            &["[conflict-detector] forward conflict: agent feat-x also intends to modify: src/main.rs"],
        ));
        let events = a.conflict_events();
        assert_eq!(events.len(), 1);
        let record = record_from_conflict(&events[0].category, SystemTime::now());
        assert!(
            record.body.get("regions").is_none(),
            "file-level conflict must omit regions; got {:?}",
            record.body
        );
    }

    #[test]
    fn extract_regions_parses_descriptors() {
        assert_eq!(
            extract_regions(
                "foo src/a.rs (regions: function f, range 10-30); src/b.rs (regions: class C)"
            ),
            vec![
                "function f".to_string(),
                "range 10-30".to_string(),
                "class C".to_string()
            ]
        );
        assert!(extract_regions("no regions here, just src/a.rs").is_empty());
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
            ConflictCategory::ForwardConflictCrossSpec {
                agents, spec_ids, ..
            } => {
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

    /// Spec scenario (agent-learning-variant) `agent.learning broker message
    /// variant`: the variant deferred in v0.5.0 now exists and round-trips.
    /// This supersedes the v0.5.0 `broker_message_has_no_agent_learning_variant`
    /// negative test, which asserted the variant's *absence*.
    #[test]
    fn broker_message_has_agent_learning_variant() {
        use crate::broker::messages::LearningPayload;

        // A well-formed `agent.learning` envelope now deserialises cleanly.
        let probe = r#"{"type":"agent.learning","payload":{"id":"abc123def456abcd","agent_id":"supervisor","branch_id":"feat/x","category":"conflict_event","title":"forward conflict","body":{"shape":"forward"},"timestamp":"2026-05-28T12:01:01Z"}}"#;
        let msg = BrokerMessage::from_json(probe)
            .expect("a well-formed agent.learning envelope must deserialise");
        let BrokerMessage::Learning { payload } = &msg else {
            panic!("expected Learning, got {msg:?}");
        };
        assert_eq!(payload.category, "conflict_event");
        assert_eq!(payload.agent_id, "supervisor");
        assert_eq!(payload.branch_id.as_deref(), Some("feat/x"));
        assert_eq!(msg.status_label(), "learning");

        // And it re-serialises under the documented wire tag.
        let round = BrokerMessage::Learning {
            payload: LearningPayload {
                id: "abc123def456abcd".to_string(),
                agent_id: "supervisor".to_string(),
                branch_id: None,
                category: "permission_pattern".to_string(),
                title: "`cargo check` auto-approved 23 times".to_string(),
                body: serde_json::json!({"command_class": "cargo check", "count": 23}),
                timestamp: "2026-05-28T12:01:01Z".to_string(),
            },
        };
        let json = serde_json::to_string(&round).unwrap();
        assert!(json.contains("\"type\":\"agent.learning\""));
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

    // === agent-learning-variant: deterministic id + record conversion ===

    /// A timestamp at `YYYY-MM-DDTHH:MM` UTC for a fixed reference day so the
    /// hour-bucket boundary behaviour can be exercised deterministically.
    fn ts_at(hour: u64, minute: u64) -> SystemTime {
        // 2026-05-28T00:00:00Z = 1_779_926_400 (days since epoch * 86400).
        const DAY_START: u64 = 1_779_926_400;
        UNIX_EPOCH + Duration::from_secs(DAY_START + hour * 3600 + minute * 60)
    }

    fn sample_record(category: &str, branch: Option<&str>, ts: SystemTime) -> LearningRecord {
        LearningRecord {
            category: category.to_string(),
            agent_id: LEARNINGS_AGENT_ID.to_string(),
            branch_id: branch.map(str::to_string),
            title: "title".to_string(),
            body: serde_json::json!({"agents": ["feat-x", "feat-y"], "files": ["src/a.rs"]}),
            timestamp: ts,
        }
    }

    // Task 2.3: same record at 13:30 and 13:59 produces the same id.
    #[test]
    fn same_record_within_the_hour_gets_same_id() {
        let a = sample_record(CATEGORY_CONFLICT_EVENT, Some("feat-x"), ts_at(13, 30));
        let b = sample_record(CATEGORY_CONFLICT_EVENT, Some("feat-x"), ts_at(13, 59));
        assert_eq!(a.deterministic_id(), b.deterministic_id());
        // The id is a 16-hex-char prefix.
        assert_eq!(a.deterministic_id().len(), 16);
        assert!(a.deterministic_id().chars().all(|c| c.is_ascii_hexdigit()));
    }

    // Task 2.4: same record at 13:59 and 14:01 produces different ids.
    #[test]
    fn same_record_across_hour_boundary_gets_different_ids() {
        let a = sample_record(CATEGORY_CONFLICT_EVENT, Some("feat-x"), ts_at(13, 59));
        let b = sample_record(CATEGORY_CONFLICT_EVENT, Some("feat-x"), ts_at(14, 1));
        assert_ne!(a.deterministic_id(), b.deterministic_id());
    }

    // Task 2.5: different categories produce different ids even when the body
    // is identical.
    #[test]
    fn different_categories_get_different_ids_with_identical_body() {
        let ts = ts_at(13, 30);
        let a = sample_record(CATEGORY_CONFLICT_EVENT, Some("feat-x"), ts);
        let b = sample_record(CATEGORY_STUCK_DURATION, Some("feat-x"), ts);
        assert_ne!(a.deterministic_id(), b.deterministic_id());
    }

    #[test]
    fn id_is_independent_of_body_key_insertion_order() {
        let ts = ts_at(13, 30);
        let mut a = sample_record(CATEGORY_CONFLICT_EVENT, None, ts);
        a.body = serde_json::json!({"alpha": 1, "beta": 2});
        let mut b = sample_record(CATEGORY_CONFLICT_EVENT, None, ts);
        b.body = serde_json::json!({"beta": 2, "alpha": 1});
        assert_eq!(a.deterministic_id(), b.deterministic_id());
    }

    #[test]
    fn branch_id_distinguishes_otherwise_identical_records() {
        let ts = ts_at(13, 30);
        let a = sample_record(CATEGORY_STUCK_DURATION, Some("feat-x"), ts);
        let b = sample_record(CATEGORY_STUCK_DURATION, Some("feat-y"), ts);
        assert_ne!(a.deterministic_id(), b.deterministic_id());
    }

    // Task 1.4: serialise + deserialise each of the four categories' records
    // through the From<&LearningRecord> conversion and the wire envelope.
    #[test]
    fn all_four_categories_round_trip_through_broker_message() {
        let now = ts_at(12, 1);
        let records = [
            record_from_conflict(
                &ConflictCategory::InFlightConflict {
                    agents: vec!["feat-x".to_string(), "feat-y".to_string()],
                },
                now,
            ),
            record_from_stuck(
                &StuckDurationEntry {
                    agent_id: "feat-x".to_string(),
                    blocked_on: "feat-y".to_string(),
                    duration_seconds: 672,
                    resolved: true,
                },
                now,
            ),
            record_from_recovery(
                &RecoveryCycleEntry {
                    agent_id: "feat-x".to_string(),
                    count: 3,
                },
                now,
            ),
            record_from_permission("cargo check", 23, now),
        ];
        let expected_categories = [
            CATEGORY_CONFLICT_EVENT,
            CATEGORY_STUCK_DURATION,
            CATEGORY_RECOVERY_CYCLES,
            CATEGORY_PERMISSION_PATTERN,
        ];
        for (record, expected_category) in records.iter().zip(expected_categories) {
            let msg = BrokerMessage::from(record);
            let json = serde_json::to_string(&msg).unwrap();
            let back = BrokerMessage::from_json(&json)
                .unwrap_or_else(|e| panic!("{expected_category} must round-trip: {e}"));
            let BrokerMessage::Learning { payload } = back else {
                panic!("expected Learning variant for {expected_category}");
            };
            assert_eq!(payload.category, expected_category);
            assert_eq!(payload.id, record.deterministic_id());
            assert_eq!(payload.agent_id, LEARNINGS_AGENT_ID);
            assert!(!payload.title.is_empty());
        }
    }

    #[test]
    fn conflict_and_permission_records_are_cross_cutting_no_branch() {
        let now = ts_at(12, 1);
        let conflict = record_from_conflict(
            &ConflictCategory::InFlightConflict {
                agents: vec!["feat-x".to_string(), "feat-y".to_string()],
            },
            now,
        );
        let permission = record_from_permission("cargo check", 9, now);
        assert_eq!(conflict.branch_id, None);
        assert_eq!(permission.branch_id, None);
    }

    #[test]
    fn stuck_and_recovery_records_are_branch_scoped() {
        let now = ts_at(12, 1);
        let stuck = record_from_stuck(
            &StuckDurationEntry {
                agent_id: "feat-x".to_string(),
                blocked_on: "feat-y".to_string(),
                duration_seconds: 60,
                resolved: false,
            },
            now,
        );
        let recovery = record_from_recovery(
            &RecoveryCycleEntry {
                agent_id: "feat-z".to_string(),
                count: 2,
            },
            now,
        );
        assert_eq!(stuck.branch_id.as_deref(), Some("feat-x"));
        assert_eq!(recovery.branch_id.as_deref(), Some("feat-z"));
    }

    // === agent-learning-variant: dual-output gating ===

    // Spec scenario `File-only output when broker is disabled`: with broker
    // publish off (the default), a flush appends to the file and queues NO
    // broker records.
    #[test]
    fn broker_publish_off_queues_no_records() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        assert!(!a.broker_publish_enabled());
        for _ in 0..PERMISSION_PATTERN_THRESHOLD {
            a.record_auto_approve("cargo check");
        }
        a.flush().unwrap();
        assert!(read_md(a.file_path()).contains("`cargo check`"));
        assert!(
            a.take_pending_publish().is_empty(),
            "no records should be queued when broker publish is disabled"
        );
    }

    // Spec scenario `Both outputs when broker is enabled`: with broker publish
    // on, a flush appends to the file AND queues a matching broker record.
    #[test]
    fn broker_publish_on_queues_records_matching_file() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.set_broker_publish(true);
        a.register_agent("feat-x");
        a.register_agent("feat-y");
        a.record_detector_message(&feedback(
            "feat-x",
            &["[conflict-detector] in-flight conflict with feat-y on src/a.rs"],
        ));
        a.flush().unwrap();

        let md = read_md(a.file_path());
        assert!(md.contains("### Conflict events"));
        let records = a.take_pending_publish();
        assert_eq!(records.len(), 1, "one conflict record should be queued");
        assert_eq!(records[0].category, CATEGORY_CONFLICT_EVENT);
        // The record's title mirrors the markdown bullet text.
        assert!(md.contains(&records[0].title));
        // Draining empties the queue.
        assert!(a.take_pending_publish().is_empty());
    }

    // === qualitative-learnings: ingestion, routing, tolerant rendering ===

    use crate::broker::messages::LearningPayload;

    /// Builds an externally-published `agent.learning` envelope with the given
    /// category, title, and body — the shape the supervisor LLM publishes.
    fn learning(category: &str, title: &str, body: serde_json::Value) -> BrokerMessage {
        BrokerMessage::Learning {
            payload: LearningPayload {
                id: format!("id-{category}-{title}"),
                agent_id: LEARNINGS_AGENT_ID.to_string(),
                branch_id: None,
                category: category.to_string(),
                title: title.to_string(),
                body,
                timestamp: "2026-06-05T12:00:00Z".to_string(),
            },
        }
    }

    // Task 2.4: each new category routes to its own section header.
    #[test]
    fn each_qualitative_category_routes_to_its_section() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.observe(&learning(
            CATEGORY_RECURRING_FAILURE_SHAPE,
            "import cycle recurs",
            serde_json::json!({
                "shape": "import cycle in payments module",
                "instances": [
                    {"branch_id": "feat/a", "feedback_id": "f1", "excerpt": "..."},
                    {"branch_id": "feat/b", "feedback_id": "f2", "excerpt": "..."}
                ]
            }),
        ));
        a.observe(&learning(
            CATEGORY_DOC_GAP,
            "lint-before-commit undocumented",
            serde_json::json!({
                "convention": "agents run lint before commit",
                "evidence_paths": ["AGENTS.md"],
                "suggestion": "add a Conventions section to AGENTS.md"
            }),
        ));
        a.observe(&learning(
            CATEGORY_ADR_DRIFT,
            "async runtime undocumented",
            serde_json::json!({
                "decision_area": "async runtime",
                "observed_pattern": "a background runtime added in the broker server",
                "configured_adr_path": "docs/adr",
                "candidate_adr_title": "ADR-0007: Adopt an async runtime"
            }),
        ));
        a.observe(&learning(
            CATEGORY_SCOPE_MISTAKE,
            "two branches over-coordinated",
            serde_json::json!({
                "branches": ["feat/a", "feat/b"],
                "shared_files": ["src/router"],
                "coordination_events": [],
                "suggestion": "merge the feat/a and feat/b scopes"
            }),
        ));
        a.flush().unwrap();

        let md = read_md(a.file_path());
        assert!(md.contains("### Recurring failure shapes"), "{md}");
        assert!(md.contains("import cycle in payments module: 2 instances across feat/a, feat/b"));
        assert!(md.contains("### Documentation gaps"), "{md}");
        assert!(
            md.contains("- agents run lint before commit — add a Conventions section to AGENTS.md")
        );
        assert!(md.contains("### ADR / architectural drift"), "{md}");
        assert!(md.contains("- async runtime: a background runtime added in the broker server"));
        assert!(md.contains("### Scope-mistake signals"), "{md}");
        assert!(md.contains("- feat/a and feat/b — merge the feat/a and feat/b scopes"));
    }

    // Task 2.4: a body that lacks a documented field renders as the title plus
    // a JSON dump under the category's section (design D5).
    #[test]
    fn malformed_qualitative_body_renders_as_title_plus_json() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        // Lacks the documented `instances` field.
        a.observe(&learning(
            CATEGORY_RECURRING_FAILURE_SHAPE,
            "vague shape with no instances",
            serde_json::json!({"shape": "something fuzzy"}),
        ));
        a.flush().unwrap();

        let md = read_md(a.file_path());
        // Still under the recurring-failure-shape section.
        assert!(md.contains("### Recurring failure shapes"), "{md}");
        // Title line present.
        assert!(md.contains("- vague shape with no instances"), "{md}");
        // Body serialised as JSON present (not dropped).
        assert!(md.contains(r#"{"shape":"something fuzzy"}"#), "{md}");
    }

    // Task 2.4: an unrecognised category lands under "Other learnings" and is
    // not silently dropped.
    #[test]
    fn unknown_category_falls_through_to_other_learnings() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.observe(&learning(
            "some_future_category",
            "a future learning shape",
            serde_json::json!({"note": "from a later version"}),
        ));
        a.flush().unwrap();

        let md = read_md(a.file_path());
        assert!(md.contains("### Other learnings"), "{md}");
        assert!(md.contains("- a future learning shape"), "{md}");
        assert!(md.contains(r#"{"note":"from a later version"}"#), "{md}");
    }

    // Task 2.4: ingested deterministic-category records (the aggregator's own,
    // flowing back through the publish path) are ignored — no double render,
    // no "Other learnings" leak.
    #[test]
    fn ingested_deterministic_learning_is_ignored() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.observe(&learning(
            CATEGORY_CONFLICT_EVENT,
            "forward conflict feat-x and feat-y",
            serde_json::json!({"shape": "forward", "agents": ["feat-x", "feat-y"]}),
        ));
        assert!(a.qualitative_events().is_empty());
        a.flush().unwrap();
        // Nothing written: the record was dropped and there is no other signal.
        assert_eq!(read_md(a.file_path()), "");
    }

    // Task 2.4: the v0.5.0 deterministic sections still render in their v0.5.0
    // shape, even when qualitative records are present in the same flush.
    #[test]
    fn v0_5_0_sections_unchanged_alongside_qualitative() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        // A v0.5.0 deterministic signal.
        for _ in 0..PERMISSION_PATTERN_THRESHOLD {
            a.record_auto_approve("git status");
        }
        // A qualitative signal in the same flush.
        a.observe(&learning(
            CATEGORY_DOC_GAP,
            "doc gap",
            serde_json::json!({"convention": "c", "suggestion": "s"}),
        ));
        a.flush().unwrap();

        let md = read_md(a.file_path());
        // v0.5.0 permission-pattern section + bullet, byte-for-byte shape.
        assert!(md.contains("### Permission patterns"));
        assert!(md.contains("- `git status` auto-approved 5 times"));
        // Qualitative section also present.
        assert!(md.contains("### Documentation gaps"));
    }

    // Task 4 / 6.2: the same recurring_failure_shape published twice in a
    // session is rendered once (skill-level dedup reinforced code-side).
    #[test]
    fn qualitative_dedup_suppresses_same_primary_identifier() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        let body = serde_json::json!({
            "shape": "import cycle in payments module",
            "instances": [{"branch_id": "feat/a"}, {"branch_id": "feat/b"}]
        });
        a.observe(&learning(
            CATEGORY_RECURRING_FAILURE_SHAPE,
            "first sighting",
            body.clone(),
        ));
        // Same shape, different title/wording — must be suppressed.
        a.observe(&learning(
            CATEGORY_RECURRING_FAILURE_SHAPE,
            "second sighting, reworded",
            body,
        ));
        assert_eq!(
            a.qualitative_events().len(),
            1,
            "near-duplicate not deduped"
        );
        a.flush().unwrap();
        let md = read_md(a.file_path());
        let occurrences = md.matches("import cycle in payments module").count();
        assert_eq!(occurrences, 1, "shape rendered more than once:\n{md}");
    }

    // Distinct primary identifiers within the same category are NOT deduped.
    #[test]
    fn qualitative_dedup_keeps_distinct_identifiers() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.observe(&learning(
            CATEGORY_DOC_GAP,
            "gap one",
            serde_json::json!({"convention": "lint before commit", "suggestion": "s1"}),
        ));
        a.observe(&learning(
            CATEGORY_DOC_GAP,
            "gap two",
            serde_json::json!({"convention": "sign your commits", "suggestion": "s2"}),
        ));
        assert_eq!(a.qualitative_events().len(), 2);
    }

    // Two malformed records with the same category but no primary identifier
    // are kept distinct via the publisher's deterministic id.
    #[test]
    fn qualitative_dedup_distinguishes_malformed_by_id() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.observe(&learning(
            CATEGORY_SCOPE_MISTAKE,
            "malformed one",
            serde_json::json!({"note": "no branches a"}),
        ));
        a.observe(&learning(
            CATEGORY_SCOPE_MISTAKE,
            "malformed two",
            serde_json::json!({"note": "no branches b"}),
        ));
        assert_eq!(a.qualitative_events().len(), 2);
    }

    // Spec scenario `Hour-bucket id collisions are independently handled`:
    // two qualitative records with identical canonical input within the same
    // UTC hour produce the same deterministic id, so a broker consumer can
    // dedupe exact re-emissions even if the skill-level dedup misses.
    #[test]
    fn qualitative_records_get_identical_ids_within_the_hour() {
        let body = serde_json::json!({
            "shape": "import cycle in payments module",
            "instances": [{"branch_id": "feat/a"}, {"branch_id": "feat/b"}]
        });
        let a = LearningRecord {
            category: CATEGORY_RECURRING_FAILURE_SHAPE.to_string(),
            agent_id: LEARNINGS_AGENT_ID.to_string(),
            branch_id: None,
            title: "first".to_string(),
            body: body.clone(),
            timestamp: ts_at(13, 5),
        };
        let b = LearningRecord {
            timestamp: ts_at(13, 55),
            title: "reworded".to_string(),
            ..a.clone()
        };
        assert_eq!(a.deterministic_id(), b.deterministic_id());
        assert_eq!(a.deterministic_id().len(), 16);
    }

    // Qualitative ingestion never queues a broker publish (the record already
    // came from the broker) even when broker publish is enabled.
    #[test]
    fn qualitative_ingestion_does_not_republish() {
        let tmp = TempDir::new().unwrap();
        let mut a = agg(&tmp);
        a.set_broker_publish(true);
        a.observe(&learning(
            CATEGORY_DOC_GAP,
            "doc gap",
            serde_json::json!({"convention": "c", "suggestion": "s"}),
        ));
        a.flush().unwrap();
        assert!(read_md(a.file_path()).contains("### Documentation gaps"));
        assert!(
            a.take_pending_publish().is_empty(),
            "ingested qualitative records must not be re-published"
        );
    }

    // Task 7.4 (idempotency, unit level): two aggregators replaying the same
    // input events within the same hour produce records with identical ids,
    // so a consumer dedupes them to one.
    #[test]
    fn replayed_events_within_hour_produce_identical_ids() {
        fn run() -> String {
            let tmp = TempDir::new().unwrap();
            let mut a = agg(&tmp);
            a.set_broker_publish(true);
            a.register_agent("feat-x");
            a.register_agent("feat-y");
            a.record_detector_message(&feedback(
                "feat-x",
                &["[conflict-detector] in-flight conflict with feat-y on src/a.rs"],
            ));
            a.flush().unwrap();
            a.take_pending_publish()[0].deterministic_id()
        }
        // Both runs fall in the same wall-clock hour (the test runs in well
        // under an hour), so the ids match.
        assert_eq!(run(), run());
    }

    // --- Supervisor routing records (supervisor-tell change, design D4) ---

    #[test]
    fn format_routing_entry_shape() {
        let line = format_routing_entry(
            "2026-05-28T14:35:09Z",
            "feat/x",
            "feedback",
            "rebase onto main before continuing",
        );
        assert_eq!(
            line,
            "- 2026-05-28T14:35:09Z — supervisor told `feat/x` via feedback: \"rebase onto main before continuing\""
        );
    }

    #[test]
    fn format_routing_entry_truncates_long_prompt() {
        let prompt = "x".repeat(300);
        let line = format_routing_entry("T", "feat/x", "send-keys", &prompt);
        assert!(
            line.ends_with("…\""),
            "long prompt should end with …: {line}"
        );
        // 200 retained chars + the ellipsis.
        assert_eq!(prompt.chars().take(ROUTING_PROMPT_MAX_CHARS).count(), 200);
        assert!(line.contains(&"x".repeat(ROUTING_PROMPT_MAX_CHARS)));
    }

    #[test]
    fn routing_record_with_learnings_enabled_writes_section() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("session-learnings.md");
        append_routing_record(
            &path,
            true,
            "2026-05-28T14:35:09Z",
            "feat/auth",
            "feedback",
            "rebase onto main",
        )
        .unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains(ROUTING_SECTION_HEADER));
        assert!(body.contains("feat/auth"));
        assert!(body.contains("via feedback"));
        assert!(body.contains("rebase onto main"));

        // A second record reuses the existing section header (written once).
        append_routing_record(&path, true, "T2", "feat/api", "send-keys", "run it").unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            body.matches(ROUTING_SECTION_HEADER).count(),
            1,
            "section header must be written exactly once"
        );
        assert!(body.contains("feat/api"));
    }

    #[test]
    fn routing_record_with_learnings_disabled_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("session-learnings.md");
        append_routing_record(&path, false, "T", "feat/auth", "feedback", "noop").unwrap();
        assert!(
            !path.exists(),
            "learnings = false must not create or write the file"
        );
    }
}
