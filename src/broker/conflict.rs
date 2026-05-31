//! Broker-internal conflict detector.
//!
//! Runs alongside the message-delivery pipeline when supervisor mode is
//! active. The detector observes `agent.intent` and `agent.status` events,
//! maintains an in-memory tracker of active intents and per-agent current
//! file claims, and auto-emits `agent.feedback` / `agent.question` when
//! one of three failure shapes triggers:
//!
//! - **Forward conflict** — two agents publish `agent.intent` messages
//!   that overlap on at least one file. Both publishers are warned via
//!   `agent.feedback`. Each ordered pair is warned at most once until one
//!   intent is replaced or expires.
//! - **In-flight conflict** — two agents' `agent.status.modified_files`
//!   sets overlap on a file. Both branches are warned. If neither agent
//!   stops touching the file within `[supervisor.conflict] window_seconds`
//!   an escalation `agent.question` is published to the supervisor inbox.
//! - **Ownership violation** — an agent's `modified_files` include a file
//!   outside its own active `agent.intent` *and* inside another active
//!   agent's intent. The violator gets `agent.feedback`. If
//!   `[supervisor.conflict] escalate_on_violation = true`, an
//!   `agent.question` also reaches the supervisor inbox.
//!
//! Auto-emitted messages use `from = "supervisor"` and prefix their text
//! with `[conflict-detector]` so dashboards and humans can distinguish
//! detector-emitted feedback from human-typed supervisor feedback.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::messages::{
    BrokerMessage, FeedbackPayload, FileIntent, IntentPayload, QuestionPayload, Region,
    StatusPayload,
};
use super::{BrokerState, delivery};
use crate::config::ConflictConfig;

/// Detector-internal normalised form of one `files` entry.
///
/// Both wire shapes ([`FileIntent::Path`] and [`FileIntent::Detailed`])
/// collapse into this single shape so the detector never branches on the
/// publisher's wire form. `regions: None` means file-level intent (the
/// v0.5.0 default and the safe fallback); `Some(..)` carries declared
/// regions. An empty `regions` vec on the wire collapses to `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedFileIntent {
    /// The file path.
    pub path: String,
    /// Declared regions, or `None` for a file-level intent.
    pub regions: Option<Vec<Region>>,
}

impl From<FileIntent> for NormalizedFileIntent {
    fn from(fi: FileIntent) -> Self {
        match fi {
            FileIntent::Path(path) => Self {
                path,
                regions: None,
            },
            FileIntent::Detailed { path, regions } => Self {
                path,
                // An empty regions vec is equivalent to no regions declared.
                regions: if regions.is_empty() {
                    None
                } else {
                    Some(regions)
                },
            },
        }
    }
}

/// One file's contribution to a forward conflict between two intents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileConflict {
    /// The shared file path.
    pub path: String,
    /// Intersecting regions naming why the file conflicts. Empty means a
    /// file-level conflict (at least one side declared no regions).
    pub regions: Vec<Region>,
    /// `true` when at least one intersecting pair was a conservative
    /// cross-kind match (a named region vs a line range).
    pub cross_kind: bool,
}

/// A forward conflict between the queried agent and one other agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardConflict {
    /// The other agent whose intent overlaps.
    pub other_agent: String,
    /// The conflicting files (each with its intersecting regions).
    pub files: Vec<FileConflict>,
}

/// Returns `true` when two inclusive line intervals overlap.
fn ranges_overlap(s1: u32, e1: u32, s2: u32, e2: u32) -> bool {
    s1 <= e2 && s2 <= e1
}

/// Computes the intersection of two region sets per the detector rules:
///
/// - Same kind + matching name (function / class / block) → intersect,
///   contributing that region.
/// - Two ranges with overlapping intervals → intersect, contributing the
///   overlapping sub-range.
/// - A named region vs a range (either order) → conservative intersect
///   (we can't resolve a name to lines without source parsing); contributes
///   both regions and sets the cross-kind flag.
/// - Named regions of differing kinds, or same kind with differing names →
///   no intersection.
///
/// Returns the de-duplicated list of intersecting regions (sorted for
/// deterministic output) and whether any cross-kind match occurred.
fn regions_intersect(a: &[Region], b: &[Region]) -> (Vec<Region>, bool) {
    let mut hits: Vec<Region> = Vec::new();
    let mut cross_kind = false;
    let push = |r: Region, hits: &mut Vec<Region>| {
        if !hits.contains(&r) {
            hits.push(r);
        }
    };
    for ra in a {
        for rb in b {
            match (ra, rb) {
                (Region::Function { name: n1 }, Region::Function { name: n2 })
                | (Region::Class { name: n1 }, Region::Class { name: n2 })
                | (Region::Block { anchor: n1 }, Region::Block { anchor: n2 })
                    if n1 == n2 =>
                {
                    push(ra.clone(), &mut hits);
                }
                (
                    Region::Range {
                        start_line: s1,
                        end_line: e1,
                    },
                    Region::Range {
                        start_line: s2,
                        end_line: e2,
                    },
                ) if ranges_overlap(*s1, *e1, *s2, *e2) => {
                    push(
                        Region::Range {
                            start_line: (*s1).max(*s2),
                            end_line: (*e1).min(*e2),
                        },
                        &mut hits,
                    );
                }
                // Cross-kind: a named region vs a range, in either order.
                // We can't resolve the name to lines without source parsing,
                // so we intersect conservatively and name both regions.
                (
                    Region::Range { .. },
                    Region::Function { .. } | Region::Class { .. } | Region::Block { .. },
                )
                | (
                    Region::Function { .. } | Region::Class { .. } | Region::Block { .. },
                    Region::Range { .. },
                ) => {
                    cross_kind = true;
                    push(ra.clone(), &mut hits);
                    push(rb.clone(), &mut hits);
                }
                // Same-shape named regions with differing names, or differing
                // named kinds: no intersection.
                _ => {}
            }
        }
    }
    hits.sort_by_key(Region::to_string);
    hits.dedup();
    (hits, cross_kind)
}

/// Sender identifier used for all auto-emitted detector messages. Lets
/// recipients (and the dashboard) treat detector output the same as
/// human-typed supervisor feedback while the leading `[conflict-detector]`
/// token disambiguates inside the text.
pub const CONFLICT_DETECTOR_SENDER: &str = "supervisor";

/// Token that prefixes every detector-emitted error / question text. Used
/// by skill tests and the dashboard to identify auto-warnings.
pub const CONFLICT_DETECTOR_TAG: &str = "[conflict-detector]";

/// One agent's currently-active intent declaration.
#[derive(Debug, Clone)]
pub struct IntentRecord {
    /// Publishing agent's ID.
    pub agent_id: String,
    /// File paths the agent intends to modify, each mapped to its declared
    /// regions (`None` = file-level intent / no regions declared).
    pub files: HashMap<String, Option<Vec<Region>>>,
    /// Human-readable summary of the planned change.
    pub summary: String,
    /// When the intent was received.
    pub received_at: Instant,
    /// Relative TTL — the entry is dropped when
    /// `now - received_at > valid_for`.
    pub valid_for: Duration,
}

impl IntentRecord {
    /// Returns `true` if this intent declares the given file path (at any
    /// granularity).
    #[must_use]
    pub fn claims_path(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }
}

impl IntentRecord {
    fn is_expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.received_at) > self.valid_for
    }
}

/// State for one in-flight-conflict triple.
#[derive(Debug, Clone)]
struct InFlightPair {
    /// When the triple was first observed.
    first_seen: Instant,
    /// Whether an escalation `agent.question` has already been emitted.
    escalated: bool,
}

/// Lex-ordered agent-id pair used as the dedup key for forward
/// conflicts and as part of the in-flight-pair key.
fn ordered_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

/// In-memory tracker for the detector. Owns the active-intent map, the
/// per-agent current-files map, and dedup sets for warnings.
#[derive(Debug, Default)]
pub struct ConflictTracker {
    intents: HashMap<String, IntentRecord>,
    current_files: HashMap<String, HashSet<String>>,
    warned_intent_pairs: HashSet<(String, String)>,
    in_flight_pairs: HashMap<(String, String, String), InFlightPair>,
    warned_violations: HashSet<(String, String)>,
}

impl ConflictTracker {
    /// Returns an empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ====================================================================
    // Mutators
    // ====================================================================

    /// Inserts or replaces the intent record for `agent_id`. When the new
    /// intent's file set differs from the prior intent's, any pair-level
    /// forward-conflict dedup entries the prior intent participated in
    /// are cleared so future overlaps with peers can re-warn. Re-publishing
    /// an *identical* file set leaves the dedup intact, so a no-op
    /// re-publish does not retrigger warnings.
    pub fn insert_intent(
        &mut self,
        agent_id: &str,
        files: Vec<NormalizedFileIntent>,
        summary: String,
        ttl: Duration,
        now: Instant,
    ) {
        let normalized: HashMap<String, Option<Vec<Region>>> = files
            .into_iter()
            .filter_map(|nfi| {
                let path = nfi.path.trim().to_string();
                if path.is_empty() {
                    None
                } else {
                    Some((path, nfi.regions))
                }
            })
            .collect();
        let files_changed = self
            .intents
            .get(agent_id)
            .is_none_or(|prior| prior.files != normalized);
        if files_changed {
            self.warned_intent_pairs
                .retain(|(a, b)| a != agent_id && b != agent_id);
        }
        self.intents.insert(
            agent_id.to_string(),
            IntentRecord {
                agent_id: agent_id.to_string(),
                files: normalized,
                summary,
                received_at: now,
                valid_for: ttl,
            },
        );
    }

    /// Replaces the current modified-file set for `agent_id`.
    /// `modified_files` is always treated as the full current set, not a
    /// delta.
    pub fn update_status(&mut self, agent_id: &str, modified_files: Vec<String>) {
        let normalized: HashSet<String> = modified_files
            .into_iter()
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect();
        self.current_files.insert(agent_id.to_string(), normalized);
    }

    /// Drops intents whose age exceeds their TTL. Forward-conflict dedup
    /// entries involving the dropped agents are also removed so that a
    /// future intent from the same agent can re-trigger warnings.
    pub fn expire_stale_intents(&mut self, now: Instant) {
        let expired: Vec<String> = self
            .intents
            .iter()
            .filter(|(_, r)| r.is_expired(now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            self.intents.remove(id);
        }
        self.warned_intent_pairs
            .retain(|(a, b)| !expired.contains(a) && !expired.contains(b));
    }

    /// Removes in-flight triples whose file is no longer in the
    /// intersection of both agents' current modified-file sets.
    pub fn sweep_in_flight_pairs(&mut self) {
        let keys: Vec<(String, String, String)> = self.in_flight_pairs.keys().cloned().collect();
        for (a, b, file) in keys {
            let a_has = self
                .current_files
                .get(&a)
                .is_some_and(|files| files.contains(&file));
            let b_has = self
                .current_files
                .get(&b)
                .is_some_and(|files| files.contains(&file));
            if !(a_has && b_has) {
                self.in_flight_pairs.remove(&(a, b, file));
            }
        }
    }

    // ====================================================================
    // Read-only queries
    // ====================================================================

    /// Returns every forward conflict between `x_id`'s intent and every other
    /// non-expired intent in the tracker.
    ///
    /// For each file shared by both intents:
    /// - If either side declared no regions (`None`), the file is a
    ///   file-level conflict (v0.5.0 fallback) — reported with empty
    ///   `regions`.
    /// - If both sides declared regions, the file conflicts only when the
    ///   region sets intersect; the intersecting regions are reported.
    ///
    /// A pair appears in the output only when at least one file conflicts.
    /// Caller is responsible for dedup against `warned_intent_pairs`.
    #[must_use]
    pub fn forward_overlaps(&self, x_id: &str) -> Vec<ForwardConflict> {
        let Some(x) = self.intents.get(x_id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (other_id, y) in &self.intents {
            if other_id == x_id {
                continue;
            }
            let mut shared: Vec<&String> = x
                .files
                .keys()
                .filter(|path| y.files.contains_key(*path))
                .collect();
            shared.sort();
            let mut file_conflicts = Vec::new();
            for path in shared {
                match (&x.files[path], &y.files[path]) {
                    (Some(xr), Some(yr)) => {
                        let (regions, cross_kind) = regions_intersect(xr, yr);
                        if !regions.is_empty() {
                            file_conflicts.push(FileConflict {
                                path: path.clone(),
                                regions,
                                cross_kind,
                            });
                        }
                    }
                    // At least one side omitted regions → file-level conflict
                    // (preserves the v0.5.0 safety net).
                    _ => {
                        file_conflicts.push(FileConflict {
                            path: path.clone(),
                            regions: Vec::new(),
                            cross_kind: false,
                        });
                    }
                }
            }
            if !file_conflicts.is_empty() {
                out.push(ForwardConflict {
                    other_agent: other_id.clone(),
                    files: file_conflicts,
                });
            }
        }
        out.sort_by(|a, b| a.other_agent.cmp(&b.other_agent));
        out
    }

    /// Returns every `(min_id, max_id, file)` triple currently in the
    /// intersection of two agents' modified-file sets.
    #[must_use]
    pub fn in_flight_overlaps(&self) -> Vec<(String, String, String)> {
        let ids: Vec<&String> = self.current_files.keys().collect();
        let mut out = Vec::new();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = ids[i];
                let b = ids[j];
                let (Some(a_files), Some(b_files)) =
                    (self.current_files.get(a), self.current_files.get(b))
                else {
                    continue;
                };
                if a_files.is_empty() || b_files.is_empty() {
                    continue;
                }
                let (lo, hi) = ordered_pair(a, b);
                let mut files: Vec<String> = a_files.intersection(b_files).cloned().collect();
                files.sort();
                for f in files {
                    out.push((lo.clone(), hi.clone(), f));
                }
            }
        }
        out.sort();
        out
    }

    /// Returns ownership violations for agent `x_id` as `(file,
    /// owner_y_id)` tuples — files in `x_id`'s `current_files` that lie
    /// outside `x_id`'s own intent (or `x_id` has no intent) and inside
    /// some other agent's active non-expired intent.
    #[must_use]
    pub fn ownership_violations(&self, x_id: &str) -> Vec<(String, String)> {
        let Some(x_files) = self.current_files.get(x_id) else {
            return Vec::new();
        };
        let x_intent = self.intents.get(x_id);
        let mut out = Vec::new();
        let mut sorted_files: Vec<&String> = x_files.iter().collect();
        sorted_files.sort();
        for file in sorted_files {
            if x_intent.is_some_and(|r| r.claims_path(file)) {
                continue;
            }
            for (other_id, other) in &self.intents {
                if other_id == x_id {
                    continue;
                }
                if other.claims_path(file) {
                    out.push((file.clone(), other_id.clone()));
                }
            }
        }
        // Deterministic ordering by file then owner.
        out.sort();
        out
    }

    // ====================================================================
    // Dedup state
    // ====================================================================

    /// Returns `true` if the ordered pair `(min(a, b), max(a, b))` has
    /// already been warned for a forward conflict.
    #[must_use]
    pub fn was_intent_pair_warned(&self, a: &str, b: &str) -> bool {
        self.warned_intent_pairs.contains(&ordered_pair(a, b))
    }

    /// Marks the ordered pair as having been warned for a forward
    /// conflict. Subsequent calls to [`was_intent_pair_warned`] will
    /// return `true` until either intent is replaced or expires.
    pub fn mark_intent_pair_warned(&mut self, a: &str, b: &str) {
        self.warned_intent_pairs.insert(ordered_pair(a, b));
    }

    /// Inserts an initial entry for the in-flight triple `(min(a, b),
    /// max(a, b), file)` if not already present. Returns `true` if the
    /// entry was newly created (i.e. this is the initial warning).
    pub fn record_in_flight_pair(&mut self, a: &str, b: &str, file: &str, now: Instant) -> bool {
        let (lo, hi) = ordered_pair(a, b);
        let key = (lo, hi, file.to_string());
        if let std::collections::hash_map::Entry::Vacant(slot) = self.in_flight_pairs.entry(key) {
            slot.insert(InFlightPair {
                first_seen: now,
                escalated: false,
            });
            true
        } else {
            false
        }
    }

    /// Returns and marks-escalated every in-flight triple whose age
    /// exceeds `window` and that has not yet been escalated. Each triple
    /// is returned at most once across the tracker's lifetime.
    pub fn take_due_escalations(
        &mut self,
        window: Duration,
        now: Instant,
    ) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        for (key, pair) in &mut self.in_flight_pairs {
            if pair.escalated {
                continue;
            }
            if now.saturating_duration_since(pair.first_seen) >= window {
                pair.escalated = true;
                out.push(key.clone());
            }
        }
        out.sort();
        out
    }

    /// Returns `true` if the violator/file pair has already been warned.
    #[must_use]
    pub fn was_ownership_warned(&self, violator: &str, file: &str) -> bool {
        self.warned_violations
            .contains(&(violator.to_string(), file.to_string()))
    }

    /// Marks the violator/file pair as warned. Subsequent
    /// [`was_ownership_warned`] calls return `true`.
    pub fn mark_ownership_warned(&mut self, violator: &str, file: &str) {
        self.warned_violations
            .insert((violator.to_string(), file.to_string()));
    }

    // ====================================================================
    // Inspection (read-only access for tests and external callers)
    // ====================================================================

    /// Returns the intent record for an agent, if one is currently
    /// tracked.
    #[must_use]
    pub fn intent_for(&self, agent_id: &str) -> Option<&IntentRecord> {
        self.intents.get(agent_id)
    }

    /// Returns the current modified-file set for an agent, if known.
    #[must_use]
    pub fn current_files_for(&self, agent_id: &str) -> Option<&HashSet<String>> {
        self.current_files.get(agent_id)
    }

    /// Returns the number of in-flight triples currently tracked.
    #[must_use]
    pub fn in_flight_pair_count(&self) -> usize {
        self.in_flight_pairs.len()
    }
}

// =========================================================================
// Auto-emit helpers and detector loop
// =========================================================================

/// Hint appended to a forward-conflict warning when a conservative
/// cross-kind region match (a named region vs a line range) drove the
/// conflict. Teaches the user why the warning fired and how to narrow it.
pub const CROSS_KIND_HINT: &str = "Note: one side declared named regions and the other declared line ranges; \
     these always intersect conservatively. If you want narrower conflict matching, \
     both sides should use the same region kind.";

/// Renders one conflicting file for warning text: bare path for a
/// file-level conflict, or `path (regions: <list>)` when regions
/// intersected.
fn describe_file_conflict(fc: &FileConflict) -> String {
    if fc.regions.is_empty() {
        fc.path.clone()
    } else {
        let regions: Vec<String> = fc.regions.iter().map(Region::to_string).collect();
        format!("{} (regions: {})", fc.path, regions.join(", "))
    }
}

/// Builds a forward-conflict feedback error string addressed to one
/// publisher of an overlapping intent pair.
///
/// When any conflicting file carries intersecting regions, those regions are
/// named explicitly (kind + name, or range). When a cross-kind conservative
/// match contributed to the conflict, the [`CROSS_KIND_HINT`] is appended.
fn forward_conflict_error(other_agent: &str, files: &[FileConflict]) -> String {
    let list = files
        .iter()
        .map(describe_file_conflict)
        .collect::<Vec<_>>()
        .join("; ");
    let mut text = format!(
        "{CONFLICT_DETECTOR_TAG} forward conflict: agent {other_agent} also intends to modify: {list}",
    );
    if files.iter().any(|fc| fc.cross_kind) {
        text.push(' ');
        text.push_str(CROSS_KIND_HINT);
    }
    text
}

/// Builds an in-flight-conflict feedback error string addressed to one
/// toucher of a contested file.
fn in_flight_conflict_error(other_agent: &str, file: &str) -> String {
    format!(
        "{CONFLICT_DETECTOR_TAG} in-flight conflict: file {file} is being modified by both you and {other_agent}",
    )
}

/// Builds an ownership-violation feedback error string addressed to the
/// violator.
fn ownership_violation_error(file: &str, owner: &str) -> String {
    format!(
        "{CONFLICT_DETECTOR_TAG} ownership violation: you edited {file} but agent {owner} declared intent over it. Update your agent.intent to declare this file or back off.",
    )
}

/// Builds the in-flight-escalation question text.
fn in_flight_escalation_question(a: &str, b: &str, file: &str, window_secs: u64) -> String {
    format!(
        "{CONFLICT_DETECTOR_TAG} in-flight conflict on {file} between {a} and {b} has not resolved within {window_secs}s. Human input requested.",
    )
}

/// Builds the ownership-violation escalation question text.
fn ownership_escalation_question(violator: &str, file: &str, owner: &str) -> String {
    format!(
        "{CONFLICT_DETECTOR_TAG} ownership violation: {violator} edited {file} which is in {owner}'s intent. Human review requested.",
    )
}

/// Publishes an `agent.feedback` message addressed to `target_id` with a
/// single error string `error_text`. The message's `from` is
/// [`CONFLICT_DETECTOR_SENDER`] (always `"supervisor"`).
pub fn emit_feedback(state: &Arc<BrokerState>, target_id: &str, error_text: String) {
    let msg = BrokerMessage::Feedback {
        agent_id: target_id.to_string(),
        payload: FeedbackPayload {
            from: CONFLICT_DETECTOR_SENDER.to_string(),
            errors: vec![error_text],
        },
    };
    delivery::publish_message(state, &msg);
}

/// Publishes an `agent.question` message into the supervisor inbox. The
/// message's `agent_id` is `"supervisor"` (the recipient by convention).
pub fn emit_question(state: &Arc<BrokerState>, question_text: String) {
    let msg = BrokerMessage::Question {
        agent_id: CONFLICT_DETECTOR_SENDER.to_string(),
        payload: QuestionPayload {
            question: question_text,
        },
    };
    delivery::publish_message(state, &msg);
}

/// Process a single message through the tracker and emit any warnings
/// that the configured policy allows.
///
/// Returns the number of auto-emitted broker messages.
///
/// This is the per-message body of the detector loop, lifted into a
/// standalone function so it can be unit-tested without spawning a
/// tokio task.
pub fn process_message(
    state: &Arc<BrokerState>,
    tracker: &mut ConflictTracker,
    msg: &BrokerMessage,
    config: &ConflictConfig,
    now: Instant,
) -> usize {
    // Re-entrancy guard: ignore any message whose sender is the detector
    // itself. The detector publishes `Feedback` (from supervisor) and
    // `Question` (agent_id = supervisor), neither of which it should
    // re-process.
    if matches!(
        msg,
        BrokerMessage::Feedback { payload, .. } if payload.from == CONFLICT_DETECTOR_SENDER
    ) || matches!(
        msg,
        BrokerMessage::Question { agent_id, .. } if agent_id == CONFLICT_DETECTOR_SENDER
    ) {
        return 0;
    }

    let mut emitted = 0usize;
    // Drop expired intents up front so neither overlap check sees them.
    tracker.expire_stale_intents(now);

    match msg {
        BrokerMessage::Intent { agent_id, payload } => {
            let IntentPayload {
                files,
                summary,
                valid_for_seconds,
            } = payload.clone();
            let normalized: Vec<NormalizedFileIntent> =
                files.into_iter().map(NormalizedFileIntent::from).collect();
            tracker.insert_intent(
                agent_id,
                normalized,
                summary,
                Duration::from_secs(valid_for_seconds),
                now,
            );
            if config.warn_on_intent_overlap {
                for conflict in tracker.forward_overlaps(agent_id) {
                    if tracker.was_intent_pair_warned(agent_id, &conflict.other_agent) {
                        continue;
                    }
                    emit_feedback(
                        state,
                        agent_id,
                        forward_conflict_error(&conflict.other_agent, &conflict.files),
                    );
                    emit_feedback(
                        state,
                        &conflict.other_agent,
                        forward_conflict_error(agent_id, &conflict.files),
                    );
                    tracker.mark_intent_pair_warned(agent_id, &conflict.other_agent);
                    emitted += 2;
                }
            }
        }
        BrokerMessage::Status { agent_id, payload } => {
            let StatusPayload { modified_files, .. } = payload.clone();
            tracker.update_status(agent_id, modified_files);

            // 1. In-flight initial warnings — look for triples involving X.
            for (a, b, file) in tracker.in_flight_overlaps() {
                if a.as_str() != agent_id.as_str() && b.as_str() != agent_id.as_str() {
                    continue;
                }
                if tracker.record_in_flight_pair(&a, &b, &file, now) {
                    emit_feedback(state, &a, in_flight_conflict_error(&b, &file));
                    emit_feedback(state, &b, in_flight_conflict_error(&a, &file));
                    emitted += 2;
                }
            }

            // 2. In-flight resolution — drop any triple whose file
            //    intersection no longer includes the file (one agent
            //    stopped touching it).
            tracker.sweep_in_flight_pairs();

            // 3. Ownership violations for X.
            for (file, owner) in tracker.ownership_violations(agent_id) {
                if tracker.was_ownership_warned(agent_id, &file) {
                    continue;
                }
                emit_feedback(state, agent_id, ownership_violation_error(&file, &owner));
                emitted += 1;
                if config.escalate_on_violation {
                    emit_question(
                        state,
                        ownership_escalation_question(agent_id, &file, &owner),
                    );
                    emitted += 1;
                }
                tracker.mark_ownership_warned(agent_id, &file);
            }
        }
        _ => {}
    }

    emitted
}

/// Run a single tick of the periodic timer-driven detector logic:
/// expire stale intents, sweep resolved in-flight pairs, then emit any
/// escalations whose window has elapsed.
///
/// Returns the number of escalation messages emitted.
pub fn tick(
    state: &Arc<BrokerState>,
    tracker: &mut ConflictTracker,
    config: &ConflictConfig,
    now: Instant,
) -> usize {
    tracker.expire_stale_intents(now);
    tracker.sweep_in_flight_pairs();
    let window = Duration::from_secs(config.window_seconds);
    let mut emitted = 0usize;
    for (a, b, file) in tracker.take_due_escalations(window, now) {
        emit_question(
            state,
            in_flight_escalation_question(&a, &b, &file, config.window_seconds),
        );
        emitted += 1;
    }
    emitted
}

/// Spawns a background tokio task that drives the detector loop.
///
/// The task tails the broker message log via a sequence cursor (matching
/// the existing watcher-style pattern). On each poll interval it:
///
/// 1. Reads any new messages since the previous cursor and feeds each
///    through [`process_message`] (which handles forward/in-flight/owner
///    detection for that message).
/// 2. Runs [`tick`] to expire stale intents, sweep resolved in-flight
///    pairs, and emit escalations whose window has elapsed.
///
/// Exits cleanly when `shutdown` is flipped to `true`.
pub async fn run_detector_loop(
    state: Arc<BrokerState>,
    config: ConflictConfig,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut tracker = ConflictTracker::new();
    let mut cursor: u64 = 0;
    let mut ticker = tokio::time::interval(DETECTOR_TICK_INTERVAL);
    ticker.tick().await; // skip the immediate first tick
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
        }

        let now = Instant::now();

        // Pull every new message since the last cursor under the read
        // lock, then release before doing any further work.
        let batch = delivery::full_log(&state, cursor);
        for (seq, _ts, msg) in &batch {
            process_message(&state, &mut tracker, msg, &config, now);
            if *seq > cursor {
                cursor = *seq;
            }
        }

        tick(&state, &mut tracker, &config, now);
    }
}

/// Poll interval for the detector loop. Matches the watcher's cadence
/// to keep cross-subsystem timing predictable.
pub const DETECTOR_TICK_INTERVAL: Duration = Duration::from_millis(500);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::messages::{ArtifactPayload, IntentPayload, StatusPayload};

    fn fresh() -> ConflictTracker {
        ConflictTracker::new()
    }

    fn ttl_secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    /// File-level normalised intents (no regions) from a path list — the
    /// v0.5.0 shape, used by the tracker tests that exercise file-level
    /// behaviour.
    fn nfi(list: &[&str]) -> Vec<NormalizedFileIntent> {
        list.iter()
            .map(|s| NormalizedFileIntent {
                path: (*s).to_string(),
                regions: None,
            })
            .collect()
    }

    /// File-level `FileIntent` wire entries (plain strings) from a path list.
    fn fi(list: &[&str]) -> Vec<FileIntent> {
        list.iter().map(|s| FileIntent::from(*s)).collect()
    }

    fn func(name: &str) -> Region {
        Region::Function {
            name: name.to_string(),
        }
    }

    // Maps to scenario `Detector stops cleanly when broker stops` from
    // conflict-detection. Spawns the detector via its existing constructor
    // and asserts the task exits within one poll interval + slack after
    // the broker's shutdown signal flips. (test-coverage-v0-5-0 task 6.1)
    #[test]
    fn detector_stops_cleanly_on_broker_stop() {
        use tokio::time::Duration;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let state = Arc::new(BrokerState::new(None));
            let cfg = ConflictConfig::default();
            let (tx, rx) = tokio::sync::watch::channel(false);
            let handle = tokio::spawn(run_detector_loop(state, cfg, rx));

            // Mirror the broker drop path: flip the shutdown channel to true.
            tx.send(true).expect("shutdown send");

            let timed =
                tokio::time::timeout(DETECTOR_TICK_INTERVAL + Duration::from_millis(100), handle)
                    .await
                    .expect("detector task did not exit within poll interval + slack");
            timed.expect("detector task should not panic");
        });
    }

    fn fresh_state() -> Arc<BrokerState> {
        Arc::new(BrokerState::new(None))
    }

    fn intent_msg(agent_id: &str, files_list: &[&str], summary: &str, ttl: u64) -> BrokerMessage {
        BrokerMessage::Intent {
            agent_id: agent_id.to_string(),
            payload: IntentPayload {
                files: fi(files_list),
                summary: summary.to_string(),
                valid_for_seconds: ttl,
            },
        }
    }

    /// Builds an intent message whose `files` carry explicit region-bearing
    /// entries. Each `(path, regions)` pair becomes a `FileIntent::Detailed`.
    fn intent_msg_with_regions(
        agent_id: &str,
        files_list: &[(&str, Vec<Region>)],
        summary: &str,
        ttl: u64,
    ) -> BrokerMessage {
        BrokerMessage::Intent {
            agent_id: agent_id.to_string(),
            payload: IntentPayload {
                files: files_list
                    .iter()
                    .map(|(path, regions)| FileIntent::Detailed {
                        path: (*path).to_string(),
                        regions: regions.clone(),
                    })
                    .collect(),
                summary: summary.to_string(),
                valid_for_seconds: ttl,
            },
        }
    }

    fn status_msg(agent_id: &str, files_list: &[&str]) -> BrokerMessage {
        BrokerMessage::Status {
            agent_id: agent_id.to_string(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: files(files_list),
                message: None,
                ..Default::default()
            },
        }
    }

    fn supervisor_feedbacks_in_inbox(state: &Arc<BrokerState>, target: &str) -> Vec<BrokerMessage> {
        let (msgs, _) = delivery::poll_messages(state, target, 0);
        msgs.into_iter()
            .filter(|m| {
                matches!(
                    m,
                    BrokerMessage::Feedback { payload, .. }
                        if payload.from == CONFLICT_DETECTOR_SENDER
                )
            })
            .collect()
    }

    fn supervisor_questions(state: &Arc<BrokerState>) -> Vec<BrokerMessage> {
        let (msgs, _) = delivery::poll_messages(state, "supervisor", 0);
        msgs.into_iter()
            .filter(|m| matches!(m, BrokerMessage::Question { .. }))
            .collect()
    }

    fn default_config() -> ConflictConfig {
        ConflictConfig::default()
    }

    // ====================================================================
    // Tracker unit tests (task 2.5)
    // ====================================================================

    #[test]
    fn tracker_insert_intent_records_files() {
        let mut t = fresh();
        let now = Instant::now();
        t.insert_intent(
            "feat-x",
            nfi(&["src/a.rs", "src/b.rs"]),
            "x".into(),
            ttl_secs(60),
            now,
        );
        let r = t.intent_for("feat-x").unwrap();
        assert!(r.files.contains_key("src/a.rs"));
        assert!(r.files.contains_key("src/b.rs"));
        assert_eq!(r.valid_for, ttl_secs(60));
    }

    #[test]
    fn tracker_insert_intent_replaces_prior_intent() {
        let mut t = fresh();
        let now = Instant::now();
        t.insert_intent(
            "feat-x",
            nfi(&["src/a.rs"]),
            "old".into(),
            ttl_secs(60),
            now,
        );
        t.insert_intent(
            "feat-x",
            nfi(&["src/a.rs", "src/b.rs"]),
            "new".into(),
            ttl_secs(60),
            now,
        );
        let r = t.intent_for("feat-x").unwrap();
        assert_eq!(r.summary, "new");
        assert_eq!(r.files.len(), 2);
    }

    #[test]
    fn tracker_expire_stale_intents_drops_aged_entries() {
        let mut t = fresh();
        let now = Instant::now();
        t.insert_intent("feat-x", nfi(&["a"]), "x".into(), ttl_secs(1), now);
        let later = now + Duration::from_secs(2);
        t.expire_stale_intents(later);
        assert!(t.intent_for("feat-x").is_none());
    }

    #[test]
    fn tracker_forward_overlaps_returns_overlap_files() {
        let mut t = fresh();
        let now = Instant::now();
        t.insert_intent("feat-x", nfi(&["a", "b"]), "x".into(), ttl_secs(60), now);
        t.insert_intent("feat-y", nfi(&["b", "c"]), "y".into(), ttl_secs(60), now);
        let overlaps = t.forward_overlaps("feat-x");
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].other_agent, "feat-y");
        // File-level conflict (no regions declared on either side): one
        // conflicting file, "b", with empty regions.
        assert_eq!(overlaps[0].files.len(), 1);
        assert_eq!(overlaps[0].files[0].path, "b");
        assert!(overlaps[0].files[0].regions.is_empty());
    }

    #[test]
    fn tracker_intent_pair_dedupe_is_ordered() {
        let mut t = fresh();
        assert!(!t.was_intent_pair_warned("feat-y", "feat-x"));
        t.mark_intent_pair_warned("feat-x", "feat-y");
        assert!(t.was_intent_pair_warned("feat-x", "feat-y"));
        assert!(t.was_intent_pair_warned("feat-y", "feat-x"));
    }

    #[test]
    fn tracker_insert_intent_clears_prior_pair_dedupe() {
        let mut t = fresh();
        let now = Instant::now();
        t.insert_intent("feat-x", nfi(&["a"]), "x".into(), ttl_secs(60), now);
        t.insert_intent("feat-y", nfi(&["a"]), "y".into(), ttl_secs(60), now);
        t.mark_intent_pair_warned("feat-x", "feat-y");
        assert!(t.was_intent_pair_warned("feat-x", "feat-y"));
        // New intent from x must clear the pair entry so subsequent overlaps re-warn.
        t.insert_intent("feat-x", nfi(&["a", "b"]), "x2".into(), ttl_secs(60), now);
        assert!(!t.was_intent_pair_warned("feat-x", "feat-y"));
    }

    #[test]
    fn tracker_in_flight_overlaps_returns_intersected_files() {
        let mut t = fresh();
        t.update_status("feat-x", files(&["src/a.rs", "src/b.rs"]));
        t.update_status("feat-y", files(&["src/a.rs"]));
        let pairs = t.in_flight_overlaps();
        assert_eq!(pairs.len(), 1);
        assert_eq!(
            pairs[0],
            (
                "feat-x".to_string(),
                "feat-y".to_string(),
                "src/a.rs".to_string()
            )
        );
    }

    #[test]
    fn tracker_record_in_flight_pair_returns_true_only_first_time() {
        let mut t = fresh();
        let now = Instant::now();
        assert!(t.record_in_flight_pair("feat-x", "feat-y", "src/a.rs", now));
        assert!(!t.record_in_flight_pair("feat-y", "feat-x", "src/a.rs", now));
        // Sweep removes triples whose file is no longer in the intersection.
        t.update_status("feat-x", files(&["src/b.rs"]));
        t.update_status("feat-y", files(&["src/a.rs"]));
        t.sweep_in_flight_pairs();
        assert!(t.record_in_flight_pair("feat-x", "feat-y", "src/a.rs", now));
    }

    #[test]
    fn tracker_take_due_escalations_returns_aged_triples_once() {
        let mut t = fresh();
        let now = Instant::now();
        t.record_in_flight_pair("feat-x", "feat-y", "f", now);
        let window = Duration::from_mins(2);
        // Too soon — nothing returned.
        let out = t.take_due_escalations(window, now + Duration::from_secs(10));
        assert!(out.is_empty());
        let due = now + Duration::from_mins(2) + Duration::from_secs(1);
        let out = t.take_due_escalations(window, due);
        assert_eq!(out.len(), 1);
        // Second call after marking — escalation is sticky.
        let out2 = t.take_due_escalations(window, due);
        assert!(out2.is_empty());
    }

    #[test]
    fn tracker_ownership_violations_file_inside_other_intent() {
        let mut t = fresh();
        let now = Instant::now();
        t.insert_intent("feat-x", nfi(&["src/a.rs"]), "x".into(), ttl_secs(60), now);
        t.update_status("feat-y", files(&["src/a.rs"]));
        let v = t.ownership_violations("feat-y");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0], ("src/a.rs".to_string(), "feat-x".to_string()));
    }

    #[test]
    fn tracker_ownership_violations_inside_own_intent_is_ok() {
        let mut t = fresh();
        let now = Instant::now();
        t.insert_intent("feat-y", nfi(&["src/a.rs"]), "y".into(), ttl_secs(60), now);
        t.update_status("feat-y", files(&["src/a.rs"]));
        assert!(t.ownership_violations("feat-y").is_empty());
    }

    #[test]
    fn tracker_ownership_violations_unclaimed_file_is_ok() {
        let mut t = fresh();
        t.update_status("feat-y", files(&["src/orphan.rs"]));
        assert!(t.ownership_violations("feat-y").is_empty());
    }

    // ====================================================================
    // Detector behavior tests (task 4)
    // ====================================================================

    #[test]
    fn detector_forward_conflict_happy_path() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        // Ensure both inboxes exist (delivery skips unregistered queues).
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));

        let now = Instant::now();
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs", "src/b.rs"], "x", 600),
            &default_config(),
            now,
        );
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/b.rs", "src/c.rs"], "y", 600),
            &default_config(),
            now,
        );

        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        let y_fb = supervisor_feedbacks_in_inbox(&state, "feat-y");
        assert_eq!(
            x_fb.len(),
            1,
            "feat-x should have one forward-conflict feedback"
        );
        assert_eq!(y_fb.len(), 1);
        // Text contains the tag, peer agent_id, and the overlap file.
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            let err = &payload.errors[0];
            assert!(err.starts_with(CONFLICT_DETECTOR_TAG));
            assert!(err.contains("forward conflict"));
            assert!(err.contains("feat-y"));
            assert!(err.contains("src/b.rs"));
        } else {
            panic!("expected Feedback");
        }
    }

    #[test]
    fn detector_forward_conflict_dedupe() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let cfg = default_config();
        let now = Instant::now();
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs"], "x", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/a.rs"], "y", 600),
            &cfg,
            now,
        );
        // Re-publishing the *same* intent must not re-emit. (The
        // tracker's clear-on-insert behaviour clears pair dedupe on
        // replace; here we use a fresh duplicate of the same agent's
        // intent which should leave dedupe in place because the pair was
        // already warned via the prior pair, not via the agent itself.
        // To make this deterministic, simulate a no-op re-publish from a
        // *new* x intent that is identical to the prior one: the
        // dedupe is cleared, so the rule we test is at the message level
        // — re-publishing the same intent message body does NOT re-emit
        // because nothing changed.)
        let before_x = supervisor_feedbacks_in_inbox(&state, "feat-x").len();
        let before_y = supervisor_feedbacks_in_inbox(&state, "feat-y").len();
        // Re-publish feat-y's identical intent.
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/a.rs"], "y", 600),
            &cfg,
            now,
        );
        let after_x = supervisor_feedbacks_in_inbox(&state, "feat-x").len();
        let after_y = supervisor_feedbacks_in_inbox(&state, "feat-y").len();
        assert_eq!(
            before_x, after_x,
            "no new feedback to x on identical re-publish"
        );
        assert_eq!(before_y, after_y);
    }

    #[test]
    fn detector_forward_conflict_suppression_when_disabled() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let cfg = ConflictConfig {
            warn_on_intent_overlap: false,
            ..ConflictConfig::default()
        };
        let now = Instant::now();
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs"], "x", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/a.rs"], "y", 600),
            &cfg,
            now,
        );
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-x").is_empty());
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-y").is_empty());
        // Tracker still has both intents — needed for in-flight + ownership detection.
        assert!(t.intent_for("feat-x").is_some());
        assert!(t.intent_for("feat-y").is_some());
    }

    #[test]
    fn detector_forward_conflict_non_overlap_no_warnings() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = default_config();
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs"], "x", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/b.rs"], "y", 600),
            &cfg,
            now,
        );
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-x").is_empty());
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-y").is_empty());
    }

    #[test]
    fn detector_self_replace_no_self_conflict() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        let now = Instant::now();
        let cfg = default_config();
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs"], "x", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs", "src/b.rs"], "x2", 600),
            &cfg,
            now,
        );
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-x").is_empty());
    }

    #[test]
    fn detector_ttl_expired_intent_does_not_overlap() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = default_config();
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs"], "x", 1),
            &cfg,
            now,
        );
        // Wait past TTL.
        let later = now + Duration::from_secs(5);
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/a.rs"], "y", 600),
            &cfg,
            later,
        );
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-x").is_empty());
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-y").is_empty());
    }

    #[test]
    fn detector_in_flight_initial_warning() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = default_config();
        process_message(
            &state,
            &mut t,
            &status_msg("feat-x", &["src/a.rs"]),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            now,
        );
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        let y_fb = supervisor_feedbacks_in_inbox(&state, "feat-y");
        assert_eq!(x_fb.len(), 1);
        assert_eq!(y_fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            assert!(payload.errors[0].contains("in-flight conflict"));
            assert!(payload.errors[0].contains("src/a.rs"));
            assert!(payload.errors[0].starts_with(CONFLICT_DETECTOR_TAG));
        }
    }

    #[test]
    fn detector_in_flight_escalation_after_window() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = ConflictConfig {
            window_seconds: 5,
            ..ConflictConfig::default()
        };
        process_message(
            &state,
            &mut t,
            &status_msg("feat-x", &["src/a.rs"]),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            now,
        );
        // Time advances past the window — tick should emit one question.
        let due = now + Duration::from_secs(10);
        let emitted = tick(&state, &mut t, &cfg, due);
        assert_eq!(emitted, 1);
        let q = supervisor_questions(&state);
        assert_eq!(q.len(), 1);
        if let BrokerMessage::Question { payload, .. } = &q[0] {
            assert!(payload.question.contains(CONFLICT_DETECTOR_TAG));
            assert!(payload.question.contains("src/a.rs"));
            assert!(payload.question.contains("feat-x"));
            assert!(payload.question.contains("feat-y"));
        }
    }

    #[test]
    fn detector_in_flight_escalation_dedupe() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = ConflictConfig {
            window_seconds: 5,
            ..ConflictConfig::default()
        };
        process_message(
            &state,
            &mut t,
            &status_msg("feat-x", &["src/a.rs"]),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            now,
        );
        let due = now + Duration::from_secs(10);
        tick(&state, &mut t, &cfg, due);
        // Subsequent tick while still overlapping must not re-emit.
        let later = due + Duration::from_secs(10);
        let emitted = tick(&state, &mut t, &cfg, later);
        assert_eq!(emitted, 0);
        let q = supervisor_questions(&state);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn detector_in_flight_resolution_drops_triple() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = ConflictConfig {
            window_seconds: 5,
            ..ConflictConfig::default()
        };
        process_message(
            &state,
            &mut t,
            &status_msg("feat-x", &["src/a.rs"]),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            now,
        );
        assert_eq!(t.in_flight_pair_count(), 1);
        // X stops touching the file.
        process_message(&state, &mut t, &status_msg("feat-x", &[]), &cfg, now);
        assert_eq!(t.in_flight_pair_count(), 0);
        let due = now + Duration::from_secs(10);
        let emitted = tick(&state, &mut t, &cfg, due);
        assert_eq!(emitted, 0, "no escalation for a resolved conflict");
    }

    #[test]
    fn detector_ownership_violation_emits_feedback_and_question() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = ConflictConfig {
            // disable forward warning to isolate ownership behaviour
            warn_on_intent_overlap: false,
            ..ConflictConfig::default()
        };
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs"], "x", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/b.rs"], "y", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            now,
        );
        let y_fb = supervisor_feedbacks_in_inbox(&state, "feat-y");
        assert_eq!(y_fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = &y_fb[0] {
            assert!(payload.errors[0].contains("ownership violation"));
            assert!(payload.errors[0].contains("src/a.rs"));
            assert!(payload.errors[0].contains("feat-x"));
        }
        let q = supervisor_questions(&state);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn detector_ownership_escalation_suppression() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = ConflictConfig {
            warn_on_intent_overlap: false,
            escalate_on_violation: false,
            ..ConflictConfig::default()
        };
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs"], "x", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            now,
        );
        // Feedback still fires.
        assert_eq!(supervisor_feedbacks_in_inbox(&state, "feat-y").len(), 1);
        // No question to supervisor.
        assert!(supervisor_questions(&state).is_empty());
    }

    #[test]
    fn detector_ownership_file_inside_own_intent_no_violation() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = default_config();
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/a.rs"], "y", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            now,
        );
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-y").is_empty());
        assert!(supervisor_questions(&state).is_empty());
    }

    #[test]
    fn detector_ownership_unclaimed_file_no_violation() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = default_config();
        // feat-y has no intent at all.
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/orphan.rs"]),
            &cfg,
            now,
        );
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-y").is_empty());
        assert!(supervisor_questions(&state).is_empty());
    }

    #[test]
    fn detector_ownership_violation_dedupe() {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = ConflictConfig {
            warn_on_intent_overlap: false,
            ..ConflictConfig::default()
        };
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-x", &["src/a.rs"], "x", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            now,
        );
        let first = supervisor_feedbacks_in_inbox(&state, "feat-y").len();
        // Second status from same violator on same file.
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            now,
        );
        let second = supervisor_feedbacks_in_inbox(&state, "feat-y").len();
        assert_eq!(
            first, second,
            "no new ownership feedback on repeated status"
        );
    }

    #[test]
    fn detector_filters_own_emissions() {
        // Re-entrancy guard — feedback/question emitted with from="supervisor"
        // (or agent_id="supervisor") must not be re-processed.
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        let now = Instant::now();
        let cfg = default_config();
        let detector_feedback = BrokerMessage::Feedback {
            agent_id: "feat-x".into(),
            payload: FeedbackPayload {
                from: CONFLICT_DETECTOR_SENDER.into(),
                errors: vec![format!("{CONFLICT_DETECTOR_TAG} test")],
            },
        };
        let emitted = process_message(&state, &mut t, &detector_feedback, &cfg, now);
        assert_eq!(emitted, 0);
        let detector_question = BrokerMessage::Question {
            agent_id: CONFLICT_DETECTOR_SENDER.into(),
            payload: QuestionPayload {
                question: format!("{CONFLICT_DETECTOR_TAG} test"),
            },
        };
        let emitted = process_message(&state, &mut t, &detector_question, &cfg, now);
        assert_eq!(emitted, 0);
    }

    #[test]
    fn detector_ignores_artifact_messages_for_warnings() {
        // Artifacts don't drive the detector (only Intent + Status do).
        // Confirms forward-coordination's broadcast pattern isn't
        // accidentally tripping warnings.
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        let now = Instant::now();
        let cfg = default_config();
        let artifact = BrokerMessage::Artifact {
            agent_id: "feat-x".into(),
            payload: ArtifactPayload {
                status: "done".into(),
                exports: vec![],
                modified_files: vec!["src/a.rs".into()],
            },
        };
        let emitted = process_message(&state, &mut t, &artifact, &cfg, now);
        assert_eq!(emitted, 0);
    }

    // ====================================================================
    // Auto-emit message conventions (task 4 / spec scenarios)
    // ====================================================================

    #[test]
    fn auto_emitted_feedback_uses_supervisor_from_and_conflict_tag() {
        let state = fresh_state();
        // Recipient must have a registered inbox or delivery silently drops.
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        emit_feedback(&state, "feat-x", "[conflict-detector] something".into());
        let (msgs, _) = delivery::poll_messages(&state, "feat-x", 0);
        let fb: Vec<&BrokerMessage> = msgs
            .iter()
            .filter(|m| matches!(m, BrokerMessage::Feedback { .. }))
            .collect();
        assert_eq!(fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = fb[0] {
            assert_eq!(payload.from, CONFLICT_DETECTOR_SENDER);
            assert!(payload.errors[0].starts_with(CONFLICT_DETECTOR_TAG));
        } else {
            panic!("expected Feedback");
        }
    }

    #[test]
    fn auto_emitted_question_targets_supervisor_inbox_with_tag() {
        let state = fresh_state();
        emit_question(&state, "[conflict-detector] test".into());
        let (msgs, _) = delivery::poll_messages(&state, "supervisor", 0);
        assert_eq!(msgs.len(), 1);
        if let BrokerMessage::Question { agent_id, payload } = &msgs[0] {
            assert_eq!(agent_id, "supervisor");
            assert!(payload.question.contains(CONFLICT_DETECTOR_TAG));
        } else {
            panic!("expected Question");
        }
    }

    // ====================================================================
    // NormalizedFileIntent conversion (task 2.2)
    // ====================================================================

    #[test]
    fn normalized_from_path_has_no_regions() {
        let n: NormalizedFileIntent = FileIntent::Path("src/a.rs".to_string()).into();
        assert_eq!(n.path, "src/a.rs");
        assert_eq!(n.regions, None);
    }

    #[test]
    fn normalized_from_detailed_with_regions_is_some() {
        let n: NormalizedFileIntent = FileIntent::Detailed {
            path: "src/a.rs".to_string(),
            regions: vec![func("validate_token")],
        }
        .into();
        assert_eq!(n.regions, Some(vec![func("validate_token")]));
    }

    #[test]
    fn normalized_from_detailed_empty_regions_collapses_to_none() {
        let n: NormalizedFileIntent = FileIntent::Detailed {
            path: "src/a.rs".to_string(),
            regions: vec![],
        }
        .into();
        assert_eq!(
            n.regions, None,
            "an empty regions vec is equivalent to no regions"
        );
    }

    // ====================================================================
    // regions_intersect unit coverage (task 3.3)
    // ====================================================================

    #[test]
    fn regions_intersect_same_function_name() {
        let (hits, cross) = regions_intersect(&[func("a")], &[func("a")]);
        assert_eq!(hits, vec![func("a")]);
        assert!(!cross);
    }

    #[test]
    fn regions_intersect_different_function_names_empty() {
        let (hits, cross) = regions_intersect(&[func("a")], &[func("b")]);
        assert!(hits.is_empty());
        assert!(!cross);
    }

    #[test]
    fn regions_intersect_different_named_kinds_empty() {
        // function "a" vs class "a" — different kinds, no intersection.
        let class_a = Region::Class {
            name: "a".to_string(),
        };
        let (hits, cross) = regions_intersect(&[func("a")], &[class_a]);
        assert!(hits.is_empty());
        assert!(!cross);
    }

    #[test]
    fn regions_intersect_overlapping_ranges() {
        let r1 = Region::Range {
            start_line: 10,
            end_line: 30,
        };
        let r2 = Region::Range {
            start_line: 25,
            end_line: 45,
        };
        let (hits, cross) = regions_intersect(&[r1], &[r2]);
        assert_eq!(
            hits,
            vec![Region::Range {
                start_line: 25,
                end_line: 30
            }]
        );
        assert!(!cross);
    }

    #[test]
    fn regions_intersect_non_overlapping_ranges_empty() {
        let r1 = Region::Range {
            start_line: 10,
            end_line: 20,
        };
        let r2 = Region::Range {
            start_line: 30,
            end_line: 40,
        };
        let (hits, _) = regions_intersect(&[r1], &[r2]);
        assert!(hits.is_empty());
    }

    #[test]
    fn regions_intersect_cross_kind_is_conservative() {
        let range = Region::Range {
            start_line: 10,
            end_line: 50,
        };
        let (hits, cross) =
            regions_intersect(&[func("validate_token")], std::slice::from_ref(&range));
        assert!(cross, "named-vs-range must flag cross_kind");
        assert!(hits.contains(&func("validate_token")));
        assert!(hits.contains(&range));
    }

    // ====================================================================
    // Region-aware detector scenarios (task 3.6 / spec.md)
    // ====================================================================

    fn run_two_intents(
        a: &BrokerMessage,
        b: &BrokerMessage,
    ) -> (Arc<BrokerState>, ConflictTracker) {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        let cfg = default_config();
        process_message(&state, &mut t, a, &cfg, now);
        process_message(&state, &mut t, b, &cfg, now);
        (state, t)
    }

    #[test]
    fn detector_non_overlapping_functions_no_conflict() {
        // Spec: Non-overlapping functions in the same file do not conflict.
        let a = intent_msg_with_regions(
            "feat-x",
            &[("src/auth.rs", vec![func("validate_token")])],
            "x",
            600,
        );
        let b = intent_msg_with_regions(
            "feat-y",
            &[("src/auth.rs", vec![func("refresh_session")])],
            "y",
            600,
        );
        let (state, _) = run_two_intents(&a, &b);
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-x").is_empty());
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-y").is_empty());
    }

    #[test]
    fn detector_overlapping_functions_conflict_names_function() {
        // Spec: Overlapping functions in the same file conflict; warning names
        // the intersecting function.
        let a = intent_msg_with_regions(
            "feat-x",
            &[("src/auth.rs", vec![func("validate_token")])],
            "x",
            600,
        );
        let b = intent_msg_with_regions(
            "feat-y",
            &[("src/auth.rs", vec![func("validate_token")])],
            "y",
            600,
        );
        let (state, _) = run_two_intents(&a, &b);
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        assert_eq!(x_fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            let err = &payload.errors[0];
            assert!(err.contains("forward conflict"));
            assert!(err.contains("feat-y"));
            assert!(err.contains("function validate_token"));
            assert!(err.contains("src/auth.rs"));
        } else {
            panic!("expected Feedback");
        }
        assert_eq!(supervisor_feedbacks_in_inbox(&state, "feat-y").len(), 1);
    }

    #[test]
    fn detector_file_level_fallback_when_one_side_omits_regions() {
        // Spec: File-level fallback when regions omitted. A declares regions,
        // B is a plain string → file-level conflict.
        let a = intent_msg_with_regions(
            "feat-x",
            &[("src/auth.rs", vec![func("validate_token")])],
            "x",
            600,
        );
        let b = intent_msg("feat-y", &["src/auth.rs"], "y", 600);
        let (state, _) = run_two_intents(&a, &b);
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        assert_eq!(x_fb.len(), 1, "file-level fallback must still warn");
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            // File-level conflict: names the path, no region detail.
            assert!(payload.errors[0].contains("src/auth.rs"));
            assert!(!payload.errors[0].contains("(regions:"));
        }
    }

    #[test]
    fn detector_cross_kind_conflict_includes_hint() {
        // Spec: Cross-kind comparison intersects conservatively + hint.
        let range = Region::Range {
            start_line: 10,
            end_line: 50,
        };
        let a = intent_msg_with_regions(
            "feat-x",
            &[("src/auth.rs", vec![func("validate_token")])],
            "x",
            600,
        );
        let b = intent_msg_with_regions("feat-y", &[("src/auth.rs", vec![range])], "y", 600);
        let (state, _) = run_two_intents(&a, &b);
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        assert_eq!(x_fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            assert!(
                payload.errors[0].contains(CROSS_KIND_HINT),
                "cross-kind conflict must include the hint; got: {}",
                payload.errors[0]
            );
        }
    }

    #[test]
    fn detector_overlapping_ranges_conflict() {
        // Spec: Overlapping ranges intersect.
        let r1 = Region::Range {
            start_line: 10,
            end_line: 30,
        };
        let r2 = Region::Range {
            start_line: 25,
            end_line: 45,
        };
        let a = intent_msg_with_regions("feat-x", &[("src/auth.rs", vec![r1])], "x", 600);
        let b = intent_msg_with_regions("feat-y", &[("src/auth.rs", vec![r2])], "y", 600);
        let (state, _) = run_two_intents(&a, &b);
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        assert_eq!(x_fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            assert!(payload.errors[0].contains("range 25-30"));
        }
    }

    #[test]
    fn detector_non_overlapping_ranges_no_conflict() {
        // Spec: Non-overlapping ranges do not intersect.
        let r1 = Region::Range {
            start_line: 10,
            end_line: 20,
        };
        let r2 = Region::Range {
            start_line: 30,
            end_line: 40,
        };
        let a = intent_msg_with_regions("feat-x", &[("src/auth.rs", vec![r1])], "x", 600);
        let b = intent_msg_with_regions("feat-y", &[("src/auth.rs", vec![r2])], "y", 600);
        let (state, _) = run_two_intents(&a, &b);
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-x").is_empty());
        assert!(supervisor_feedbacks_in_inbox(&state, "feat-y").is_empty());
    }

    #[test]
    fn detector_warning_enumerates_multiple_intersecting_regions() {
        // Spec: Detector warning identifies intersecting regions — both
        // functions named.
        let a = intent_msg_with_regions(
            "feat-x",
            &[(
                "src/auth.rs",
                vec![func("validate_token"), func("refresh_session")],
            )],
            "x",
            600,
        );
        let b = intent_msg_with_regions(
            "feat-y",
            &[(
                "src/auth.rs",
                vec![func("validate_token"), func("refresh_session")],
            )],
            "y",
            600,
        );
        let (state, _) = run_two_intents(&a, &b);
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        assert_eq!(x_fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            assert!(payload.errors[0].contains("function validate_token"));
            assert!(payload.errors[0].contains("function refresh_session"));
        }
    }

    #[test]
    fn detector_v050_string_only_intents_behave_file_level() {
        // Backwards compatibility: string-only intents (v0.5.0 shape) warn at
        // file level exactly as before regions existed.
        let a = intent_msg("feat-x", &["src/foo.rs", "src/bar.rs"], "x", 600);
        let b = intent_msg("feat-y", &["src/bar.rs"], "y", 600);
        let (state, _) = run_two_intents(&a, &b);
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        assert_eq!(x_fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            assert!(payload.errors[0].contains("src/bar.rs"));
            assert!(!payload.errors[0].contains("(regions:"));
        }
    }

    #[test]
    fn detector_region_conflict_only_on_intersecting_file() {
        // Two shared files: one with overlapping regions, one with disjoint
        // regions. Only the intersecting file appears in the warning.
        let a = intent_msg_with_regions(
            "feat-x",
            &[
                ("src/auth.rs", vec![func("validate_token")]),
                ("src/db.rs", vec![func("connect")]),
            ],
            "x",
            600,
        );
        let b = intent_msg_with_regions(
            "feat-y",
            &[
                ("src/auth.rs", vec![func("validate_token")]),
                ("src/db.rs", vec![func("migrate")]),
            ],
            "y",
            600,
        );
        let (state, _) = run_two_intents(&a, &b);
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        assert_eq!(x_fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            assert!(payload.errors[0].contains("src/auth.rs"));
            assert!(
                !payload.errors[0].contains("src/db.rs"),
                "db.rs has disjoint functions and must not appear: {}",
                payload.errors[0]
            );
        }
    }
}
