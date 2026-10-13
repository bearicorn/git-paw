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
//!   stops touching the file within `[supervisor.conflict] window_seconds`,
//!   the overlap is classified from the two agents' declared intent regions
//!   for the file: a **true** collision (both declared regions that
//!   intersect, or either side declared no regions / no active intent for
//!   the file) escalates via `agent.question` to the supervisor inbox,
//!   while an **additive** overlap (both declared disjoint regions) is
//!   downgraded to an informational `agent.feedback` ("shared file,
//!   additive — resolve at merge") and never reaches the human. Either
//!   decision marks the triple handled once; the triple stays tracked until
//!   an agent stops touching the file, so the overlap is recorded, not
//!   dropped. The decision itself is held separately from that live view
//!   and outlives it, so a commit — which empties `modified_files` for a
//!   tick — cannot make an already-escalated conflict escalate a second
//!   time or rewind its clock. An orchestrator that has adjudicated an
//!   escalation can say so by publishing an `agent.feedback` error of the
//!   form `[conflict-ack] <agent_a> <agent_b> <file>`, which suppresses
//!   further `agent.question`s for that pair and file while leaving the
//!   overlap visible. Both the decision and the acknowledgement are
//!   invalidated if the two agents' region declarations for the file later
//!   describe a materially different overlap.
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
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::messages::{
    BrokerMessage, FeedbackPayload, FileIntent, IntentPayload, QuestionPayload, Region,
    StatusPayload,
};
use super::{BrokerState, delivery};
use crate::agents::is_managed_path;
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
    /// cross-kind match (a named region vs a line range, or named regions
    /// of differing kinds with matching normalized names).
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

/// Deterministically normalizes a declared region name so spelling variants
/// of the same symbol compare equal: case-folded, trimmed, a trailing `()`
/// stripped, a leading declaration keyword (`fn`, `def`, `function`,
/// `class`) stripped, and separator runs (space, underscore, hyphen)
/// collapsed to a single `_`.
///
/// Normalization is deliberately rule-based rather than fuzzy: every step
/// maps to an observed declaration habit (`validate_token()` vs
/// `Validate Token` vs `fn validate_token`), so a match is always
/// explainable. Genuinely distinct symbols (`foo_bar` vs `foobar`) stay
/// distinct.
fn normalize_region_name(name: &str) -> String {
    let lowered = name.trim().to_lowercase();
    // Strip one trailing `()` — `validate_token()` names the same symbol
    // as `validate_token`.
    let stripped = lowered.strip_suffix("()").unwrap_or(&lowered).trim_end();
    // Strip one leading declaration keyword when a separator follows it
    // (`fn validate_token`, `class Config`). A bare keyword or a name that
    // merely starts with one (`classify`) is left alone.
    let mut rest = stripped;
    for kw in ["function", "class", "def", "fn"] {
        if let Some(tail) = rest.strip_prefix(kw)
            && tail.starts_with([' ', '\t', '_', '-'])
        {
            rest = tail;
            break;
        }
    }
    // Collapse each separator run to a single `_`, dropping leading and
    // trailing runs.
    let mut out = String::with_capacity(rest.len());
    let mut pending_sep = false;
    for c in rest.chars() {
        if matches!(c, ' ' | '\t' | '_' | '-') {
            pending_sep = true;
        } else {
            if pending_sep && !out.is_empty() {
                out.push('_');
            }
            pending_sep = false;
            out.push(c);
        }
    }
    out
}

/// Computes the intersection of two region sets per the detector rules:
///
/// - Same kind + matching name (function / class / block) → intersect,
///   contributing that region. Names are compared NORMALIZED (see
///   [`normalize_region_name`]) so spelling variants of one symbol match.
/// - Two ranges with overlapping intervals → intersect, contributing the
///   overlapping sub-range.
/// - A named region vs a range (either order) → conservative intersect
///   (we can't resolve a name to lines without source parsing); contributes
///   both regions and sets the cross-kind flag.
/// - Named regions of DIFFERING kinds with matching normalized names →
///   conservative intersect (almost always the same construct declared
///   through different lenses); contributes both regions and sets the
///   cross-kind flag.
/// - Same kind, or differing named kinds, with differing normalized names →
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
                    if normalize_region_name(n1) == normalize_region_name(n2) =>
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
                // Cross-kind: named regions of DIFFERING kinds whose
                // normalized names match — `function foo` vs `block foo` is
                // almost always the same construct declared through
                // different lenses, so intersect conservatively and name
                // both regions. (Same-kind matches were consumed by the
                // first arm, so only genuine cross-kind pairs reach this.)
                (
                    Region::Function { name: n1 }
                    | Region::Class { name: n1 }
                    | Region::Block { anchor: n1 },
                    Region::Function { name: n2 }
                    | Region::Class { name: n2 }
                    | Region::Block { anchor: n2 },
                ) if normalize_region_name(n1) == normalize_region_name(n2) => {
                    cross_kind = true;
                    push(ra.clone(), &mut hits);
                    push(rb.clone(), &mut hits);
                }
                // Same-shape named regions with differing names, or differing
                // named kinds with differing names: no intersection.
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

/// Durable per-`(min_agent, max_agent, file)` record of what the detector
/// has decided about a conflict.
///
/// Deliberately separate from the in-flight tracker's live overlap view: the
/// overlap is as volatile as `modified_files` (a commit empties the set for a
/// tick), while the decision is not — once a conflict has been escalated to a
/// human, or adjudicated by an orchestrator, that stays true across the
/// commit. Keeping the decision on the tracker entry made a commit discard it
/// and the identical escalation fire again.
#[derive(Debug, Clone, Default)]
struct ConflictDecision {
    /// When the overlap was first observed in the live view. Lives here
    /// rather than on the live view itself so a transient empty
    /// `modified_files` cannot rewind the escalation clock.
    ///
    /// `None` while no overlap has been observed yet — a record can exist
    /// before that, because an orchestrator may acknowledge a conflict it
    /// anticipates. The window is measured from the first real sighting, not
    /// from the record's creation.
    first_seen: Option<Instant>,
    /// Set once the two agents have been warned that they share the file.
    /// Tracked separately from the record's existence so a pre-emptive
    /// acknowledgement cannot swallow the initial warning.
    warned: bool,
    /// Set once this conflict's escalation decision has been made and acted
    /// on — a `True` collision emitted an `agent.question`, or an `Additive`
    /// overlap emitted a downgrade `agent.feedback`. Once set, later ticks
    /// neither re-escalate nor re-downgrade while the declarations are
    /// unchanged.
    decided: Option<InFlightClass>,
    /// Set when an orchestrator has adjudicated the conflict (see
    /// [`ConflictTracker::acknowledge`]). While it stands, no further
    /// `agent.question` is emitted for the pair and file.
    acknowledged: bool,
    /// The overlap's shape under the existing intersection rules at the
    /// moment the decision or acknowledgement was recorded. `None` until
    /// either happens. A materially different shape later invalidates both.
    shape: Option<OverlapShape>,
    /// When the triple left the live overlap view, if it is currently
    /// absent. Drives the disposal rule (see
    /// [`ConflictTracker::sweep_in_flight_pairs`]).
    absent_since: Option<Instant>,
}

/// The shape of an in-flight overlap on one file, derived purely from the two
/// agents' active-intent region declarations through the existing
/// [`regions_intersect`] rules.
///
/// This single value answers both questions the detector asks about a
/// conflict: how to classify it (additive vs true), and whether it is still
/// the *same* conflict it decided on earlier. Defining "materially changed"
/// as "a different shape under the same intersection rules" avoids inventing
/// a second notion of sameness alongside the classification.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OverlapShape {
    /// At least one side has no active intent for the file at all — it never
    /// published one, or the one it published has expired.
    ///
    /// This is the ABSENCE of a declaration, not a declaration. It is
    /// deliberately distinct from [`OverlapShape::FileLevel`]: an intent
    /// lapsing must not look like the agents changing their minds, or every
    /// settled conflict would re-open one TTL after it was settled.
    NoIntent,
    /// Both sides have an active intent for the file, but at least one
    /// declares it at file level (`regions == None`) — "I am touching the
    /// whole file".
    ///
    /// That IS a declaration, and a comparable one: an agent widening from a
    /// named region to the whole file changes the overlap even though no
    /// region set can be intersected. Disjointness still cannot be proven, so
    /// it classifies [`InFlightClass::True`].
    FileLevel,
    /// Both sides declared regions for the file; carries the intersection
    /// under [`regions_intersect`], rendered deterministically. An empty
    /// intersection means the declared regions are disjoint.
    Declared {
        /// Rendered intersecting regions, sorted (empty = disjoint).
        intersection: Vec<String>,
    },
}

impl OverlapShape {
    /// Classifies the overlap: `Additive` only when both sides declared
    /// regions and those regions are disjoint, `True` otherwise.
    fn class(&self) -> InFlightClass {
        match self {
            Self::Declared { intersection } if intersection.is_empty() => InFlightClass::Additive,
            _ => InFlightClass::True,
        }
    }

    /// Returns `true` when `self` (the shape observed now) is a *material*
    /// change from `recorded` (the shape when the decision or
    /// acknowledgement was made).
    ///
    /// A [`OverlapShape::NoIntent`] observation is never a material change:
    /// the declarations are absent — an intent expired, say — not changed.
    /// Every other difference counts, including a side widening from regions
    /// to a whole-file claim ([`OverlapShape::FileLevel`]), which is how an
    /// overlap the detector downgraded as additive turns into a true
    /// collision that has never been put to a human. The design leans toward
    /// invalidating: a spurious re-escalation costs a turn, a missed one can
    /// cost a merge.
    fn materially_differs_from(&self, recorded: &Self) -> bool {
        !matches!(self, Self::NoIntent) && self != recorded
    }
}

/// Classification of a due in-flight overlap on a shared file, derived from
/// the two agents' declared intent regions for that file.
///
/// The window-elapsed escalation path acts on this: a [`InFlightClass::True`]
/// collision escalates to the supervisor via `agent.question`; an
/// [`InFlightClass::Additive`] overlap downgrades to an informational
/// `agent.feedback` and never reaches the human.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InFlightClass {
    /// A true collision — the detector cannot prove the two agents' edits
    /// are disjoint: a side lacks an active intent for the file, a side
    /// declares the file at file level (`regions == None`), or both
    /// declared regions that intersect. Escalates as `agent.question`.
    True,
    /// An additive overlap — both agents declared regions for the file and
    /// the region sets are disjoint (well-separated hunks or differently
    /// named regions). Downgraded to an informational `agent.feedback`.
    Additive,
}

/// Computes the [`OverlapShape`] of an in-flight overlap on `file` between
/// agents `a` and `b` from their active-intent region declarations.
///
/// Returns [`OverlapShape::Declared`] only when BOTH agents have an active
/// intent that declares `file` with regions; its intersection comes straight
/// from [`regions_intersect`]. When both have an active intent for `file`
/// but at least one declares it at file level (`regions == None`), the shape
/// is [`OverlapShape::FileLevel`]. When either side has no active intent for
/// `file` at all, it is [`OverlapShape::NoIntent`]. The latter two both
/// classify as the conservative [`InFlightClass::True`], preserving the
/// pre-region always-escalate behaviour whenever disjointness cannot be
/// proven; they differ only in whether a later change counts as material.
///
/// Takes `intents` by reference (rather than being a `&self` method) so it
/// can be called while the decision map is mutably borrowed inside
/// [`ConflictTracker::take_due_escalations`].
fn overlap_shape(
    intents: &HashMap<String, IntentRecord>,
    a: &str,
    b: &str,
    file: &str,
) -> OverlapShape {
    let a_regions = intents.get(a).and_then(|r| r.files.get(file));
    let b_regions = intents.get(b).and_then(|r| r.files.get(file));
    match (a_regions, b_regions) {
        (Some(Some(ra)), Some(Some(rb))) => {
            let (hits, _cross_kind) = regions_intersect(ra, rb);
            OverlapShape::Declared {
                intersection: hits.iter().map(Region::to_string).collect(),
            }
        }
        // Both have an active intent for the file, at least one at file
        // level: a declaration the detector can compare against later.
        (Some(_), Some(_)) => OverlapShape::FileLevel,
        // A side has no active intent for the file: nothing to compare.
        _ => OverlapShape::NoIntent,
    }
}

/// Returns `true` when `path` is one of git-paw's own managed bookkeeping
/// paths (config, bundled scripts, session state under `.git-paw/`) rather
/// than genuine source — GP-04a. These are shared, untracked scaffolding
/// git-paw itself writes into every worktree, so their presence in an
/// agent's `git status` is not evidence of a code conflict and is excluded
/// from the overlap computation in [`ConflictTracker::forward_overlaps`] and
/// [`ConflictTracker::in_flight_overlaps`].
///
/// Delegates to [`crate::agents::is_managed_path`], the single source of
/// truth for what counts as git-paw's managed subtree. Its `worktree_root`
/// parameter is only consulted for the tracked-`AGENTS.md` special case,
/// which the conflict tracker (a cross-agent, path-only view with no
/// filesystem access) cannot evaluate — a placeholder is passed since
/// `.git-paw/`-prefixed paths, the only ones this rule needs to catch, are
/// matched purely syntactically and never touch it.
fn is_managed_bookkeeping_path(path: &str) -> bool {
    is_managed_path(Path::new("."), path)
}

/// Returns `true` when `path` is the shared spec-task-tracking artifact —
/// the tasks file git-paw directs every agent to update with its own
/// checkbox (a Spec Kit `specs/<feature>/tasks.md` or the `OpenSpec`
/// `tasks.md`). git-paw's writeback protocol has every agent tick its own
/// line in that single shared file, so a writeback there is expected
/// coordination, not a collision, and is excluded from
/// [`ConflictTracker::ownership_violations`]. Both spec backends name the
/// file identically, so a filename match identifies it without needing the
/// per-agent spec path or filesystem access; ownership detection on every
/// other file, including a real source-file collision, is unaffected.
fn is_shared_task_tracking_path(path: &str) -> bool {
    Path::new(path).file_name().and_then(|f| f.to_str()) == Some("tasks.md")
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
/// per-agent current-files map, dedup sets for warnings, the live in-flight
/// overlap view, and the durable conflict-decision map.
///
/// The live view (`in_flight_pairs`) and the decision map (`decisions`) are
/// deliberately separate: the first answers "which overlaps exist right
/// now", the second "what has already been decided about this pair and
/// file". Removing a triple from the live view never discards its decision.
#[derive(Debug, Default)]
pub struct ConflictTracker {
    intents: HashMap<String, IntentRecord>,
    current_files: HashMap<String, HashSet<String>>,
    warned_intent_pairs: HashSet<(String, String)>,
    in_flight_pairs: HashSet<(String, String, String)>,
    decisions: HashMap<(String, String, String), ConflictDecision>,
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
    /// intersection of both agents' current modified-file sets, then applies
    /// the disposal rule to the durable decision records.
    ///
    /// Removal affects only the live-overlap view. A decision record whose
    /// triple has left that view is kept — a commit empties `modified_files`
    /// for a tick or two, and discarding the decision there is exactly what
    /// let an adjudicated conflict re-escalate. The record is disposed of
    /// only once its triple has been continuously absent for `retention`,
    /// which is the detector's definition of "the conflict is genuinely
    /// over"; a triple that reappears before then clears its absence and
    /// keeps both its decision and its `first_seen` clock. This bounds the
    /// decision map to the overlaps seen within the last `retention`,
    /// rather than letting it grow for the session's life.
    pub fn sweep_in_flight_pairs(&mut self, now: Instant, retention: Duration) {
        let current_files = &self.current_files;
        self.in_flight_pairs.retain(|(a, b, file)| {
            let a_has = current_files.get(a).is_some_and(|f| f.contains(file));
            let b_has = current_files.get(b).is_some_and(|f| f.contains(file));
            a_has && b_has
        });
        let live = &self.in_flight_pairs;
        for (key, record) in &mut self.decisions {
            if live.contains(key) {
                record.absent_since = None;
            } else if record.absent_since.is_none() {
                record.absent_since = Some(now);
            }
        }
        self.decisions.retain(|_, record| {
            record
                .absent_since
                .is_none_or(|since| now.saturating_duration_since(since) < retention)
        });
    }

    // ====================================================================
    // Read-only queries
    // ====================================================================

    /// Returns every forward conflict between `x_id`'s intent and every other
    /// non-expired intent in the tracker.
    ///
    /// git-paw's own managed bookkeeping paths (under `.git-paw/`) are
    /// excluded from the shared-file set before conflicts are computed
    /// (GP-04a) — see [`is_managed_bookkeeping_path`].
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
                .filter(|path| !is_managed_bookkeeping_path(path))
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
    ///
    /// git-paw's own managed bookkeeping paths are excluded from the
    /// intersection (GP-04a) — see [`is_managed_bookkeeping_path`] — so a
    /// shared untracked `.git-paw/` entry never fabricates an in-flight
    /// conflict.
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
                let mut files: Vec<String> = a_files
                    .intersection(b_files)
                    .filter(|f| !is_managed_bookkeeping_path(f))
                    .cloned()
                    .collect();
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
    ///
    /// The shared spec-task-tracking artifact (see
    /// [`is_shared_task_tracking_path`]) is exempt: git-paw's own writeback
    /// protocol has every agent tick its own line there, so a writeback is
    /// expected coordination rather than a collision.
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
            if is_shared_task_tracking_path(file) {
                continue;
            }
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

    /// Records the in-flight triple `(min(a, b), max(a, b), file)` in the
    /// live-overlap view and ensures a durable decision record exists for
    /// it. Returns `true` only if the two agents have not yet been warned
    /// about this pair and file — i.e. this is the initial warning.
    ///
    /// A triple that re-enters the live view after a commit emptied an
    /// agent's `modified_files` finds its decision record still present, so
    /// it is not re-warned and its `first_seen` clock is not rewound.
    /// Conversely a record that exists only because the conflict was
    /// acknowledged ahead of time has not been warned, so the agents still
    /// get told they share the file.
    pub fn record_in_flight_pair(&mut self, a: &str, b: &str, file: &str, now: Instant) -> bool {
        let (lo, hi) = ordered_pair(a, b);
        let key = (lo, hi, file.to_string());
        self.in_flight_pairs.insert(key.clone());
        let record = self.decisions.entry(key).or_default();
        record.absent_since = None;
        if record.first_seen.is_none() {
            record.first_seen = Some(now);
        }
        if record.warned {
            false
        } else {
            record.warned = true;
            true
        }
    }

    /// Records that an orchestrator has adjudicated the conflict on `file`
    /// between `a` and `b`.
    ///
    /// While the acknowledgement stands, [`take_due_escalations`] skips the
    /// pair and file entirely, so no further `agent.question` is emitted for
    /// it. The acknowledgement does NOT remove the overlap from the live
    /// in-flight view — acknowledging suppresses the *question*, not the
    /// *fact*, so the detector keeps tracking the conflict and a later
    /// material change can still re-escalate it.
    ///
    /// The overlap's current shape is recorded alongside the
    /// acknowledgement; if the two agents' region declarations for `file`
    /// later yield a materially different shape, the acknowledgement is
    /// invalidated and the changed conflict may escalate again.
    ///
    /// An acknowledgement that arrives before the overlap does creates the
    /// record but does NOT mark it warned or start its clock: the two agents
    /// are still told they share the file when they actually do, and the
    /// escalation window is still measured from that first sighting.
    ///
    /// [`take_due_escalations`]: ConflictTracker::take_due_escalations
    pub fn acknowledge(&mut self, a: &str, b: &str, file: &str) {
        let (lo, hi) = ordered_pair(a, b);
        let shape = overlap_shape(&self.intents, &lo, &hi, file);
        let record = self
            .decisions
            .entry((lo, hi, file.to_string()))
            .or_default();
        record.acknowledged = true;
        record.shape = Some(shape);
    }

    /// Returns and marks-decided every live in-flight triple whose age
    /// exceeds `window` and whose escalation decision has not yet been made.
    /// Each returned triple is paired with its additive-vs-true
    /// classification (computed from the two agents' declared regions) so
    /// the caller can act — escalate a `True` collision, downgrade an
    /// `Additive` overlap — without a second lookup.
    ///
    /// The decision is recorded durably, keyed on the pair and file rather
    /// than on the triple's residency in the live view, so each conflict is
    /// returned at most once no matter how many times a commit empties an
    /// agent's `modified_files` in between.
    ///
    /// Two things suppress a triple: a decision already made, and a standing
    /// acknowledgement. Both are invalidated first if the agents' region
    /// declarations for the file now yield a materially different
    /// [`OverlapShape`], letting a conflict that genuinely changed shape be
    /// decided afresh.
    pub fn take_due_escalations(
        &mut self,
        window: Duration,
        now: Instant,
    ) -> Vec<(String, String, String, InFlightClass)> {
        let mut out = Vec::new();
        let intents = &self.intents;
        for key in &self.in_flight_pairs {
            let (a, b, file) = key;
            let Some(record) = self.decisions.get_mut(key) else {
                continue;
            };
            let shape = overlap_shape(intents, a, b, file);
            if record
                .shape
                .as_ref()
                .is_some_and(|recorded| shape.materially_differs_from(recorded))
            {
                record.decided = None;
                record.acknowledged = false;
                record.shape = None;
            }
            if record.acknowledged || record.decided.is_some() {
                continue;
            }
            let Some(first_seen) = record.first_seen else {
                continue;
            };
            if now.saturating_duration_since(first_seen) < window {
                continue;
            }
            let class = shape.class();
            record.decided = Some(class);
            record.shape = Some(shape);
            out.push((a.clone(), b.clone(), file.clone(), class));
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

    /// Returns the number of in-flight triples currently in the live
    /// overlap view.
    #[must_use]
    pub fn in_flight_pair_count(&self) -> usize {
        self.in_flight_pairs.len()
    }

    /// Returns `true` when the live overlap view currently contains the
    /// triple `(min(a, b), max(a, b), file)`.
    #[must_use]
    pub fn has_in_flight_pair(&self, a: &str, b: &str, file: &str) -> bool {
        let (lo, hi) = ordered_pair(a, b);
        self.in_flight_pairs.contains(&(lo, hi, file.to_string()))
    }

    /// Returns the number of durable conflict-decision records retained.
    ///
    /// Decision records outlive the live overlap view by design, so this is
    /// normally at least [`in_flight_pair_count`]; the disposal rule in
    /// [`sweep_in_flight_pairs`] keeps it bounded.
    ///
    /// [`in_flight_pair_count`]: ConflictTracker::in_flight_pair_count
    /// [`sweep_in_flight_pairs`]: ConflictTracker::sweep_in_flight_pairs
    #[must_use]
    pub fn decision_record_count(&self) -> usize {
        self.decisions.len()
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

/// Builds the informational additive-downgrade feedback error string.
///
/// Emitted to both agents (with `from = "supervisor"`) instead of an
/// escalation `agent.question` when a due in-flight overlap classifies as
/// [`InFlightClass::Additive`]: both agents declared disjoint regions on the
/// shared file, so their edits do not collide in flight. The text tells them
/// the file is shared but the changes are additive and any residual
/// reconciliation is deferred to merge time.
fn additive_downgrade_feedback(a: &str, b: &str, file: &str) -> String {
    format!(
        "{CONFLICT_DETECTOR_TAG} shared file, additive — resolve at merge: {file} is modified by both {a} and {b}, but their declared regions are disjoint so the edits do not collide in flight",
    )
}

/// Token that marks an `agent.feedback` error string as a conflict
/// acknowledgement rather than ordinary feedback.
///
/// Acknowledgement rides on `agent.feedback` rather than a new
/// `BrokerMessage` variant: publishing feedback to both agents is already
/// what an orchestrator does after adjudicating an escalation, and adding a
/// variant would ripple into every exhaustive match listed in AGENTS.md §7
/// for no behavioural gain. The detector never emits this token itself, so
/// recognising it cannot feed back into the detector.
pub const CONFLICT_ACK_TAG: &str = "[conflict-ack]";

/// Parses a conflict acknowledgement out of one `agent.feedback` error
/// string, returning the `(agent_a, agent_b, file)` it adjudicates.
///
/// The expected form is `[conflict-ack] <agent_a> <agent_b> <file>`. Agent
/// ids are slugified and so never contain whitespace; the remainder of the
/// line is the file path, which may. Anything after the path (free-form
/// prose the orchestrator adds) is NOT supported — the path runs to the end
/// of the string, so the acknowledgement must be its own error entry.
/// Returns `None` for any string that is not an acknowledgement or is
/// missing a field.
#[must_use]
pub fn parse_conflict_ack(text: &str) -> Option<(String, String, String)> {
    let rest = text.trim().strip_prefix(CONFLICT_ACK_TAG)?;
    let mut parts = rest.trim_start().splitn(3, char::is_whitespace);
    let a = parts.next()?.trim();
    let b = parts.next()?.trim();
    let file = parts.next()?.trim();
    if a.is_empty() || b.is_empty() || file.is_empty() {
        return None;
    }
    Some((a.to_string(), b.to_string(), file.to_string()))
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
    // Drop expired intents up front so neither the acknowledgement's shape
    // snapshot nor either overlap check sees them.
    tracker.expire_stale_intents(now);

    // Conflict acknowledgement — an orchestrator adjudicating an escalation.
    // Handled before the re-entrancy guard below, which would otherwise
    // swallow it along with the detector's own supervisor-sourced feedback.
    // Only feedback from the supervisor is honoured. That is a filter, not
    // an authentication boundary: the broker does not authenticate senders
    // and `from` is self-asserted, so an agent could deliberately forge an
    // acknowledgement for a conflict it is party to. What the filter buys
    // is that an agent's ORDINARY feedback is never mistaken for an
    // adjudication; what limits a forgery is that every acknowledgement
    // stays in the broker log and so is auditable. (A helper-published one
    // also reaches both named agents, but this path honours an
    // acknowledgement whatever its envelope `agent_id`, so delivery to both
    // is a property of the helper, not of the wire form.) Sender
    // authentication is v0.18.0 broker-authority work.
    if let BrokerMessage::Feedback { payload, .. } = msg
        && payload.from == CONFLICT_DETECTOR_SENDER
    {
        let acks: Vec<(String, String, String)> = payload
            .errors
            .iter()
            .filter_map(|e| parse_conflict_ack(e))
            .collect();
        if !acks.is_empty() {
            for (a, b, file) in acks {
                tracker.acknowledge(&a, &b, &file);
            }
            return 0;
        }
    }

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
            //    stopped touching it) from the live view, and dispose of
            //    decision records whose conflict is genuinely over.
            tracker.sweep_in_flight_pairs(now, decision_retention(config));

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

/// Floor on how long a conflict-decision record outlives the disappearance
/// of its overlap from the live view.
///
/// A commit empties an agent's `modified_files` for a tick or two, so the
/// retention must comfortably exceed that blip. The escalation window is the
/// natural scale for "how long before we call this conflict settled", but it
/// is configurable down to a few seconds, so this floor keeps a short window
/// from reintroducing the commit-blip defect.
pub const DECISION_RECORD_MIN_RETENTION: Duration = Duration::from_secs(60);

/// How long a conflict-decision record is retained after its triple leaves
/// the live overlap view — the escalation window, floored at
/// [`DECISION_RECORD_MIN_RETENTION`].
fn decision_retention(config: &ConflictConfig) -> Duration {
    Duration::from_secs(config.window_seconds).max(DECISION_RECORD_MIN_RETENTION)
}

/// Run a single tick of the periodic timer-driven detector logic:
/// expire stale intents, sweep resolved in-flight pairs, then act on any
/// escalation decisions whose window has elapsed.
///
/// Each due triple is classified additive-vs-true from the two agents'
/// declared regions (see [`overlap_shape`]). A `True` collision
/// escalates to the supervisor inbox via `agent.question`; an `Additive`
/// overlap downgrades to a single informational `agent.feedback` delivered
/// to both agents and NO `agent.question`. Both branches mark the triple
/// decided, so neither re-emits on later ticks; the triple itself stays in
/// the tracker until `sweep_in_flight_pairs` drops it when an agent stops
/// touching the file, so the overlap is recorded rather than discarded.
///
/// Returns the number of auto-emitted broker messages (a question counts as
/// one; an additive downgrade counts as two — one feedback per agent).
pub fn tick(
    state: &Arc<BrokerState>,
    tracker: &mut ConflictTracker,
    config: &ConflictConfig,
    now: Instant,
) -> usize {
    tracker.expire_stale_intents(now);
    tracker.sweep_in_flight_pairs(now, decision_retention(config));
    let window = Duration::from_secs(config.window_seconds);
    let mut emitted = 0usize;
    for (a, b, file, class) in tracker.take_due_escalations(window, now) {
        match class {
            InFlightClass::True => {
                emit_question(
                    state,
                    in_flight_escalation_question(&a, &b, &file, config.window_seconds),
                );
                emitted += 1;
            }
            InFlightClass::Additive => {
                let text = additive_downgrade_feedback(&a, &b, &file);
                emit_feedback(state, &a, text.clone());
                emit_feedback(state, &b, text);
                emitted += 2;
            }
        }
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

    /// GP-04a: git-paw's own managed bookkeeping (`.git-paw/`) never
    /// fabricates a forward conflict, but a real source overlap alongside it
    /// still conflicts on the source path only.
    #[test]
    fn tracker_forward_overlaps_excludes_managed_bookkeeping() {
        let mut t = fresh();
        let now = Instant::now();
        t.insert_intent(
            "feat-x",
            nfi(&[".git-paw/config.toml"]),
            "x".into(),
            ttl_secs(60),
            now,
        );
        t.insert_intent(
            "feat-y",
            nfi(&[".git-paw/config.toml"]),
            "y".into(),
            ttl_secs(60),
            now,
        );
        assert!(
            t.forward_overlaps("feat-x").is_empty(),
            "a shared .git-paw/ path must not fabricate a forward conflict"
        );

        t.insert_intent(
            "feat-x",
            nfi(&["src/a.rs", ".git-paw/config.toml"]),
            "x2".into(),
            ttl_secs(60),
            now,
        );
        t.insert_intent(
            "feat-y",
            nfi(&["src/a.rs", ".git-paw/config.toml"]),
            "y2".into(),
            ttl_secs(60),
            now,
        );
        let overlaps = t.forward_overlaps("feat-x");
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].files.len(), 1);
        assert_eq!(overlaps[0].files[0].path, "src/a.rs");
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

    /// GP-04a spec scenarios: a shared untracked `.git-paw/` path does not
    /// fabricate an in-flight conflict, and a real source overlap alongside
    /// it still conflicts on the source path only.
    #[test]
    fn tracker_in_flight_overlaps_excludes_managed_bookkeeping() {
        let mut t = fresh();
        t.update_status("feat-x", files(&[".git-paw/config.toml"]));
        t.update_status("feat-y", files(&[".git-paw/config.toml"]));
        assert!(
            t.in_flight_overlaps().is_empty(),
            "a shared .git-paw/ path must not fabricate an in-flight conflict"
        );

        t.update_status("feat-x", files(&["src/a.rs", ".git-paw/config.toml"]));
        t.update_status("feat-y", files(&["src/a.rs", ".git-paw/config.toml"]));
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
        let retention = Duration::from_secs(60);
        assert!(t.record_in_flight_pair("feat-x", "feat-y", "src/a.rs", now));
        assert!(!t.record_in_flight_pair("feat-y", "feat-x", "src/a.rs", now));
        // Sweep removes triples whose file is no longer in the intersection
        // from the LIVE view, but the decision record survives, so a
        // re-appearance is not treated as a brand-new conflict.
        t.update_status("feat-x", files(&["src/b.rs"]));
        t.update_status("feat-y", files(&["src/a.rs"]));
        t.sweep_in_flight_pairs(now, retention);
        assert_eq!(t.in_flight_pair_count(), 0);
        assert!(!t.record_in_flight_pair("feat-x", "feat-y", "src/a.rs", now));
    }

    #[test]
    fn tracker_discards_decision_record_once_conflict_is_genuinely_over() {
        let mut t = fresh();
        let now = Instant::now();
        let retention = Duration::from_secs(60);
        t.update_status("feat-x", files(&["src/a.rs"]));
        t.update_status("feat-y", files(&["src/a.rs"]));
        assert!(t.record_in_flight_pair("feat-x", "feat-y", "src/a.rs", now));
        assert_eq!(t.decision_record_count(), 1);
        // X stops touching the file and never comes back.
        t.update_status("feat-x", files(&["src/b.rs"]));
        t.sweep_in_flight_pairs(now, retention);
        assert_eq!(
            t.decision_record_count(),
            1,
            "a just-vacated conflict is still within its retention"
        );
        t.sweep_in_flight_pairs(now + retention + Duration::from_secs(1), retention);
        assert_eq!(
            t.decision_record_count(),
            0,
            "decision records do not accumulate for the session's life"
        );
        // With the record gone, the same overlap is a brand-new conflict.
        t.update_status("feat-x", files(&["src/a.rs"]));
        assert!(t.record_in_flight_pair(
            "feat-x",
            "feat-y",
            "src/a.rs",
            now + retention + Duration::from_secs(2)
        ));
    }

    #[test]
    fn tracker_reappearing_triple_keeps_its_decision_record() {
        let mut t = fresh();
        let now = Instant::now();
        let retention = Duration::from_secs(60);
        t.update_status("feat-x", files(&["src/a.rs"]));
        t.update_status("feat-y", files(&["src/a.rs"]));
        t.record_in_flight_pair("feat-x", "feat-y", "src/a.rs", now);
        // A commit empties X's set for a tick, then the file is back.
        t.update_status("feat-x", files(&[]));
        t.sweep_in_flight_pairs(now + Duration::from_secs(2), retention);
        t.update_status("feat-x", files(&["src/a.rs"]));
        t.record_in_flight_pair("feat-x", "feat-y", "src/a.rs", now + Duration::from_secs(4));
        // Sweeping long after the blip must not dispose of the record: the
        // triple is live again, so its absence clock was cleared.
        t.sweep_in_flight_pairs(now + retention + Duration::from_secs(10), retention);
        assert_eq!(t.decision_record_count(), 1);
        assert!(t.has_in_flight_pair("feat-x", "feat-y", "src/a.rs"));
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

    #[test]
    fn tracker_ownership_violations_exempts_shared_tasks_file() {
        let mut t = fresh();
        let now = Instant::now();
        t.insert_intent(
            "feat-x",
            nfi(&["specs/001/tasks.md"]),
            "x".into(),
            ttl_secs(60),
            now,
        );
        t.update_status("feat-y", files(&["specs/001/tasks.md"]));
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
    fn detector_ownership_shared_tasks_file_no_violation() {
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
            &intent_msg("feat-x", &["specs/001/tasks.md"], "x", 600),
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
            &status_msg("feat-y", &["specs/001/tasks.md"]),
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
    fn regions_intersect_different_named_kinds_different_names_empty() {
        // function "a" vs class "b" — different kinds AND different names,
        // no intersection (keeps the additive downgrade meaningful).
        let class_b = Region::Class {
            name: "b".to_string(),
        };
        let (hits, cross) = regions_intersect(&[func("a")], &[class_b]);
        assert!(hits.is_empty());
        assert!(!cross);
    }

    #[test]
    fn regions_intersect_named_cross_kind_same_name_conservative() {
        // Spec: Named regions of different kinds with the same name
        // intersect conservatively. function "foo" vs block "foo" is almost
        // always the same construct declared through different lenses.
        let block_foo = Region::Block {
            anchor: "foo".to_string(),
        };
        let (hits, cross) = regions_intersect(&[func("foo")], std::slice::from_ref(&block_foo));
        assert!(cross, "named cross-kind name match must flag cross_kind");
        assert!(hits.contains(&func("foo")));
        assert!(hits.contains(&block_foo));
    }

    #[test]
    fn regions_intersect_named_cross_kind_different_names_empty() {
        // function "foo" vs block "bar" — different kinds, different names:
        // still no intersection.
        let block_bar = Region::Block {
            anchor: "bar".to_string(),
        };
        let (hits, cross) = regions_intersect(&[func("foo")], &[block_bar]);
        assert!(hits.is_empty());
        assert!(!cross);
    }

    // ====================================================================
    // Region-name normalization (conflict-detector-same-region task 1.1)
    // ====================================================================

    #[test]
    fn regions_intersect_normalizes_case() {
        let (hits, cross) = regions_intersect(&[func("Validate_Token")], &[func("validate_token")]);
        assert_eq!(hits, vec![func("Validate_Token")]);
        assert!(!cross, "same-kind normalized match is not cross-kind");
    }

    #[test]
    fn regions_intersect_normalizes_separators() {
        // Space, underscore, and hyphen collapse to one separator form.
        let (hits, _) = regions_intersect(&[func("validate token")], &[func("validate_token")]);
        assert!(!hits.is_empty());
        let (hits, _) = regions_intersect(&[func("validate-token")], &[func("validate_token")]);
        assert!(!hits.is_empty());
        let (hits, _) = regions_intersect(&[func("validate  token")], &[func("validate-token")]);
        assert!(!hits.is_empty());
    }

    #[test]
    fn regions_intersect_normalizes_trim() {
        let (hits, _) = regions_intersect(&[func("  validate_token  ")], &[func("validate_token")]);
        assert!(!hits.is_empty());
    }

    #[test]
    fn regions_intersect_normalizes_trailing_parens() {
        let (hits, _) = regions_intersect(&[func("validate_token()")], &[func("validate_token")]);
        assert!(!hits.is_empty());
    }

    #[test]
    fn regions_intersect_normalizes_leading_declaration_keyword() {
        for declared in [
            "fn validate_token",
            "def validate_token",
            "function validate_token",
        ] {
            let (hits, _) = regions_intersect(&[func(declared)], &[func("validate_token")]);
            assert!(!hits.is_empty(), "`{declared}` must equate the bare symbol");
        }
        let class_bare = Region::Class {
            name: "Config".to_string(),
        };
        let class_kw = Region::Class {
            name: "class Config".to_string(),
        };
        let (hits, _) = regions_intersect(&[class_kw], &[class_bare]);
        assert!(!hits.is_empty());
    }

    #[test]
    fn regions_intersect_combined_spelling_variant() {
        // Spec: Spelling variants of the same symbol intersect —
        // `validate_token` vs `Validate Token()`.
        let (hits, cross) =
            regions_intersect(&[func("validate_token")], &[func("Validate Token()")]);
        assert!(!hits.is_empty());
        assert!(!cross);
    }

    #[test]
    fn regions_intersect_distinct_symbols_stay_distinct() {
        // Normalization must not equate genuinely different symbols.
        let (hits, _) = regions_intersect(&[func("validate_token")], &[func("validate_tokens")]);
        assert!(hits.is_empty());
        // Separator collapse keeps runs as ONE separator — it does not
        // delete them, so `foo_bar` != `foobar`.
        let (hits, _) = regions_intersect(&[func("foo_bar")], &[func("foobar")]);
        assert!(hits.is_empty());
        // A leading keyword is stripped only when a separator follows —
        // `classify` is not `class ify`.
        let (hits, _) = regions_intersect(&[func("classify")], &[func("ify")]);
        assert!(hits.is_empty());
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
    fn detector_spelling_variants_conflict() {
        // Spec: Spelling variants of the same symbol intersect —
        // `function validate_token` vs `function Validate Token()` must
        // trigger a forward-conflict warning.
        let a = intent_msg_with_regions(
            "feat-x",
            &[("src/auth.rs", vec![func("validate_token")])],
            "x",
            600,
        );
        let b = intent_msg_with_regions(
            "feat-y",
            &[("src/auth.rs", vec![func("Validate Token()")])],
            "y",
            600,
        );
        let (state, _) = run_two_intents(&a, &b);
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        assert_eq!(x_fb.len(), 1, "spelling variants must warn");
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            assert!(payload.errors[0].contains("forward conflict"));
            assert!(payload.errors[0].contains("src/auth.rs"));
        }
        assert_eq!(supervisor_feedbacks_in_inbox(&state, "feat-y").len(), 1);
    }

    #[test]
    fn detector_named_cross_kind_same_name_conflict_includes_hint() {
        // Spec: Named regions of different kinds with the same name
        // intersect conservatively — `function SHARED_REGION_NAME` vs
        // `block SHARED_REGION_NAME` conflicts with the conservative hint.
        let a = intent_msg_with_regions(
            "feat-x",
            &[("src/config.rs", vec![func("SHARED_REGION_NAME")])],
            "x",
            600,
        );
        let b = intent_msg_with_regions(
            "feat-y",
            &[("src/config.rs", vec![block("SHARED_REGION_NAME")])],
            "y",
            600,
        );
        let (state, _) = run_two_intents(&a, &b);
        let x_fb = supervisor_feedbacks_in_inbox(&state, "feat-x");
        assert_eq!(x_fb.len(), 1);
        if let BrokerMessage::Feedback { payload, .. } = &x_fb[0] {
            let err = &payload.errors[0];
            assert!(err.contains("forward conflict"));
            assert!(err.contains("SHARED_REGION_NAME"));
            assert!(
                err.contains(CROSS_KIND_HINT),
                "named cross-kind conflict must carry the conservative hint; got: {err}"
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

    // ====================================================================
    // Additive-vs-true in-flight escalation (conflict-detection spec)
    // ====================================================================

    fn block(anchor: &str) -> Region {
        Region::Block {
            anchor: anchor.to_string(),
        }
    }

    fn range(start_line: u32, end_line: u32) -> Region {
        Region::Range {
            start_line,
            end_line,
        }
    }

    /// Config with a short window and forward-overlap warnings disabled, so
    /// these tests isolate the in-flight escalation decision from
    /// forward-conflict feedback noise while intents (and their regions) are
    /// still tracked (`insert_intent` runs regardless of the flag).
    fn escalation_config() -> ConflictConfig {
        ConflictConfig {
            window_seconds: 5,
            warn_on_intent_overlap: false,
            ..ConflictConfig::default()
        }
    }

    /// Registers both inboxes, feeds each agent's region-bearing intent for
    /// `file`, then their overlapping status on `file` (recording the
    /// in-flight triple). The caller advances past the window and ticks.
    fn setup_in_flight(
        x_regions: Vec<Region>,
        y_regions: Vec<Region>,
        file: &str,
    ) -> (Arc<BrokerState>, ConflictTracker, ConflictConfig, Instant) {
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        let cfg = escalation_config();
        // Register both inboxes (delivery skips unregistered queues).
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        process_message(
            &state,
            &mut t,
            &intent_msg_with_regions("feat-x", &[(file, x_regions)], "x", 600),
            &cfg,
            now,
        );
        process_message(
            &state,
            &mut t,
            &intent_msg_with_regions("feat-y", &[(file, y_regions)], "y", 600),
            &cfg,
            now,
        );
        // Both agents now touch the file → in-flight triple recorded.
        process_message(&state, &mut t, &status_msg("feat-x", &[file]), &cfg, now);
        process_message(&state, &mut t, &status_msg("feat-y", &[file]), &cfg, now);
        (state, t, cfg, now)
    }

    /// Feedback messages in `target`'s inbox whose text is the additive
    /// downgrade (distinguished from the initial in-flight warning by the
    /// "additive" wording).
    fn additive_downgrade_feedbacks(state: &Arc<BrokerState>, target: &str) -> Vec<BrokerMessage> {
        supervisor_feedbacks_in_inbox(state, target)
            .into_iter()
            .filter(|m| {
                matches!(
                    m,
                    BrokerMessage::Feedback { payload, .. }
                        if payload.errors.iter().any(|e| e.contains("additive"))
                )
            })
            .collect()
    }

    #[test]
    fn detector_true_collision_same_anchor_escalates_after_window() {
        // Spec: True collision (same anchor) escalates after the window.
        // Both agents insert at the same block anchor → regions intersect →
        // true collision → agent.question to the supervisor.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![block("L82")], vec![block("L82")], "coordination.md");
        let due = now + Duration::from_secs(10);
        let emitted = tick(&state, &mut t, &cfg, due);
        assert_eq!(emitted, 1, "a true collision emits exactly one question");
        let q = supervisor_questions(&state);
        assert_eq!(q.len(), 1);
        if let BrokerMessage::Question { payload, .. } = &q[0] {
            assert!(payload.question.contains(CONFLICT_DETECTOR_TAG));
            assert!(payload.question.contains("coordination.md"));
            assert!(payload.question.contains("feat-x"));
            assert!(payload.question.contains("feat-y"));
        } else {
            panic!("expected Question");
        }
        // No additive downgrade for a true collision.
        assert!(additive_downgrade_feedbacks(&state, "feat-x").is_empty());
        // Triple marked decided: a later tick emits nothing more.
        let later = due + Duration::from_secs(10);
        assert_eq!(tick(&state, &mut t, &cfg, later), 0);
        assert_eq!(supervisor_questions(&state).len(), 1);
    }

    #[test]
    fn detector_true_collision_overlapping_ranges_escalates() {
        // Spec: overlapping ranges are a true collision (the "/" alternative
        // in task 3.1).
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![range(10, 30)], vec![range(25, 45)], "src/a.rs");
        let due = now + Duration::from_secs(10);
        assert_eq!(tick(&state, &mut t, &cfg, due), 1);
        assert_eq!(supervisor_questions(&state).len(), 1);
        assert!(additive_downgrade_feedbacks(&state, "feat-x").is_empty());
    }

    #[test]
    fn detector_additive_overlap_downgrades_no_question() {
        // Spec: Additive overlap is downgraded, not escalated to the human.
        // Disjoint declared ranges on a shared file → one informational
        // agent.feedback to each agent, and NO agent.question.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![range(10, 30)], vec![range(80, 120)], "src/config.rs");
        let due = now + Duration::from_secs(10);
        let emitted = tick(&state, &mut t, &cfg, due);
        assert_eq!(
            emitted, 2,
            "additive downgrade emits one feedback per agent"
        );
        assert!(
            supervisor_questions(&state).is_empty(),
            "additive overlap must not escalate to the human"
        );
        for target in ["feat-x", "feat-y"] {
            let fb = additive_downgrade_feedbacks(&state, target);
            assert_eq!(
                fb.len(),
                1,
                "{target} should receive one additive downgrade"
            );
            if let BrokerMessage::Feedback { payload, .. } = &fb[0] {
                let err = &payload.errors[0];
                assert!(err.starts_with(CONFLICT_DETECTOR_TAG));
                assert!(err.contains("shared file, additive — resolve at merge"));
                assert!(err.contains("src/config.rs"));
            } else {
                panic!("expected Feedback");
            }
        }
    }

    #[test]
    fn detector_normalized_match_upgrades_additive_to_true() {
        // conflict-detector-same-region task 1.3: a same-symbol pair spelled
        // two ways used to slip through name equality and downgrade to
        // Additive; normalization upgrades it to a True collision that
        // escalates as a question, with no additive downgrade.
        let (state, mut t, cfg, now) = setup_in_flight(
            vec![func("validate_token")],
            vec![func("Validate Token()")],
            "src/auth.rs",
        );
        let due = now + Duration::from_secs(10);
        let emitted = tick(&state, &mut t, &cfg, due);
        assert_eq!(emitted, 1, "a true collision emits exactly one question");
        assert_eq!(supervisor_questions(&state).len(), 1);
        assert!(
            additive_downgrade_feedbacks(&state, "feat-x").is_empty(),
            "normalized-match pair must not downgrade to additive"
        );
        assert!(additive_downgrade_feedbacks(&state, "feat-y").is_empty());
    }

    #[test]
    fn detector_additive_downgrade_recorded_and_not_reemitted() {
        // Spec: Additive downgrade records the overlap and does not re-emit.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![range(10, 30)], vec![range(80, 120)], "src/config.rs");
        let due = now + Duration::from_secs(10);
        tick(&state, &mut t, &cfg, due);
        assert_eq!(
            t.in_flight_pair_count(),
            1,
            "triple is recorded, not dropped"
        );
        let fb_before = additive_downgrade_feedbacks(&state, "feat-x").len();
        // Subsequent ticks while regions unchanged: no new feedback, no question.
        let later = due + Duration::from_secs(30);
        assert_eq!(tick(&state, &mut t, &cfg, later), 0);
        assert_eq!(
            additive_downgrade_feedbacks(&state, "feat-x").len(),
            fb_before
        );
        assert!(supervisor_questions(&state).is_empty());
        assert_eq!(
            t.in_flight_pair_count(),
            1,
            "triple still recorded after later ticks"
        );
    }

    #[test]
    fn detector_conservative_fallback_file_level_intents_escalate() {
        // Spec: Conservative escalation when regions are not declared. Both
        // agents declare the file at file level (no regions) → cannot prove
        // disjointness → escalate as a question.
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        let cfg = escalation_config();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
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
        assert_eq!(tick(&state, &mut t, &cfg, due), 1);
        let q = supervisor_questions(&state);
        assert_eq!(q.len(), 1);
        if let BrokerMessage::Question { payload, .. } = &q[0] {
            assert!(payload.question.contains(CONFLICT_DETECTOR_TAG));
            assert!(payload.question.contains("src/a.rs"));
            assert!(payload.question.contains("feat-x"));
            assert!(payload.question.contains("feat-y"));
        }
        assert!(additive_downgrade_feedbacks(&state, "feat-x").is_empty());
    }

    #[test]
    fn detector_conservative_fallback_one_side_file_level_escalates() {
        // Conservative fallback: one agent declares regions, the other
        // declares the same file at file level → disjointness cannot be
        // proven → escalate as a question (not a downgrade).
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        let cfg = escalation_config();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();
        process_message(
            &state,
            &mut t,
            &intent_msg_with_regions("feat-x", &[("src/a.rs", vec![range(10, 30)])], "x", 600),
            &cfg,
            now,
        );
        // feat-y declares the same file at file level (bare string).
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
        assert_eq!(tick(&state, &mut t, &cfg, due), 1);
        assert_eq!(supervisor_questions(&state).len(), 1);
        assert!(additive_downgrade_feedbacks(&state, "feat-x").is_empty());
    }

    // ====================================================================
    // Escalation persistence across commits, and acknowledgement
    // ====================================================================

    /// An orchestrator's acknowledgement, carried on `agent.feedback` from
    /// the supervisor.
    fn ack_msg(target: &str, a: &str, b: &str, file: &str) -> BrokerMessage {
        BrokerMessage::Feedback {
            agent_id: target.to_string(),
            payload: FeedbackPayload {
                from: CONFLICT_DETECTOR_SENDER.to_string(),
                errors: vec![format!("{CONFLICT_ACK_TAG} {a} {b} {file}")],
            },
        }
    }

    #[test]
    fn parse_conflict_ack_extracts_pair_and_file() {
        assert_eq!(
            parse_conflict_ack("[conflict-ack] feat-x feat-y src/a.rs"),
            Some((
                "feat-x".to_string(),
                "feat-y".to_string(),
                "src/a.rs".to_string()
            ))
        );
        // A path containing spaces runs to the end of the string.
        assert_eq!(
            parse_conflict_ack("[conflict-ack] feat-x feat-y docs/my notes.md"),
            Some((
                "feat-x".to_string(),
                "feat-y".to_string(),
                "docs/my notes.md".to_string()
            ))
        );
    }

    #[test]
    fn parse_conflict_ack_rejects_non_acknowledgements() {
        assert!(parse_conflict_ack("[conflict-detector] in-flight conflict").is_none());
        assert!(parse_conflict_ack("[conflict-ack] feat-x feat-y").is_none());
        assert!(parse_conflict_ack("[conflict-ack] feat-x").is_none());
        assert!(parse_conflict_ack("[conflict-ack]").is_none());
        assert!(parse_conflict_ack("please [conflict-ack] feat-x feat-y f").is_none());
    }

    #[test]
    fn detector_commit_does_not_reescalate_an_already_escalated_conflict() {
        // Spec: A commit does not re-escalate an already-escalated conflict.
        // Reproduces the dogfood sequence — overlap, escalation, one agent
        // publishes modified_files = [] (its commit), republishes the file,
        // the window elapses again.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![block("L82")], vec![block("L82")], "src/a.rs");
        let due = now + Duration::from_secs(10);
        assert_eq!(tick(&state, &mut t, &cfg, due), 1);
        assert_eq!(supervisor_questions(&state).len(), 1);

        // feat-x commits: its dirty set empties, then the file is back.
        let committed = due + Duration::from_secs(1);
        process_message(&state, &mut t, &status_msg("feat-x", &[]), &cfg, committed);
        let republished = committed + Duration::from_secs(2);
        process_message(
            &state,
            &mut t,
            &status_msg("feat-x", &["src/a.rs"]),
            &cfg,
            republished,
        );

        // Well past the window again, with both agents reporting the file.
        let later = republished + Duration::from_secs(30);
        assert_eq!(
            tick(&state, &mut t, &cfg, later),
            0,
            "a commit must not resurrect the escalation"
        );
        assert_eq!(
            supervisor_questions(&state).len(),
            1,
            "no further agent.question for the pair and file"
        );
        assert_eq!(
            t.decision_record_count(),
            1,
            "the escalation record still reflects the earlier decision"
        );
    }

    #[test]
    fn detector_commit_does_not_reset_the_first_seen_clock() {
        // Spec: A commit does not reset the first-seen clock. The overlap is
        // seen at `now`; at T=2s (< window=5s) feat-x blips empty and comes
        // back. The escalation must still fire at now + window, not at
        // republish + window.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![block("L82")], vec![block("L82")], "src/a.rs");
        let committed = now + Duration::from_secs(2);
        process_message(&state, &mut t, &status_msg("feat-x", &[]), &cfg, committed);
        let republished = now + Duration::from_secs(3);
        process_message(
            &state,
            &mut t,
            &status_msg("feat-x", &["src/a.rs"]),
            &cfg,
            republished,
        );

        // window_seconds = 5 — due relative to the ORIGINAL first sighting.
        let due = now + Duration::from_secs(5);
        assert_eq!(
            tick(&state, &mut t, &cfg, due),
            1,
            "escalation fires at first_seen + window despite the blip"
        );
        assert_eq!(supervisor_questions(&state).len(), 1);
    }

    #[test]
    fn detector_acknowledgement_suppresses_escalation_and_keeps_overlap() {
        // Spec: An acknowledged conflict is not escalated again, and the
        // overlap stays in the in-flight tracker.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![block("L82")], vec![block("L82")], "src/a.rs");
        process_message(
            &state,
            &mut t,
            &ack_msg("feat-x", "feat-x", "feat-y", "src/a.rs"),
            &cfg,
            now,
        );
        let due = now + Duration::from_secs(10);
        assert_eq!(
            tick(&state, &mut t, &cfg, due),
            0,
            "an acknowledged conflict is not put to the human"
        );
        assert!(supervisor_questions(&state).is_empty());
        assert!(
            t.has_in_flight_pair("feat-x", "feat-y", "src/a.rs"),
            "acknowledgement suppresses the question, not the fact"
        );
    }

    #[test]
    fn detector_acknowledged_conflict_stays_quiet_across_commits() {
        // Spec: ... including across commits.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![block("L82")], vec![block("L82")], "src/a.rs");
        let due = now + Duration::from_secs(10);
        assert_eq!(tick(&state, &mut t, &cfg, due), 1);
        // The orchestrator adjudicates the escalation.
        process_message(
            &state,
            &mut t,
            &ack_msg("feat-y", "feat-y", "feat-x", "src/a.rs"),
            &cfg,
            due,
        );
        // feat-y commits and comes back.
        let committed = due + Duration::from_secs(1);
        process_message(&state, &mut t, &status_msg("feat-y", &[]), &cfg, committed);
        process_message(
            &state,
            &mut t,
            &status_msg("feat-y", &["src/a.rs"]),
            &cfg,
            committed + Duration::from_secs(2),
        );
        let later = committed + Duration::from_secs(30);
        assert_eq!(tick(&state, &mut t, &cfg, later), 0);
        assert_eq!(supervisor_questions(&state).len(), 1);
        assert!(t.has_in_flight_pair("feat-x", "feat-y", "src/a.rs"));
    }

    #[test]
    fn detector_early_acknowledgement_still_warns_both_agents() {
        // An orchestrator may acknowledge a conflict it anticipates, before
        // the two agents actually touch the file. That must suppress the
        // escalation QUESTION only — the agents still need to be told they
        // share the file, and the window still runs from the first real
        // sighting.
        let state = fresh_state();
        let mut t = ConflictTracker::new();
        let cfg = escalation_config();
        delivery::publish_message(&state, &status_msg("feat-x", &[]));
        delivery::publish_message(&state, &status_msg("feat-y", &[]));
        let now = Instant::now();

        // Acknowledged up front, before either agent reports the file.
        process_message(
            &state,
            &mut t,
            &ack_msg("feat-x", "feat-x", "feat-y", "src/a.rs"),
            &cfg,
            now,
        );
        // Now the overlap actually happens.
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

        for target in ["feat-x", "feat-y"] {
            let warnings: Vec<_> = supervisor_feedbacks_in_inbox(&state, target)
                .into_iter()
                .filter(|m| {
                    matches!(
                        m,
                        BrokerMessage::Feedback { payload, .. }
                            if payload.errors.iter().any(|e| e.contains("in-flight conflict"))
                    )
                })
                .collect();
            assert_eq!(
                warnings.len(),
                1,
                "{target} must still be warned that the file is shared"
            );
        }
        // The acknowledgement still suppresses the escalation question.
        let due = now + Duration::from_secs(10);
        assert_eq!(tick(&state, &mut t, &cfg, due), 0);
        assert!(supervisor_questions(&state).is_empty());
        assert!(t.has_in_flight_pair("feat-x", "feat-y", "src/a.rs"));
    }

    #[test]
    fn detector_acknowledgement_from_a_peer_agent_is_ignored() {
        // Only supervisor-sourced feedback acknowledges — an agent must not
        // be able to silence a conflict it is party to.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![block("L82")], vec![block("L82")], "src/a.rs");
        let forged = BrokerMessage::Feedback {
            agent_id: "feat-y".to_string(),
            payload: FeedbackPayload {
                from: "feat-x".to_string(),
                errors: vec![format!("{CONFLICT_ACK_TAG} feat-x feat-y src/a.rs")],
            },
        };
        process_message(&state, &mut t, &forged, &cfg, now);
        let due = now + Duration::from_secs(10);
        assert_eq!(
            tick(&state, &mut t, &cfg, due),
            1,
            "a peer-sourced acknowledgement must not suppress the escalation"
        );
        assert_eq!(supervisor_questions(&state).len(), 1);
    }

    #[test]
    fn detector_materially_changed_conflict_escalates_despite_acknowledgement() {
        // Spec: A materially changed conflict escalates again despite
        // acknowledgement. Start additive (disjoint ranges) and acknowledged;
        // feat-y then re-declares a range that intersects feat-x's.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![range(10, 30)], vec![range(80, 120)], "src/a.rs");
        process_message(
            &state,
            &mut t,
            &ack_msg("feat-x", "feat-x", "feat-y", "src/a.rs"),
            &cfg,
            now,
        );
        let due = now + Duration::from_secs(10);
        assert_eq!(tick(&state, &mut t, &cfg, due), 0);
        assert!(supervisor_questions(&state).is_empty());

        // feat-y moves into feat-x's range — the overlap changes shape.
        process_message(
            &state,
            &mut t,
            &intent_msg_with_regions("feat-y", &[("src/a.rs", vec![range(15, 25)])], "y", 600),
            &cfg,
            due,
        );
        let after = due + Duration::from_secs(1);
        assert_eq!(
            tick(&state, &mut t, &cfg, after),
            1,
            "the acknowledgement is invalidated by a material change"
        );
        let q = supervisor_questions(&state);
        assert_eq!(q.len(), 1);
        if let BrokerMessage::Question { payload, .. } = &q[0] {
            assert!(payload.question.contains("src/a.rs"));
        } else {
            panic!("expected Question");
        }
    }

    #[test]
    fn detector_file_level_redeclaration_reescalates_an_additive_decision() {
        // An additive downgrade never reaches a human. If one side then
        // widens its claim to the WHOLE file, disjointness no longer holds
        // and the overlap becomes a true collision that has never been put
        // to anyone — it must escalate.
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![range(10, 30)], vec![range(80, 120)], "src/a.rs");
        let due = now + Duration::from_secs(10);
        assert_eq!(tick(&state, &mut t, &cfg, due), 2, "additive downgrade");
        assert!(supervisor_questions(&state).is_empty());

        // feat-y re-declares src/a.rs at file level (a bare path, no regions).
        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/a.rs"], "y widens to the whole file", 600),
            &cfg,
            due,
        );
        let after = due + Duration::from_secs(1);
        assert_eq!(
            tick(&state, &mut t, &cfg, after),
            1,
            "widening to the whole file flips additive to true"
        );
        let q = supervisor_questions(&state);
        assert_eq!(q.len(), 1, "exactly one escalation question");
        if let BrokerMessage::Question { payload, .. } = &q[0] {
            assert!(payload.question.contains("src/a.rs"));
        } else {
            panic!("expected Question");
        }
        // And it settles again: no second question on later ticks.
        assert_eq!(
            tick(&state, &mut t, &cfg, after + Duration::from_secs(30)),
            0
        );
        assert_eq!(supervisor_questions(&state).len(), 1);
    }

    #[test]
    fn detector_file_level_redeclaration_reescalates_an_acknowledged_conflict() {
        // The orchestrator adjudicated a region-scoped collision. One side
        // then claims the whole file — a materially different conflict, so
        // the acknowledgement no longer covers it.
        let (state, mut t, cfg, now) = setup_in_flight(
            vec![func("validate_token")],
            vec![func("validate_token")],
            "src/auth.rs",
        );
        process_message(
            &state,
            &mut t,
            &ack_msg("feat-x", "feat-x", "feat-y", "src/auth.rs"),
            &cfg,
            now,
        );
        let due = now + Duration::from_secs(10);
        assert_eq!(tick(&state, &mut t, &cfg, due), 0, "acknowledged — quiet");
        assert!(supervisor_questions(&state).is_empty());

        process_message(
            &state,
            &mut t,
            &intent_msg("feat-y", &["src/auth.rs"], "y takes the whole file", 600),
            &cfg,
            due,
        );
        let after = due + Duration::from_secs(1);
        assert_eq!(
            tick(&state, &mut t, &cfg, after),
            1,
            "the acknowledgement does not cover the widened conflict"
        );
        assert_eq!(supervisor_questions(&state).len(), 1);
    }

    #[test]
    fn detector_expiring_intents_do_not_invalidate_a_decision() {
        // An intent lapsing is an ABSENCE of declarations, not a change to
        // them — it must not re-open a settled conflict (otherwise every
        // acknowledged conflict re-escalates one TTL later).
        let (state, mut t, cfg, now) =
            setup_in_flight(vec![block("L82")], vec![block("L82")], "src/a.rs");
        let due = now + Duration::from_secs(10);
        assert_eq!(tick(&state, &mut t, &cfg, due), 1);
        // Intents were published with a 600s TTL; walk past it while both
        // agents keep reporting the file.
        let expired = now + Duration::from_secs(700);
        process_message(
            &state,
            &mut t,
            &status_msg("feat-x", &["src/a.rs"]),
            &cfg,
            expired,
        );
        assert_eq!(tick(&state, &mut t, &cfg, expired), 0);
        assert_eq!(supervisor_questions(&state).len(), 1);
    }
}
