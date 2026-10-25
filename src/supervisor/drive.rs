//! The `--unattended` drive loop (`unattended-operation` capability).
//!
//! `cmd_supervisor` runs this loop in-process (in the foreground
//! `git paw start --unattended` process) after the tmux session is built. The
//! loop keeps a multi-agent supervised wave moving with no human in the seat:
//! it polls on a ~15-second cadence, sweeps every coding-agent surface,
//! escalates each live permission prompt for a human decision WITHOUT blocking
//! the rest of the wave, detects completion, and exits with a summary.
//!
//! No orchestrator runs inside the session — it drives from outside tmux — so
//! the loop sweeps, nudges and submits on coding-agent surfaces only, never on
//! the dashboard or an unattributed surface, and it types no escalation into
//! any surface: the broker record, the exit summary and the manual-decision
//! log are the whole escalation. It approves no prompt.
//!
//! # Battle-tested heuristics encoded here
//!
//! The v0.6.0 dogfood proved a handful of operator-loop rules are load-bearing;
//! they are encoded as normative behaviour so they survive in-tool:
//!
//! - **Act only on a LIVE prompt** — the prompt's structural markers (option
//!   glyphs / `Do you want to …` / `Esc to cancel`) at the capture's tail,
//!   over a window spanning a full multi-option prompt block
//!   ([`crate::supervisor::permission_prompt::is_live_prompt`]); prompt-like text
//!   scrolled into history is ignored (D4).
//! - **Explicit per-pane capture** — one screen-tail read per surface through
//!   the terminal port, never a `for p in …` shell loop (D3).
//! - **Pane→agent resolution by owner** — the terminal backend reports which
//!   worktree owns each surface, matched by exact canonical path; never pane
//!   index or CLI-argument order ([`resolve_pane_agent`], D2).
//! - **Coding surfaces only** — every keystroke the loop sends (a nudge, a
//!   stale-input recovery, a buffered-input submit) reaches a surface the
//!   layout resolves as a coding agent; the dashboard and unattributed
//!   surfaces are never swept for a decision (`approval-auto`, GP-46).
//! - **Identity-keyed dedup** — repeated alerts collapse on
//!   `(agent_id, command-shape)` within a 5-minute window, never on the
//!   prompt's boilerplate text ([`DedupWindow`], W15-19, D7).
//! - **Non-blocking escalation** — a risky prompt is recorded for later human
//!   review while the rest of the wave keeps progressing (D10).
//!
//! # Testability
//!
//! Every side effect (pane enumeration, capture, keystroke dispatch, broker
//! `/status` fetch, the clock, broker publishing, and `sweep.sh learn`) is
//! behind a trait so [`drive_loop`] can be exercised end-to-end in memory with
//! fakes — no tmux, no real LLM, no interactive terminal. Enumeration,
//! capture and keystrokes go through the terminal port
//! ([`crate::terminal::TerminalBackend`]), which implements the three seams;
//! the production entry point [`run_drive_loop`] wires the tmux backend and
//! the real implementations of the rest.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};

use regex::Regex;
use serde::Deserialize;

use crate::broker::messages::{BrokerMessage, FeedbackPayload};
use crate::config::{
    CorrectionConfig, CustomCli, OnExhausted, claude_prompt_profile, resolve_prompt_profile,
    resolve_submit_delay_ms,
};
use crate::error::PawError;
use crate::session::{self, SessionStatus};
use crate::static_regex::static_regex;
use crate::surface::{SessionLayout, SurfaceRole};
use crate::terminal::TerminalBackend;

use super::approval_gate::{approval_dedup_key, live_prompt_in_tail};
use super::claim::PaneClaim;
use super::keys::KeyDispatcher;
use super::loop_binding::DriveLoopBinding;
use super::permission_prompt::{
    LIVE_PROMPT_TAIL, anchor_on_trailing_region, extract_command_slice,
    extract_path_from_file_prompt, is_idle_input_box_line, is_live_prompt, is_rule_line,
    strip_input_sigil,
};

/// Poll cadence: the loop re-sweeps every pane on approximately this interval.
pub const POLL_INTERVAL: Duration = Duration::from_secs(15);

/// The poll cadence this process's drive loop will actually run at:
/// [`POLL_INTERVAL`], or the `GIT_PAW_DRIVE_POLL_MS` override.
///
/// Exposed because the drive-loop binding's staleness window is derived from
/// the cadence (four ticks), and the binding is taken by the caller *before*
/// [`run_drive_loop`] builds its [`DriveConfig`] — so both sides have to read
/// the same answer from one place rather than each resolving the override.
#[must_use]
pub fn effective_poll_interval() -> Duration {
    duration_from_env_ms("GIT_PAW_DRIVE_POLL_MS", POLL_INTERVAL)
}

/// Heartbeat window: after approximately this long with no completion the loop
/// re-engages the human by exiting with a status summary rather than running
/// forever silently.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_mins(25);

/// Alert dedup window: a repeated `(agent_id, shape)` escalation collapses to a
/// single alert within this window (W15-19).
pub const DEDUP_WINDOW: Duration = Duration::from_mins(5);

pub use crate::surface::SUPERVISOR_AGENT_ID;

/// Learning category recorded when the loop absorbs friction it could not
/// auto-approve (per `learnings-supervisor-observation-channel`).
const LEARNING_CATEGORY: &str = "tooling_friction";

// ---------------------------------------------------------------------------
// Roster + pane resolution
// ---------------------------------------------------------------------------

/// A coding agent in the session, used to resolve a pane to its agent by
/// working directory. The dashboard is not listed here — it resolves by the
/// index the session's layout reserves for it.
#[derive(Debug, Clone)]
pub struct AgentPane {
    /// Broker agent id (slugified branch name).
    pub agent_id: String,
    /// Absolute worktree root the agent's surface runs in — its owner.
    pub worktree_path: PathBuf,
    /// The agent's git branch name, used at exit to ask git whether the
    /// branch is merged into the default branch (`drive-loop-exit-summary-truth`).
    pub branch: String,
}

/// Reads the session's coding-agent roster as the persisted state currently
/// records it.
///
/// A port, not a direct read, because [`drive_loop`] re-reads the roster every
/// sweep: an agent attached with `git paw add` while the loop runs must be
/// driven, and one detached with `git paw remove` must stop being driven. The
/// seam is what lets a test hand the loop a roster that changes mid-run.
pub trait RosterReader: std::fmt::Debug {
    /// The session's coding agents right now, or `None` when the state could
    /// not be read on this sweep — in which case the loop keeps the roster it
    /// last read rather than driving nothing.
    fn read_roster(&self) -> Option<Vec<AgentPane>>;
}

/// The production [`RosterReader`]: the session receipt for one repository.
#[derive(Debug, Clone)]
pub struct SessionFileRoster {
    repo_root: PathBuf,
}

impl SessionFileRoster {
    /// A reader for the session recorded against `repo_root`.
    #[must_use]
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }
}

impl RosterReader for SessionFileRoster {
    fn read_roster(&self) -> Option<Vec<AgentPane>> {
        let session = crate::session::find_session_for_repo(&self.repo_root).ok()??;
        Some(
            session
                .worktrees
                .iter()
                .map(|w| AgentPane {
                    agent_id: crate::broker::messages::slugify_branch(&w.branch),
                    worktree_path: w.worktree_path.clone(),
                    branch: w.branch.clone(),
                })
                .collect(),
        )
    }
}

/// Borrows `agents` as the `(agent_id, worktree root)` owner pairs
/// [`SessionLayout::resolve`] matches a surface's owning directory against.
fn surface_owners(agents: &[AgentPane]) -> impl Iterator<Item = (&str, &Path)> {
    agents
        .iter()
        .map(|a| (a.agent_id.as_str(), a.worktree_path.as_path()))
}

/// Resolves a pane to its [`SurfaceRole`] under `layout`.
///
/// A thin delegate to the single resolver in [`crate::surface`], kept here as
/// the drive loop's name for the operation. Coding agents resolve by matching
/// the surface's owner, as the terminal backend reports it, against a known
/// `worktree_path` by exact canonical path ([`crate::surface::resolve_agent`])
/// — NEVER by pane index or CLI-argument order, because pane indices
/// are neither alphabetical nor argument-ordered and drift on layout changes
/// (D2). Only then does the layout's reserved-index table decide.
#[must_use]
pub fn resolve_pane_agent(
    layout: SessionLayout,
    pane_index: usize,
    owner: &str,
    agents: &[AgentPane],
) -> SurfaceRole {
    layout.resolve(pane_index, owner, surface_owners(agents))
}

// ---------------------------------------------------------------------------
// Permission prompts
// ---------------------------------------------------------------------------

/// Escalation verdict label for a live agent-CLI permission prompt.
///
/// There is exactly one: git-paw does not judge the prompted command, so it
/// has nothing to say about the prompt beyond "a human needs to decide this".
/// It replaced the retired classifier's `danger` / `unknown` pair, which
/// implied a safety verdict git-paw cannot reliably make.
const PERMISSION_PROMPT_VERDICT: &str = "permission-prompt";

// ---------------------------------------------------------------------------
// Dedup
// ---------------------------------------------------------------------------

/// Derives the dedup *shape* of a prompt from its command/agent identity, never
/// from the boilerplate footer text (W15-19).
///
/// The shape is the prompted command slice (the text between the `Bash command`
/// / `Bash(…)` header and the confirmation question). When no command header is
/// present the file-operation target path is used as a stable fallback; when
/// neither is present the shape is empty (a wait-for-clear token — the next
/// distinct prompt re-confirms fresh once this one clears).
#[must_use]
pub fn dedup_shape(captured: &str) -> String {
    if let Some(cmd) = extract_command_slice(captured) {
        return cmd;
    }
    extract_path_from_file_prompt(captured).unwrap_or_default()
}

/// Tracks `(agent_id, shape)` alert keys within a rolling window so a repeated
/// prompt observed on every poll produces exactly one alert per window.
#[derive(Debug)]
pub struct DedupWindow {
    window: Duration,
    seen: HashMap<String, Instant>,
}

impl DedupWindow {
    /// Creates a dedup tracker with the given window.
    #[must_use]
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            seen: HashMap::new(),
        }
    }

    /// Returns `true` when an alert for `key` should be emitted now — i.e. it
    /// has not been emitted within the window — and records the emission.
    /// Returns `false` for a repeat within the window.
    pub fn should_emit(&mut self, key: &str, now: Instant) -> bool {
        match self.seen.get(key) {
            Some(&last) if now.duration_since(last) < self.window => false,
            _ => {
                self.seen.insert(key.to_string(), now);
                true
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Worker lifecycle phase
// ---------------------------------------------------------------------------

/// A coding agent's lifecycle phase, derived from its broker status string.
///
/// The loop reacts to *phases* — "has this worker finished?", "is a merge
/// decision live?" — not to status spellings. Deriving the phase through the
/// single [`WorkerPhase::from_status`] mapping keeps that judgment in one named
/// type instead of spreading status-string comparisons across the sweep.
///
/// [`WorkerPhase::Other`] absorbs every status the loop does not react to: the
/// broker's status vocabulary is open, and an unrecognized status must never be
/// mistaken for a phase the loop acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerPhase {
    /// `working` — the worker is actively progressing its task.
    Working,
    /// `idle` — the worker is alive but not progressing.
    Idle,
    /// `blocked` — the worker is waiting on a peer or the supervisor.
    Blocked,
    /// `committed` — the worker committed; verification and merge are due.
    Committed,
    /// `verified` — the supervisor's gates passed for this worker.
    Verified,
    /// `done` — the worker reported its task complete.
    Done,
    /// Any status the loop does not react to.
    Other,
}

impl WorkerPhase {
    /// Maps a broker status string to its lifecycle phase.
    ///
    /// This is the loop's only status→phase mapping; anything unrecognized is
    /// [`WorkerPhase::Other`].
    fn from_status(status: &str) -> Self {
        match status {
            "working" => Self::Working,
            "idle" => Self::Idle,
            "blocked" => Self::Blocked,
            "committed" => Self::Committed,
            "verified" => Self::Verified,
            "done" => Self::Done,
            _ => Self::Other,
        }
    }

    /// The broker status string this phase is derived from — the empty string
    /// for [`WorkerPhase::Other`], which stands for no particular status and so
    /// belongs to none of the phase sets below.
    fn as_status(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Idle => "idle",
            Self::Blocked => "blocked",
            Self::Committed => "committed",
            Self::Verified => "verified",
            Self::Done => "done",
            Self::Other => "",
        }
    }

    /// Whether the worker has FINISHED — the [`AGENT_COMPLETE_STATUSES`] phases,
    /// i.e. [`WorkerPhase::Verified`] / [`WorkerPhase::Done`].
    ///
    /// Delegating to the array rather than matching the two variants directly
    /// keeps the array the single authoritative definition, so the predicate
    /// cannot drift from it.
    fn is_completed(self) -> bool {
        AGENT_COMPLETE_STATUSES.contains(&self.as_status())
    }

    /// Whether the phase makes a **merge decision** live — the
    /// [`MERGE_CANDIDATE_STATUSES`] phases, i.e. [`WorkerPhase::Committed`] /
    /// [`WorkerPhase::Done`]. Delegates to the array for the same reason
    /// [`WorkerPhase::is_completed`] does.
    ///
    /// No production caller while no orchestrator runs inside the session: the
    /// headless hand-off (`orchestrator-escalation-handoff`) is what keys a
    /// merge decision off it. Kept because `code-architecture` requires the
    /// predicate on the enum.
    #[cfg_attr(not(test), expect(dead_code))]
    fn is_merge_candidate(self) -> bool {
        MERGE_CANDIDATE_STATUSES.contains(&self.as_status())
    }

    /// How far along the worker's lifecycle this phase represents, used to
    /// track the most-progressed phase ever observed for an agent (GP-05).
    ///
    /// [`WorkerPhase::Working`], [`WorkerPhase::Idle`], [`WorkerPhase::Blocked`],
    /// and [`WorkerPhase::Other`] are all "still going" (rank 0);
    /// [`WorkerPhase::Committed`] is closer to done (rank 1); the two terminal
    /// phases, [`WorkerPhase::Verified`] and [`WorkerPhase::Done`], share the
    /// top rank (2) — a resolved worker is never displayed as regressing back
    /// to a lower phase because of a later generic heartbeat.
    fn rank(self) -> u8 {
        match self {
            Self::Working | Self::Idle | Self::Blocked | Self::Other => 0,
            Self::Committed => 1,
            Self::Verified | Self::Done => 2,
        }
    }
}

// ---------------------------------------------------------------------------
// Completion detection
// ---------------------------------------------------------------------------

/// Agent statuses that count as "task complete" for the all-agents-checked
/// completion rule. `committed` is deliberately excluded — a commit is not yet
/// a verified completion.
///
/// The authoritative definition of [`WorkerPhase::is_completed`]; decision sites
/// read the phase predicate, never this array.
const AGENT_COMPLETE_STATUSES: &[&str] = &["verified", "done"];

/// Supervisor statuses that count as a terminal PASS/FAIL verdict for the wave.
const SUPERVISOR_VERDICT_STATUSES: &[&str] = &[
    "done", "verified", "pass", "fail", "passed", "failed", "complete",
];

/// Why the loop considers the wave complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionReason {
    /// The supervisor published a terminal PASS/FAIL verdict.
    Verdict,
    /// Every coding agent's tasks are checked complete.
    AllTasksChecked,
}

/// Detects wave completion from a broker `/status` snapshot.
///
/// Completion is recognized when either the supervisor row carries a terminal
/// verdict status, or every coding agent in `coding_agent_ids` is in a
/// task-complete status. Returns `None` when the wave is still in progress.
#[must_use]
pub fn detect_completion(
    rows: &[AgentStatusRow],
    coding_agent_ids: &[String],
) -> Option<CompletionReason> {
    if let Some(sup) = rows.iter().find(|r| r.agent_id == SUPERVISOR_AGENT_ID)
        && SUPERVISOR_VERDICT_STATUSES.contains(&sup.status.as_str())
    {
        return Some(CompletionReason::Verdict);
    }
    if !coding_agent_ids.is_empty()
        && coding_agent_ids.iter().all(|id| {
            rows.iter()
                .any(|r| &r.agent_id == id && WorkerPhase::from_status(&r.status).is_completed())
        })
    {
        return Some(CompletionReason::AllTasksChecked);
    }
    None
}

// ---------------------------------------------------------------------------
// Correction loop (`supervisor-correction-loop`)
// ---------------------------------------------------------------------------

/// Escalation verdict label for a branch whose correction budget is spent.
const CORRECTION_EXHAUSTED_VERDICT: &str = "correction-exhausted";

/// Escalation verdict label for the one-time early heads-up emitted before a
/// slow-converging branch reaches `max_cycles`.
const CORRECTION_SLOW_VERDICT: &str = "correction-slow";

/// Returns the gate tag of a failing gate verdict carried by `payload`, or
/// `None` when the feedback is not a gate failure addressed to a branch.
///
/// Only feedback published by the supervisor and tagged with one of
/// `gate_tags` (`[supervisor.correction] gate_tags`) counts; everything else
/// on the shared `agent.feedback` channel passes through untouched.
#[must_use]
pub fn gate_failure(payload: &FeedbackPayload, gate_tags: &[String]) -> Option<String> {
    if payload.from != SUPERVISOR_AGENT_ID {
        return None;
    }
    payload
        .errors
        .iter()
        .find_map(|error| bracketed_gate(error, gate_tags))
}

/// Extracts an error line's leading `[<gate>]` tag when it names one of
/// `gate_tags` (case-insensitive).
fn bracketed_gate(error: &str, gate_tags: &[String]) -> Option<String> {
    let (name, _) = error.trim_start().strip_prefix('[')?.split_once(']')?;
    let name = name.trim();
    gate_tags
        .iter()
        .find(|tag| name.eq_ignore_ascii_case(tag))
        .cloned()
}

/// One branch's correction bookkeeping.
#[derive(Debug, Default)]
struct BranchCorrection {
    /// Re-engagements actually sent to this branch's pane so far.
    cycles: u32,
    /// Gate-tagged feedback awaiting injection into the worker's pane.
    pending: Option<String>,
    /// Whether the one-time early heads-up has already fired.
    early_flagged: bool,
    /// Whether the exhaustion policy has already been applied.
    exhausted: bool,
}

/// Per-branch correction-cycle state for the drive loop.
///
/// Held in the loop's in-process state alongside the alert [`DedupWindow`] and
/// deliberately NOT persisted: a supervisor restart resets every branch's
/// budget, which is the intuitive behaviour for a within-session policy.
#[derive(Debug, Default)]
pub struct CorrectionState {
    branches: HashMap<String, BranchCorrection>,
}

impl CorrectionState {
    /// Creates empty correction state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks `agent_id` as needing correction, carrying the gate-tagged
    /// feedback text that will be injected into its pane.
    ///
    /// A branch whose exhaustion policy has already been applied is never
    /// re-marked — the loop must not re-enter a cycle it already gave up on.
    pub fn mark_gate_failure(&mut self, agent_id: &str, feedback: String) {
        let entry = self.branches.entry(agent_id.to_string()).or_default();
        if entry.exhausted {
            return;
        }
        entry.pending = Some(feedback);
    }

    /// Clears the correction bookkeeping of every branch that reached a
    /// terminal PASS verdict in `rows`, so a later unrelated failure starts
    /// from a fresh budget and the exhaustion policy is not applied.
    pub fn clear_completed(&mut self, rows: &[AgentStatusRow]) {
        for row in rows {
            if WorkerPhase::from_status(&row.status).is_completed() {
                self.branches.remove(&row.agent_id);
            }
        }
    }

    /// Correction-cycle count for `agent_id`; `0` for an untracked branch.
    #[must_use]
    pub fn cycles(&self, agent_id: &str) -> u32 {
        self.branches.get(agent_id).map_or(0, |b| b.cycles)
    }

    /// Branches currently marked for correction, as `(agent_id, feedback)`
    /// pairs sorted by agent id so a sweep's ordering is deterministic.
    fn pending(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .branches
            .iter()
            .filter_map(|(id, b)| b.pending.clone().map(|f| (id.clone(), f)))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Records a delivered re-engagement: clears the pending feedback and
    /// returns the branch's new cycle count.
    fn record_reengagement(&mut self, agent_id: &str) -> u32 {
        let entry = self.branches.entry(agent_id.to_string()).or_default();
        entry.pending = None;
        entry.cycles += 1;
        entry.cycles
    }

    /// Marks the one-time early heads-up as fired, returning `true` the first
    /// time it is called for `agent_id`.
    fn take_early_flag(&mut self, agent_id: &str) -> bool {
        let entry = self.branches.entry(agent_id.to_string()).or_default();
        if entry.early_flagged {
            return false;
        }
        entry.early_flagged = true;
        true
    }

    /// Marks the exhaustion policy as applied, returning `true` the first time
    /// it is called for `agent_id`.
    fn take_exhaustion(&mut self, agent_id: &str) -> bool {
        let entry = self.branches.entry(agent_id.to_string()).or_default();
        if entry.exhausted {
            return false;
        }
        entry.exhausted = true;
        entry.pending = None;
        true
    }
}

/// A branch whose correction budget was spent, with the policy applied to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExhaustedBranch {
    /// The branch's broker agent id.
    pub agent_id: String,
    /// Number of correction cycles the loop drove before giving up.
    pub cycles: u32,
    /// The `on_exhausted` policy applied.
    pub policy: OnExhausted,
}

/// Maximum length of free text the loop types into a pane. A gate verdict or an
/// escalation can carry a long error list; the pane only needs enough to
/// re-orient its model, which then reads the full detail from the broker.
const INJECTED_TEXT_MAX_CHARS: usize = 600;

/// Flattens `text` to a single line and caps it at [`INJECTED_TEXT_MAX_CHARS`].
///
/// Both steps are `send-keys` requirements, not cosmetics: an embedded newline
/// would submit the message early (splitting one prompt into several), and an
/// unbounded payload would flood the pane's input box.
fn one_line_capped(text: &str) -> String {
    let mut body = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if body.chars().count() > INJECTED_TEXT_MAX_CHARS {
        body = body
            .chars()
            .take(INJECTED_TEXT_MAX_CHARS)
            .collect::<String>();
        body.push_str(" ...");
    }
    body
}

/// Renders the text injected into a gate-failed worker's pane.
///
/// Names the failing gate, carries its feedback, and instructs the worker to
/// fix and re-verify — so a blocked worker acts on the verdict without polling
/// an inbox it is not polling. Flattened and capped by [`one_line_capped`].
#[must_use]
pub fn reengagement_text(gate: &str, feedback: &str) -> String {
    one_line_capped(&format!(
        "Supervisor gate '{gate}' failed - fix the reported errors in your worktree, \
         then commit and stand by for re-verification. Feedback: {feedback}"
    ))
}

/// Renders the escalation context for a worker's ambiguous question — the text
/// a headless orchestrator invocation reads (`orchestrator-escalation-handoff`).
/// A pure renderer: no orchestrator runs inside the session, so nothing here is
/// typed into a surface.
///
/// The worker published `agent.question` precisely because it could not decide,
/// so the question itself is the ambiguity signal — the loop adds no classifier
/// of its own. The text tells the orchestrator to answer from the specs and
/// cross-agent state and to reach for the human only when the call is genuinely
/// undecidable.
#[must_use]
pub fn question_handoff_text(agent_id: &str, question: &str) -> String {
    one_line_capped(&format!(
        "Judgment call handed to you: {agent_id} is waiting on an answer and is \
         blocked until it arrives. Answer it from the specs and cross-agent state, \
         publish the answer to {agent_id}, and escalate to the human only if it is \
         genuinely undecidable. Question: {question}"
    ))
}

/// Renders the escalation context for a worker artifact at a
/// `committed`/`done` status — the point at which a **merge decision**
/// becomes live. A pure renderer, kept for the headless
/// orchestrator invocation (`orchestrator-escalation-handoff`); nothing here is
/// typed into a surface.
///
/// A committed or done branch is what verify-then-merge sequencing keys off.
#[must_use]
pub fn merge_handoff_text(agent_id: &str, status: &str) -> String {
    one_line_capped(&format!(
        "Merge decision handed to you: {agent_id} reached status '{status}'. Verify \
         it, then decide where it sits in the merge sequence relative to the other \
         branches in flight; escalate to the human only if the sequencing is \
         genuinely undecidable."
    ))
}

// ---------------------------------------------------------------------------
// Escalation + summary
// ---------------------------------------------------------------------------

/// Something surfaced for later human review: a risky/unknown permission
/// prompt, or a correction-loop signal about a branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Escalation {
    /// Agent the escalation concerns.
    pub agent_id: String,
    /// Verdict label: the classifier's `danger` / `unknown` for a prompt, or
    /// [`CORRECTION_EXHAUSTED_VERDICT`] / [`CORRECTION_SLOW_VERDICT`] for a
    /// correction-loop signal.
    pub verdict: String,
    /// The prompted command, or a short description of the signal, for the
    /// summary.
    pub command: String,
}

impl Escalation {
    /// Renders the supervisor-facing question text for this escalation.
    ///
    /// The framing follows the verdict, because the kinds ask the human for
    /// different things. A `permission-prompt` verdict means a pane is waiting
    /// on a decision only a human can make, and says plainly that git-paw does
    /// not approve prompts, so nobody waits for it to. A correction-loop
    /// verdict means a branch is not converging; there is no prompt to review,
    /// so the text must not claim one.
    #[must_use]
    pub fn question(&self) -> String {
        match self.verdict.as_str() {
            CORRECTION_EXHAUSTED_VERDICT => format!(
                "{} has exhausted its supervisor correction budget without passing its \
                 gate; the unattended loop will not re-engage it again. Please review the \
                 branch and decide how to proceed. Detail: {}",
                self.agent_id, self.command
            ),
            CORRECTION_SLOW_VERDICT => format!(
                "{} is converging slowly under the supervisor correction loop — a heads-up \
                 before its correction budget is spent; no action is required yet. \
                 Detail: {}",
                self.agent_id, self.command
            ),
            PERMISSION_PROMPT_VERDICT => format!(
                "{} is waiting on a permission prompt that needs a human decision. \
                 git-paw does not approve prompts — review the pane and answer it, or \
                 tell the orchestrator to run `sweep.sh approve`. Command: {}",
                self.agent_id, self.command
            ),
            _ => format!(
                "{} is stalled and needs a human decision ({}). Please review the pane. \
                 Detail: {}",
                self.agent_id, self.verdict, self.command
            ),
        }
    }

    /// Renders the task text a headless orchestrator invocation reads for this
    /// escalation (`orchestrator-escalation-handoff`). A pure renderer: no
    /// orchestrator runs inside the session, so the drive loop types this into
    /// no surface.
    ///
    /// Most verdicts ask the *orchestrator* to decide and act — it can publish
    /// an answer or declare a branch unrecoverable itself. The human framing is
    /// deliberately not reused there: telling a model to "review the pane and
    /// decide manually" invites it to wait for someone else.
    ///
    /// A `permission-prompt` verdict is the exception, and asks for a RELAY
    /// rather than a decision (design D2). An orchestrator that approves on its
    /// own reading of the command is just another command classifier — one
    /// reading untrusted pane text, so exposed to prompt injection — and
    /// git-paw's own bundled skill would be the thing teaching it to do that.
    /// So the hand-off tells it to put the prompt in front of the human and to
    /// run `sweep.sh approve` only when the human says so.
    ///
    /// A `"cli-unavailable"` verdict paraphrases its `command` instead of
    /// quoting it: the `cli-unavailable` verdict's `command` is the matched
    /// banner LINE VERBATIM (`sweep_pane`), and a reader that echoes this text
    /// into a terminal git-paw scans must not plant a copy of the marker there
    /// (`cli-unavailable-detection-gaps`).
    #[must_use]
    pub fn handoff_text(&self) -> String {
        if self.verdict == "cli-unavailable" {
            return one_line_capped(&format!(
                "Judgment call handed to you (cli-unavailable / {}): that agent's CLI \
                 appears to have stopped serving (a usage/session-limit or login-loss \
                 banner was detected on its pane). Decide it now from the specs and \
                 cross-agent state and act on it; escalate to the human only if it is \
                 genuinely undecidable.",
                self.agent_id
            ));
        }
        if self.verdict == PERMISSION_PROMPT_VERDICT {
            return one_line_capped(&format!(
                "Relay to the human (permission-prompt / {}): that agent is waiting on a \
                 permission prompt for: {}. Do NOT decide it yourself and do NOT approve \
                 it on your own reading of the command. Tell the human what is being \
                 asked and run `sweep.sh approve` only if they explicitly tell you to.",
                self.agent_id, self.command
            ));
        }
        one_line_capped(&format!(
            "Judgment call handed to you ({} / {}): {}. Decide it now from the specs \
             and cross-agent state and act on it; escalate to the human only if it is \
             genuinely undecidable.",
            self.verdict, self.agent_id, self.command
        ))
    }
}

/// The reason the loop exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveOutcome {
    /// The wave completed with no prompts left for human review.
    Completed,
    /// The wave completed (or the loop exited) with escalations awaiting review.
    EscalatedForReview,
    /// A stuck/bloat signal fired.
    Stuck,
    /// The heartbeat elapsed without completion.
    Heartbeat,
    /// The loop's bound session instance was purged, stopped, or replaced by a
    /// new same-named session (GP-06) — it exits without acting on the new
    /// session's panes.
    SessionTornDown,
}

impl DriveOutcome {
    /// Human-readable outcome label for the summary.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            DriveOutcome::Completed => "completed",
            DriveOutcome::EscalatedForReview => "escalated-for-review",
            DriveOutcome::Stuck => "stuck",
            DriveOutcome::Heartbeat => "heartbeat",
            DriveOutcome::SessionTornDown => "session-torn-down",
        }
    }
}

/// The exit summary the loop prints and returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveSummary {
    /// Overall outcome.
    pub outcome: DriveOutcome,
    /// Per-agent final state `(agent_id, status)` at exit.
    pub agent_states: Vec<(String, String)>,
    /// Deduped escalations awaiting human review.
    pub escalations: Vec<Escalation>,
    /// Branches whose correction budget was spent, with the applied policy.
    pub exhausted: Vec<ExhaustedBranch>,
    /// Pointer to the broker log (path or URL), if known.
    pub broker_log_hint: Option<String>,
    /// Pointer to the captured learnings file, if known.
    pub learnings_hint: Option<String>,
    /// Wave-level "N/M branches merged" line, present for a [`DriveOutcome::Completed`]
    /// wave (GP-05) — the fallback the per-agent list has when its own
    /// resolution is sparse or unavailable, and a useful roll-up otherwise.
    pub merged_summary: Option<String>,
}

/// The summary line stating the loop's CLI-unavailability coverage: the
/// orchestrator runs outside the session, so the loop never watches its CLI.
pub const ORCHESTRATOR_NOT_MONITORED_LINE: &str =
    "The orchestrator runs outside the session; its CLI is not monitored by the loop.";

impl DriveSummary {
    /// Renders the summary as human-readable text for the terminal.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        // Writing to a `String` is infallible, so the `write!` results are
        // deliberately discarded rather than unwrapped (no `unwrap` in
        // non-test code).
        let mut out = String::new();
        let _ = writeln!(
            out,
            "Unattended drive loop exited: {}",
            self.outcome.label()
        );
        if let Some(merged) = &self.merged_summary {
            let _ = writeln!(out, "{merged}");
        }
        let _ = writeln!(out, "{ORCHESTRATOR_NOT_MONITORED_LINE}");

        out.push_str("\nPer-agent final state:\n");
        if self.agent_states.is_empty() {
            out.push_str("  (no agent status recorded)\n");
        } else {
            for (agent, status) in &self.agent_states {
                let _ = writeln!(out, "  - {agent}: {status}");
            }
        }

        let _ = writeln!(
            out,
            "\nEscalations awaiting human review: {}",
            self.escalations.len()
        );
        for e in &self.escalations {
            let _ = writeln!(out, "  - [{}] {}: {}", e.verdict, e.agent_id, e.command);
        }

        if !self.exhausted.is_empty() {
            let _ = writeln!(
                out,
                "\nCorrection budget exhausted: {}",
                self.exhausted.len()
            );
            for b in &self.exhausted {
                let policy = match b.policy {
                    OnExhausted::Escalate => "escalated",
                    OnExhausted::Abandon => "abandoned (marked failed)",
                };
                let _ = writeln!(out, "  - {}: {} cycle(s), {policy}", b.agent_id, b.cycles);
            }
        }

        if let Some(log) = &self.broker_log_hint {
            let _ = writeln!(out, "\nBroker log: {log}");
        }
        if let Some(learn) = &self.learnings_hint {
            let _ = writeln!(out, "Captured learnings: {learn}");
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Side-effect traits (injected so the loop is testable without tmux/broker)
// ---------------------------------------------------------------------------

/// One surface in the session, as the terminal backend lists it.
#[derive(Debug, Clone)]
pub struct PaneInfo {
    /// The surface's position in the session's layout.
    pub pane_index: usize,
    /// The working tree that owns the surface, as the backend reports it.
    pub owner: String,
}

/// Enumerates the session's surfaces (one explicit listing per sweep).
pub trait PaneEnumerator {
    /// Returns every pane in `session`.
    fn list_panes(&self, session: &str) -> Vec<PaneInfo>;
}

/// Captures a single surface's screen (one explicit read per surface — never
/// a shell `for` loop).
pub trait PaneCapture {
    /// Returns the captured text of pane `pane_index` in `session`.
    fn capture(&self, session: &str, pane_index: usize) -> String;
}

/// Fetches the broker `/status` snapshot for completion detection.
pub trait StatusFetcher {
    /// Returns the current agent status rows, or an empty vec on error.
    fn fetch(&self) -> Vec<AgentStatusRow>;
}

/// Observes the broker's message stream so the loop can react to messages the
/// `/status` snapshot does not carry — currently `agent.feedback` gate
/// verdicts, which drive the correction loop.
///
/// Implementations return only messages published SINCE the previous call, so
/// one gate failure starts exactly one correction cycle.
pub trait MessageObserver {
    /// Returns broker messages published since the previous call.
    fn poll_new(&self) -> Vec<BrokerMessage>;
}

/// The loop's clock, so heartbeat/poll timing is deterministic in tests.
pub trait Clock {
    /// The current instant.
    fn now(&self) -> Instant;
    /// Sleeps for `dur` (advances a fake clock in tests).
    fn sleep(&self, dur: Duration);
}

/// Publishes approval-audit and escalation alerts to the broker.
pub trait AlertSink {
    /// Records an auto-approval in the broker log BEFORE keystrokes are sent
    /// (per `automatic-approval`), so a crash mid-action still leaves a trail.
    /// Surfaces a risky/unknown prompt for later human review.
    fn escalate(&mut self, escalation: &Escalation);
}

/// Records qualitative learnings via `sweep.sh learn` (never raw curl).
pub trait LearningSink {
    /// Records a learning. `body` is a JSON object string.
    fn record(&mut self, category: &str, title: &str, body: &str);
}

/// Records a permission prompt the loop escalated for a human decision
/// (`approval-pattern-surfacing`).
///
/// git-paw cannot observe the in-pane answer — it runs outside the agent's
/// CLI process — so what it records is the prompt it *forwarded*, not an
/// approval. The recorded semantic is "this command required a manual
/// decision", a superset of approvals, and `git paw approvals` aggregates it
/// into a hint about which commands belong in the agent CLI's own allow
/// rules.
///
/// Only a live permission prompt is a subject here. A CLI-unavailability
/// banner, a wedged nudge and a correction-loop signal are escalations too,
/// but none of them is a command awaiting a decision, so none reaches this
/// sink — see [`sweep_pane`], which calls it from the permission-prompt arm
/// alone.
pub trait ManualDecisionSink {
    /// Records that `agent_id`'s pane, whose content is `captured`, is
    /// waiting on a permission prompt that was escalated for a human.
    ///
    /// Called once per emitted escalation: after the dedup window has
    /// admitted it, so a prompt re-seen inside the window is not recorded
    /// twice.
    fn record_forwarded(&mut self, agent_id: &str, captured: &str);
}

/// Performs the git-level actions of `supervisor-branch-refresh`: the two
/// gate inputs that require shelling out to git (conflict prediction,
/// behind-default), the rebase itself, and the post-refresh worker
/// notification. Abstracted so the whole orchestration
/// ([`refresh_branches_after_merge`]) is testable without a real git
/// repository or broker.
///
/// Every method takes the worktree path rather than a branch name: the
/// caller ([`AgentPane`]) tracks worktree paths, not branch names, and the
/// production implementation resolves the currently-checked-out branch from
/// the worktree itself ([`crate::git::current_branch`]) — the same source of
/// truth `git` itself would use, rather than a name that could drift from
/// what is actually checked out.
pub trait BranchRefresher {
    /// Whether rebasing the branch checked out in `worktree_path` onto
    /// `default_branch` is predicted to conflict (`git merge-tree`). An
    /// unresolvable comparison (branch name or prediction failure) is
    /// reported as a conflict — the safe default for an unreadable gate.
    fn predict_conflict(&self, worktree_path: &Path, default_branch: &str) -> bool;
    /// Whether the branch checked out in `worktree_path` is behind
    /// `default_branch`. An unresolvable comparison is reported as NOT
    /// behind — the safe default for an unreadable gate (no rebase is
    /// attempted).
    fn is_behind(&self, worktree_path: &Path, default_branch: &str) -> bool;
    /// Rebases the branch checked out in `worktree_path` onto the default
    /// branch.
    fn rebase(&mut self, worktree_path: &Path) -> Result<(), PawError>;
    /// Notifies `agent_id` that its branch HEAD was rewritten by a
    /// successful refresh.
    fn notify(&mut self, agent_id: &str, default_branch: &str);
}

/// Checks whether the drive loop's bound session instance is still the live,
/// active one (GP-06) — not purged, stopped, or replaced by a newer
/// same-named session.
///
/// Implementations compare a durable instance token captured at loop start
/// (the session's start receipt / creation timestamp) against the live
/// session record on disk, re-read on every call so a mid-run teardown is
/// caught on the very next tick.
pub trait SessionInstanceGuard {
    /// Returns `true` when the bound session instance is still current.
    fn is_current(&self) -> bool;
}

/// The ref-derived merge truth for one coding branch, resolved at exit
/// (`drive-loop-exit-summary-truth`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeState {
    /// Whether the branch tip is an ancestor of the default branch.
    pub merged: bool,
    /// Commits on the branch not on the default branch (0 when `merged`).
    pub ahead: usize,
    /// The branch's short tip SHA.
    pub tip: String,
}

/// Answers the exit-summary merge question for one branch by asking git —
/// never loop state (D1). Injected on [`DriveDeps`] so [`drive_loop`] stays
/// testable without a real repository.
pub trait MergeCheck {
    /// Resolves `branch`'s merge state against the default branch.
    ///
    /// # Errors
    /// Returns an error when the comparison cannot be made (a deleted
    /// branch, a bad ref). Callers must treat an `Err` as **unmerged** — the
    /// fail-safe direction (D2) — never as an unknown to be skipped.
    fn merge_state(&self, branch: &str) -> Result<MergeState, PawError>;

    /// Reads `branch`'s current full tip SHA — the narrow query the
    /// heartbeat uses to observe a branch tip moving between two ticks
    /// (`unattended-heartbeat-progress`). Full, so an abbreviation can never
    /// make two different tips compare equal. Distinct from
    /// [`MergeState::tip`], which is short and empty for a merged branch.
    ///
    /// # Errors
    /// Returns an error when the tip cannot be read (a deleted branch, a bad
    /// ref). Callers treat an `Err` as no observation — never as progress,
    /// and never as a reason to stop the loop.
    fn branch_tip(&self, branch: &str) -> Result<String, PawError>;
}

/// Bundled dependencies for [`drive_loop`].
pub struct DriveDeps<'a> {
    /// Enumerates panes each sweep.
    pub enumerator: &'a dyn PaneEnumerator,
    /// Captures pane content.
    pub capturer: &'a dyn PaneCapture,
    /// Dispatches approval keystrokes.
    pub dispatcher: &'a mut dyn KeyDispatcher,
    /// Fetches broker `/status`.
    pub status: &'a dyn StatusFetcher,
    /// Observes newly published broker messages (gate-failure feedback).
    pub messages: &'a dyn MessageObserver,
    /// The clock.
    pub clock: &'a dyn Clock,
    /// Resolves the ref-derived merge truth for the exit summary.
    pub merge_check: &'a dyn MergeCheck,
    /// Publishes alerts.
    pub alerts: &'a mut dyn AlertSink,
    /// Records learnings.
    pub learnings: &'a mut dyn LearningSink,
    /// Records each escalated permission prompt to the session's
    /// manual-decision log (`approval-pattern-surfacing`).
    pub manual_decisions: &'a mut dyn ManualDecisionSink,
    /// Performs branch-refresh git actions and worker notification.
    pub refresher: &'a mut dyn BranchRefresher,
    /// Wedged nudges recorded this run, deduped on `(agent_id, command)` so a
    /// persistently stuck line is escalated once rather than every sweep
    /// (`idle-pane-nudge-submission`). Folded into the exit summary's
    /// `escalations` by [`drive_loop`] before the outcome is derived — the
    /// same place `drive-loop-exit-summary-truth` appends its `[unmerged]`
    /// entries.
    pub wedged: Vec<Escalation>,
}

impl DriveDeps<'_> {
    /// Records `escalation` as wedged and returns whether it is NEW — the
    /// first time this `(agent_id, command)` pair has wedged this run.
    ///
    /// Deduping here, rather than in each caller, is what keeps a
    /// persistently stuck line from re-escalating to the broker on every
    /// sweep: [`recover_stale_input`]'s callers only call
    /// [`AlertSink::escalate`] when this returns `true`.
    fn record_wedged(&mut self, escalation: Escalation) -> bool {
        let is_new = !self
            .wedged
            .iter()
            .any(|e| e.agent_id == escalation.agent_id && e.command == escalation.command);
        if is_new {
            self.wedged.push(escalation);
        }
        is_new
    }
}

/// Tuning knobs for [`drive_loop`].
#[derive(Debug, Clone)]
pub struct DriveConfig {
    /// Poll cadence.
    pub poll_interval: Duration,
    /// Heartbeat window.
    pub heartbeat: Duration,
    /// Dedup window.
    pub dedup_window: Duration,
    /// Correction-loop policy (`[supervisor.correction]`). The default leaves
    /// `auto_loopback` off, so the loop sends no re-engagement keystrokes.
    pub correction: CorrectionConfig,
    /// Whether `[supervisor] learnings` is enabled — gates the exhaustion
    /// learning (no telemetry without consent).
    pub learnings_enabled: bool,
    /// Broker log pointer for the summary.
    pub broker_log_hint: Option<String>,
    /// Learnings file pointer for the summary.
    pub learnings_hint: Option<String>,
    /// Whether `supervisor-branch-refresh` is enabled
    /// (`[supervisor] branch_refresh`). Default-off: a live agent's branch is
    /// rewritten only on explicit opt-in.
    pub branch_refresh_enabled: bool,
    /// `[clis.<name>]` table, consulted by [`resolve_submit_delay_ms`] to
    /// resolve the per-CLI nudge settle delay (`drive-loop-actuator-robustness`,
    /// GP-15) — the same resolver the boot-prompt injection path uses.
    pub clis: HashMap<String, CustomCli>,
    /// The repository's default branch name, resolved once via
    /// [`crate::git::default_branch`] — used only to label the ref-derived
    /// merged/unmerged text in the exit summary (`drive-loop-exit-summary-truth`).
    pub default_branch: String,
    /// The session's reserved-index record. Every role the loop resolves comes
    /// from it, so no decision compares a pane index to a literal.
    pub layout: SessionLayout,
    /// Where [`drive_loop`] re-reads its coding-agent roster each sweep, so an
    /// agent attached or detached while the loop runs is picked up or dropped
    /// (`supervisor-unattended-operation`: "The drive loop follows the live
    /// agent roster").
    ///
    /// Every production launch supplies a [`SessionFileRoster`]. `None` —
    /// the default, which exists so a unit test that is not about the roster
    /// need not construct one — pins the loop to the roster it was started
    /// with, the behaviour before the roster followed the session.
    pub roster: Option<std::sync::Arc<dyn RosterReader + Send + Sync>>,
}

impl Default for DriveConfig {
    fn default() -> Self {
        Self {
            poll_interval: POLL_INTERVAL,
            heartbeat: HEARTBEAT_INTERVAL,
            dedup_window: DEDUP_WINDOW,
            correction: CorrectionConfig::default(),
            learnings_enabled: false,
            broker_log_hint: None,
            learnings_hint: None,
            branch_refresh_enabled: false,
            clis: HashMap::new(),
            default_branch: String::new(),
            // The loop runs only under `cmd_supervisor`, so the supervised
            // layout (dashboard 0, agents 1+) is the default.
            layout: SessionLayout::planned(crate::session::SessionMode::Supervisor, true),
            roster: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Heartbeat decision (pure)
// ---------------------------------------------------------------------------

/// Whether the heartbeat is due: a full `interval` has passed since
/// `last_progress`, the most recent instant the loop observed the wave make
/// progress (or the loop's start, if it has observed none).
///
/// A pure function of its three inputs, so the exit rule is decided in one
/// place and tested without a clock. An instant earlier than `last_progress`
/// (which a monotonic clock never produces) counts as no time elapsed.
fn heartbeat_due(last_progress: Instant, now: Instant, interval: Duration) -> bool {
    now.saturating_duration_since(last_progress) >= interval
}

/// Whether this tick observed the wave make progress: a coding agent's phase
/// advanced (`phase_advanced`), or a coding branch's tip differs from the tip
/// read on an earlier tick.
///
/// `tips` holds only this tick's successful reads, so a failed read is no
/// observation; a branch absent from `prev_tips` is a first read, recorded by
/// the caller but not counted. Nothing the loop does itself — an escalation,
/// a re-escalation of a prompt still unanswered, a learning, a nudge or a
/// buffered-input submit — is an input, so none of them can defer the
/// heartbeat.
fn tick_made_progress(
    prev_tips: &HashMap<String, String>,
    tips: &HashMap<String, String>,
    phase_advanced: bool,
) -> bool {
    phase_advanced
        || tips
            .iter()
            .any(|(branch, tip)| prev_tips.get(branch).is_some_and(|prev| prev != tip))
}

/// Raises each agent's recorded phase to the most-progressed one ever seen
/// (GP-05) and reports whether a CODING agent's phase rank rose this tick.
///
/// The rank only rises on `committed`, `verified` or `done`; every other
/// status — including the stuck-detection helper's synthetic statuses, which
/// carry `working` with a free-form `phase` such as `stuck-on-prompt` or
/// `no-progress` — is rank 0 and can never advance it. An agent's first
/// observed status is recorded, not counted.
fn advance_resolved_phases(
    resolved: &mut HashMap<String, WorkerPhase>,
    rows: &[AgentStatusRow],
    coding_ids: &[String],
) -> bool {
    let mut advanced = false;
    for row in rows {
        let phase = WorkerPhase::from_status(&row.status);
        match resolved.get_mut(&row.agent_id) {
            Some(best) if phase.rank() > best.rank() => {
                *best = phase;
                advanced |= coding_ids.contains(&row.agent_id);
            }
            Some(_) => {}
            None => {
                resolved.insert(row.agent_id.clone(), phase);
            }
        }
    }
    advanced
}

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

/// Runs the drive loop against the injected dependencies until an exit
/// condition (completion or heartbeat) is reached, then returns the summary.
///
/// Each poll iteration:
/// 1. Enumerates surfaces (one listing), resolving each to its agent by the
///    owner the terminal backend reports.
/// 2. Captures each surface explicitly (one screen read per surface) and acts only
///    when a LIVE prompt footer is in the tail.
/// 3. Classifies the live prompt; safe prompts are approved (audit-logged
///    first, then the option digit + `Enter` as separate keystrokes, gated on a
///    fresh re-confirm and on holding the pane's exclusive claim under
///    `repo_root`), risky/unknown prompts are escalated non-blocking and
///    deduped on `(agent_id, shape)`.
/// 4. Fetches `/status`; on completion the loop exits, otherwise it checks the
///    heartbeat and sleeps for the poll interval.
///
/// The pane sweep is **pane-keyed**: every pane returned by the enumerator is
/// evaluated, including a pane that has booted but not yet published any
/// `agent.status` (W15-7). The loop never treats a feedback→fix→re-verify cycle
/// as stuck — there is no cycle counter, only the completion and heartbeat exit
/// conditions.
///
/// Before acting on a tick, `instance` is checked (GP-06): once the loop's
/// bound session instance is purged, stopped, or replaced by a newer
/// same-named session, the loop exits immediately, without sweeping or
/// sending any keystrokes to whatever now owns that session name.
#[allow(clippy::too_many_lines)]
pub fn drive_loop(
    session: &str,
    repo_root: &Path,
    agents: &[AgentPane],
    deps: &mut DriveDeps<'_>,
    config: &DriveConfig,
    instance: &dyn SessionInstanceGuard,
) -> DriveSummary {
    let layout = config.layout;
    // The roster the loop drives, as last read. It starts as the one the
    // launch handed over and is refreshed from the session state at the top of
    // every sweep, so `git paw add` and `git paw remove` take effect on the
    // next tick without restarting the loop. Completion, the branch-refresh
    // pass and the exit summary all read this, so they see the roster as last
    // read rather than the one the wave started with.
    let mut agents: Vec<AgentPane> = agents.to_vec();
    let mut coding_ids: Vec<String> = agents.iter().map(|a| a.agent_id.clone()).collect();
    let mut dedup = DedupWindow::new(config.dedup_window);
    let mut escalations: Vec<Escalation> = Vec::new();
    let mut correction = CorrectionState::new();
    let mut exhausted: Vec<ExhaustedBranch> = Vec::new();
    // Highest-progressed `WorkerPhase` ever observed per agent (GP-05), so a
    // generic `working` heartbeat republished after a `verified`/`done`
    // artifact event can never regress the exit summary's resolved state.
    let mut resolved_phase: HashMap<String, WorkerPhase> = HashMap::new();

    let start = deps.clock.now();
    // The heartbeat is measured from the most recent progress the loop
    // observed — a coding branch tip moving or a coding agent's phase
    // advancing — or from the start until it has observed any.
    let mut last_progress = start;
    // Each coding branch's tip as last read, keyed by branch name.
    let mut branch_tips: HashMap<String, String> = HashMap::new();
    // Whether the heartbeat's fresh look already deferred the exit for a
    // completion with no progress since; it may do so only once.
    let mut heartbeat_deferred = false;
    // The most recent `/status` snapshot, used to resolve a coding pane's
    // nudge settle delay during the sweep — which runs BEFORE this tick's own
    // fetch below (GP-16's sweep/fetch ordering stays as-is). Empty on the
    // first tick, which resolves every agent to the config-driven agnostic
    // default via `resolve_submit_delay_ms`'s empty-CLI fallback, same as the
    // correction pass's pre-first-status handling.
    let mut status_for_sweep: Vec<AgentStatusRow> = Vec::new();
    let ctx = SweepContext { session, repo_root };

    // The loop is an expression that breaks with the terminal `(outcome,
    // latest_status)`: every exit path assigns both, so there are no
    // pre-initialised placeholders to leave dead.
    let (outcome, latest_status) = loop {
        // --- Session-instance check (GP-06), before any action this tick -----
        if !instance.is_current() {
            break (DriveOutcome::SessionTornDown, Vec::new());
        }

        // --- Refresh the roster from the session state -----------------------
        // Before any surface is resolved, so an agent added since the last
        // sweep resolves as a coding agent on this one and a removed agent
        // stops resolving. A read that fails leaves the previous roster in
        // place: driving the agents we know about beats driving none.
        if let Some(reader) = config.roster.as_ref()
            && let Some(fresh) = reader.read_roster()
        {
            coding_ids = fresh.iter().map(|a| a.agent_id.clone()).collect();
            agents = fresh;
        }

        // --- Sweep every coding surface (explicit per-pane capture) ----------
        // Only a surface the layout resolves to a coding agent is captured or
        // typed into (GP-46): the dashboard and an unowned surface are skipped
        // before any capture, so nothing drawn there can drive a decision.
        let mut pane_by_agent: HashMap<String, usize> = HashMap::new();
        for pane in deps.enumerator.list_panes(session) {
            let role = resolve_pane_agent(layout, pane.pane_index, &pane.owner, &agents);
            let Some(agent_id) = role.coding_agent() else {
                continue; // not a coding surface — never acted on
            };
            pane_by_agent.insert(agent_id.to_string(), pane.pane_index);

            if let Some(escalation) = sweep_pane(
                ctx,
                &pane,
                agent_id,
                deps,
                config,
                &mut dedup,
                &status_for_sweep,
            ) {
                escalations.push(escalation);
            }
        }

        // --- Fetch status once this tick, BEFORE dispatching any nudge -------
        // (GP-16): the freshest possible completion signal gates every nudge
        // dispatched below; the stale (pre-fetch) view it replaced let a
        // finished wave be nudged on its way out.
        let latest_status = deps.status.fetch();
        status_for_sweep.clone_from(&latest_status);
        // A branch that reached a terminal PASS leaves the correction cycle,
        // so an unrelated later failure starts from a fresh budget.
        correction.clear_completed(&latest_status);

        // Track the most-progressed phase ever seen per agent (GP-05): only
        // ever raises the recorded phase, never lowers it, so a later generic
        // `working` heartbeat cannot erase an earlier `verified`/`done` signal.
        // A coding agent's rank rising is wave progress for the heartbeat.
        let phase_advanced =
            advance_resolved_phases(&mut resolved_phase, &latest_status, &coding_ids);

        // Wind-down guard (GP-16): once this tick's own fetch shows the wave
        // complete, the loop is about to exit — no nudge is dispatched below,
        // on this tick or any later one (there is no later one; the loop
        // breaks at the bottom of this same tick).
        let winding_down = detect_completion(&latest_status, &coding_ids).is_some();

        // --- Observe newly published worker messages -------------------------
        let merged_bases = observe_worker_messages(
            &coding_ids,
            deps,
            &mut correction,
            &config.correction.gate_tags,
        );

        // --- Branch refresh (post-merge, opt-in) ------------------------------
        let refreshed_branches = trigger_branch_refresh(
            config.branch_refresh_enabled,
            ctx,
            &agents,
            &pane_by_agent,
            &latest_status,
            &merged_bases,
            deps,
        );

        if winding_down {
            let outcome = if escalations.is_empty() {
                DriveOutcome::Completed
            } else {
                DriveOutcome::EscalatedForReview
            };
            break (outcome, latest_status);
        }

        // --- Correction pass (auto-loopback) ---------------------------------
        let mut pass = CorrectionPass {
            ctx,
            panes: &pane_by_agent,
            status: &latest_status,
            correction: &mut correction,
            escalations: &mut escalations,
            exhausted: &mut exhausted,
        };
        run_correction_pass(&mut pass, deps, config);

        // --- Progress observation (unattended-heartbeat-progress) ------------
        // A failed tip read is no observation. A branch this loop rebased
        // itself (branch refresh) is re-baselined rather than compared: its
        // new tip is the loop's own doing, not the wave progressing.
        for branch in &refreshed_branches {
            branch_tips.remove(branch);
        }
        let tips: HashMap<String, String> = agents
            .iter()
            .filter_map(|a| {
                deps.merge_check
                    .branch_tip(&a.branch)
                    .ok()
                    .map(|tip| (a.branch.clone(), tip))
            })
            .collect();
        if tick_made_progress(&branch_tips, &tips, phase_advanced) {
            last_progress = deps.clock.now();
            heartbeat_deferred = false;
        }
        branch_tips.extend(tips);

        // --- Heartbeat check -------------------------------------------------
        if heartbeat_due(last_progress, deps.clock.now(), config.heartbeat) {
            // One last look before giving up: this tick's status was fetched
            // at its top, and a long tick (nudges, buffered-input submits,
            // branch-refresh rebases) can miss a phase
            // advance or completion published while it worked. A phase advance
            // is progress; a completion defers the exit once, for the next
            // tick to act on — never twice without progress between, so a
            // completion that never reaches a tick cannot hold the loop open.
            let fresh = deps.status.fetch();
            if advance_resolved_phases(&mut resolved_phase, &fresh, &coding_ids) {
                last_progress = deps.clock.now();
                heartbeat_deferred = false;
            } else if !heartbeat_deferred && detect_completion(&fresh, &coding_ids).is_some() {
                heartbeat_deferred = true;
            } else {
                let final_status = if fresh.is_empty() {
                    latest_status
                } else {
                    fresh
                };
                break (DriveOutcome::Heartbeat, final_status);
            }
        }

        deps.clock.sleep(config.poll_interval);
    };
    let mut outcome = outcome;

    // --- Wedged nudges reach the summary (idle-pane-nudge-submission) -------
    // A nudge or stranded line still present after every recovery attempt was
    // escalated to the broker (deduped) as it happened; it now joins the exit
    // summary's own escalation list, same as the `[unmerged]` entries appended
    // just below — one list, so the summary is never silently shorter than
    // what was actually escalated.
    escalations.append(&mut deps.wedged);

    // --- Ref-derived merge truth (drive-loop-exit-summary-truth, D1) --------
    // Merged-ness is answered by git, once per coding branch, never by loop
    // or phase state: a worker reaching `verified`/`done` says nothing about
    // whether its branch ever reached the default branch. A comparison git
    // cannot make (a deleted branch, a bad ref) counts as unmerged — the
    // fail-safe direction (D2), because this summary is the input to a
    // destructive `purge`.
    let merge_results: HashMap<String, Result<MergeState, PawError>> = agents
        .iter()
        .map(|a| (a.agent_id.clone(), deps.merge_check.merge_state(&a.branch)))
        .collect();

    // A wave the loop reported finished (`Completed` / `EscalatedForReview` —
    // the two outcomes the `winding_down` break above produces) is only ever
    // `Completed` when every branch is actually merged (D3). An unmerged
    // branch becomes a `[unmerged]` escalation entry, pushed directly onto
    // `escalations` — never via `record_escalation`, both because the loop's
    // sweep context no longer exists here and because nothing may be typed
    // into a pane during wind-down (GP-16).
    if matches!(
        outcome,
        DriveOutcome::Completed | DriveOutcome::EscalatedForReview
    ) {
        for agent in agents {
            let detail = match merge_results.get(&agent.agent_id) {
                Some(Ok(state)) if state.merged => None,
                Some(Ok(state)) => Some(format!(
                    "{}: {} commit(s) not in {} (tip {})",
                    agent.branch, state.ahead, config.default_branch, state.tip
                )),
                Some(Err(e)) => Some(format!(
                    "{}: could not compare against {}: {e}",
                    agent.branch, config.default_branch
                )),
                None => None,
            };
            if let Some(command) = detail {
                escalations.push(Escalation {
                    agent_id: agent.agent_id.clone(),
                    verdict: "unmerged".to_string(),
                    command,
                });
            }
        }
        outcome = if escalations.is_empty() {
            DriveOutcome::Completed
        } else {
            DriveOutcome::EscalatedForReview
        };
    }

    // --- Wind-down synthesis learning (deduped against in-session friction) --
    if !escalations.is_empty() {
        deps.learnings.record(
            LEARNING_CATEGORY,
            "unattended wave wind-down synthesis",
            &winddown_learning_body(outcome, &escalations),
        );
    }

    // Wave-level "N/M branches merged" line: the ref-derived count, printed
    // for every outcome with coding agents (not only `Completed`) — a
    // `heartbeat` exit benefits from the same truth as a finished one.
    let merged_summary = (!coding_ids.is_empty()).then(|| {
        let merged = merge_results
            .values()
            .filter(|r| matches!(r, Ok(s) if s.merged))
            .count();
        format!("{merged}/{} branches merged", coding_ids.len())
    });

    DriveSummary {
        outcome,
        agent_states: resolved_agent_states(
            &coding_ids,
            &latest_status,
            &resolved_phase,
            &merge_results,
            &config.default_branch,
        ),
        escalations,
        exhausted,
        broker_log_hint: config.broker_log_hint.clone(),
        learnings_hint: config.learnings_hint.clone(),
        merged_summary,
    }
}

/// `agent.artifact` statuses that make a **merge decision** live.
///
/// Deliberately NOT the whole terminal-status set: `blocked` and `verified`
/// never arrive as an artifact status (they are `agent.blocked` and
/// `agent.verified`), and treating a hypothetical `blocked` artifact as a merge
/// candidate would hand the orchestrator a merge-sequencing task for a branch
/// that is not ready — a misleading prompt for work that does not exist.
///
/// The authoritative definition of [`WorkerPhase::is_merge_candidate`]; decision
/// sites read the phase predicate, never this array.
const MERGE_CANDIDATE_STATUSES: &[&str] = &["committed", "done"];

/// Reacts to the broker messages published since the previous sweep.
///
/// Only **`agent.feedback`** carrying a `[<gate>]` tag acts here: it is a
/// FAILING gate verdict and marks the branch for the correction pass. (A
/// passing gate publishes `agent.verified`, which the `/status` sweep handles
/// instead.) The feedback arm filters on `coding_ids` FIRST, so the loop's own
/// `supervisor` traffic never feeds back into itself.
///
/// A worker's **`agent.question`** and its **`agent.artifact`** at a
/// `committed`/`done` status are judgment calls too, but no
/// orchestrator runs inside the session: the worker's own broker message is
/// the review item, and the loop neither copies it nor types it into any
/// surface. A headless orchestrator invocation reads them from the broker
/// (`orchestrator-escalation-handoff`).
///
/// Returns the default-branch name (`payload.base`) of every
/// `agent.advanced-main` event observed this sweep, in publish order.
///
/// `AdvancedMain`'s sender identity is the payload's `from` field (typically
/// `"supervisor"`), never a coding agent's id, so it is handled here as a
/// dedicated match arm BEFORE the `coding_ids`-gated dispatch below — that
/// gate exists to keep the loop's own escalations from feeding back into
/// themselves (see the arm-by-arm doc above) and would otherwise silently
/// drop every merge event. This is the drive loop's post-merge trigger for
/// `supervisor-branch-refresh` (design D4): refresh is evaluated only in
/// reaction to an observed merge, never on a timer.
fn observe_worker_messages(
    coding_ids: &[String],
    deps: &mut DriveDeps<'_>,
    correction: &mut CorrectionState,
    gate_tags: &[String],
) -> Vec<String> {
    let mut merged_bases = Vec::new();
    for msg in deps.messages.poll_new() {
        if let BrokerMessage::AdvancedMain { payload } = &msg {
            merged_bases.push(payload.base.clone());
            continue;
        }
        let Some(agent_id) = coding_ids.iter().find(|id| *id == msg.agent_id()) else {
            continue;
        };
        if let BrokerMessage::Feedback { payload, .. } = &msg
            && let Some(gate) = gate_failure(payload, gate_tags)
        {
            correction.mark_gate_failure(
                agent_id,
                reengagement_text(&gate, &payload.errors.join("; ")),
            );
        }
    }
    merged_bases
}

/// Runs [`refresh_branches_after_merge`] once per merge event observed this
/// sweep, but only when `enabled`. Factored out of [`drive_loop`] purely to
/// keep that function's own body short; the enablement check and the
/// no-timer guarantee (design D4 — `merged_bases` is non-empty only when an
/// `agent.advanced-main` event was actually observed this sweep) live here.
#[allow(clippy::too_many_arguments)]
fn trigger_branch_refresh(
    enabled: bool,
    ctx: SweepContext<'_>,
    agents: &[AgentPane],
    pane_by_agent: &HashMap<String, usize>,
    latest_status: &[AgentStatusRow],
    merged_bases: &[String],
    deps: &mut DriveDeps<'_>,
) -> Vec<String> {
    if !enabled {
        return Vec::new();
    }
    let mut rebased = Vec::new();
    for base in merged_bases {
        rebased.extend(refresh_branches_after_merge(
            ctx,
            agents,
            pane_by_agent,
            latest_status,
            base,
            deps,
        ));
    }
    rebased
}

/// Evaluates the `supervisor-branch-refresh` preconditions for every live
/// worker branch after a successful merge onto `base`, and rebases +
/// notifies each branch that passes every gate.
///
/// Called only when [`DriveConfig::branch_refresh_enabled`] is `true`, and
/// only from [`drive_loop`] in reaction to an observed
/// `agent.advanced-main` event — never on a timer (design D4). Every branch
/// is evaluated independently as a strict conjunction (task 4.6): a branch
/// that fails any single gate is left untouched, with no partial or
/// degraded attempt, and the skip is neither retried within this merge event
/// nor escalated to the supervisor inbox (task 4.7) — it is simply not
/// acted on, and the next merge re-evaluates it.
///
/// Returns the branches it rebased, so the heartbeat can tell a tip the loop
/// moved itself from the wave progressing.
fn refresh_branches_after_merge(
    ctx: SweepContext<'_>,
    agents: &[AgentPane],
    pane_by_agent: &HashMap<String, usize>,
    latest_status: &[AgentStatusRow],
    base: &str,
    deps: &mut DriveDeps<'_>,
) -> Vec<String> {
    let mut rebased = Vec::new();
    for agent in agents {
        // No live pane this sweep (booted but not yet resolved, or the pane
        // closed) — nothing to capture or claim, so nothing to evaluate.
        let Some(&pane_index) = pane_by_agent.get(&agent.agent_id) else {
            continue;
        };
        let status_row = latest_status.iter().find(|r| r.agent_id == agent.agent_id);
        // An agent with no status row on record is unresolved, not
        // affirmatively clean — the safe default is to skip it, matching
        // every other "cannot determine" case in this conjunction.
        let clean = status_row.is_some_and(|r| r.modified_files.is_empty());
        let verified_awaiting_merge = status_row.is_some_and(|r| r.status == "verified");

        let capture = deps.capturer.capture(ctx.session, pane_index);
        let mid_response = pane_is_mid_response(&capture);

        // Holding the claim through the rebase call (it is dropped at the end
        // of this iteration) narrows the window between the cleanliness
        // check and the rebase (design's disclosed residual risk).
        let claim = PaneClaim::try_acquire(ctx.repo_root, pane_index);
        let claim_held = claim.is_some();

        let predicted_conflict = deps.refresher.predict_conflict(&agent.worktree_path, base);
        let behind_default = deps.refresher.is_behind(&agent.worktree_path, base);

        let gates = super::branch_refresh::RefreshPreconditions {
            verified_awaiting_merge,
            clean,
            mid_response,
            claim_held,
            predicted_conflict,
            behind_default,
        };
        if gates.evaluate().is_err() {
            continue;
        }

        if deps.refresher.rebase(&agent.worktree_path).is_ok() {
            rebased.push(agent.branch.clone());
            deps.refresher.notify(&agent.agent_id, base);
        }
    }
    rebased
}

/// The tmux session and the repository root for the current sweep.
///
/// Bundled rather than passed as separate parameters because every acting path
/// needs them, and threading them separately pushes the sweep helpers past the
/// argument-count lint.
#[derive(Debug, Clone, Copy)]
struct SweepContext<'a> {
    /// tmux session name.
    session: &'a str,
    /// Repository root, from which the per-pane approval claim path is built.
    repo_root: &'a Path,
}

/// Records `escalation` on the broker as a drainable review item — and only
/// there. No orchestrator runs inside the session, so nothing is typed into
/// any surface: an orchestrator outside the session reads the item from the
/// broker.
fn record_escalation(deps: &mut DriveDeps<'_>, escalation: &Escalation) {
    deps.alerts.escalate(escalation);
}

/// A CLI-unavailability marker found in a pane's capture.
struct CliUnavailableMatch {
    /// The configured marker string that matched (stable across sweeps, even
    /// when the matching line itself carries a reset time or countdown).
    marker: String,
    /// The matching line, verbatim (trimmed), for quoting in the exit summary.
    line: String,
}

/// Tail-only scan for a CLI-unavailability marker — a usage/session limit or
/// an expired/missing login (`drive-loop-exit-summary-truth`, D4;
/// `cli-unavailable-detection-gaps`). Anchors on the same bounded trailing
/// region [`is_live_prompt`] uses ([`anchor_on_trailing_region`]), so a CLI
/// that renders a task list or status strip below the banner does not hide
/// it — while a marker separated from the end of the capture by more than
/// that bound still counts as scrolled away and is ignored. Matched
/// case-insensitively as a substring, by the same convention every other
/// marker field uses.
///
/// `markers` is the CALLER's resolved [`CliPromptProfile::cli_unavailable_markers`]
/// for the pane's own CLI ([`resolve_prompt_profile`]) — this function sources
/// nothing itself, so the loop and the bundled `sweep.sh` helper (which reads
/// the same resolved field via `git paw __prompt-profile`) cannot drift apart
/// on what counts as unavailable for a given CLI.
///
/// [`CliPromptProfile::cli_unavailable_markers`]: crate::config::CliPromptProfile::cli_unavailable_markers
#[must_use]
fn cli_unavailable_line(capture: &str, markers: &[String]) -> Option<CliUnavailableMatch> {
    let nonblank: Vec<&str> = capture.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut found: Option<CliUnavailableMatch> = None;
    anchor_on_trailing_region(&nonblank, |lines, _offset| {
        let tail = &lines[lines.len().saturating_sub(LIVE_PROMPT_TAIL)..];
        for line in tail {
            let lower = line.to_ascii_lowercase();
            if let Some(marker) = markers
                .iter()
                .find(|m| lower.contains(&m.to_ascii_lowercase()))
            {
                found = Some(CliUnavailableMatch {
                    marker: marker.clone(),
                    line: line.trim().to_string(),
                });
                return true;
            }
        }
        false
    });
    found
}

/// Dedup key for a `cli-unavailable` escalation: `(agent_id, matched marker)`
/// — never the banner line's full text, which may carry a reset time or
/// countdown that changes between sweeps (`cli-unavailable-detection-gaps`).
/// So the same banner observed across sweeps is one escalation, and two
/// distinct banners on one agent are two.
#[must_use]
fn cli_unavailable_dedup_key(agent_id: &str, marker: &str) -> String {
    format!("cli-unavailable|{agent_id}|{marker}")
}

/// Sweeps one coding-agent pane: captures it, and when a LIVE prompt is in the
/// tail records a deduped escalation for a human decision without blocking
/// the rest of the wave. Returns the escalation emitted this sweep, if any.
///
/// The caller passes only a surface the layout resolved as
/// [`SurfaceRole::Coding`] — `agent_id` is that surface's owner — so the
/// dashboard and an unowned surface are never captured here.
///
/// It answers NO prompt, whatever the command text says. Whether the prompted
/// command may run is decided by the agent CLI's own permission system or by
/// the human — a decision git-paw cannot make reliably from pane text, and no
/// longer tries to.
///
/// Before the live-prompt check, the pane's tail is also scanned for a
/// CLI-unavailability marker (D4): a match becomes a deduped `cli-unavailable`
/// escalation and the sweep continues driving the rest of the wave.
///
/// A pane with no live prompt and no CLI-unavailability marker — mere
/// narration, or a resolved prompt scrolled into history — is left untouched.
/// The one exception is an IDLE pane whose input box still holds an
/// unsubmitted directive: that gets a follow-up `Enter` and, if the text
/// survives, the same clear+re-type+`Enter` recovery the nudge path uses, via
/// [`submit_buffered_input`] — because no other part of the loop can see a
/// stall that never became a prompt.
fn sweep_pane(
    ctx: SweepContext<'_>,
    pane: &PaneInfo,
    agent_id: &str,
    deps: &mut DriveDeps<'_>,
    config: &DriveConfig,
    dedup: &mut DedupWindow,
    status: &[AgentStatusRow],
) -> Option<Escalation> {
    let session = ctx.session;
    let capture = deps.capturer.capture(session, pane.pane_index);

    // The pane's own broker-reported CLI, consulted both to resolve its
    // per-CLI `cli_unavailable_markers` override below and (further down) its
    // nudge settle delay — an agent with no status row yet falls back to the
    // config-driven/embedded-default behaviour via an empty CLI name.
    let cli = status
        .iter()
        .find(|r| r.agent_id == agent_id)
        .map_or("", |r| r.cli.as_str());

    let markers = &resolve_prompt_profile(cli, &config.clis).cli_unavailable_markers;
    if let Some(m) = cli_unavailable_line(&capture, markers) {
        // Non-blocking escalation, deduped on (agent_id, matched marker) —
        // never on incidental transcript content, so a persistent marker does
        // not re-fire every sweep and two distinct banners stay distinct
        // (`cli-unavailable-detection-gaps`).
        let key = cli_unavailable_dedup_key(agent_id, &m.marker);
        if !dedup.should_emit(&key, deps.clock.now()) {
            return None;
        }
        let escalation = Escalation {
            agent_id: agent_id.to_string(),
            verdict: "cli-unavailable".to_string(),
            command: m.line,
        };
        record_escalation(deps, &escalation);
        return Some(escalation);
    }

    if !is_live_prompt(&capture) {
        let settle_delay = Duration::from_millis(resolve_submit_delay_ms(cli, &config.clis));
        let _ = submit_buffered_input(
            deps,
            ctx.repo_root,
            session,
            pane.pane_index,
            agent_id,
            &capture,
            settle_delay,
        );
        return None;
    }

    // A live prompt. git-paw answers NONE of them: whether the prompted
    // command may run is the agent CLI's decision (through its own resolved
    // permission mode and allow rules) or the human's. The loop's job is to
    // get it in front of a human without stalling the rest of the wave.
    //
    // Non-blocking escalation, deduped on (agent_id, shape) — a prompt still
    // unanswered after the window is escalated again as a reminder.
    let key = approval_dedup_key(agent_id, &capture);
    if !dedup.should_emit(&key, deps.clock.now()) {
        return None;
    }
    let escalation = Escalation {
        agent_id: agent_id.to_string(),
        verdict: PERMISSION_PROMPT_VERDICT.to_string(),
        command: dedup_shape(&capture),
    };
    // Record the forwarded decision BEFORE the escalation goes out, and only
    // on this arm. Sitting after `should_emit` means one emitted escalation
    // produces exactly one log line — a prompt re-seen inside the dedup
    // window returned above and never reaches here — and sitting on the
    // permission-prompt arm alone keeps `cli-unavailable` banners, wedged
    // nudges and correction-loop signals out of a log that `git paw
    // approvals` reads as "commands that needed a human".
    deps.manual_decisions.record_forwarded(agent_id, &capture);
    record_escalation(deps, &escalation);
    // Opportunistic learning: a prompt the CLI's own permission rules did not
    // cover cost the wave a human round-trip.
    deps.learnings.record(
        LEARNING_CATEGORY,
        "unattended loop escalated a prompt",
        &friction_learning_body(&escalation),
    );
    Some(escalation)
}

/// The mutable slice of one sweep the correction pass operates on, bundled so
/// [`run_correction_pass`] keeps a small signature.
struct CorrectionPass<'a> {
    /// tmux session + repository root for this sweep.
    ctx: SweepContext<'a>,
    /// Pane index per agent id, resolved during this sweep.
    panes: &'a HashMap<String, usize>,
    /// The `/status` snapshot taken this sweep (source of the worker CLI).
    status: &'a [AgentStatusRow],
    /// Per-branch correction bookkeeping.
    correction: &'a mut CorrectionState,
    /// Escalations accumulated for the exit summary.
    escalations: &'a mut Vec<Escalation>,
    /// Exhausted branches accumulated for the exit summary.
    exhausted: &'a mut Vec<ExhaustedBranch>,
}

/// Drives one sweep of the correction loop: re-engages each branch marked by a
/// failing gate verdict, bounded by `max_cycles`, applying `on_exhausted` once
/// the budget is spent.
///
/// When `auto_loopback` is disabled (the default, and every config with no
/// `[supervisor.correction]` table) the pass is inert: no keystrokes are sent,
/// no cycle is counted, and no exhaustion policy fires — the gate feedback is
/// published exactly as it was before this feature existed.
///
/// The exhaustion policy is applied when a gate failure arrives for a branch
/// whose budget is ALREADY spent, not the instant the last re-engagement goes
/// out: the final fix attempt is given its chance to pass first (a branch that
/// passes clears its state and never exhausts).
fn run_correction_pass(
    pass: &mut CorrectionPass<'_>,
    deps: &mut DriveDeps<'_>,
    config: &DriveConfig,
) {
    if !config.correction.auto_loopback {
        return;
    }
    let policy = &config.correction;
    for (agent_id, feedback) in pass.correction.pending() {
        // Budget already spent: apply the exhaustion policy once, never
        // re-engage again.
        if pass.correction.cycles(&agent_id) >= policy.max_cycles {
            if pass.correction.take_exhaustion(&agent_id) {
                apply_exhaustion(pass, deps, config, &agent_id);
            }
            continue;
        }
        // No pane resolved this sweep (the agent's pane has not been
        // enumerated yet): keep the correction pending and retry next sweep.
        let Some(&pane_index) = pass.panes.get(&agent_id) else {
            continue;
        };
        // Finished-status gate: a finished worker is never nudged. The signal is
        // the worker's BROKER status, never a pane-content diff — a finished
        // worker's pane is just as unchanging as a stuck one's, so a diff cannot
        // tell "idle because done" from "idle because stuck". Keeping the
        // correction pending means a worker that leaves the finished state is
        // re-engaged on a later sweep rather than losing the verdict.
        if is_finished_worker(pass.status, &agent_id) {
            continue;
        }
        // The worker's own broker-reported CLI resolves its nudge settle
        // delay ([`resolve_submit_delay_ms`]) — an agent with no status row
        // yet (booted but not published) falls back to the config-driven
        // agnostic default via an empty CLI name.
        let cli = pass
            .status
            .iter()
            .find(|r| r.agent_id == agent_id)
            .map_or("", |r| r.cli.as_str());
        let settle_delay = Duration::from_millis(resolve_submit_delay_ms(cli, &config.clis));
        let sent = match send_reengagement(
            deps,
            pass.ctx.repo_root,
            pass.ctx.session,
            pane_index,
            &agent_id,
            &feedback,
            settle_delay,
        ) {
            Ok(outcome) => outcome.was_sent(),
            Err(_) => false,
        };
        if !sent {
            // The worker moved on between the sweep and the send (TOCTOU), the
            // pane's claim was held by another approver, or the dispatch
            // failed: leave the correction pending for the next sweep and do
            // NOT spend a cycle.
            continue;
        }
        let cycles = pass.correction.record_reengagement(&agent_id);
        // One-time early heads-up for a slow-converging worker, emitted only
        // when it lands strictly before the budget is spent.
        if cycles == policy.escalate_after_cycles
            && policy.escalate_after_cycles < policy.max_cycles
            && pass.correction.take_early_flag(&agent_id)
        {
            let escalation = Escalation {
                agent_id: agent_id.clone(),
                verdict: CORRECTION_SLOW_VERDICT.to_string(),
                command: format!(
                    "correction cycle {cycles} of {} — worker is converging slowly",
                    policy.max_cycles
                ),
            };
            record_escalation(deps, &escalation);
            pass.escalations.push(escalation);
        }
    }
}

/// Whether `agent_id`'s broker status in `rows` means the worker has FINISHED —
/// [`AGENT_COMPLETE_STATUSES`], i.e. `done` / `verified`.
///
/// Deliberately narrower than `stall.rs`'s `TERMINAL_STATUSES`, which also lists
/// `blocked` and `committed`. Those two are precisely the states a worker sits in
/// while it awaits correction — committed and standing by for re-verification, or
/// blocked and not polling its inbox — and they are what
/// `supervisor-correction-loop` exists to re-engage. Suppressing a nudge there
/// would silently disable that loop for its dominant path; "finished" and "quiet"
/// are different conditions, and only the former ends nudge eligibility.
///
/// An agent with no row yet has NOT finished: a pane that has booted but never
/// published is still nudge-eligible, matching the loop's pane-keyed sweep.
fn is_finished_worker(rows: &[AgentStatusRow], agent_id: &str) -> bool {
    rows.iter()
        .any(|r| r.agent_id == agent_id && WorkerPhase::from_status(&r.status).is_completed())
}

/// Applies the configured `on_exhausted` action to a branch whose correction
/// budget is spent, and records it in the exit summary.
///
/// `escalate` flags the branch to the orchestrator/human through the loop's
/// existing escalation path; `abandon` marks it failed without sending an
/// escalation. Neither re-engages the branch again.
fn apply_exhaustion(
    pass: &mut CorrectionPass<'_>,
    deps: &mut DriveDeps<'_>,
    config: &DriveConfig,
    agent_id: &str,
) {
    let cycles = pass.correction.cycles(agent_id);
    let policy = config.correction.on_exhausted;
    if policy == OnExhausted::Escalate {
        let escalation = Escalation {
            agent_id: agent_id.to_string(),
            verdict: CORRECTION_EXHAUSTED_VERDICT.to_string(),
            command: format!(
                "branch is unrecoverable after {cycles} correction cycle(s); needs a human"
            ),
        };
        record_escalation(deps, &escalation);
        pass.escalations.push(escalation);
    }
    pass.exhausted.push(ExhaustedBranch {
        agent_id: agent_id.to_string(),
        cycles,
        policy,
    });
    if config.learnings_enabled {
        deps.learnings.record(
            crate::broker::learnings::CATEGORY_CORRECTION_EXHAUSTED,
            "correction budget exhausted",
            &exhaustion_learning_body(agent_id, worker_cli(pass.status, agent_id), cycles),
        );
    }
}

/// Resolves the worker CLI for `agent_id` from the `/status` snapshot, falling
/// back to `"unknown"` when the broker has no CLI on record.
fn worker_cli<'a>(rows: &'a [AgentStatusRow], agent_id: &str) -> &'a str {
    rows.iter()
        .find(|r| r.agent_id == agent_id && !r.cli.is_empty())
        .map_or("unknown", |r| r.cli.as_str())
}

/// Bounded number of clear (`C-u`) + re-type + `Enter` recovery attempts
/// [`send_nudge`] makes after the initial send's follow-up `Enter` fails to
/// submit (GP-15). Sized to give a genuinely paste-aware CLI a couple of
/// chances to settle while still failing fast — an escalation, not an
/// unbounded retry loop — on a pane that is truly wedged.
const NUDGE_RECOVERY_ATTEMPTS: u8 = 2;

/// Escalation verdict label for a nudge whose text was still sitting
/// unsubmitted on a pane's input line after every recovery attempt (GP-15).
const NUDGE_WEDGED_VERDICT: &str = "nudge-wedged";

/// Outcome of a guarded nudge send ([`send_nudge`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NudgeOutcome {
    /// Another approver held the pane's claim, or `ready` rejected the fresh
    /// capture — no keystrokes went out.
    Skipped,
    /// The text was verified submitted, either on the first `Enter` or after
    /// a bounded number of clear+re-type+Enter recovery attempts.
    Delivered,
    /// Keystrokes went out but the text was still sitting on the input line
    /// after every recovery attempt; escalated to the broker for review.
    Wedged,
}

impl NudgeOutcome {
    /// Whether any keystroke was actually dispatched to the pane — `true` for
    /// both [`Self::Delivered`] and [`Self::Wedged`], since a caller tracking
    /// "did we act on this pane" (e.g. the orchestration-nudge cadence timer,
    /// or the correction loop's cycle count) cares about dispatch, not
    /// confirmed submission.
    fn was_sent(self) -> bool {
        matches!(self, NudgeOutcome::Delivered | NudgeOutcome::Wedged)
    }
}

/// Re-engages a gate-failed worker by typing `text` into its pane, gated on a
/// fresh re-confirm capture taken immediately before the send.
///
/// The guard mirrors the approval path's: a pane whose tail now shows a LIVE
/// permission prompt is mid-action rather than blocked awaiting correction,
/// and free text typed there would land in the prompt instead of the input
/// box — so nothing is sent and the correction stays pending for the next
/// sweep. See [`send_nudge`] for the settle-delay, verify, and recovery
/// behaviour.
fn send_reengagement(
    deps: &mut DriveDeps<'_>,
    repo_root: &Path,
    session: &str,
    pane_index: usize,
    agent_id: &str,
    text: &str,
    settle_delay: Duration,
) -> Result<NudgeOutcome, PawError> {
    send_nudge(
        deps,
        repo_root,
        session,
        pane_index,
        agent_id,
        text,
        settle_delay,
        |capture| !live_prompt_in_tail(capture),
    )
}

/// Types `text` into `pane_index`, verifies it submitted, and recovers a
/// stale input line — the drive loop's only actuator, made reliable (GP-15,
/// then made reachable in the field by `idle-pane-nudge-submission`).
///
/// Gated on a fresh capture taken immediately before the send satisfying
/// `ready` — the TOCTOU guard every free-text path shares, so no caller can
/// skip it. The worker re-engagement needs the pane merely to be free of a
/// live permission prompt. Every caller targets a coding-agent surface.
///
/// The whole operation — initial send through every recovery attempt — runs
/// under `pane_index`'s exclusive [`PaneClaim`], so a `C-u` this function (or
/// [`recover_stale_input`] on its behalf) sends is always clearing a line
/// THIS call itself just typed, never agent-authored input another approver
/// is mid-way through: a pane already claimed by a concurrent approver is
/// skipped this tick ([`NudgeOutcome::Skipped`]) rather than waited on,
/// matching every other claim-gated send in this module.
///
/// The send-verify-recover sequence:
/// 1. Send the text ([`KeyDispatcher::send_text`], literal), wait
///    `settle_delay` (the per-CLI settle delay [`resolve_submit_delay_ms`]
///    resolves, the same resolver the boot prompt uses), then send a
///    SEPARATE `Enter` — never a single combined text+Enter.
/// 2. Wait `settle_delay` again, allowing the pane to redraw, then capture
///    and judge by the input box's content via [`nudge_submitted`] — never
///    by exact equality with `text` — so a wrapped rendering (whose sigil
///    row shows only the start of a longer send) is correctly recognised as
///    still present rather than misreported delivered.
/// 3. Otherwise recover via [`recover_stale_input`]: clear the line (`C-u`),
///    re-send the text, wait `settle_delay`, send `Enter` again — up to
///    [`NUDGE_RECOVERY_ATTEMPTS`] times. A bare second `Enter` alone is a
///    proven no-op against a stale input line, so recovery always
///    re-establishes the input rather than repeating just the `Enter`.
/// 4. A pane still showing the text after every attempt is
///    [`NudgeOutcome::Wedged`] — escalated to the broker
///    ([`AlertSink::escalate`]) rather than retried further.
#[allow(clippy::too_many_arguments)]
fn send_nudge(
    deps: &mut DriveDeps<'_>,
    repo_root: &Path,
    session: &str,
    pane_index: usize,
    agent_id: &str,
    text: &str,
    settle_delay: Duration,
    ready: impl Fn(&str) -> bool,
) -> Result<NudgeOutcome, PawError> {
    let Some(_claim) = PaneClaim::try_acquire(repo_root, pane_index) else {
        return Ok(NudgeOutcome::Skipped); // another approver owns this pane — skip, never wait
    };
    let capture = deps.capturer.capture(session, pane_index);
    if !ready(&capture) {
        return Ok(NudgeOutcome::Skipped);
    }

    let [body, submit] = nudge_keystrokes(text);
    deps.dispatcher
        .send_text(session, pane_index, &body)
        .map_err(|e| PawError::TmuxError(format!("send-keys -l failed: {e}")))?;
    deps.clock.sleep(settle_delay);
    deps.dispatcher
        .send_key(session, pane_index, &submit)
        .map_err(|e| PawError::TmuxError(format!("send-keys {submit} failed: {e}")))?;
    deps.clock.sleep(settle_delay);

    let verify = deps.capturer.capture(session, pane_index);
    if nudge_submitted(&verify, text) {
        return Ok(NudgeOutcome::Delivered);
    }
    recover_stale_input(deps, session, pane_index, agent_id, text, settle_delay)
}

/// Whether a post-`Enter` capture shows `text` already submitted, judging by
/// the input box's content rather than exact equality with `text` (D1).
///
/// Submitted when the pane has left idle (a mid-response marker is showing)
/// or the input box is empty or holds content this call did not type — never
/// `C-u`ing a line the loop cannot account for. Still PRESENT only when the
/// box is non-empty and either its sigil row is a prefix of `text` (a
/// wrapped rendering's sigil row shows only the start of a longer send, so
/// the FULL text still starts with it) or the box shows a paste-buffer
/// placeholder matching the profile's `paste_buffer_pattern`.
fn nudge_submitted(verify: &str, text: &str) -> bool {
    if pane_is_mid_response(verify) {
        return true;
    }
    let Some(rows) = input_box_rows(verify) else {
        return true; // no recognised box (or a numbered option) — unknown CLI, treat as submitted
    };
    let sigil_row = rows.first().map_or("", String::as_str);
    if sigil_row.is_empty() {
        return true; // empty box
    }
    !(text.starts_with(sigil_row) || input_box_shows_placeholder(verify))
}

/// Runs the bounded clear (`C-u`) + re-type + `Enter` recovery loop against
/// `pane_index`'s input box, assuming the caller already holds the pane's
/// exclusive [`PaneClaim`] and has already confirmed `text` is still
/// present.
///
/// Shared by [`send_nudge`] (after its own initial plain send) and
/// [`submit_buffered_input`] (after its own initial lone `Enter`) — the two
/// callers differ only in what they tried before calling this, never in how
/// recovery itself behaves (design D4).
fn recover_stale_input(
    deps: &mut DriveDeps<'_>,
    session: &str,
    pane_index: usize,
    agent_id: &str,
    text: &str,
    settle_delay: Duration,
) -> Result<NudgeOutcome, PawError> {
    for _ in 0..NUDGE_RECOVERY_ATTEMPTS {
        // Clear the stale line a prior step (this function's own earlier
        // iteration, or the caller's initial send/`Enter`) put there — never
        // agent-authored input another approver is mid-way through, since
        // the caller holds the pane's exclusive claim throughout.
        deps.dispatcher
            .send_key(session, pane_index, "C-u")
            .map_err(|e| PawError::TmuxError(format!("send-keys C-u failed: {e}")))?;
        deps.dispatcher
            .send_text(session, pane_index, text)
            .map_err(|e| PawError::TmuxError(format!("send-keys -l failed: {e}")))?;
        deps.clock.sleep(settle_delay);
        deps.dispatcher
            .send_key(session, pane_index, "Enter")
            .map_err(|e| PawError::TmuxError(format!("send-keys Enter failed: {e}")))?;
        deps.clock.sleep(settle_delay);

        let verify = deps.capturer.capture(session, pane_index);
        if nudge_submitted(&verify, text) {
            return Ok(NudgeOutcome::Delivered);
        }
    }

    let escalation = Escalation {
        agent_id: agent_id.to_string(),
        verdict: NUDGE_WEDGED_VERDICT.to_string(),
        command: one_line_capped(text),
    };
    // Escalate to the broker only the first time this exact line wedges —
    // deduped per `(agent_id, command)` — so a persistently stuck line is
    // reported once, not every ~15s sweep.
    if deps.record_wedged(escalation.clone()) {
        deps.alerts.escalate(&escalation);
    }
    Ok(NudgeOutcome::Wedged)
}

/// The input box is recognised via [`is_idle_input_box_line`] — either the
/// `│`-bordered style (`│ > … │`) or current Claude Code's borderless,
/// rule-framed style (a rule line, the sigil line, another rule line),
/// ground-truthed against a live `tmux capture-pane -p`. The LAST such line
/// wins, because a pane's live input box is always its most recent one. A
/// wrapped rendering's continuation rows carry no sigil of their own, so they
/// are read by position rather than by matching a sigil again: every
/// `│ … │` line immediately below the sigil row for the bordered style, or
/// every plain line up to the closing rule line for the borderless style
/// (never crossing into the mode strip below it).
///
/// The sigil set comes from the embedded profile's `input_sigils`
/// (`cli-prompt-profiles` capability) rather than a compiled constant.
fn input_box_rows(capture: &str) -> Option<Vec<String>> {
    let lines: Vec<&str> = capture.lines().collect();
    let preceding_non_blank = |idx: usize| -> Option<&str> {
        lines[..idx]
            .iter()
            .rev()
            .find(|l| !l.trim().is_empty())
            .copied()
    };
    let sigil_idx = (0..lines.len())
        .rev()
        .find(|&idx| is_idle_input_box_line(lines[idx], preceding_non_blank(idx)))?;
    let sigil_text = strip_input_sigil(lines[sigil_idx])?.trim().to_string();
    if sigil_text.is_empty() {
        return None;
    }

    let bordered = lines[sigil_idx].trim().starts_with('│');
    let mut rows = vec![sigil_text];
    for raw in &lines[sigil_idx + 1..] {
        if bordered {
            let Some(inner) = raw.trim().strip_prefix('│') else {
                break; // the box's bottom border, or the box has ended
            };
            rows.push(inner.trim_end_matches('│').trim().to_string());
        } else {
            if is_rule_line(raw) {
                break; // the box's closing rule line
            }
            rows.push(raw.trim().to_string());
        }
    }
    Some(rows)
}

/// Extracts the text sitting in a pane's input box as a single
/// whitespace-joined string — the sigil row plus any continuation rows, in
/// display order — or `None` when [`input_box_rows`] finds no box.
fn input_box_text(capture: &str) -> Option<String> {
    Some(input_box_rows(capture)?.join(" "))
}

/// Compiled regex for the embedded profile's `paste_buffer_pattern`
/// (`cli-prompt-profiles` capability).
fn paste_buffer_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| static_regex(claude_prompt_profile().paste_buffer_pattern.as_str()))
}

/// Whether `capture`'s input box shows a paste-buffer placeholder — e.g.
/// `Pasted text #1` — matching the resolved profile's `paste_buffer_pattern`,
/// rather than text that can be re-typed verbatim.
fn input_box_shows_placeholder(capture: &str) -> bool {
    input_box_text(capture).is_some_and(|text| paste_buffer_regex().is_match(&text))
}

/// Whether `capture` shows an IDLE pane holding unsubmitted text in its input
/// box — a directive whose first `Enter` the CLI swallowed into its paste
/// buffer.
///
/// Idle means both markers are absent: no live permission prompt
/// ([`live_prompt_in_tail`], the wider marker set so an `approval`-worded prompt
/// also counts) and no mid-response footer ([`pane_is_mid_response`]). Such a
/// pane is invisible to the approval sweep (there is no prompt to classify) and
/// to the long stuck detector (it never reports itself stalled), so nothing else
/// in the loop would surface it.
fn pane_has_buffered_input(capture: &str) -> bool {
    pane_is_responsive(capture) && input_box_text(capture).is_some()
}

/// Whether `capture` shows a pane that can accept a keystroke at all: no live
/// permission prompt ([`live_prompt_in_tail`], the wider marker set so an
/// `approval`-worded prompt also counts) and no mid-response footer
/// ([`pane_is_mid_response`]).
///
/// This is the shared half of two different questions. The drive loop asks it
/// and then wants text in the input box ([`pane_has_buffered_input`] — a
/// directive stranded by a swallowed `Enter`); the launch flow asks it and
/// then wants the box *empty* ([`pane_is_idle_and_empty`] — safe to type a
/// workflow entry point into). Keeping the responsiveness half in one place
/// means a marker set that improves for one improves for both.
#[must_use]
pub fn pane_is_responsive(capture: &str) -> bool {
    !live_prompt_in_tail(capture) && !pane_is_mid_response(capture)
}

/// Whether `capture` shows an IDLE pane whose composer is free — responsive
/// per [`pane_is_responsive`], with an input box that is empty or whose shape
/// the prompt profile does not recognise.
///
/// This is the precondition for typing a launch-time workflow entry point
/// (`boot-block`, "A declared workflow entry point is submitted as its own
/// composer line"). Both "empty" and "unrecognised" answer `true` for the
/// same reason the mid-response marker set is conservative: an unrecognised
/// CLI matches nothing, so it is treated as idle, which at worst submits the
/// entry point right behind the first message — exactly what the CLI's own
/// input queue would see from a human typing twice. Text the profile DOES
/// recognise in the box is never typed over: that is someone else's content,
/// and appending to it would submit a line neither party wrote.
#[must_use]
pub fn pane_is_idle_and_empty(capture: &str) -> bool {
    pane_is_responsive(capture) && input_box_text(capture).is_none()
}

/// Submits a directive stranded in an idle pane's input box: a follow-up
/// `Enter` first (the same keystroke [`nudge_keystrokes`] uses to flush a
/// swallowed-`Enter` paste), then — if the text survives — the same bounded
/// clear (`C-u`) + re-type + `Enter` recovery the nudge path uses
/// ([`recover_stale_input`]), re-typing the text read from the WHOLE input
/// box (`idle-pane-nudge-submission`).
///
/// Returns `Ok(true)` when any keystroke went out and `Ok(false)` when
/// nothing was buffered. The shape is checked twice before the `Enter`: once
/// on `swept`, the capture the sweep already took (so a pane with nothing
/// buffered costs no extra `capture-pane`), and again on a fresh capture
/// taken immediately before the send — the same TOCTOU discipline
/// [`send_approval`] applies, so a pane that started responding or raised a
/// prompt in between receives no stray keystroke. The `Enter`-first step
/// types no free text, so it runs without the pane's claim, preserving
/// today's behaviour for the case it alone fixes (no free text is
/// typed); recovery, which DOES type free text,
/// runs only under the pane's exclusive [`PaneClaim`] — a concurrent approver
/// holding it defers recovery to the next sweep, with the `Enter` above still
/// having gone out. A paste-buffer placeholder is never cleared: the loop
/// escalates it as wedged instead of `C-u`ing content it cannot restore.
fn submit_buffered_input(
    deps: &mut DriveDeps<'_>,
    repo_root: &Path,
    session: &str,
    pane_index: usize,
    agent_id: &str,
    swept: &str,
    settle_delay: Duration,
) -> Result<bool, PawError> {
    if !pane_has_buffered_input(swept)
        || !pane_has_buffered_input(&deps.capturer.capture(session, pane_index))
    {
        return Ok(false);
    }
    deps.dispatcher
        .send_key(session, pane_index, "Enter")
        .map_err(|e| PawError::TmuxError(format!("send-keys Enter failed: {e}")))?;
    deps.clock.sleep(settle_delay);

    let verify = deps.capturer.capture(session, pane_index);
    if !pane_has_buffered_input(&verify) {
        return Ok(true); // the Enter alone submitted it
    }

    let Some(_claim) = PaneClaim::try_acquire(repo_root, pane_index) else {
        return Ok(true); // another approver owns this pane — retry next sweep
    };
    if input_box_shows_placeholder(&verify) {
        let escalation = Escalation {
            agent_id: agent_id.to_string(),
            verdict: NUDGE_WEDGED_VERDICT.to_string(),
            command: one_line_capped(&input_box_text(&verify).unwrap_or_default()),
        };
        if deps.record_wedged(escalation.clone()) {
            deps.alerts.escalate(&escalation);
        }
        return Ok(true);
    }
    let Some(text) = input_box_text(&verify) else {
        return Ok(true); // cleared between the check above and the claim
    };
    recover_stale_input(deps, session, pane_index, agent_id, &text, settle_delay)
        .map(NudgeOutcome::was_sent)
}

/// Whether `capture` shows a pane mid-response — the CLI is mid-turn with its
/// interrupt footer showing, rather than sitting at its input box.
///
/// Sources its markers from the embedded Claude Code profile's
/// `mid_response_markers` (`cli-prompt-profiles` capability) rather than a
/// compiled constant. The set is deliberately conservative: an unrecognised
/// CLI matches nothing and is treated as idle, which at worst reproduces the
/// injection behaviour of a pane whose state the loop cannot read.
fn pane_is_mid_response(capture: &str) -> bool {
    let lowered = capture.to_ascii_lowercase();
    claude_prompt_profile()
        .mid_response_markers
        .iter()
        .any(|marker| lowered.contains(marker.as_str()))
}

/// Builds the keystroke sequence for a *nudge* — free text the loop wants a
/// pane to submit (e.g. re-prompting a stalled agent). The text is one
/// keystroke and the submitting `Enter` is a SEPARATE keystroke, because on
/// paste-aware CLIs a single combined text+`Enter` buffers the input rather
/// than submitting it (D6, memory `feedback_sendkeys_nudge_needs_followup_enter`).
///
/// This encodes the follow-up-`Enter` discipline as a reusable unit so any
/// nudge path obeys it by construction; the loop's approval path applies the
/// same rule via [`approval_keystrokes`] (the option digit and its `Enter` are
/// likewise separate keystrokes).
///
/// The pair is `[body, submit]`. They are dispatched differently on purpose:
/// `body` is free text and goes out literally via
/// [`KeyDispatcher::send_text`], while `submit` is a key NAME for
/// [`KeyDispatcher::send_key`]. See [`send_reengagement`].
#[must_use]
pub fn nudge_keystrokes(text: &str) -> [String; 2] {
    [text.to_string(), "Enter".to_string()]
}

/// Reads a [`Duration`] in milliseconds from environment variable `key`,
/// falling back to `default` when the variable is unset or unparseable. Lets
/// the E2E harness shorten the poll/heartbeat cadence; it is not a documented
/// user knob.
fn duration_from_env_ms(key: &str, default: Duration) -> Duration {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(default, Duration::from_millis)
}

/// Builds the per-agent final-state list, preferring each agent's resolved
/// terminal phase over the final `/status` snapshot's literal status when the
/// resolved phase outranks it (GP-05), and appending the ref-derived
/// merged/unmerged annotation (`drive-loop-exit-summary-truth`, D3).
///
/// A `/status` snapshot alone can go stale: a generic `working` heartbeat
/// republished after a `verified`/`done` artifact event overwrites the
/// broker's on-record status for that agent, so reading only the latest
/// snapshot can misreport a finished worker as still working. `resolved`
/// tracks the best phase ever observed across the whole run, so it wins
/// whenever it is more progressed than the literal snapshot row; otherwise the
/// literal row's raw status string is kept verbatim (preserving any
/// unrecognized/future status text). An agent absent from both falls back to
/// `"unknown"`.
fn resolved_agent_states(
    coding_ids: &[String],
    latest_status: &[AgentStatusRow],
    resolved: &HashMap<String, WorkerPhase>,
    merge_results: &HashMap<String, Result<MergeState, PawError>>,
    default_branch: &str,
) -> Vec<(String, String)> {
    coding_ids
        .iter()
        .map(|id| {
            let literal = latest_status.iter().find(|r| &r.agent_id == id);
            let literal_rank = literal.map_or(0, |r| WorkerPhase::from_status(&r.status).rank());
            let status = match resolved.get(id) {
                Some(phase) if phase.rank() > literal_rank => phase.as_status().to_string(),
                _ => literal.map_or_else(|| "unknown".to_string(), |r| r.status.clone()),
            };
            let merge_note = match merge_results.get(id) {
                Some(Ok(state)) if state.merged => "merged".to_string(),
                Some(Ok(state)) => format!(
                    "unmerged ({} ahead of {default_branch}, tip {})",
                    state.ahead, state.tip
                ),
                Some(Err(e)) => {
                    format!("unmerged (could not compare against {default_branch}: {e})")
                }
                None => "unmerged (no merge check result)".to_string(),
            };
            (id.clone(), format!("{status} · {merge_note}"))
        })
        .collect()
}

/// JSON body for the opportunistic friction learning.
fn friction_learning_body(e: &Escalation) -> String {
    serde_json::json!({
        "observation": format!(
            "unattended drive loop escalated a {} from {} for a human decision",
            e.verdict, e.agent_id
        ),
        "command": e.command,
    })
    .to_string()
}

/// JSON body for the exhaustion learning (`correction_exhausted`).
///
/// Carries the branch, the worker CLI, and the cycle count — the tiered-model
/// tuning signal that tells an operator a worker's model tier is under-powered
/// for a class of task.
fn exhaustion_learning_body(agent_id: &str, cli: &str, cycles: u32) -> String {
    serde_json::json!({
        "observation": format!(
            "{agent_id} exhausted its correction budget after {cycles} cycle(s) on {cli}"
        ),
        "branch": agent_id,
        "cli": cli,
        "cycles": cycles,
    })
    .to_string()
}

/// JSON body for the wind-down synthesis learning.
fn winddown_learning_body(outcome: DriveOutcome, escalations: &[Escalation]) -> String {
    serde_json::json!({
        "observation": format!(
            "unattended wave exited ({}) with {} prompt(s) escalated for human review",
            outcome.label(),
            escalations.len()
        ),
        "escalation_count": escalations.len(),
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// Production wiring
// ---------------------------------------------------------------------------

/// Production [`SessionInstanceGuard`]: re-reads the global session receipt
/// for `repo_root` on every call and compares it against the instance bound
/// at loop start (GP-06).
struct FileSessionInstanceGuard {
    /// Repository root the session receipt is looked up by.
    repo_root: PathBuf,
    /// The tmux session name the loop is bound to.
    session_name: String,
    /// The receipt's creation timestamp when the loop started — the durable
    /// instance token. A receipt for the same repo with a different
    /// `created_at` is a newer, unrelated session of the same name.
    bound_created_at: SystemTime,
}

impl SessionInstanceGuard for FileSessionInstanceGuard {
    fn is_current(&self) -> bool {
        match session::find_session_for_repo(&self.repo_root) {
            Ok(Some(s)) => {
                s.session_name == self.session_name
                    && s.created_at == self.bound_created_at
                    && s.status != SessionStatus::Stopped
            }
            _ => false,
        }
    }
}

/// Adds the drive-loop binding to another guard's verdict: the loop is current
/// only while it still *holds* the binding for its session instance, and every
/// affirmative check re-asserts the lease for one more tick.
///
/// The binding is the half of the check the session receipt cannot make.
/// Recovery clones the receipt, `created_at` and all, so a session stopped and
/// recovered inside one poll interval looks unchanged to
/// [`FileSessionInstanceGuard`] alone — and `git paw stop` revoking the binding
/// is what tells the loop of the *stopped* instance that it is done. It also
/// covers the other direction: a loop whose binding was superseded by a
/// re-attach learns so here, before it acts.
///
/// A loop with no binding (`None`) degrades to the inner guard's verdict
/// unchanged.
struct BoundInstanceGuard<G: SessionInstanceGuard> {
    /// The session-receipt check this composes with.
    inner: G,
    /// The lease proving this loop is the session instance's sole driver.
    binding: Option<DriveLoopBinding>,
}

impl<G: SessionInstanceGuard> SessionInstanceGuard for BoundInstanceGuard<G> {
    fn is_current(&self) -> bool {
        // The binding first: it is the cheaper read and the stronger signal.
        if let Some(binding) = &self.binding
            && !binding.refresh()
        {
            eprintln!(
                "drive loop: the drive-loop binding was revoked or taken over by another loop \
                 — exiting without acting."
            );
            return false;
        }
        self.inner.is_current()
    }
}

/// Every terminal backend enumerates the session's surfaces for the loop: one
/// listing per sweep, each surface reported by its layout position and owner.
/// A listing that fails (the session is gone) is no surfaces, as before.
impl<T: TerminalBackend + ?Sized> PaneEnumerator for T {
    fn list_panes(&self, session: &str) -> Vec<PaneInfo> {
        self.list_surfaces(session)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|surface| {
                Some(PaneInfo {
                    pane_index: surface.position,
                    owner: surface.owner?.to_string_lossy().into_owned(),
                })
            })
            .collect()
    }
}

/// Every terminal backend captures a surface's screen for the loop. A failed
/// read is an empty capture here, as it always was for the loop's predicates.
impl<T: TerminalBackend + ?Sized> PaneCapture for T {
    fn capture(&self, session: &str, pane_index: usize) -> String {
        self.capture_tail(&self.surface_at(session, pane_index))
            .unwrap_or_default()
    }
}

/// Production [`StatusFetcher`] over the broker `/status` HTTP endpoint.
struct HttpStatusFetcher {
    broker_url: Option<String>,
}

impl StatusFetcher for HttpStatusFetcher {
    fn fetch(&self) -> Vec<AgentStatusRow> {
        let Some(url) = &self.broker_url else {
            return Vec::new();
        };
        fetch_status_over_http(url).unwrap_or_default()
    }
}

/// Production [`MessageObserver`] over the broker's existing `GET /log`
/// stream, advancing a per-call cursor so each message is observed once.
struct HttpLogObserver {
    broker_url: Option<String>,
    /// Cursor plus the epoch it belongs to. `RefCell` because the trait
    /// takes `&self` (the loop is single-threaded).
    reader: std::cell::RefCell<crate::broker::publish::CursorReader>,
}

impl MessageObserver for HttpLogObserver {
    fn poll_new(&self) -> Vec<BrokerMessage> {
        let Some(url) = &self.broker_url else {
            return Vec::new();
        };
        let since = self.reader.borrow().cursor();
        // Ask the broker for only what this observer has not seen. The
        // client-side `seq > since` filter below stays as a guard, so a
        // broker that ignores the cursor still yields each message once.
        let Ok(page) = crate::broker::publish::fetch_log_page_over_http(url, since, None) else {
            return Vec::new();
        };

        // A pause/start cycle — or any broker restart — begins sequence
        // numbers again at zero. Without this the cursor would sit above
        // everything the new broker ever appends and the loop would go
        // permanently deaf. On a reset, re-read from the beginning: the
        // entries are the new broker's, so none of them has been seen.
        let mut reader = self.reader.borrow_mut();
        let since = if reader.observe_epoch(page.epoch) {
            0
        } else {
            since
        };

        let mut out = Vec::new();
        for entry in page.entries {
            if entry.seq > since {
                reader.advance_to(entry.seq);
                out.push(entry.message);
            }
        }
        out
    }
}

/// Production [`Clock`] backed by the real wall clock.
struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn sleep(&self, dur: Duration) {
        std::thread::sleep(dur);
    }
}

/// Production [`AlertSink`] that publishes to the broker over HTTP.
struct BrokerAlertSink {
    broker_url: Option<String>,
}

impl AlertSink for BrokerAlertSink {
    fn escalate(&mut self, escalation: &Escalation) {
        let Some(url) = &self.broker_url else {
            return;
        };
        let question = escalation.question();
        let msg = crate::broker::messages::BrokerMessage::Question {
            agent_id: SUPERVISOR_AGENT_ID.to_string(),
            payload: crate::broker::messages::QuestionPayload { question },
        };
        if let Err(e) = crate::broker::publish::publish_to_broker_http(url, &msg) {
            eprintln!(
                "drive: failed to publish escalation for {}: {e}",
                escalation.agent_id
            );
        }
    }
}

/// Production [`LearningSink`] that shells out to the bundled `sweep.sh learn`
/// (never raw curl, per `learnings-supervisor-observation-channel`).
struct SweepLearningSink {
    repo_root: PathBuf,
    /// Deduplicates identical `(category, title, body)` learnings within the
    /// session so the wind-down pass does not re-record friction already
    /// captured opportunistically.
    seen: std::collections::HashSet<String>,
}

impl LearningSink for SweepLearningSink {
    fn record(&mut self, category: &str, title: &str, body: &str) {
        let key = format!("{category}\u{1f}{title}\u{1f}{body}");
        if !self.seen.insert(key) {
            return; // already recorded in-session
        }
        let script = self
            .repo_root
            .join(".git-paw")
            .join("scripts")
            .join("sweep.sh");
        if !script.exists() {
            return;
        }
        let status = Command::new("bash")
            .arg(&script)
            .arg("learn")
            .arg(category)
            .arg(title)
            .arg(body)
            .current_dir(&self.repo_root)
            .status();
        if let Err(e) = status {
            eprintln!("drive: failed to record learning via sweep.sh: {e}");
        }
    }
}

/// Production [`ManualDecisionSink`]: appends to the session's
/// manual-decision JSONL log and publishes the first-seen learning.
///
/// The recorder owns the opt-out (`[supervisor] manual_approvals_log`) and
/// the learnings gate, so this wrapper adds only the broker publish: on the
/// first sighting of a pattern `record_forwarded` hands back a
/// `permission_pattern` learning, which goes out over HTTP.
///
/// Every failure is a warning, never an abort. A wedged broker or an
/// unwritable log must not stop the loop from escalating the prompt — the
/// escalation is the thing the human actually needs; the log is bookkeeping.
pub struct RecorderManualDecisionSink {
    recorder: crate::supervisor::manual_approvals::ManualDecisionRecorder,
    broker_url: Option<String>,
}

impl RecorderManualDecisionSink {
    /// Wraps `recorder`, publishing any first-seen learning to `broker_url`
    /// when one is configured.
    #[must_use]
    pub fn new(
        recorder: crate::supervisor::manual_approvals::ManualDecisionRecorder,
        broker_url: Option<String>,
    ) -> Self {
        Self {
            recorder,
            broker_url,
        }
    }
}

impl ManualDecisionSink for RecorderManualDecisionSink {
    fn record_forwarded(&mut self, agent_id: &str, captured: &str) {
        let Some(learning) = self.recorder.record_forwarded(agent_id, captured) else {
            return;
        };
        let Some(url) = &self.broker_url else {
            return;
        };
        if let Err(e) = crate::broker::publish::publish_to_broker_http(url, &learning) {
            eprintln!("drive: failed to publish permission_pattern learning: {e}");
        }
    }
}

/// Production [`BranchRefresher`]: resolves each worktree's checked-out
/// branch via [`crate::git::current_branch`] and delegates the git-level
/// gate inputs and the rebase itself to `crate::git`'s
/// `supervisor-branch-refresh` entry points. Notification publishes a
/// `[branch-refresh]`-tagged `agent.feedback` over the broker HTTP API.
struct GitBranchRefresher {
    repo_root: PathBuf,
    broker_url: Option<String>,
}

impl BranchRefresher for GitBranchRefresher {
    fn predict_conflict(&self, worktree_path: &Path, default_branch: &str) -> bool {
        // An unresolvable branch name or prediction failure is reported as a
        // conflict — the safe default that skips the branch rather than
        // guessing it is clean.
        crate::git::current_branch(worktree_path)
            .ok()
            .and_then(|branch| {
                crate::git::predict_rebase_conflict(&self.repo_root, &branch, default_branch).ok()
            })
            .unwrap_or(true)
    }

    fn is_behind(&self, worktree_path: &Path, default_branch: &str) -> bool {
        // An unresolvable comparison is reported as NOT behind — the safe
        // default that skips the branch rather than guessing a rebase is due.
        crate::git::current_branch(worktree_path)
            .ok()
            .and_then(|branch| {
                crate::git::branch_is_behind_default(&self.repo_root, &branch, default_branch).ok()
            })
            .unwrap_or(false)
    }

    fn rebase(&mut self, worktree_path: &Path) -> Result<(), PawError> {
        let branch = crate::git::current_branch(worktree_path)?;
        crate::git::rebase_branch_onto_default(&self.repo_root, &branch)
    }

    fn notify(&mut self, agent_id: &str, default_branch: &str) {
        let Some(url) = &self.broker_url else {
            return;
        };
        let msg = crate::broker::messages::BrokerMessage::Feedback {
            agent_id: agent_id.to_string(),
            payload: crate::broker::messages::FeedbackPayload {
                from: SUPERVISOR_AGENT_ID.to_string(),
                errors: vec![super::branch_refresh::refreshed_notification_text(
                    default_branch,
                )],
            },
        };
        if let Err(e) = crate::broker::publish::publish_to_broker_http(url, &msg) {
            eprintln!("drive: failed to publish branch-refresh notification for {agent_id}: {e}");
        }
    }
}

/// Production [`MergeCheck`]: resolves each branch's merge state against a
/// default branch resolved ONCE at loop start via [`crate::git::default_branch`]
/// (`drive-loop-exit-summary-truth`, D1) — never re-derived per branch, so
/// every comparison in a run is against the same target.
struct GitMergeCheck {
    repo_root: PathBuf,
    default_branch: String,
}

impl MergeCheck for GitMergeCheck {
    fn merge_state(&self, branch: &str) -> Result<MergeState, PawError> {
        let merged =
            crate::git::branch_is_merged_into(&self.repo_root, branch, &self.default_branch)?;
        if merged {
            return Ok(MergeState {
                merged: true,
                ahead: 0,
                tip: String::new(),
            });
        }
        let ahead =
            crate::git::unmerged_commit_count(&self.repo_root, branch, &self.default_branch)?;
        let tip = crate::git::branch_tip_short(&self.repo_root, branch)?;
        Ok(MergeState {
            merged: false,
            ahead,
            tip,
        })
    }

    fn branch_tip(&self, branch: &str) -> Result<String, PawError> {
        crate::git::branch_tip(&self.repo_root, branch)
    }
}

/// Inputs for [`run_drive_loop`] beyond the session name, repo root, and agent
/// roster — bundled so the production entry point stays under the
/// argument-count lint and so `cmd_supervisor` builds them in one place.
pub struct DriveRunOptions {
    /// Broker `/status` + publish endpoint, or `None` when the broker is off.
    pub broker_url: Option<String>,
    /// Sink for the manual-decision log, built from the supervisor config so
    /// the `[supervisor] manual_approvals_log` opt-out and the learnings gate
    /// are resolved once, in the caller.
    pub manual_decisions: Box<dyn ManualDecisionSink>,
    /// Correction-loop policy resolved from `[supervisor.correction]`.
    pub correction: CorrectionConfig,
    /// Whether `[supervisor] learnings` is enabled (gates the exhaustion
    /// learning).
    pub learnings_enabled: bool,
    /// Broker-log pointer for the exit summary.
    pub broker_log_hint: Option<String>,
    /// Learnings-file pointer for the exit summary.
    pub learnings_hint: Option<String>,
    /// Whether `supervisor-branch-refresh` is enabled
    /// (`[supervisor] branch_refresh`). Default-off.
    pub branch_refresh_enabled: bool,
    /// The session receipt's creation timestamp — the durable instance token
    /// [`SessionInstanceGuard`] binds to (GP-06), so the loop can tell its own
    /// session instance apart from a later, unrelated same-named one.
    pub session_created_at: SystemTime,
    /// `[clis.<name>]` table, for [`resolve_submit_delay_ms`] on the nudge
    /// path (`drive-loop-actuator-robustness`, GP-15) — the same resolver the
    /// boot-prompt injection path uses.
    pub clis: HashMap<String, CustomCli>,
    /// The session's reserved-index record — the single answer to which
    /// surface plays which role, so the loop never compares an index to a
    /// literal.
    pub layout: SessionLayout,
    /// The drive-loop binding proving this loop is the session instance's
    /// sole driver, taken by the caller *before* it revived, recovered or
    /// changed anything. The loop holds it for the whole run, refreshes it
    /// every tick, and releases it on exit.
    ///
    /// `None` runs the loop with no exclusion at all; it exists only so a
    /// caller that cannot take a binding is not forced to fabricate one.
    pub binding: Option<DriveLoopBinding>,
}

/// Runs the unattended drive loop with production dependencies, prints the exit
/// summary, and returns.
///
/// This is the step-15 entry `cmd_supervisor` calls when `--unattended` is set.
/// It blocks (in the foreground process) until a completion or heartbeat exit
/// condition is reached; it does NOT require an attached interactive terminal.
///
/// # Errors
///
/// Returns an error only for an unrecoverable setup failure (the default
/// branch cannot be resolved). The loop itself swallows transient
/// tmux/broker errors and keeps polling; every outcome returns `Ok`.
pub fn run_drive_loop(
    session: &str,
    repo_root: &Path,
    agents: &[AgentPane],
    options: DriveRunOptions,
) -> Result<DriveSummary, PawError> {
    let DriveRunOptions {
        broker_url,
        manual_decisions,
        correction,
        learnings_enabled,
        broker_log_hint,
        learnings_hint,
        branch_refresh_enabled,
        session_created_at,
        clis,
        layout,
        binding,
    } = options;

    let instance = BoundInstanceGuard {
        inner: FileSessionInstanceGuard {
            repo_root: repo_root.to_path_buf(),
            session_name: session.to_string(),
            bound_created_at: session_created_at,
        },
        binding,
    };
    let default_branch = crate::git::default_branch(repo_root)?;
    let merge_check = GitMergeCheck {
        repo_root: repo_root.to_path_buf(),
        default_branch: default_branch.clone(),
    };

    // One stateless tmux backend serves as enumerator, capturer and
    // dispatcher; it is `Copy`, so each seam holds its own handle.
    let backend = crate::terminal::tmux::TmuxBackend::new();
    let enumerator = backend;
    let capturer = backend;
    let mut dispatcher = backend;
    let status = HttpStatusFetcher {
        broker_url: broker_url.clone(),
    };
    let messages = HttpLogObserver {
        broker_url: broker_url.clone(),
        reader: std::cell::RefCell::new(crate::broker::publish::CursorReader::new()),
    };
    let clock = SystemClock;
    let mut alerts = BrokerAlertSink {
        broker_url: broker_url.clone(),
    };
    let mut learnings = SweepLearningSink {
        repo_root: repo_root.to_path_buf(),
        seen: std::collections::HashSet::new(),
    };
    let mut refresher = GitBranchRefresher {
        repo_root: repo_root.to_path_buf(),
        broker_url,
    };

    let config = DriveConfig {
        // Poll cadence and heartbeat default to the production constants but
        // may be shortened via env for the E2E harness (no real LLM, no
        // interactive terminal) so a completion/heartbeat exit is observable
        // in seconds rather than minutes. These are advanced/test overrides,
        // not a documented user knob (configurability is deferred per the
        // design's open questions).
        poll_interval: effective_poll_interval(),
        heartbeat: duration_from_env_ms("GIT_PAW_DRIVE_HEARTBEAT_MS", HEARTBEAT_INTERVAL),
        correction,
        learnings_enabled,
        broker_log_hint,
        learnings_hint,
        branch_refresh_enabled,
        clis,
        default_branch,
        layout,
        // Every production run follows the live roster: the loop re-reads the
        // session receipt each sweep so `git paw add` / `git paw remove` take
        // effect without restarting it.
        roster: Some(std::sync::Arc::new(SessionFileRoster::new(repo_root))),
        ..DriveConfig::default()
    };

    let mut manual_decisions = manual_decisions;
    let mut deps = DriveDeps {
        enumerator: &enumerator,
        capturer: &capturer,
        dispatcher: &mut dispatcher,
        status: &status,
        messages: &messages,
        clock: &clock,
        merge_check: &merge_check,
        alerts: &mut alerts,
        learnings: &mut learnings,
        manual_decisions: manual_decisions.as_mut(),
        refresher: &mut refresher,
        wedged: Vec::new(),
    };

    let summary = drive_loop(session, repo_root, agents, &mut deps, &config, &instance);
    println!("{}", summary.render());
    Ok(summary)
}

// ---------------------------------------------------------------------------
// Broker `/status` access
// ---------------------------------------------------------------------------

/// Subset of an agent record returned by the broker `/status` endpoint that
/// the drive loop cares about.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentStatusRow {
    /// Agent identifier (slugified branch name).
    pub agent_id: String,
    /// Status label (e.g. `"working"`, `"done"`).
    pub status: String,
    /// Seconds since the agent was last seen.
    pub last_seen_seconds: u64,
    /// CLI running in this agent's pane (e.g. `"claude"`), as reported by the
    /// broker. Empty when the broker has no CLI on record for the agent.
    #[serde(default)]
    pub cli: String,
    /// The agent's most recently reported modified-file set (the broker
    /// watcher's tracked cleanliness signal). Empty when the broker has none
    /// on record, or the field is absent from an older broker response.
    /// Consumed by `supervisor-branch-refresh`'s cleanliness gate (design
    /// D1) so a caller with only HTTP `/status` access never needs a second
    /// working-tree probe.
    #[serde(default)]
    pub modified_files: Vec<String>,
}

/// Fetches the broker `/status` endpoint and returns the agent summary.
///
/// Used by the drive loop because the broker state lives in the broker host
/// process (`git paw __broker`), not in the loop's own. Errors are surfaced so the caller can
/// decide whether to retry. Still `pub` because
/// `tests/unattended_drive_e2e.rs` drives the broker through it.
pub fn fetch_status_over_http(broker_url: &str) -> Result<Vec<AgentStatusRow>, PawError> {
    let body = crate::broker::client::BrokerClient::new(broker_url).status_body()?;
    let parsed: StatusResponse = serde_json::from_str(&body)
        .map_err(|e| PawError::SessionError(format!("broker /status parse error: {e}")))?;
    Ok(parsed.agents)
}

#[derive(Deserialize)]
struct StatusResponse {
    agents: Vec<AgentStatusRow>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::sync::{Arc, Mutex};

    /// The layout the loop runs under in production: `cmd_supervisor` is its
    /// only caller, so the dashboard sits at 0 and coding agents from 1.
    fn supervised_layout() -> SessionLayout {
        SessionLayout::planned(crate::session::SessionMode::Supervisor, true)
    }

    /// `core-project-conventions`' "Every static pattern compiles under test":
    /// forces every static regex in this module to compile.
    #[test]
    fn static_regexes_compile() {
        assert!(paste_buffer_regex().is_match("Pasted text #1"));
    }

    /// Serves `bodies` in order, one per connection, recording each request
    /// line. Returns the bound address and the join handle.
    fn stub_log_broker(bodies: Vec<String>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub broker");
        let addr = listener.local_addr().expect("stub broker address");
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for body in bodies {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 2048];
                let read = stream.read(&mut buf).unwrap_or(0);
                requests.push(String::from_utf8_lossy(&buf[..read]).to_string());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
            requests
        });
        (format!("http://{addr}"), handle)
    }

    fn request_line(request: &str) -> &str {
        request.lines().next().unwrap_or_default()
    }

    /// `broker-runtime` scenario "The drive loop reads only new entries": an
    /// observer that has consumed up to sequence 12 asks the broker for
    /// `since=12` instead of re-fetching the whole log and filtering it.
    ///
    /// Also `broker-runtime` scenario "An unchanged epoch leaves the cursor
    /// alone": the second poll against the same broker still carries 12.
    #[test]
    fn the_drive_loop_observer_requests_only_entries_after_its_cursor() {
        let (url, server) = stub_log_broker(vec![
            r#"{"entries":[{"seq":12,"timestamp_unix_secs":0,"message":{"type":"agent.status","agent_id":"feat-a","payload":{"status":"working","modified_files":[]}}}],"last_seq":12,"epoch":1000}"#.to_string(),
            r#"{"entries":[],"last_seq":12,"epoch":1000}"#.to_string(),
        ]);

        let observer = HttpLogObserver {
            broker_url: Some(url),
            reader: std::cell::RefCell::new(crate::broker::publish::CursorReader::new()),
        };
        assert_eq!(observer.poll_new().len(), 1, "the entry at seq 12");
        assert!(observer.poll_new().is_empty(), "nothing newer than 12");

        let requests = server.join().expect("stub broker thread");
        assert!(
            request_line(&requests[1]).contains("since=12"),
            "the observer must carry its cursor on the wire; got {:?}",
            request_line(&requests[1])
        );
    }

    /// `broker-runtime` scenario "A reader resets its cursor when the epoch
    /// changes": a pause/start cycle restarts sequence numbers at zero, so
    /// an observer that kept its old cursor would sit above everything the
    /// new broker appends and go permanently deaf.
    #[test]
    fn the_drive_loop_observer_resets_its_cursor_when_the_broker_epoch_changes() {
        let entry = |seq: u64, agent: &str| {
            format!(
                r#"{{"seq":{seq},"timestamp_unix_secs":0,"message":{{"type":"agent.status","agent_id":"{agent}","payload":{{"status":"working","modified_files":[]}}}}}}"#
            )
        };
        let (url, server) = stub_log_broker(vec![
            // First broker: the observer consumes up to 12.
            format!(
                r#"{{"entries":[{}],"last_seq":12,"epoch":1000}}"#,
                entry(12, "feat-a")
            ),
            // A NEW broker on the same port, numbering from 1 again.
            format!(
                r#"{{"entries":[{},{},{}],"last_seq":3,"epoch":2000}}"#,
                entry(1, "feat-b"),
                entry(2, "feat-c"),
                entry(3, "feat-d")
            ),
            // And the poll after that holds the new cursor.
            r#"{"entries":[],"last_seq":3,"epoch":2000}"#.to_string(),
        ]);

        let observer = HttpLogObserver {
            broker_url: Some(url),
            reader: std::cell::RefCell::new(crate::broker::publish::CursorReader::new()),
        };
        assert_eq!(observer.poll_new().len(), 1);

        let after_restart = observer.poll_new();
        assert_eq!(
            after_restart.len(),
            3,
            "every entry of the restarted broker must be observed, not skipped \
             as older than the stale cursor"
        );
        assert!(observer.poll_new().is_empty());

        let requests = server.join().expect("stub broker thread");
        assert!(
            request_line(&requests[2]).contains("since=3"),
            "after the reset the cursor must track the NEW broker; got {:?}",
            request_line(&requests[2])
        );
    }

    /// Runs the loop against a throwaway repository root.
    ///
    /// Every approval the sweep dispatches first takes that pane's claim under
    /// this root, so tests running in parallel — several of which approve the
    /// same pane index — never contend for one another's claim files, and none
    /// writes into the real repository's `.git-paw/tmp/`.
    fn drive_loop_in_tmp(
        session: &str,
        agents: &[AgentPane],
        deps: &mut DriveDeps<'_>,
        config: &DriveConfig,
    ) -> DriveSummary {
        let repo = tempfile::tempdir().expect("temp repo root");
        drive_loop(
            session,
            repo.path(),
            agents,
            deps,
            config,
            &AlwaysCurrentGuard,
        )
    }

    /// A [`SessionInstanceGuard`] that never reports its instance torn down —
    /// the default for every test that is not itself exercising GP-06.
    struct AlwaysCurrentGuard;
    impl SessionInstanceGuard for AlwaysCurrentGuard {
        fn is_current(&self) -> bool {
            true
        }
    }

    /// A [`SessionInstanceGuard`] fake that reports current for the first `n`
    /// calls, then torn down forever after — models a session
    /// purged/stopped/replaced partway through the loop's run (GP-06).
    struct TornDownAfter {
        remaining_true: Cell<u32>,
    }
    impl TornDownAfter {
        fn new(n: u32) -> Self {
            Self {
                remaining_true: Cell::new(n),
            }
        }
    }
    impl SessionInstanceGuard for TornDownAfter {
        fn is_current(&self) -> bool {
            let n = self.remaining_true.get();
            if n == 0 {
                return false;
            }
            self.remaining_true.set(n - 1);
            true
        }
    }

    // --- Fakes --------------------------------------------------------------

    struct FakeEnumerator {
        panes: Vec<PaneInfo>,
    }
    impl PaneEnumerator for FakeEnumerator {
        fn list_panes(&self, _session: &str) -> Vec<PaneInfo> {
            self.panes.clone()
        }
    }

    /// Captures a fixed string per pane index, and counts capture calls so a
    /// test can assert one capture per pane (explicit per-pane capture).
    struct FakeCapturer {
        by_pane: RefCell<HashMap<usize, String>>,
        calls: Cell<usize>,
    }
    impl FakeCapturer {
        fn new(entries: &[(usize, &str)]) -> Self {
            let mut m = HashMap::new();
            for (idx, cap) in entries {
                m.insert(*idx, (*cap).to_string());
            }
            Self {
                by_pane: RefCell::new(m),
                calls: Cell::new(0),
            }
        }
    }
    impl PaneCapture for FakeCapturer {
        fn capture(&self, _session: &str, pane_index: usize) -> String {
            self.calls.set(self.calls.get() + 1);
            self.by_pane
                .borrow()
                .get(&pane_index)
                .cloned()
                .unwrap_or_default()
        }
    }

    /// Returns scripted captures in call order — one per `capture` call, the
    /// last repeating once exhausted — so a test can model a prompt that
    /// clears between the sweep capture and the send-time re-confirm capture.
    struct SequencedCapturer {
        captures: Vec<String>,
        idx: Cell<usize>,
    }
    impl SequencedCapturer {
        fn new(captures: &[&str]) -> Self {
            Self {
                captures: captures.iter().map(|c| (*c).to_string()).collect(),
                idx: Cell::new(0),
            }
        }
    }
    impl PaneCapture for SequencedCapturer {
        fn capture(&self, _session: &str, _pane_index: usize) -> String {
            let i = self.idx.get();
            self.idx.set(i + 1);
            self.captures
                .get(i)
                .or_else(|| self.captures.last())
                .cloned()
                .unwrap_or_default()
        }
    }

    /// Records every dispatch in order in `events`, and additionally records
    /// the LITERAL (`send-keys -l`) sends in `literal_sends`, so a test can
    /// assert not just *what* was typed but *by which* path.
    #[derive(Default)]
    struct RecordingDispatcher {
        events: Vec<(usize, String)>,
        literal_sends: Vec<(usize, String)>,
    }
    impl KeyDispatcher for RecordingDispatcher {
        fn send_key(
            &mut self,
            _session: &str,
            pane_index: usize,
            key: &str,
        ) -> std::io::Result<()> {
            self.events.push((pane_index, key.to_string()));
            Ok(())
        }
        fn send_text(
            &mut self,
            _session: &str,
            pane_index: usize,
            text: &str,
        ) -> std::io::Result<()> {
            self.events.push((pane_index, text.to_string()));
            self.literal_sends.push((pane_index, text.to_string()));
            Ok(())
        }
    }

    /// Serves a scripted sequence of status snapshots — one per poll iteration.
    /// The last snapshot repeats once the sequence is exhausted.
    struct ScriptedStatus {
        snapshots: Vec<Vec<AgentStatusRow>>,
        idx: Cell<usize>,
    }
    impl ScriptedStatus {
        fn new(snapshots: Vec<Vec<AgentStatusRow>>) -> Self {
            Self {
                snapshots,
                idx: Cell::new(0),
            }
        }
    }
    impl StatusFetcher for ScriptedStatus {
        fn fetch(&self) -> Vec<AgentStatusRow> {
            let i = self.idx.get();
            let snap = self
                .snapshots
                .get(i)
                .or_else(|| self.snapshots.last())
                .cloned()
                .unwrap_or_default();
            self.idx.set(i + 1);
            snap
        }
    }

    /// Fake clock: `now` advances by whatever `sleep` is called with, plus an
    /// explicit tick so a zero-poll-interval loop still makes heartbeat
    /// progress.
    struct FakeClock {
        now: Cell<Instant>,
    }
    impl FakeClock {
        fn new() -> Self {
            Self {
                now: Cell::new(Instant::now()),
            }
        }
    }
    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            self.now.get()
        }
        fn sleep(&self, dur: Duration) {
            self.now
                .set(self.now.get() + dur + Duration::from_millis(1));
        }
    }

    #[derive(Default)]
    struct RecordingAlerts {
        escalations: Vec<Escalation>,
    }
    impl AlertSink for RecordingAlerts {
        fn escalate(&mut self, escalation: &Escalation) {
            self.escalations.push(escalation.clone());
        }
    }

    #[derive(Default)]
    struct RecordingLearnings {
        records: Vec<(String, String, String)>,
    }
    impl RecordingLearnings {
        /// Every recorded body for `category`.
        fn bodies(&self, category: &str) -> Vec<&str> {
            self.records
                .iter()
                .filter(|(c, _, _)| c == category)
                .map(|(_, _, body)| body.as_str())
                .collect()
        }
    }
    impl LearningSink for RecordingLearnings {
        fn record(&mut self, category: &str, title: &str, body: &str) {
            self.records
                .push((category.to_string(), title.to_string(), body.to_string()));
        }
    }

    /// Records every `(agent_id, captured)` the loop forwards for a manual
    /// decision, so a test can assert both the count and the payload.
    #[derive(Default)]
    struct RecordingManualDecisions {
        calls: Vec<(String, String)>,
    }
    impl ManualDecisionSink for RecordingManualDecisions {
        fn record_forwarded(&mut self, agent_id: &str, captured: &str) {
            self.calls
                .push((agent_id.to_string(), captured.to_string()));
        }
    }

    /// Inert sink for the tests whose subject is not the decision log.
    #[derive(Default)]
    struct NoopManualDecisions;
    impl ManualDecisionSink for NoopManualDecisions {
        fn record_forwarded(&mut self, _agent_id: &str, _captured: &str) {}
    }

    /// Scripted, call-recording [`BranchRefresher`] fake.
    ///
    /// `predict_conflict`/`is_behind` default to "conflict-free and behind"
    /// (the all-gates-pass shape) so a test only needs to override what it
    /// cares about via [`Self::with_conflict`] / [`Self::with_not_behind`].
    /// Every `rebase`/`notify` call is recorded so a test can assert the
    /// orchestration never invoked them when a gate should have skipped the
    /// branch.
    #[derive(Default)]
    struct RecordingRefresher {
        conflicting: std::collections::HashSet<PathBuf>,
        not_behind: std::collections::HashSet<PathBuf>,
        rebase_should_fail: bool,
        rebase_calls: Vec<PathBuf>,
        notify_calls: Vec<(String, String)>,
    }
    impl RecordingRefresher {
        fn with_conflict(mut self, worktree_path: &Path) -> Self {
            self.conflicting.insert(worktree_path.to_path_buf());
            self
        }
        fn with_not_behind(mut self, worktree_path: &Path) -> Self {
            self.not_behind.insert(worktree_path.to_path_buf());
            self
        }
        fn with_failing_rebase(mut self) -> Self {
            self.rebase_should_fail = true;
            self
        }
    }
    impl BranchRefresher for RecordingRefresher {
        fn predict_conflict(&self, worktree_path: &Path, _default_branch: &str) -> bool {
            self.conflicting.contains(worktree_path)
        }
        fn is_behind(&self, worktree_path: &Path, _default_branch: &str) -> bool {
            !self.not_behind.contains(worktree_path)
        }
        fn rebase(&mut self, worktree_path: &Path) -> Result<(), PawError> {
            self.rebase_calls.push(worktree_path.to_path_buf());
            if self.rebase_should_fail {
                return Err(PawError::WorktreeError(
                    "scripted rebase failure".to_string(),
                ));
            }
            Ok(())
        }
        fn notify(&mut self, agent_id: &str, default_branch: &str) {
            self.notify_calls
                .push((agent_id.to_string(), default_branch.to_string()));
        }
    }

    /// Scripted [`MergeCheck`] fake. Defaults to "every branch already
    /// merged" (the all-pass shape tests unconcerned with merge state want),
    /// so a test only opts into the ref-truth override via
    /// [`Self::with_unmerged`] / [`Self::with_error`].
    #[derive(Default)]
    struct FakeMergeCheck {
        unmerged: HashMap<String, (usize, String)>,
        errors: HashMap<String, String>,
        /// Scripted tip reads per branch, one per `branch_tip` call; the last
        /// repeats once exhausted. A branch with no script has no readable tip
        /// (the read errors), which the heartbeat treats as no observation.
        tips: RefCell<HashMap<String, Vec<String>>>,
        tip_reads: Cell<usize>,
    }
    impl FakeMergeCheck {
        fn with_tips(self, branch: &str, tips: &[&str]) -> Self {
            self.tips.borrow_mut().insert(
                branch.to_string(),
                tips.iter().map(|t| (*t).to_string()).collect(),
            );
            self
        }
        fn with_unmerged(mut self, branch: &str, ahead: usize, tip: &str) -> Self {
            self.unmerged
                .insert(branch.to_string(), (ahead, tip.to_string()));
            self
        }
        fn with_error(mut self, branch: &str, message: &str) -> Self {
            self.errors.insert(branch.to_string(), message.to_string());
            self
        }
    }
    impl MergeCheck for FakeMergeCheck {
        fn merge_state(&self, branch: &str) -> Result<MergeState, PawError> {
            if let Some(msg) = self.errors.get(branch) {
                return Err(PawError::WorktreeError(msg.clone()));
            }
            if let Some((ahead, tip)) = self.unmerged.get(branch) {
                return Ok(MergeState {
                    merged: false,
                    ahead: *ahead,
                    tip: tip.clone(),
                });
            }
            Ok(MergeState {
                merged: true,
                ahead: 0,
                tip: String::new(),
            })
        }
        fn branch_tip(&self, branch: &str) -> Result<String, PawError> {
            self.tip_reads.set(self.tip_reads.get() + 1);
            let mut tips = self.tips.borrow_mut();
            let Some(script) = tips.get_mut(branch) else {
                return Err(PawError::WorktreeError(format!("no tip for {branch}")));
            };
            let tip = if script.len() > 1 {
                script.remove(0)
            } else {
                script
                    .first()
                    .cloned()
                    .ok_or_else(|| PawError::WorktreeError(format!("no tip for {branch}")))?
            };
            Ok(tip)
        }
    }

    /// Serves a scripted sequence of newly-published broker messages — one
    /// batch per poll iteration, empty once the script is exhausted (mirroring
    /// a real observer, which only ever returns messages published *since* the
    /// previous call).
    #[derive(Default)]
    struct ScriptedMessages {
        batches: RefCell<std::collections::VecDeque<Vec<BrokerMessage>>>,
    }
    impl ScriptedMessages {
        /// An observer that never reports a message.
        fn none() -> Self {
            Self::default()
        }
        fn new(batches: Vec<Vec<BrokerMessage>>) -> Self {
            Self {
                batches: RefCell::new(batches.into()),
            }
        }
    }
    impl MessageObserver for ScriptedMessages {
        fn poll_new(&self) -> Vec<BrokerMessage> {
            self.batches.borrow_mut().pop_front().unwrap_or_default()
        }
    }

    fn row(agent: &str, status: &str) -> AgentStatusRow {
        AgentStatusRow {
            agent_id: agent.to_string(),
            status: status.to_string(),
            last_seen_seconds: 0,
            cli: String::new(),
            modified_files: Vec::new(),
        }
    }

    /// A `/status` row that also carries the agent's CLI, for the exhaustion
    /// learning's tiered-model tuning payload.
    fn row_with_cli(agent: &str, status: &str, cli: &str) -> AgentStatusRow {
        AgentStatusRow {
            cli: cli.to_string(),
            ..row(agent, status)
        }
    }

    /// A `[clis]` table pinning every CLI name this test file's status-row
    /// fixtures use — the empty string ([`row`], and [`DriveConfig::default`]'s
    /// `supervisor_cli`) and `"claude"` ([`row_with_cli`]) — to a zero nudge
    /// settle delay.
    ///
    /// [`send_nudge`] now spends `deps.clock.sleep(settle_delay)` on every
    /// dispatch (GP-15), and [`FakeClock::sleep`] advances the loop's own
    /// notion of elapsed time. A cadence- or cycle-count-sensitive test that
    /// does not care about settle-delay timing wires this in so a
    /// re-engagement costs no simulated time,
    /// preserving the tick/round arithmetic it was written against.
    fn zero_settle_delay_clis() -> HashMap<String, CustomCli> {
        ["", "claude"]
            .into_iter()
            .map(|cli| {
                (
                    cli.to_string(),
                    CustomCli {
                        command: cli.to_string(),
                        display_name: None,
                        submit_delay_ms: Some(0),
                        settings_path: None,
                        approval_args: HashMap::new(),
                        prompt_profile: None,
                        session_id_args: None,
                        resume_args: None,
                        mcp: None,
                    },
                )
            })
            .collect()
    }

    /// A `/status` row that also carries a modified-file set, for the
    /// branch-refresh cleanliness gate.
    fn row_with_modified_files(agent: &str, status: &str, files: &[&str]) -> AgentStatusRow {
        AgentStatusRow {
            modified_files: files.iter().map(|f| (*f).to_string()).collect(),
            ..row(agent, status)
        }
    }

    /// A supervisor gate-failure `agent.feedback` addressed to `agent`.
    fn gate_feedback(agent: &str, gate: &str, message: &str) -> BrokerMessage {
        BrokerMessage::Feedback {
            agent_id: agent.to_string(),
            payload: FeedbackPayload {
                from: SUPERVISOR_AGENT_ID.to_string(),
                errors: vec![format!("[{gate}] {message}")],
            },
        }
    }

    fn live_safe_capture(cmd: &str) -> String {
        format!(
            "Bash command\n  {cmd}\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n(esc to cancel)"
        )
    }

    // --- resolve_pane_agent (task 3.5) --------------------------------------

    /// Pane→agent resolution is by working directory, NOT pane index. The pane
    /// indices here are deliberately non-alphabetical and non-arg-order.
    #[test]
    fn resolves_coding_agent_by_path_not_index() {
        let agents = vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
                branch: "feat/a".to_string(),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
                branch: "feat/b".to_string(),
            },
        ];
        // Pane index 4 holds feat/b's worktree; index 2 holds feat/a's.
        assert_eq!(
            resolve_pane_agent(supervised_layout(), 4, "/repo-feat-b", &agents),
            SurfaceRole::Coding("feat-b".to_string())
        );
        assert_eq!(
            resolve_pane_agent(supervised_layout(), 2, "/repo-feat-a", &agents),
            SurfaceRole::Coding("feat-a".to_string())
        );
    }

    /// Pane 0 is the dashboard; any other repo-root surface owns no worktree
    /// and is `Unknown` — neither is a coding surface the loop acts on.
    #[test]
    fn resolves_pane_zero_to_the_dashboard_and_a_stray_root_surface_to_unknown() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let dashboard = resolve_pane_agent(supervised_layout(), 0, "/repo", &agents);
        assert_eq!(dashboard, SurfaceRole::Dashboard);
        assert_eq!(dashboard.coding_agent(), None);
        let stray = resolve_pane_agent(supervised_layout(), 1, "/repo", &agents);
        assert_eq!(stray, SurfaceRole::Unknown);
        assert_eq!(stray.coding_agent(), None);
    }

    // --- every live prompt is escalated, none is approved -------------------

    /// Everything one direct [`sweep_pane`] call did.
    struct SweepProbe {
        outcome: Option<Escalation>,
        keys: Vec<(usize, String)>,
        escalations: Vec<Escalation>,
        learnings: Vec<(String, String, String)>,
    }

    impl SweepProbe {
        /// The single escalation this sweep produced.
        fn escalation(&self) -> &Escalation {
            assert_eq!(
                self.escalations.len(),
                1,
                "expected exactly one escalation, got {:?}",
                self.escalations
            );
            &self.escalations[0]
        }
    }

    /// Sweeps one coding pane showing `capture` and reports what happened.
    ///
    /// Calls [`sweep_pane`] directly rather than driving the whole loop, so a
    /// test asserts the sweep's own decision without a status script or a
    /// completion condition in the way.
    fn sweep_probe(capture: &str, agent_id: &str, pane_index: usize) -> SweepProbe {
        let enumerator = FakeEnumerator { panes: Vec::new() };
        let capturer = FakeCapturer::new(&[(pane_index, capture)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![Vec::new()]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let tmp = tempfile::TempDir::new().expect("tmp repo root");
        let outcome = {
            let mut deps = DriveDeps {
                enumerator: &enumerator,
                capturer: &capturer,
                dispatcher: &mut dispatcher,
                status: &status,
                messages: &messages,
                clock: &clock,
                merge_check: &FakeMergeCheck::default(),
                alerts: &mut alerts,
                learnings: &mut learnings,
                manual_decisions: &mut manual_decisions,
                refresher: &mut refresher,
                wedged: Vec::new(),
            };
            let config = DriveConfig::default();
            let mut dedup = DedupWindow::new(config.dedup_window);
            let ctx = SweepContext {
                session: "paw-test",
                repo_root: tmp.path(),
            };
            let pane = PaneInfo {
                pane_index,
                owner: "/repo-feat-a".to_string(),
            };
            sweep_pane(ctx, &pane, agent_id, &mut deps, &config, &mut dedup, &[])
        };
        SweepProbe {
            outcome,
            keys: dispatcher.events,
            escalations: alerts.escalations,
            learnings: learnings.records,
        }
    }

    /// Spec scenario "A command formerly classified safe is escalated, not
    /// approved" (`approval-command-safety`): `ls -la` was a read-mostly verb
    /// the retired classifier auto-approved. It now escalates like any other
    /// prompt, and no keystroke is dispatched.
    #[test]
    fn formerly_safe_command_is_escalated_not_approved() {
        let probe = sweep_probe(&live_safe_capture("ls -la"), "feat-a", 2);
        assert!(
            probe.outcome.is_some(),
            "expected an escalation, got {:?}",
            probe.outcome
        );
        let e = probe.escalation();
        assert_eq!(e.agent_id, "feat-a");
        assert_eq!(e.verdict, PERMISSION_PROMPT_VERDICT);
        assert_eq!(e.command, "ls -la");
        assert!(
            probe.keys.is_empty(),
            "no keystroke may be dispatched, got {:?}",
            probe.keys
        );
        assert_eq!(
            probe.learnings.len(),
            1,
            "the escalation records one friction learning"
        );
    }

    /// Spec scenario "A file-edit prompt is escalated by its target path"
    /// (`supervisor-unattended-operation`): a write/edit prompt carries no
    /// `Bash command` header, so the escalation names the target path — the
    /// same value the dedup key is built from.
    #[test]
    fn file_edit_prompt_is_escalated_by_its_target_path() {
        let capture = "Do you want to make this edit to src/lib.rs?\n\
                       ❯ 1. Yes\n  2. No\n(esc to cancel)";
        let probe = sweep_probe(capture, "feat-a", 2);
        assert!(
            probe.outcome.is_some(),
            "expected an escalation, got {:?}",
            probe.outcome
        );
        assert_eq!(probe.escalation().command, "src/lib.rs");
        assert!(
            probe.keys.is_empty(),
            "no keystroke may be dispatched, got {:?}",
            probe.keys
        );
    }

    /// Spec scenario "Command text cannot steer the outcome"
    /// (`approval-command-safety`): every capture below classified `safe` on
    /// the shipped 0.16.0 classifier — the false-SAFE evidence that retired
    /// it (boundary poisoning, `$'…'` quoting, environment injection, code
    /// execution through a read-mostly verb, secret reads, and the leaky
    /// `git` family). The outcome no longer depends on the command text at
    /// all: each escalates, and none dispatches a keystroke.
    #[test]
    fn command_text_cannot_steer_the_outcome() {
        // Boundary poisoning: an approval marker INSIDE the command text used
        // to move the command-slice boundary.
        let poisoned = "Bash command\n  ls\n  rm -rf /important # [y/N]\n\
                        Do you want to proceed?\n❯ 1. Yes\n  2. No\n(esc to cancel)";
        let cases: &[&str] = &[
            poisoned,
            &live_safe_capture("git reset $'--hard' HEAD~1"),
            &live_safe_capture("find . $'-delete'"),
            &live_safe_capture("export GIT_SSH_COMMAND=id; git fetch"),
            &live_safe_capture("awk 'BEGIN{system(\"id\")}'"),
            &live_safe_capture("cat ~/.ssh/id_rsa"),
            &live_safe_capture("git stash"),
            &live_safe_capture("git paw purge"),
        ];
        for capture in cases {
            let probe = sweep_probe(capture, "feat-a", 2);
            assert!(
                probe.outcome.is_some(),
                "capture must escalate: {capture:?} (got {:?})",
                probe.outcome
            );
            assert_eq!(
                probe.escalation().verdict,
                PERMISSION_PROMPT_VERDICT,
                "one verdict for every prompt: {capture:?}"
            );
            assert!(
                probe.keys.is_empty(),
                "no keystroke may be dispatched for {capture:?}, got {:?}",
                probe.keys
            );
        }
    }

    /// The broker review item for a permission prompt names the agent and the
    /// command and says plainly that git-paw approves none — it is the whole
    /// escalation an orchestrator outside the session reads.
    #[test]
    fn permission_prompt_review_item_names_the_prompt_and_approves_nothing() {
        let e = Escalation {
            agent_id: "feat-a".to_string(),
            verdict: PERMISSION_PROMPT_VERDICT.to_string(),
            command: "ls -la".to_string(),
        };
        let q = e.question();
        assert!(q.contains("feat-a") && q.contains("ls -la"), "{q}");
        assert!(
            q.contains("does not approve prompts"),
            "the question must not imply git-paw might answer it: {q}"
        );
        assert!(
            !q.to_ascii_lowercase().contains("auto-approve"),
            "no auto-approve wording survives: {q}"
        );
    }

    // --- the manual-decision log (approval-pattern-surfacing) --------------

    /// Sweeps the same pane `sweeps` times through ONE dedup window, routing
    /// every forwarded decision to `sink`.
    ///
    /// Sharing the window across sweeps is the point: it is what lets a test
    /// distinguish "logged once per escalation" from "logged once per sweep".
    fn sweep_n_with_sink(
        capture: &str,
        agent_id: &str,
        pane_index: usize,
        sweeps: usize,
        sink: &mut dyn ManualDecisionSink,
    ) -> Vec<Escalation> {
        let enumerator = FakeEnumerator { panes: Vec::new() };
        let capturer = FakeCapturer::new(&[(pane_index, capture)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![Vec::new()]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let tmp = tempfile::TempDir::new().expect("tmp repo root");
        {
            let mut deps = DriveDeps {
                enumerator: &enumerator,
                capturer: &capturer,
                dispatcher: &mut dispatcher,
                status: &status,
                messages: &messages,
                clock: &clock,
                merge_check: &FakeMergeCheck::default(),
                alerts: &mut alerts,
                learnings: &mut learnings,
                manual_decisions: sink,
                refresher: &mut refresher,
                wedged: Vec::new(),
            };
            let config = DriveConfig::default();
            let mut dedup = DedupWindow::new(config.dedup_window);
            for _ in 0..sweeps {
                let ctx = SweepContext {
                    session: "paw-test",
                    repo_root: tmp.path(),
                };
                let pane = PaneInfo {
                    pane_index,
                    owner: "/repo-feat-a".to_string(),
                };
                sweep_pane(ctx, &pane, agent_id, &mut deps, &config, &mut dedup, &[]);
            }
        }
        alerts.escalations
    }

    /// Spec scenario "A drive-loop escalation appends a JSONL line", sink
    /// half: one escalated prompt produces exactly one sink call, carrying
    /// the agent and the capture the pattern is later extracted from.
    #[test]
    fn one_escalation_records_one_manual_decision() {
        let capture = live_safe_capture("ls -la");
        let mut sink = RecordingManualDecisions::default();
        let escalations = sweep_n_with_sink(&capture, "feat-a", 2, 1, &mut sink);

        assert_eq!(escalations.len(), 1, "one escalation");
        assert_eq!(sink.calls.len(), 1, "exactly one record: {:?}", sink.calls);
        assert_eq!(sink.calls[0].0, "feat-a");
        assert_eq!(sink.calls[0].1, capture);
    }

    /// Spec scenario "A deduplicated drive-loop escalation is logged once":
    /// the same unresolved prompt seen again inside the dedup window emits no
    /// second escalation, so it must emit no second log line either —
    /// otherwise `git paw approvals` would count one stuck prompt once per
    /// sweep and rank it above genuinely frequent commands.
    #[test]
    fn a_deduplicated_escalation_records_once() {
        let capture = live_safe_capture("ls -la");
        let mut sink = RecordingManualDecisions::default();
        let escalations = sweep_n_with_sink(&capture, "feat-a", 2, 2, &mut sink);

        assert_eq!(escalations.len(), 1, "the second sweep is deduped");
        assert_eq!(
            sink.calls.len(),
            1,
            "the deduped sweep must not re-record: {:?}",
            sink.calls
        );
    }

    /// Spec scenario "Non-prompt escalations are not logged", CLI-unavailable
    /// half: a usage-limit banner is an escalation, but it is not a command
    /// awaiting a decision, so it must not enter a log `git paw approvals`
    /// reads as "commands that needed a human".
    #[test]
    fn a_cli_unavailable_escalation_records_no_manual_decision() {
        let mut sink = RecordingManualDecisions::default();
        let escalations =
            sweep_n_with_sink("You've hit your session limit", "feat-a", 2, 1, &mut sink);

        assert_eq!(escalations.len(), 1, "the banner still escalates");
        assert_eq!(escalations[0].verdict, "cli-unavailable");
        assert!(
            sink.calls.is_empty(),
            "a banner is not a permission prompt: {:?}",
            sink.calls
        );
    }

    /// Spec scenario "Non-prompt escalations are not logged", wedged-nudge
    /// half: an input box still holding a paste placeholder after recovery is
    /// escalated as `nudge-wedged`. It is a stuck pane, not a pending
    /// command, so nothing is recorded.
    #[test]
    fn a_nudge_wedged_escalation_records_no_manual_decision() {
        let mut sink = RecordingManualDecisions::default();
        let escalations = sweep_n_with_sink(
            &idle_input_box("[Pasted text #1 +5 lines]"),
            "feat-a",
            2,
            1,
            &mut sink,
        );

        assert!(
            escalations
                .iter()
                .any(|e| e.verdict == NUDGE_WEDGED_VERDICT),
            "the wedged pane still escalates: {escalations:?}"
        );
        assert!(
            sink.calls.is_empty(),
            "a wedged nudge is not a permission prompt: {:?}",
            sink.calls
        );
    }

    /// Spec scenario "Non-prompt escalations are not logged", correction-loop
    /// half: a branch that exhausts its correction budget is escalated by
    /// `run_correction_pass`, never by the pane sweep, so no command is
    /// recorded for it.
    #[test]
    fn a_correction_loop_signal_records_no_manual_decision() {
        let agents = correction_agents();
        let enumerator = correction_panes();
        // No live prompt on the pane: the only escalations are the correction
        // loop's own.
        let capturer = FakeCapturer::new(&[(2, "standing by...\n$ ")]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "working")]]);
        let messages = ScriptedMessages::new(
            (0..6)
                .map(|_| vec![gate_feedback("feat-a", "testing", "still failing")])
                .collect(),
        );
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = RecordingManualDecisions::default();
        let mut refresher = RecordingRefresher::default();
        {
            let mut deps = DriveDeps {
                enumerator: &enumerator,
                capturer: &capturer,
                dispatcher: &mut dispatcher,
                status: &status,
                messages: &messages,
                clock: &clock,
                merge_check: &FakeMergeCheck::default(),
                alerts: &mut alerts,
                learnings: &mut learnings,
                manual_decisions: &mut manual_decisions,
                refresher: &mut refresher,
                wedged: Vec::new(),
            };
            let config = DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_secs(5),
                correction: CorrectionConfig {
                    auto_loopback: true,
                    max_cycles: 3,
                    on_exhausted: OnExhausted::Escalate,
                    ..CorrectionConfig::default()
                },
                clis: zero_settle_delay_clis(),
                ..DriveConfig::default()
            };
            drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        }

        assert!(
            alerts
                .escalations
                .iter()
                .any(|e| e.verdict == CORRECTION_EXHAUSTED_VERDICT),
            "the branch was flagged unrecoverable: {:?}",
            alerts.escalations
        );
        assert!(
            manual_decisions.calls.is_empty(),
            "a correction-loop signal is not a permission prompt: {:?}",
            manual_decisions.calls
        );
    }

    /// Spec scenario "A drive-loop escalation appends a JSONL line",
    /// cross-module: the production sink around a REAL
    /// [`ManualDecisionRecorder`] writes the line to disk, with every field
    /// the spec requires. Exercised end to end from one escalating sweep, so
    /// the sweep → sink → recorder → file path is covered as a whole rather
    /// than per-layer.
    #[test]
    fn a_drive_loop_escalation_appends_a_jsonl_line() {
        let tmp = tempfile::TempDir::new().expect("tmp repo root");
        let log = crate::supervisor::manual_approvals::log_path(tmp.path(), "paw-test");
        let recorder = crate::supervisor::manual_approvals::ManualDecisionRecorder::new(
            log.clone(),
            true,
            false,
            "proj".to_string(),
            Some("claude".to_string()),
        );
        let mut sink = RecorderManualDecisionSink::new(recorder, None);
        sweep_n_with_sink(&live_safe_capture("ls -la"), "feat-a", 2, 1, &mut sink);

        let body = std::fs::read_to_string(&log).expect("the log must exist");
        let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), 1, "exactly one line: {body}");
        let entry: serde_json::Value =
            serde_json::from_str(lines[0]).expect("the line must be JSON");
        assert_eq!(entry["agent_id"], "feat-a");
        assert_eq!(entry["pattern"], "ls -la");
        assert_eq!(entry["first_seen"], true);
        assert!(
            entry["timestamp"]
                .as_str()
                .is_some_and(|t| t.ends_with('Z')),
            "an ISO-8601 UTC timestamp: {entry}"
        );
    }

    /// Spec scenario "The opt-out applies to the drive loop": with
    /// `manual_approvals_log = false` — and learnings ON, so the gate under
    /// test is the opt-out and not the learnings switch — the sweep still
    /// escalates, but writes no file and returns no learning to publish.
    #[test]
    fn the_opt_out_applies_to_the_drive_loop() {
        let tmp = tempfile::TempDir::new().expect("tmp repo root");
        let log = crate::supervisor::manual_approvals::log_path(tmp.path(), "paw-test");
        let recorder = crate::supervisor::manual_approvals::ManualDecisionRecorder::new(
            log.clone(),
            false, // manual_approvals_log = false
            true,  // learnings = true
            "proj".to_string(),
            Some("claude".to_string()),
        );
        // No broker URL: a learning that somehow escaped the opt-out would
        // have nowhere to go, so the file check below is the real assertion.
        let mut sink = RecorderManualDecisionSink::new(recorder, None);
        let escalations =
            sweep_n_with_sink(&live_safe_capture("ls -la"), "feat-a", 2, 1, &mut sink);

        assert_eq!(
            escalations.len(),
            1,
            "the opt-out suppresses the LOG, never the escalation"
        );
        assert!(
            !log.exists(),
            "no log file may be written under the opt-out: {}",
            log.display()
        );
    }

    /// The opt-out is the recorder's own gate, so it also withholds the
    /// first-seen learning the sink would otherwise publish.
    #[test]
    fn the_opt_out_withholds_the_first_seen_learning() {
        let tmp = tempfile::TempDir::new().expect("tmp repo root");
        let mut recorder = crate::supervisor::manual_approvals::ManualDecisionRecorder::new(
            crate::supervisor::manual_approvals::log_path(tmp.path(), "paw-test"),
            false,
            true,
            "proj".to_string(),
            Some("claude".to_string()),
        );
        assert!(
            recorder
                .record_forwarded("feat-a", &live_safe_capture("ls -la"))
                .is_none(),
            "the opt-out returns no learning to publish"
        );
    }

    // --- input-box reading: wrapped rows + placeholder (task 1) ------------

    /// An idle pane sitting at its input box with `text` unsubmitted.
    fn idle_input_box(text: &str) -> String {
        format!(
            "● Ran tool\n╭────────────────────────────────╮\n│ > {text}                       │\n╰────────────────────────────────╯\n  ? for shortcuts"
        )
    }

    /// A two-row wrapped input box: the sigil row carries only `first`,
    /// a continuation row (no sigil) carries `second`.
    fn wrapped_input_box(first: &str, second: &str) -> String {
        format!(
            "╭──────────────────────────────────────╮\n│ > {first}      │\n│   {second}      │\n╰──────────────────────────────────────╯"
        )
    }

    #[test]
    fn input_box_rows_single_row_is_unchanged() {
        let capture = idle_input_box("please continue");
        assert_eq!(
            input_box_rows(&capture),
            Some(vec!["please continue".to_string()])
        );
        assert_eq!(
            input_box_text(&capture),
            Some("please continue".to_string())
        );
    }

    /// Spec scenario "A wrapped nudge still on the input line is recognised as
    /// unsubmitted": the continuation row carries no sigil, and
    /// `input_box_rows` reads it in order anyway.
    #[test]
    fn input_box_rows_wrapped_two_row_box_joins_in_order() {
        let capture = wrapped_input_box(
            "please review the changes and",
            "continue with the next task",
        );
        assert_eq!(
            input_box_rows(&capture),
            Some(vec![
                "please review the changes and".to_string(),
                "continue with the next task".to_string(),
            ])
        );
        assert_eq!(
            input_box_text(&capture),
            Some("please review the changes and continue with the next task".to_string())
        );
    }

    /// Spec scenario "A paste-buffer placeholder is recognised as unsubmitted".
    #[test]
    fn input_box_shows_placeholder_is_detected() {
        let placeholder = idle_input_box("[Pasted text #1 +5 lines]");
        assert!(input_box_shows_placeholder(&placeholder));
        assert!(!input_box_shows_placeholder(&idle_input_box(
            "please continue"
        )));
    }

    #[test]
    fn input_box_rows_excludes_a_numbered_option_line() {
        assert_eq!(input_box_rows("│ ❯ 1. Yes │"), None);
    }

    #[test]
    fn input_box_rows_is_none_for_an_unrecognised_shape() {
        assert_eq!(input_box_rows("standing by...\n$ "), None);
    }

    // --- idle pane with buffered input (D) ----------------------------------

    /// Spec scenario "A buffered directive on an idle pane is submitted".
    #[test]
    fn idle_pane_with_buffered_input_gets_a_follow_up_enter() {
        assert!(pane_has_buffered_input(&idle_input_box(
            "please continue with task 3"
        )));
    }

    /// Spec scenario "A mid-response pane is left alone": the same non-empty
    /// input box while the CLI is generating is NOT submitted.
    #[test]
    fn mid_response_pane_is_never_submitted() {
        let mut capture = idle_input_box("please continue with task 3");
        capture.push_str("\n  Thinking… (esc to interrupt)");
        assert!(!pane_has_buffered_input(&capture));
    }

    /// An empty input box is not a stranded directive, and neither is a pane
    /// showing a live prompt (the approval path owns that state).
    #[test]
    fn empty_box_and_live_prompt_are_not_buffered_input() {
        assert!(!pane_has_buffered_input(&idle_input_box("")));
        assert!(!pane_has_buffered_input(&live_safe_capture("cargo test")));
        assert!(
            !pane_has_buffered_input("│ ❯ 1. Yes │"),
            "a boxed option line is not buffered input"
        );
    }

    /// An idle pane sitting at its input box with `text` unsubmitted,
    /// rendered in current Claude Code's REAL shape — ground-truthed against
    /// a live `tmux capture-pane -p`: NO side borders at all, just a rule
    /// line, the bare sigil line, another rule line, then the mode strip.
    fn idle_input_box_rule_style(text: &str) -> String {
        format!(
            "● Ran tool\n────────────────────────────────\n❯ {text}\n────────────────────────────────\n  ⏵⏵ accept edits on (shift+tab to cycle) · ? for shortcuts"
        )
    }

    /// Gate-review safety finding: `input_box_text` originally only
    /// recognised a `│`-bordered box, so current Claude Code's real
    /// borderless shape was invisible to buffered-input detection — an idle
    /// pane holding unsubmitted text would never get its follow-up `Enter`.
    #[test]
    fn idle_pane_with_buffered_input_gets_a_follow_up_enter_borderless_shape() {
        assert!(pane_has_buffered_input(&idle_input_box_rule_style(
            "please continue with task 3"
        )));
    }

    /// An empty borderless input box is not a stranded directive either.
    #[test]
    fn empty_borderless_box_is_not_buffered_input() {
        assert!(!pane_has_buffered_input(&idle_input_box_rule_style("")));
    }

    /// Gate-review safety finding: a borderless input box whose composer text
    /// wraps across multiple plain lines (no sigil, no border) up to the
    /// closing rule line must have every row read as buffered input, exactly
    /// like a bordered box's wrapped continuation rows.
    #[test]
    fn borderless_multi_row_buffered_input_is_detected() {
        let capture = "● Ran tool\n\
            ────────────────────────────────\n\
            ❯ please continue with the long\n\
            running task from before\n\
            ────────────────────────────────\n\
              ⏵⏵ accept edits on (shift+tab to cycle) · ? for shortcuts";
        assert!(pane_has_buffered_input(capture));
        assert_eq!(
            input_box_text(capture).as_deref(),
            Some("please continue with the long running task from before")
        );
    }

    /// Gate-review safety finding: a paste-buffer placeholder in the
    /// borderless input box is recognised the same way as the bordered one —
    /// never re-typed verbatim, always escalated as wedged if it survives a
    /// recovery attempt.
    #[test]
    fn borderless_paste_buffer_placeholder_is_detected() {
        let placeholder = idle_input_box_rule_style("[Pasted text #1 +5 lines]");
        assert!(input_box_shows_placeholder(&placeholder));
        assert!(!input_box_shows_placeholder(&idle_input_box_rule_style(
            "please continue"
        )));
    }

    /// Builds a one-agent, single-pane `drive_loop_in_tmp` run against
    /// `capturer` and `status`, returning the dispatcher events and the
    /// recorded alerts. Shared by the idle-pane stranded-input scenarios
    /// below (task 3), which differ only in capture sequence and completion
    /// schedule.
    fn run_stranded_idle_pane_case(
        capturer: &dyn PaneCapture,
        status: &ScriptedStatus,
    ) -> (Vec<(usize, String)>, RecordingAlerts) {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        let mut dispatcher = RecordingDispatcher::default();
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer,
            dispatcher: &mut dispatcher,
            status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        (dispatcher.events, alerts)
    }

    /// Spec scenario "A buffered directive on an idle pane is submitted": the
    /// follow-up `Enter` alone clears the box, so no recovery keystrokes
    /// follow and no free text is ever typed.
    #[test]
    fn loop_submits_a_stranded_directive_on_an_idle_pane() {
        let stuck = idle_input_box("/opsx:apply my-change");
        let capturer = SequencedCapturer::new(&[&stuck, &stuck, IDLE_PANE]);
        let (events, _alerts) = run_stranded_idle_pane_case(
            &capturer,
            &ScriptedStatus::new(vec![vec![row("supervisor", "done")]]),
        );
        assert_eq!(
            events,
            vec![(2, "Enter".to_string())],
            "only the submitting Enter, never free text"
        );
    }

    /// Spec scenario "A stale line that survives the Enter is cleared,
    /// re-typed, and submitted": the box still shows the text after the
    /// follow-up `Enter`, so the shared recovery helper clears it (`C-u`),
    /// re-types the WHOLE box's text, and sends `Enter` again.
    #[test]
    fn loop_recovers_a_stale_line_on_an_idle_pane() {
        let stuck = idle_input_box("/opsx:apply my-change");
        let capturer = SequencedCapturer::new(&[&stuck, &stuck, &stuck, IDLE_PANE]);
        let (events, alerts) = run_stranded_idle_pane_case(
            &capturer,
            &ScriptedStatus::new(vec![vec![row("supervisor", "done")]]),
        );
        assert_eq!(
            events,
            vec![
                (2, "Enter".to_string()),
                (2, "C-u".to_string()),
                (2, "/opsx:apply my-change".to_string()),
                (2, "Enter".to_string()),
            ],
            "the Enter alone does not clear a genuinely stale line, so \
             recovery clears, re-types the whole box, and submits"
        );
        assert!(alerts.escalations.is_empty(), "recovered — no escalation");
    }

    /// Spec scenario "A paste-buffer placeholder is never cleared": a
    /// placeholder surviving the `Enter` is never `C-u`'d (it cannot be
    /// re-typed verbatim) — the pane is escalated as wedged instead.
    #[test]
    fn loop_escalates_a_placeholder_stranded_on_an_idle_pane() {
        let placeholder = idle_input_box("[Pasted text #1 +5 lines]");
        let capturer = SequencedCapturer::new(&[&placeholder, &placeholder, &placeholder]);
        let (events, alerts) = run_stranded_idle_pane_case(
            &capturer,
            &ScriptedStatus::new(vec![vec![row("supervisor", "done")]]),
        );
        assert_eq!(
            events,
            vec![(2, "Enter".to_string())],
            "a placeholder is never cleared with C-u"
        );
        assert_eq!(alerts.escalations.len(), 1, "escalated as wedged");
        assert_eq!(alerts.escalations[0].verdict, NUDGE_WEDGED_VERDICT);
    }

    /// Spec scenario "A persistently stranded line is escalated once": the
    /// same stuck line surviving every recovery attempt on two successive
    /// sweeps records exactly one broker escalation, not one per sweep —
    /// otherwise a wave with a wedged idle pane re-floods the broker every
    /// ~15s exactly like the bug this change fixes for the nudge path.
    #[test]
    fn persistently_stranded_idle_pane_escalates_once_across_two_sweeps() {
        let stuck = idle_input_box("/opsx:apply my-change");
        // FakeCapturer never changes, so every capture across both sweeps
        // shows the same stuck line — a genuinely wedged pane.
        let capturer = FakeCapturer::new(&[(2, &stuck)]);
        let status = ScriptedStatus::new(vec![
            vec![row("feat-a", "working")],  // tick 1: not yet complete
            vec![row("supervisor", "done")], // tick 2: complete
        ]);
        let (_events, alerts) = run_stranded_idle_pane_case(&capturer, &status);
        assert_eq!(
            alerts.escalations.len(),
            1,
            "deduped across two sweeps, not one escalation per sweep"
        );
        assert_eq!(alerts.escalations[0].verdict, NUDGE_WEDGED_VERDICT);
    }

    // --- dedup (task 5.1/5.2) ----------------------------------------------

    #[test]
    fn dedup_emits_once_per_window_for_repeated_prompt() {
        let mut win = DedupWindow::new(Duration::from_mins(5));
        let now = Instant::now();
        let key = "feat-a\u{1f}cargo test";
        assert!(win.should_emit(key, now), "first sighting emits");
        assert!(
            !win.should_emit(key, now + Duration::from_secs(10)),
            "repeat within window is suppressed"
        );
        assert!(
            win.should_emit(key, now + Duration::from_secs(301)),
            "after the window it emits again"
        );
    }

    #[test]
    fn dedup_shape_distinguishes_commands_sharing_boilerplate() {
        let footer = "Do you want to proceed?\n❯ 1. Yes\n  2. No\n(esc to cancel)";
        let cargo = format!("Bash command\n  cargo test --workspace\n{footer}");
        let push = format!("Bash command\n  git push origin main\n{footer}");
        // Distinct commands under the SAME boilerplate must yield distinct keys.
        assert_ne!(
            approval_dedup_key("feat-a", &cargo),
            approval_dedup_key("feat-a", &push),
            "dedup must key on command identity, not boilerplate"
        );
    }

    // --- detect_completion (task 6.1) --------------------------------------

    #[test]
    fn completion_on_supervisor_verdict() {
        let rows = vec![row("supervisor", "done"), row("feat-a", "working")];
        assert_eq!(
            detect_completion(&rows, &["feat-a".to_string()]),
            Some(CompletionReason::Verdict)
        );
    }

    #[test]
    fn completion_when_all_agents_checked() {
        let rows = vec![row("feat-a", "verified"), row("feat-b", "done")];
        assert_eq!(
            detect_completion(&rows, &["feat-a".to_string(), "feat-b".to_string()]),
            Some(CompletionReason::AllTasksChecked)
        );
    }

    #[test]
    fn no_completion_while_an_agent_still_works() {
        let rows = vec![row("feat-a", "verified"), row("feat-b", "working")];
        assert_eq!(
            detect_completion(&rows, &["feat-a".to_string(), "feat-b".to_string()]),
            None
        );
    }

    #[test]
    fn committed_alone_is_not_completion() {
        let rows = vec![row("feat-a", "committed")];
        assert_eq!(detect_completion(&rows, &["feat-a".to_string()]), None);
    }

    // --- summary renderer (task 6.3) ---------------------------------------

    #[test]
    fn summary_reports_outcome_states_and_escalations() {
        let summary = DriveSummary {
            outcome: DriveOutcome::EscalatedForReview,
            agent_states: vec![
                ("feat-a".to_string(), "verified".to_string()),
                ("feat-b".to_string(), "working".to_string()),
            ],
            escalations: vec![Escalation {
                agent_id: "feat-b".to_string(),
                verdict: "danger".to_string(),
                command: "git push origin main".to_string(),
            }],
            exhausted: Vec::new(),
            broker_log_hint: Some("/tmp/broker.log".to_string()),
            learnings_hint: Some(".git-paw/session-learnings.md".to_string()),
            merged_summary: None,
        };
        let text = summary.render();
        assert!(text.contains("escalated-for-review"), "states outcome");
        assert!(text.contains("feat-a: verified"), "per-agent state");
        assert!(text.contains("git push origin main"), "escalation listed");
        assert!(text.contains("/tmp/broker.log"), "broker log pointer");
        assert!(
            text.contains(".git-paw/session-learnings.md"),
            "learnings pointer"
        );
    }

    /// Scenario "The summary does not claim orchestrator coverage"
    /// (`supervisor-unattended-operation`): the orchestrator runs outside the
    /// session, so every summary says its CLI is not monitored by the loop —
    /// whatever the outcome.
    #[test]
    fn summary_states_the_orchestrator_cli_is_not_monitored() {
        for outcome in [
            DriveOutcome::Completed,
            DriveOutcome::EscalatedForReview,
            DriveOutcome::Heartbeat,
        ] {
            let summary = DriveSummary {
                outcome,
                agent_states: vec![("feat-a".to_string(), "verified".to_string())],
                escalations: Vec::new(),
                exhausted: Vec::new(),
                broker_log_hint: None,
                learnings_hint: None,
                merged_summary: None,
            };
            let text = summary.render();
            assert!(
                text.contains("its CLI is not monitored by the loop"),
                "{outcome:?}: the summary must disclaim orchestrator coverage; got:\n{text}"
            );
        }
    }

    // --- wedged nudges reach the summary, once (task 4) ---------------------

    /// Spec scenario "A wedged nudge appears in the exit summary": a coding
    /// pane whose buffered input is still wedged after every recovery attempt
    /// is folded into `DriveSummary.escalations` — once, however many sweeps
    /// see it — which upgrades an otherwise-`Completed` wave to
    /// `escalated-for-review`, fixing the 0.16.0 field bug where a wedged
    /// nudge (broker-only) left the printed summary reading `0 escalations`.
    #[test]
    fn a_wedged_buffered_input_reaches_the_summary_once() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 1,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        // An unreadable paste placeholder on every capture: the follow-up
        // `Enter` and every recovery attempt leave it in place, so the pane
        // wedges on both sweeps.
        let placeholder = idle_input_box("[Pasted text #1 +5 lines]");
        let capturer = FakeCapturer::new(&[(1, &placeholder)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![vec![], vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        assert_eq!(summary.escalations.len(), 1, "wedge reported exactly once");
        assert_eq!(summary.escalations[0].verdict, NUDGE_WEDGED_VERDICT);
        assert_eq!(summary.escalations[0].agent_id, "feat-a");
    }

    // --- GP-05: exit summary resolves terminal state (unattended-wave-lifecycle) --

    #[test]
    fn completed_wave_resolves_terminal_state_not_working() {
        let agents = vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
                branch: "feat/a".to_string(),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
                branch: "feat/b".to_string(),
            },
        ];
        let enumerator = FakeEnumerator { panes: vec![] };
        let capturer = FakeCapturer::new(&[]);
        let mut dispatcher = RecordingDispatcher::default();
        // Tick 1: feat-a reaches its terminal artifact, feat-b is only
        // committed (awaiting merge) — no completion yet either way. Tick 2:
        // the supervisor's own verdict fires completion, but feat-a's row has
        // since regressed to a generic `working` heartbeat while feat-b's own
        // row only now catches up to `verified`. The literal snapshot at exit
        // alone would misreport feat-a as still working.
        let status = ScriptedStatus::new(vec![
            vec![row("feat-a", "verified"), row("feat-b", "committed")],
            vec![
                row("supervisor", "done"),
                row("feat-a", "working"),
                row("feat-b", "verified"),
            ],
        ]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_millis(0),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };

        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        assert_eq!(summary.outcome, DriveOutcome::Completed);
        assert!(
            summary
                .agent_states
                .iter()
                .all(|(_, s)| !s.starts_with("working")),
            "resolved state must never regress to working: {:?}",
            summary.agent_states
        );
        assert!(
            summary
                .agent_states
                .contains(&("feat-a".to_string(), "verified · merged".to_string())),
            "feat-a resolves to its earlier terminal artifact: {:?}",
            summary.agent_states
        );
        assert!(summary.escalations.is_empty());
        let text = summary.render();
        assert!(
            text.contains("2/2 branches merged"),
            "wave-level outcome line: {text}"
        );
    }

    // --- GP-06: drive loop bound to a session instance (unattended-wave-lifecycle) --

    #[test]
    fn instance_torn_down_stops_the_loop_before_the_next_sweep() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 1,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(1, &live_safe_capture("cargo test"))]);
        let mut dispatcher = RecordingDispatcher::default();
        // Never completes on its own — the instance teardown is what ends it.
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "working")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_millis(0),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        // Current for tick 1 (the approval happens), torn down from tick 2 on
        // — modelling a purge/stop, or replacement by a new same-named
        // session, partway through the run.
        let instance = TornDownAfter::new(1);

        let repo = tempfile::tempdir().expect("temp repo root");
        let summary = drive_loop(
            "paw-test",
            repo.path(),
            &agents,
            &mut deps,
            &config,
            &instance,
        );

        assert_eq!(summary.outcome, DriveOutcome::SessionTornDown);
        assert_eq!(
            alerts.escalations.len(),
            1,
            "only the first (pre-teardown) sweep acted — the loop must never \
             act on a subsequently-created same-named session's panes: {:?}",
            alerts.escalations
        );
        assert!(
            dispatcher.events.is_empty(),
            "no pane is typed into either way: {:?}",
            dispatcher.events
        );
    }

    /// Spec scenarios "Stop revokes the binding before a quick recovery" and
    /// "Recovering a stopped session supersedes a leftover binding": a loop
    /// whose binding was revoked or taken over exits with `session-torn-down`
    /// *before* it acts, even though the session record still says it is the
    /// current instance (recovery keeps `created_at`, so the record check
    /// alone cannot see a quick stop-then-recover).
    ///
    /// The pane on offer carries a live permission prompt, so a sweep that
    /// happened at all would leave an escalation behind. None does.
    #[test]
    fn lost_binding_ends_the_loop_before_it_acts() {
        use super::super::loop_binding::{BindMode, DriveLoopBinding, revoke};

        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 1,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(1, &live_safe_capture("cargo test"))]);
        let mut dispatcher = RecordingDispatcher::default();
        // Never completes on its own — only the lost binding ends it.
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "working")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_millis(0),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };

        let repo = tempfile::tempdir().expect("temp repo root");
        let binding = DriveLoopBinding::acquire(
            repo.path(),
            "paw-test",
            "1700000000",
            BindMode::Fresh,
            Duration::from_secs(120),
        )
        .expect("the loop binds before it runs");
        // `git paw stop` (or a re-attach superseding this loop) between the
        // bind and the first tick.
        revoke(repo.path());

        let instance = BoundInstanceGuard {
            // The record half still reports the instance current, so the
            // binding is demonstrably what ends the loop.
            inner: AlwaysCurrentGuard,
            binding: Some(binding),
        };

        let summary = drive_loop(
            "paw-test",
            repo.path(),
            &agents,
            &mut deps,
            &config,
            &instance,
        );

        assert_eq!(
            summary.outcome,
            DriveOutcome::SessionTornDown,
            "a revoked binding ends the loop as a torn-down instance"
        );
        assert!(
            alerts.escalations.is_empty(),
            "the loop must exit before sweeping — a sweep of this pane would \
             have escalated its live prompt: {:?}",
            alerts.escalations
        );
        assert!(
            dispatcher.events.is_empty(),
            "no keystroke reaches a session this loop no longer drives: {:?}",
            dispatcher.events
        );
    }

    #[test]
    fn instance_guard_true_lets_a_live_session_keep_driving() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator { panes: vec![] };
        let capturer = FakeCapturer::new(&[]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "verified")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };

        // `drive_loop_in_tmp` binds an always-current guard, so a live session
        // instance completes exactly as it did before GP-06.
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        assert_eq!(summary.outcome, DriveOutcome::Completed);
    }

    // --- the loop against the fake terminal backend ---------------------------
    //
    // These drive the loop through the terminal port's fake: one
    // `FakeBackend` serves as enumerator, capturer and dispatcher, answering
    // scripted owners and screens and recording every operation. The fake
    // spawns no process, so no `tmux` process is spawned by construction.

    use crate::terminal::fake::{FakeBackend, Operation};
    use crate::terminal::{Key, SurfaceId};

    const FAKE_SESSION: &str = "paw-test";

    /// Lends a shared [`FakeBackend`] to the loop as its dispatcher; the
    /// port's operations take `&self`, so the same fake is also the
    /// enumerator and capturer. Delegates to the production key mapping.
    struct SharedFakeDispatcher<'a, B: TerminalBackend + ?Sized>(&'a B);

    impl<B: TerminalBackend + ?Sized> KeyDispatcher for SharedFakeDispatcher<'_, B> {
        fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()> {
            super::super::keys::dispatch_key(self.0, session, pane_index, key)
        }
        fn send_text(
            &mut self,
            session: &str,
            pane_index: usize,
            text: &str,
        ) -> std::io::Result<()> {
            super::super::keys::dispatch_text(self.0, session, pane_index, text)
        }
    }

    fn agent_pane(agent_id: &str, worktree: &str) -> AgentPane {
        AgentPane {
            agent_id: agent_id.to_string(),
            worktree_path: PathBuf::from(worktree),
            branch: agent_id.replacen('-', "/", 1),
        }
    }

    /// Adds a surface at `position` owned by `owner` showing `screens`.
    fn fake_surface(
        fake: &FakeBackend,
        position: usize,
        owner: &str,
        screens: &[&str],
    ) -> SurfaceId {
        let surface = fake.add_surface(FAKE_SESSION, position, Some(Path::new(owner)));
        let screens: Vec<Option<&str>> = screens.iter().map(|s| Some(*s)).collect();
        fake.script_screens(&surface, &screens);
        surface
    }

    /// Runs the loop with `fake` as its terminal and returns the summary and
    /// the recorded alerts.
    fn drive_with_fake_backend<B: TerminalBackend>(
        fake: &B,
        agents: &[AgentPane],
        status: &ScriptedStatus,
    ) -> (DriveSummary, RecordingAlerts) {
        let mut dispatcher = SharedFakeDispatcher(fake);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let summary = {
            let mut deps = DriveDeps {
                enumerator: fake,
                capturer: fake,
                dispatcher: &mut dispatcher,
                status,
                messages: &messages,
                clock: &clock,
                merge_check: &FakeMergeCheck::default(),
                alerts: &mut alerts,
                learnings: &mut learnings,
                manual_decisions: &mut manual_decisions,
                refresher: &mut refresher,
                wedged: Vec::new(),
            };
            let config = DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                ..DriveConfig::default()
            };
            drive_loop_in_tmp(FAKE_SESSION, agents, &mut deps, &config)
        };
        (summary, alerts)
    }

    fn keystrokes_on(fake: &FakeBackend, surface: &SurfaceId) -> Vec<Operation> {
        fake.operations_on(surface)
            .into_iter()
            .filter(Operation::is_keystroke)
            .collect()
    }

    /// Spec scenario "Drive-loop escalation is asserted without tmux"
    /// (`code-architecture`): a live permission prompt on an owned surface is
    /// escalated for that worktree's agent, and nothing is typed into it.
    #[test]
    fn drive_loop_escalation_is_asserted_without_tmux() {
        let fake = FakeBackend::new();
        let dashboard = fake_surface(&fake, 0, "/repo", &[""]);
        let prompting = fake_surface(
            &fake,
            2,
            "/repo-feat-a",
            &[&live_safe_capture("cargo test")],
        );
        let agents = [agent_pane("feat-a", "/repo-feat-a")];
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "verified")]]);

        let (summary, alerts) = drive_with_fake_backend(&fake, &agents, &status);

        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        assert_eq!(alerts.escalations.len(), 1, "{:?}", alerts.escalations);
        assert_eq!(alerts.escalations[0].verdict, PERMISSION_PROMPT_VERDICT);
        assert_eq!(alerts.escalations[0].agent_id, "feat-a");
        assert!(
            keystrokes_on(&fake, &prompting).is_empty(),
            "the prompting surface receives no keystroke: {:?}",
            fake.operations_on(&prompting)
        );
        assert!(
            fake.operations_on(&dashboard).is_empty(),
            "the dashboard is never captured or typed into"
        );
    }

    /// Spec scenario "Drive-loop buffered-input submit is asserted without
    /// tmux" (`code-architecture`): a directive left unsubmitted on an idle
    /// input box gets exactly the submit key, and no text.
    #[test]
    fn drive_loop_buffered_input_submit_is_asserted_without_tmux() {
        let fake = FakeBackend::new();
        let stuck = idle_input_box("/opsx:apply my-change");
        let surface = fake_surface(&fake, 2, "/repo-feat-a", &[&stuck, &stuck, IDLE_PANE]);
        let agents = [agent_pane("feat-a", "/repo-feat-a")];
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);

        drive_with_fake_backend(&fake, &agents, &status);

        assert_eq!(
            keystrokes_on(&fake, &surface),
            vec![Operation::SendKey {
                surface: surface.clone(),
                key: Key::Enter
            }],
            "only the submitting Enter, never free text"
        );
    }

    /// Spec scenario "Drive loop sweeps through the backend with identical
    /// effects" (`code-architecture`): the loop wired to the tmux backend over
    /// a recording runner lists, captures and submits a buffered input with
    /// the same `list-panes` / `capture-pane` / `send-keys` invocations the
    /// pre-port loop issued, with the same targets and arguments.
    #[test]
    fn drive_loop_sweeps_through_the_tmux_backend_with_identical_effects() {
        use crate::command_runner::CommandOutput;
        use crate::command_runner::test_support::FakeCommandRunner;
        use crate::terminal::tmux::TmuxBackend;

        let stuck = idle_input_box("/opsx:apply my-change");
        let captures = std::cell::Cell::new(0usize);
        let runner = FakeCommandRunner::scripted(move |_, args| {
            let stdout = match args.first().copied() {
                Some("list-panes") => "2 /repo-feat-a\n".to_string(),
                Some("capture-pane") => {
                    let n = captures.get();
                    captures.set(n + 1);
                    if n < 2 {
                        stuck.clone()
                    } else {
                        IDLE_PANE.to_string()
                    }
                }
                _ => String::new(),
            };
            Ok(CommandOutput {
                success: true,
                code: Some(0),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
            })
        });
        let backend = TmuxBackend::with_runner(&runner);
        let agents = [agent_pane("feat-a", "/repo-feat-a")];
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);

        drive_with_fake_backend(&backend, &agents, &status);

        let target = format!("{FAKE_SESSION}:0.2");
        let argv = |parts: &[&str]| -> (String, Vec<String>) {
            (
                "tmux".to_string(),
                parts.iter().map(|p| (*p).to_string()).collect(),
            )
        };
        let calls = runner.calls();
        let list = argv(&[
            "list-panes",
            "-t",
            &format!("{FAKE_SESSION}:0"),
            "-F",
            "#{pane_index} #{pane_current_path}",
        ]);
        let capture = argv(&["capture-pane", "-p", "-t", &target]);
        let enter = argv(&["send-keys", "-t", &target, "Enter"]);
        assert_eq!(
            calls.first(),
            Some(&list),
            "a sweep starts by listing: {calls:?}"
        );
        assert!(calls.contains(&capture), "{calls:?}");
        let keys: Vec<_> = calls.iter().filter(|c| c.1[0] == "send-keys").collect();
        assert_eq!(keys, vec![&enter], "exactly the submit key: {calls:?}");
        assert!(
            calls
                .iter()
                .all(|c| c == &list || c == &capture || c == &enter),
            "no other tmux invocation: {calls:?}"
        );
    }

    /// Spec scenarios "Agent resolved by working directory, not index" and "A
    /// captured prompt is attributed to the correct agent"
    /// (`supervisor-unattended-operation`): the backend lists surfaces in an
    /// order unrelated to the roster, and the prompt is still attributed to
    /// the agent whose worktree owns the prompting surface.
    #[test]
    fn a_captured_prompt_is_attributed_by_owner_not_index() {
        let fake = FakeBackend::new();
        // Listed feat-b first, at a lower index than feat-a, against a
        // roster ordered feat-a, feat-b.
        let prompting = fake_surface(&fake, 3, "/repo-feat-b", &[&live_safe_capture("make")]);
        let quiet = fake_surface(&fake, 5, "/repo-feat-a", &[IDLE_PANE]);
        let agents = [
            agent_pane("feat-a", "/repo-feat-a"),
            agent_pane("feat-b", "/repo-feat-b"),
        ];
        let status = ScriptedStatus::new(vec![vec![
            row("feat-a", "verified"),
            row("feat-b", "verified"),
        ]]);

        let (_summary, alerts) = drive_with_fake_backend(&fake, &agents, &status);

        assert_eq!(alerts.escalations.len(), 1, "{:?}", alerts.escalations);
        assert_eq!(alerts.escalations[0].agent_id, "feat-b");
        assert_eq!(alerts.escalations[0].command, "make");
        assert!(keystrokes_on(&fake, &prompting).is_empty());
        assert!(keystrokes_on(&fake, &quiet).is_empty());
    }

    /// Spec scenarios "A basename collision does not misattribute" and "A
    /// path below a worktree root is not an owner match"
    /// (`supervisor-unattended-operation`): surfaces owned by
    /// `/elsewhere/other-feat-a` and by a directory below feat-a's root are
    /// not feat-a's, so their live prompts are neither attributed nor swept.
    #[test]
    fn a_basename_collision_or_a_subdirectory_owner_is_not_attributed_in_the_loop() {
        let root = "/work/repo/.git-paw/worktrees/feat-a";
        let below = format!("{root}/src");
        let fake = FakeBackend::new();
        let collision = fake_surface(
            &fake,
            2,
            "/elsewhere/other-feat-a",
            &[&live_safe_capture("rm -rf x")],
        );
        let subdir = fake_surface(&fake, 3, &below, &[&live_safe_capture("rm -rf y")]);
        let owned = fake_surface(&fake, 4, root, &[IDLE_PANE]);
        let agents = [agent_pane("feat-a", root)];
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "verified")]]);

        let (summary, alerts) = drive_with_fake_backend(&fake, &agents, &status);

        assert!(
            alerts.escalations.is_empty(),
            "no unowned surface is attributed to feat-a: {:?}",
            alerts.escalations
        );
        assert_eq!(summary.outcome, DriveOutcome::Completed);
        for surface in [&collision, &subdir] {
            assert!(
                fake.operations_on(surface).is_empty(),
                "an unattributed surface is never captured or typed into"
            );
        }
        assert!(keystrokes_on(&fake, &owned).is_empty());
    }

    // --- poll / heartbeat / dedup cadence constants -------------------------

    /// The loop's default cadences match the spec: ~15s poll, ~25min heartbeat,
    /// 5-minute dedup window.
    #[test]
    fn cadence_constants_match_spec() {
        assert_eq!(POLL_INTERVAL, Duration::from_secs(15));
        assert_eq!(HEARTBEAT_INTERVAL, Duration::from_mins(25));
        assert_eq!(DEDUP_WINDOW, Duration::from_mins(5));
        let cfg = DriveConfig::default();
        assert_eq!(cfg.poll_interval, POLL_INTERVAL);
        assert_eq!(cfg.heartbeat, HEARTBEAT_INTERVAL);
        assert_eq!(cfg.dedup_window, DEDUP_WINDOW);
    }

    // --- nudge follow-up Enter (task 4.5 / D6) ------------------------------

    #[test]
    fn nudge_sends_text_then_a_separate_enter() {
        let keys = nudge_keystrokes("please continue");
        assert_eq!(
            keys,
            ["please continue".to_string(), "Enter".to_string()],
            "a nudge sends the text, then a SEPARATE Enter (never a combined text+Enter)"
        );
        // The submitting Enter is its own keystroke, never fused onto the text.
        assert!(
            !keys[0].contains('\n'),
            "the text keystroke carries no newline"
        );
    }

    // --- explicit per-pane capture (task 3.4 / D3) --------------------------

    /// The loop captures each swept pane with its OWN `capture-pane` call — one
    /// per pane, never a single shell `for` loop — and skips every surface
    /// that is not a coding agent (the dashboard, a stray repo-root surface)
    /// entirely. With no live prompt there is no approval re-capture, so the
    /// call count equals the number of coding panes.
    #[test]
    fn sweep_captures_each_pane_exactly_once() {
        let agents = vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
                branch: "feat/a".to_string(),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
                branch: "feat/b".to_string(),
            },
        ];
        let enumerator = FakeEnumerator {
            panes: vec![
                PaneInfo {
                    pane_index: 0,
                    owner: "/repo".to_string(),
                },
                PaneInfo {
                    pane_index: 1,
                    owner: "/repo-feat-a".to_string(),
                },
                PaneInfo {
                    pane_index: 2,
                    owner: "/repo-feat-b".to_string(),
                },
                PaneInfo {
                    pane_index: 3,
                    owner: "/repo".to_string(),
                },
            ],
        };
        let capturer = FakeCapturer::new(&[
            (0, "dashboard\n$ "),
            (1, "working...\n$ "),
            (2, "working...\n$ "),
            (3, "stray shell\n$ "),
        ]);
        let mut dispatcher = RecordingDispatcher::default();
        // Complete on the first poll so exactly one sweep runs.
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        // Panes 1 and 2 captured once each; panes 0 and 3 never captured.
        assert_eq!(
            capturer.calls.get(),
            2,
            "one explicit capture per coding pane, non-coding surfaces skipped"
        );
    }

    // --- full loop: escalation + completion (task 8.1 in-memory) ----------

    /// Spec scenario "A command formerly classified safe is escalated, not
    /// approved" (end-to-end through the loop): the live prompt is escalated,
    /// the loop still detects completion, and NOTHING is typed into the pane.
    #[test]
    fn loop_escalates_live_prompt_then_exits_on_completion() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![
                PaneInfo {
                    pane_index: 0,
                    owner: "/repo".to_string(),
                },
                PaneInfo {
                    pane_index: 2,
                    owner: "/repo-feat-a".to_string(),
                },
            ],
        };
        let capturer = FakeCapturer::new(&[
            (0, ""), // dashboard: never captured
            (2, &live_safe_capture("cargo test")),
        ]);
        let mut dispatcher = RecordingDispatcher::default();
        // The agent's task is verified by the first status poll, so the loop
        // escalates the live prompt once, detects completion, and exits.
        // (The fake capture never clears, so completing on the first sweep
        // keeps the escalation count deterministic.)
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "verified")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        // The wave still finished — the escalation did not block it — but an
        // unanswered prompt means the run ends for review, not clean.
        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        // Pane 2 is the prompting pane: it receives nothing, and nothing is
        // typed anywhere else for the escalation either.
        assert!(
            !dispatcher.events.iter().any(|(p, _)| *p == 2),
            "the prompting pane must receive no keystroke; dispatched: {:?}",
            dispatcher.events
        );
        assert_eq!(alerts.escalations.len(), 1, "one escalation recorded");
        assert_eq!(alerts.escalations[0].verdict, PERMISSION_PROMPT_VERDICT);
        assert_eq!(alerts.escalations[0].command, "cargo test");
    }

    /// Spec scenario "The Rust loop and the shell helper are mutually
    /// exclusive", reverse direction: another sender (here `sweep.sh approve`,
    /// which creates the very same claim file from its own process) already
    /// holds pane 2, so the loop types NO free text into it while recovering
    /// buffered input — and pane 3, unclaimed, is recovered in the same sweep,
    /// proving the skip is per-pane and does not block the wave.
    ///
    /// The claim guards the keystrokes the loop still sends (its nudge,
    /// stale-input and buffered-input recovery paths); it no longer guards an
    /// approval, because the loop sends none.
    #[test]
    fn a_pane_claimed_by_another_sender_receives_no_typed_text() {
        let agents = vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
                branch: "feat/a".to_string(),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
                branch: "feat/b".to_string(),
            },
        ];
        let enumerator = FakeEnumerator {
            panes: vec![
                PaneInfo {
                    pane_index: 2,
                    owner: "/repo-feat-a".to_string(),
                },
                PaneInfo {
                    pane_index: 3,
                    owner: "/repo-feat-b".to_string(),
                },
            ],
        };
        let capturer = FakeCapturer::new(&[
            (2, &idle_input_box("please continue with task 3")),
            (3, &idle_input_box("please continue with task 3")),
        ]);
        let mut dispatcher = RecordingDispatcher::default();
        // Both agents are terminal on the first poll, so exactly one sweep runs
        // and the keystroke count is deterministic.
        let status = ScriptedStatus::new(vec![vec![
            row("feat-a", "verified"),
            row("feat-b", "verified"),
        ]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };

        let repo = tempfile::tempdir().expect("temp repo root");
        // Stand in for the other approver: the claim file, at the path both
        // sides compute, taken before the sweep runs.
        let claimed = crate::supervisor::claim::claim_path(repo.path(), 2);
        std::fs::create_dir_all(claimed.parent().expect("claim parent")).expect("mk tmp dir");
        std::fs::write(&claimed, "").expect("write foreign claim");

        drive_loop(
            "paw-test",
            repo.path(),
            &agents,
            &mut deps,
            &config,
            &AlwaysCurrentGuard,
        );

        assert!(
            !dispatcher.literal_sends.iter().any(|(p, _)| *p == 2),
            "the claimed pane must have no free text typed into it; literal sends were {:?}",
            dispatcher.literal_sends
        );
        assert!(
            dispatcher.literal_sends.iter().any(|(p, _)| *p == 3),
            "the unclaimed pane is still recovered; literal sends were {:?}",
            dispatcher.literal_sends
        );
        assert!(
            claimed.exists(),
            "the loop must leave the other sender's claim in place"
        );
    }

    // --- full loop: escalation is non-blocking (task 8.2 in-memory) -------

    /// Spec scenario "A live prompt is escalated without blocking the other
    /// agent" (`supervisor-unattended-operation`): both panes are prompting,
    /// both escalate, neither is answered, and the wave still reaches its
    /// exit instead of stalling on either one.
    #[test]
    fn loop_escalates_live_prompts_without_blocking_either_agent() {
        let agents = vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
                branch: "feat/a".to_string(),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
                branch: "feat/b".to_string(),
            },
        ];
        let enumerator = FakeEnumerator {
            panes: vec![
                PaneInfo {
                    pane_index: 2,
                    owner: "/repo-feat-a".to_string(),
                },
                PaneInfo {
                    pane_index: 3,
                    owner: "/repo-feat-b".to_string(),
                },
            ],
        };
        // feat-a shows a danger prompt; feat-b shows a safe prompt.
        let capturer = FakeCapturer::new(&[
            (2, &live_safe_capture("git push --force origin main")),
            (3, &live_safe_capture("cargo build")),
        ]);
        let mut dispatcher = RecordingDispatcher::default();
        // Complete on the second poll so we can observe one full sweep.
        let status = ScriptedStatus::new(vec![
            vec![row("feat-a", "working"), row("feat-b", "working")],
            vec![row("supervisor", "done")],
        ]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        // Both prompts escalated under the one verdict, neither was answered
        // — the command text (`git push --force` vs `cargo build`) makes no
        // difference to the outcome.
        let by_agent: Vec<(&str, &str)> = alerts
            .escalations
            .iter()
            .map(|e| (e.agent_id.as_str(), e.verdict.as_str()))
            .collect();
        assert_eq!(
            by_agent,
            vec![
                ("feat-a", PERMISSION_PROMPT_VERDICT),
                ("feat-b", PERMISSION_PROMPT_VERDICT),
            ],
            "both panes escalate, under one verdict"
        );
        assert!(
            dispatcher.events.is_empty(),
            "neither prompting pane may receive a keystroke, got {:?}",
            dispatcher.events
        );
        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        // A friction learning was recorded opportunistically + at wind-down.
        assert!(!learnings.records.is_empty(), "friction learning recorded");
    }

    // --- heartbeat exit (task 6.2) -----------------------------------------

    #[test]
    fn loop_exits_on_heartbeat_when_never_completing() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        // Agent iterates (no live prompt) forever — never completes.
        let capturer = FakeCapturer::new(&[(2, "working on it...\n$ ")]);
        let mut dispatcher = RecordingDispatcher::default();
        // Always "working": completion never fires.
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "working")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_secs(5),
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        assert_eq!(summary.outcome, DriveOutcome::Heartbeat);
        // No keystrokes and no escalation for a plainly-iterating agent
        // (feedback-cycle tolerance, task 5.4).
        assert!(dispatcher.events.is_empty());
        assert!(alerts.escalations.is_empty());
    }

    // === `unattended-heartbeat-progress`: the heartbeat counts from progress ===

    /// What one fake-clock heartbeat scenario observed.
    struct HeartbeatRun {
        summary: DriveSummary,
        /// Fake-clock time from the loop's start to its exit.
        elapsed: Duration,
        escalations: usize,
        learnings: usize,
        rebases: usize,
    }

    /// Runs a single coding agent (`feat-a` on branch `feat/a`, pane 2) under
    /// a fake clock with a 1s poll and a 5s heartbeat, so tick `k` happens at
    /// roughly `k` seconds. `configure` adjusts the config per test.
    fn run_heartbeat_scenario(
        capture: &str,
        statuses: Vec<Vec<AgentStatusRow>>,
        merge_check: &FakeMergeCheck,
        message_batches: Vec<Vec<BrokerMessage>>,
        configure: impl FnOnce(&mut DriveConfig),
    ) -> HeartbeatRun {
        let agents = one_agent_at("/repo-feat-a");
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(2, capture)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(statuses);
        let messages = ScriptedMessages::new(message_batches);
        let clock = FakeClock::new();
        let start = clock.now();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check,
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let mut config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_secs(5),
            ..DriveConfig::default()
        };
        configure(&mut config);
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        HeartbeatRun {
            summary,
            elapsed: clock.now().duration_since(start),
            escalations: alerts.escalations.len(),
            learnings: learnings.records.len(),
            rebases: refresher.rebase_calls.len(),
        }
    }

    /// Spec scenario "A progressing wave is not ended by the heartbeat": the
    /// branch tip moves at tick 3, so the 5s heartbeat — which would fire at
    /// tick 5 counted from the start — fires only one full quiet interval
    /// after that move, at tick 8.
    #[test]
    fn branch_tip_movement_defers_the_heartbeat() {
        let merge_check = FakeMergeCheck::default().with_tips("feat/a", &["a1", "a1", "a1", "a2"]);
        let run = run_heartbeat_scenario(
            "working on it...\n$ ",
            vec![vec![row("feat-a", "working")]],
            &merge_check,
            Vec::new(),
            |_| {},
        );
        assert_eq!(run.summary.outcome, DriveOutcome::Heartbeat);
        assert!(
            run.elapsed >= Duration::from_secs(8),
            "the tip move at ~3s must defer the heartbeat to ~8s, exited at {:?}",
            run.elapsed
        );
        assert!(
            run.elapsed < Duration::from_secs(9),
            "and it fires one quiet interval later, not never: {:?}",
            run.elapsed
        );
    }

    /// Control for the test above: with the tip never moving, the same loop
    /// exits on the heartbeat one interval after its start.
    #[test]
    fn an_unmoving_tip_lets_the_heartbeat_fire_from_the_start() {
        let merge_check = FakeMergeCheck::default().with_tips("feat/a", &["a1"]);
        let run = run_heartbeat_scenario(
            "working on it...\n$ ",
            vec![vec![row("feat-a", "working")]],
            &merge_check,
            Vec::new(),
            |_| {},
        );
        assert_eq!(run.summary.outcome, DriveOutcome::Heartbeat);
        assert!(
            run.elapsed < Duration::from_secs(6),
            "no progress, so the heartbeat fires at ~5s: {:?}",
            run.elapsed
        );
    }

    /// A coding agent's phase advancing (`working` → `committed`) is progress
    /// too: it moves the heartbeat out by one interval from the advance.
    #[test]
    fn a_phase_advance_defers_the_heartbeat() {
        let merge_check = FakeMergeCheck::default().with_tips("feat/a", &["a1"]);
        let run = run_heartbeat_scenario(
            "working on it...\n$ ",
            vec![
                vec![row("feat-a", "working")],
                vec![row("feat-a", "working")],
                vec![row("feat-a", "committed")],
            ],
            &merge_check,
            Vec::new(),
            |_| {},
        );
        assert_eq!(run.summary.outcome, DriveOutcome::Heartbeat);
        assert!(
            run.elapsed >= Duration::from_secs(7),
            "the advance at ~2s defers the heartbeat to ~7s: {:?}",
            run.elapsed
        );
    }

    /// Spec scenario "Only a tip move or a phase advance resets the
    /// interval", as a pure table over the two decisions the loop feeds.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn progress_kinds_reset_the_heartbeat() {
        let tips = |pairs: &[(&str, &str)]| -> HashMap<String, String> {
            pairs
                .iter()
                .map(|(b, t)| ((*b).to_string(), (*t).to_string()))
                .collect()
        };
        let coding = vec!["feat-a".to_string()];
        let working = HashMap::from([("feat-a".to_string(), WorkerPhase::Working)]);

        // (case, previous tips, this tick's tips, prior phases, this tick's rows, expected)
        #[allow(clippy::type_complexity)]
        let cases: Vec<(
            &str,
            HashMap<String, String>,
            HashMap<String, String>,
            HashMap<String, WorkerPhase>,
            Vec<AgentStatusRow>,
            bool,
        )> = vec![
            (
                "a branch tip move",
                tips(&[("feat/a", "a1")]),
                tips(&[("feat/a", "a2")]),
                working.clone(),
                vec![row("feat-a", "working")],
                true,
            ),
            (
                "a phase advance",
                tips(&[("feat/a", "a1")]),
                tips(&[("feat/a", "a1")]),
                working.clone(),
                vec![row("feat-a", "committed")],
                true,
            ),
            (
                "changed pane output only (no tip move, no phase change)",
                tips(&[("feat/a", "a1")]),
                tips(&[("feat/a", "a1")]),
                working.clone(),
                vec![row("feat-a", "working")],
                false,
            ),
            (
                "a repeated working status",
                tips(&[("feat/a", "a1")]),
                tips(&[("feat/a", "a1")]),
                working.clone(),
                vec![row("feat-a", "working"), row("feat-a", "working")],
                false,
            ),
            (
                "a re-escalation of an unanswered prompt (not an input at all)",
                tips(&[("feat/a", "a1")]),
                tips(&[("feat/a", "a1")]),
                working.clone(),
                vec![row("feat-a", "working")],
                false,
            ),
            (
                "a learning the loop recorded (not an input at all)",
                tips(&[("feat/a", "a1")]),
                tips(&[("feat/a", "a1")]),
                working.clone(),
                Vec::new(),
                false,
            ),
            (
                "a branch's first tip read",
                HashMap::new(),
                tips(&[("feat/a", "a1")]),
                working.clone(),
                vec![row("feat-a", "working")],
                false,
            ),
            (
                "a failed tip read",
                tips(&[("feat/a", "a1")]),
                HashMap::new(),
                working.clone(),
                vec![row("feat-a", "working")],
                false,
            ),
            (
                "an agent's first observed status, even a late phase",
                tips(&[("feat/a", "a1")]),
                tips(&[("feat/a", "a1")]),
                HashMap::new(),
                vec![row("feat-a", "committed")],
                false,
            ),
            (
                "the supervisor's own status rising (not a coding agent)",
                tips(&[("feat/a", "a1")]),
                tips(&[("feat/a", "a1")]),
                HashMap::from([
                    ("feat-a".to_string(), WorkerPhase::Working),
                    ("supervisor".to_string(), WorkerPhase::Working),
                ]),
                vec![row("supervisor", "done")],
                false,
            ),
        ];

        for (case, prev_tips, now_tips, mut phases, rows, expected) in cases {
            let advanced = advance_resolved_phases(&mut phases, &rows, &coding);
            assert_eq!(
                tick_made_progress(&prev_tips, &now_tips, advanced),
                expected,
                "{case}"
            );
        }
    }

    /// The stuck-detection helper's synthetic statuses carry `working` with a
    /// free-form `phase` (`stuck-on-prompt`, `no-progress`, ...); they report
    /// a wave that is NOT moving, so they never advance the resolved phase.
    #[test]
    fn stuck_detection_statuses_are_not_a_phase_advance() {
        let coding = vec!["feat-a".to_string()];
        let mut phases = HashMap::from([("feat-a".to_string(), WorkerPhase::Working)]);
        // `row` builds the status only; the synthetic `phase` field is not
        // part of the loop's phase model, which derives from `status` alone.
        let synthetic = row("feat-a", "working");
        assert!(!advance_resolved_phases(&mut phases, &[synthetic], &coding));
        assert_eq!(phases["feat-a"], WorkerPhase::Working);
    }

    /// `heartbeat_due` is due exactly once a full interval has passed since
    /// the last progress, and never for an instant before it.
    #[test]
    fn heartbeat_due_is_a_pure_interval_check() {
        let t0 = Instant::now();
        let interval = Duration::from_secs(5);
        assert!(!heartbeat_due(t0, t0, interval));
        assert!(!heartbeat_due(
            t0,
            t0 + Duration::from_millis(4_999),
            interval
        ));
        assert!(heartbeat_due(t0, t0 + interval, interval));
        assert!(heartbeat_due(t0, t0 + Duration::from_secs(60), interval));
        // A `now` before the last progress is no time elapsed, not a panic.
        assert!(!heartbeat_due(t0 + interval, t0, interval));
    }

    /// Spec scenario "Re-escalating an unanswered prompt does not defer the
    /// heartbeat": the loop re-escalates the same live prompt every dedup
    /// window and records a learning for it, and still exits on the heartbeat
    /// one interval after its start.
    #[test]
    fn re_escalations_do_not_defer_the_heartbeat() {
        let merge_check = FakeMergeCheck::default().with_tips("feat/a", &["a1"]);
        let run = run_heartbeat_scenario(
            &live_safe_capture("ls -la"),
            vec![vec![row("feat-a", "working")]],
            &merge_check,
            Vec::new(),
            |config| {
                config.dedup_window = Duration::from_secs(2);
                config.learnings_enabled = true;
            },
        );
        assert_eq!(run.summary.outcome, DriveOutcome::Heartbeat);
        assert!(
            run.escalations >= 2,
            "the unanswered prompt is re-escalated each dedup window, got {}",
            run.escalations
        );
        assert!(run.learnings >= 1, "and its learnings are recorded");
        assert!(
            run.elapsed < Duration::from_secs(6),
            "yet the heartbeat still fires ~5s after start: {:?}",
            run.elapsed
        );
    }

    /// A merge check whose tip query always errors never counts as progress
    /// and never stops the loop: it exits only on the heartbeat, on time.
    #[test]
    fn a_failing_tip_read_is_no_observation() {
        let merge_check = FakeMergeCheck::default();
        let run = run_heartbeat_scenario(
            "working on it...\n$ ",
            vec![vec![row("feat-a", "working")]],
            &merge_check,
            Vec::new(),
            |_| {},
        );
        assert!(merge_check.tip_reads.get() > 0, "the tip was queried");
        assert_eq!(run.summary.outcome, DriveOutcome::Heartbeat);
        assert!(run.elapsed < Duration::from_secs(6), "{:?}", run.elapsed);
    }

    /// A long tick can miss a completion published while it worked, because
    /// the tick's status was fetched at its top. Before exiting on the
    /// heartbeat the loop takes one fresh look; here the agent's `done` shows
    /// up only in that fresh fetch, and the run ends `completed`, not on the
    /// heartbeat.
    #[test]
    fn a_heartbeat_takes_a_fresh_look_before_exiting() {
        let working = vec![row("feat-a", "working")];
        let mut statuses = vec![working; 6];
        statuses.push(vec![row("feat-a", "done")]);
        let run = run_heartbeat_scenario(
            "working on it...\n$ ",
            statuses,
            &FakeMergeCheck::default(),
            Vec::new(),
            |_| {},
        );
        assert_eq!(
            run.summary.outcome,
            DriveOutcome::Completed,
            "a completion visible at heartbeat time must not be cut off"
        );
    }

    /// The fresh look's deferral is capped: a completion it sees that never
    /// reaches a tick's own fetch (a flapping status) defers the heartbeat
    /// exit once, not on every tick. The agent's first `done` shows up only in
    /// the fresh look at tick 5 (a phase advance — progress); from then on
    /// every tick's fetch reads `working` while every fresh look reads `done`
    /// (no advance — already resolved). The heartbeat falls due again at
    /// tick 10, is deferred once, and exits at tick 11 rather than running
    /// until the flapping stops.
    #[test]
    fn a_fresh_look_defers_the_heartbeat_at_most_once_without_progress() {
        let working = || vec![row("feat-a", "working")];
        let done = || vec![row("feat-a", "done")];
        let mut statuses = vec![working(); 6];
        statuses.push(done());
        statuses.extend(std::iter::repeat_with(working).take(5));
        for _ in 0..20 {
            statuses.push(done());
            statuses.push(working());
        }
        let run = run_heartbeat_scenario(
            "working on it...\n$ ",
            statuses,
            &FakeMergeCheck::default(),
            Vec::new(),
            |_| {},
        );
        assert_eq!(run.summary.outcome, DriveOutcome::Heartbeat);
        assert!(
            run.elapsed < Duration::from_secs(12),
            "a flapping completion must defer the exit only once: {:?}",
            run.elapsed
        );
    }

    /// A tip the loop moved itself — its own post-merge branch refresh — is
    /// re-baselined, not counted: the heartbeat still fires ~5s after start
    /// even though the refreshed branch's tip changed at tick 1.
    #[test]
    fn a_tip_moved_by_the_loops_own_branch_refresh_is_not_progress() {
        let merge_check = FakeMergeCheck::default().with_tips("feat/a", &["a1", "a2"]);
        let run = run_heartbeat_scenario(
            IDLE_PANE,
            vec![vec![row_with_modified_files("feat-a", "working", &[])]],
            &merge_check,
            vec![Vec::new(), vec![advanced_main("main")]],
            |config| config.branch_refresh_enabled = true,
        );
        assert_eq!(run.rebases, 1, "the refresh rebased the branch");
        assert_eq!(run.summary.outcome, DriveOutcome::Heartbeat);
        assert!(
            run.elapsed < Duration::from_secs(6),
            "the refresh's tip move must not defer the heartbeat: {:?}",
            run.elapsed
        );
    }

    /// Spec scenario "A prompt nobody answers does not end the run"
    /// (`supervisor-unattended-operation`): with nothing approving prompts,
    /// a pane can now sit on one indefinitely. The loop must keep driving
    /// the rest of the wave and exit only on its normal conditions — an
    /// unanswered prompt is not itself an exit condition.
    ///
    /// Distinct from `loop_exits_on_heartbeat_when_never_completing`, whose
    /// pane shows no prompt at all: this one holds a LIVE prompt for every
    /// sweep, which is the state the retirement makes common.
    #[test]
    fn a_prompt_nobody_answers_does_not_end_the_run() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        // The prompt never clears, and the agent never leaves `working`.
        let capturer = FakeCapturer::new(&[(2, &live_safe_capture("ls -la"))]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "working")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_secs(30),
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        // It ran to the HEARTBEAT — the only reason it stopped — rather than
        // bailing out the moment a prompt went unanswered.
        assert_eq!(
            summary.outcome,
            DriveOutcome::Heartbeat,
            "an unanswered prompt must not be an exit condition of its own"
        );
        // The fake clock advances only by the poll interval, so reaching a
        // 30s heartbeat means the loop really kept sweeping.
        assert!(
            !alerts.escalations.is_empty(),
            "the prompt was escalated at least once"
        );
        assert!(
            dispatcher.events.is_empty(),
            "and never answered: {:?}",
            dispatcher.events
        );
    }

    /// Spec scenario "A prompted boot publish is escalated, not answered"
    /// (`cli-resolution`): an agent's very first action is a `broker.sh`
    /// status publish. When the CLI's permission mode prompts for it, that
    /// prompt is escalated like any other — git-paw does not clear its own
    /// helper's prompt to get the agent registered.
    #[test]
    fn a_prompted_boot_publish_is_escalated_not_answered() {
        let capture = live_safe_capture(".git-paw/scripts/broker.sh --agent feat-a status booting");
        let probe = sweep_probe(&capture, "feat-a", 2);
        assert!(
            probe.outcome.is_some(),
            "a prompted boot publish must escalate, got {:?}",
            probe.outcome
        );
        let e = probe.escalation();
        assert_eq!(e.verdict, PERMISSION_PROMPT_VERDICT);
        assert!(
            e.command.contains("broker.sh"),
            "the escalation names the helper invocation: {}",
            e.command
        );
        assert!(
            probe.keys.is_empty(),
            "git-paw does not clear its own helper's prompt: {:?}",
            probe.keys
        );
    }

    // --- pane-keyed sweep: a pane with no broker record is still swept -------

    #[test]
    fn sweeps_pane_with_no_broker_record() {
        // feat-a has NOT published any status (empty status snapshot), but its
        // pane still shows a live safe prompt — it must be swept and approved.
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(2, &live_safe_capture("cargo test"))]);
        let mut dispatcher = RecordingDispatcher::default();
        // First poll: empty roster (no broker record). Second: supervisor done.
        let status = ScriptedStatus::new(vec![vec![], vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        assert!(
            alerts
                .escalations
                .iter()
                .any(|e| e.verdict == PERMISSION_PROMPT_VERDICT),
            "a pane with no broker record was still swept and escalated: {:?}",
            alerts.escalations
        );
    }

    // --- scrollback prompt is ignored (task 3.3 / D4) ----------------------

    #[test]
    fn scrollback_prompt_is_not_acted_on() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        // A resolved prompt scrolled into history: the footer is followed by
        // several lines of ordinary output, so it is NOT live.
        let scrollback =
            "Do you want to proceed?\n(esc to cancel)\nran it\nline b\nline c\nline d\n$ ";
        let capturer = FakeCapturer::new(&[(2, scrollback)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        assert!(
            dispatcher.events.is_empty(),
            "a scrollback prompt must not trigger keystrokes"
        );
    }

    // --- correction loop (`supervisor-correction-loop`) ---------------------

    /// Only a supervisor-published, gate-tagged `agent.feedback` is a gate
    /// failure. Conflict-detector warnings and peer feedback share the channel
    /// and must never start a correction cycle.
    #[test]
    fn gate_failure_recognises_only_tagged_supervisor_feedback() {
        let gate_tags = CorrectionConfig::default().gate_tags;

        let gate = FeedbackPayload {
            from: SUPERVISOR_AGENT_ID.to_string(),
            errors: vec!["[regression] 2 suites fail on main".to_string()],
        };
        assert_eq!(
            gate_failure(&gate, &gate_tags),
            Some("regression".to_string())
        );

        let conflict = FeedbackPayload {
            from: SUPERVISOR_AGENT_ID.to_string(),
            errors: vec!["[conflict-detector] feat-b also claims src/foo.rs".to_string()],
        };
        assert_eq!(
            gate_failure(&conflict, &gate_tags),
            None,
            "a conflict warning is not a gate verdict"
        );

        let from_peer = FeedbackPayload {
            from: "feat-b".to_string(),
            errors: vec!["[testing] your change broke my build".to_string()],
        };
        assert_eq!(
            gate_failure(&from_peer, &gate_tags),
            None,
            "only the supervisor publishes gate verdicts"
        );

        let untagged = FeedbackPayload {
            from: SUPERVISOR_AGENT_ID.to_string(),
            errors: vec!["please rebase onto main".to_string()],
        };
        assert_eq!(gate_failure(&untagged, &gate_tags), None);
    }

    /// Task 3.6/3.7: a configured custom gate vocabulary recognises its own
    /// tags and starts a correction cycle, while a tag from the *default*
    /// vocabulary that is no longer configured (and the conflict detector's
    /// `[conflict-detector]` tag) stay non-gate producers.
    #[test]
    fn custom_gate_vocabulary_recognises_configured_tags_only() {
        let gate_tags = vec!["lint".to_string(), "perf-budget".to_string()];

        let custom_gate = FeedbackPayload {
            from: SUPERVISOR_AGENT_ID.to_string(),
            errors: vec!["[perf-budget] request latency regressed".to_string()],
        };
        assert_eq!(
            gate_failure(&custom_gate, &gate_tags),
            Some("perf-budget".to_string()),
            "a configured custom gate name must be recognised"
        );

        let stale_default_tag = FeedbackPayload {
            from: SUPERVISOR_AGENT_ID.to_string(),
            errors: vec!["[testing] 2 suites fail on main".to_string()],
        };
        assert_eq!(
            gate_failure(&stale_default_tag, &gate_tags),
            None,
            "a tag not in the configured vocabulary must not start a correction cycle"
        );

        let conflict = FeedbackPayload {
            from: SUPERVISOR_AGENT_ID.to_string(),
            errors: vec!["[conflict-detector] feat-b also claims src/foo.rs".to_string()],
        };
        assert_eq!(
            gate_failure(&conflict, &gate_tags),
            None,
            "the conflict detector's tag is never a gate verdict, even with a custom vocabulary"
        );
    }

    /// Task 2.4: repeated failures accumulate cycles, and a terminal PASS
    /// clears the branch so a later unrelated failure starts from a fresh
    /// budget (spec scenario "Passing the gate exits the cycle early").
    #[test]
    fn correction_state_counts_repeat_failures_and_clears_on_pass() {
        let mut state = CorrectionState::new();
        state.mark_gate_failure("feat-a", "[testing] fail".to_string());
        assert_eq!(state.record_reengagement("feat-a"), 1);
        state.mark_gate_failure("feat-a", "[testing] still failing".to_string());
        assert_eq!(state.record_reengagement("feat-a"), 2);
        assert_eq!(state.cycles("feat-a"), 2);

        // A terminal PASS clears the counter…
        state.clear_completed(&[row("feat-a", "verified")]);
        assert_eq!(state.cycles("feat-a"), 0);
        // …and a `committed` status does NOT (a commit is not a verdict).
        state.mark_gate_failure("feat-a", "[testing] fail".to_string());
        state.record_reengagement("feat-a");
        state.clear_completed(&[row("feat-a", "committed")]);
        assert_eq!(state.cycles("feat-a"), 1, "committed is not a PASS verdict");
    }

    /// Builds a single-coding-agent drive-loop fixture whose pane is idle (no
    /// live prompt), so the correction pass is the only thing that can type.
    fn correction_agents() -> Vec<AgentPane> {
        vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }]
    }

    fn correction_panes() -> FakeEnumerator {
        FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                owner: "/repo-feat-a".to_string(),
            }],
        }
    }

    /// Spec scenario "Gate failure re-engages the worker pane": with
    /// `auto_loopback = true` the resolved pane receives the gate-tagged
    /// feedback as free text plus a SEPARATE submitting Enter.
    #[test]
    fn auto_loopback_reengages_the_gate_failed_worker_pane() {
        let agents = correction_agents();
        let enumerator = correction_panes();
        let capturer = FakeCapturer::new(&[(2, "standing by...\n$ ")]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![
            vec![row("feat-a", "working")],
            vec![row("supervisor", "done")],
        ]);
        let messages = ScriptedMessages::new(vec![vec![gate_feedback(
            "feat-a",
            "testing",
            "3 unit tests fail",
        )]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            correction: CorrectionConfig {
                auto_loopback: true,
                ..CorrectionConfig::default()
            },
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        assert_eq!(dispatcher.events.len(), 2, "one text keystroke + one Enter");
        let (pane, text) = &dispatcher.events[0];
        assert_eq!(*pane, 2, "sent to the resolved pane");
        assert!(
            text.contains("testing") && text.contains("3 unit tests fail"),
            "the pane receives the gate-tagged feedback, got: {text}"
        );
        assert!(
            !text.contains('\n'),
            "the text keystroke carries no newline; Enter is separate"
        );
        assert_eq!(dispatcher.events[1], (2, "Enter".to_string()));
        // The feedback is free text git-paw did not choose word-by-word, so it
        // goes out LITERALLY (`send-keys -l`); the submitting `Enter` does NOT,
        // because it must resolve through tmux's key table.
        assert_eq!(
            dispatcher.literal_sends,
            vec![(2, text.clone())],
            "the feedback is the only literal send; Enter stays a key name"
        );
    }

    /// A correction-loop escalation must NOT be framed as a stalled permission
    /// prompt: there is no prompt to review, and the misleading wording would
    /// send the human looking for one. Classifier verdicts keep the prompt
    /// framing, which is accurate for them.
    #[test]
    fn escalation_question_framing_follows_the_verdict() {
        let exhausted = Escalation {
            agent_id: "feat-a".to_string(),
            verdict: CORRECTION_EXHAUSTED_VERDICT.to_string(),
            command: "branch is unrecoverable after 5 correction cycle(s); needs a human"
                .to_string(),
        };
        let text = exhausted.question();
        assert!(
            !text.contains("permission prompt"),
            "an exhausted correction budget is not a permission prompt, got: {text}"
        );
        assert!(
            text.contains("feat-a") && text.contains("correction budget"),
            "it names the branch and why it was raised, got: {text}"
        );

        let slow = Escalation {
            agent_id: "feat-a".to_string(),
            verdict: CORRECTION_SLOW_VERDICT.to_string(),
            command: "correction cycle 2 of 5 — worker is converging slowly".to_string(),
        };
        let text = slow.question();
        assert!(
            !text.contains("permission prompt"),
            "the early heads-up is not a permission prompt either, got: {text}"
        );
        assert!(
            text.contains("heads-up"),
            "it reads as advisory, not as work to do, got: {text}"
        );

        // A permission-prompt verdict keeps the prompt framing, and says
        // plainly that git-paw will not answer it.
        let prompt = Escalation {
            agent_id: "feat-a".to_string(),
            verdict: PERMISSION_PROMPT_VERDICT.to_string(),
            command: "git push --force origin main".to_string(),
        };
        let text = prompt.question();
        assert!(
            text.contains("permission prompt")
                && text.contains("does not approve prompts")
                && text.contains("git push --force origin main"),
            "a permission prompt asks the human to decide, got: {text}"
        );
    }

    /// Safety finding: `handoff_text` is TYPED into the orchestrator's own
    /// pane, which the loop re-scans on the very next sweep. A `cli-unavailable`
    /// escalation's `command` is the matched banner LINE VERBATIM
    /// (`sweep_pane`); quoting it in the hand-off would plant a live copy of
    /// the marker in a scanned pane, and the bounded-trailing-region scan
    /// (`cli-unavailable-detection-gaps`) is wide enough to see its own echo
    /// on the following sweep. `handoff_text` must paraphrase instead of
    /// quoting for this verdict, for EVERY marker in the embedded profile —
    /// not just the one fixture this change happened to use.
    #[test]
    fn cli_unavailable_handoff_text_never_quotes_the_matched_banner() {
        for marker in &claude_prompt_profile().cli_unavailable_markers {
            let escalation = Escalation {
                agent_id: "feat-a".to_string(),
                verdict: "cli-unavailable".to_string(),
                command: format!("{marker} (reset in 2h14m)"),
            };
            let text = escalation.handoff_text();
            assert!(
                text.contains("feat-a") && text.contains("cli-unavailable"),
                "still identifies the agent and verdict: {text}"
            );
            for m in &claude_prompt_profile().cli_unavailable_markers {
                assert!(
                    !text.to_ascii_lowercase().contains(&m.to_ascii_lowercase()),
                    "hand-off for marker {marker:?} must not quote any cli_unavailable \
                     marker verbatim (would self-trigger on the next sweep): {text}"
                );
            }
        }
    }

    /// Defence-in-depth: the re-engagement text always travels the literal
    /// (`send-keys -l`) path, so a payload that happens to be a single token
    /// matching a tmux key name is TYPED, never resolved as a key. Without
    /// `-l`, a bare `C-c` would fire an interrupt in the worker's pane.
    #[test]
    fn reengagement_text_is_never_interpreted_as_a_tmux_key_name() {
        let mut dispatcher = RecordingDispatcher::default();
        let capturer = FakeCapturer::new(&[(2, "standing by...\n$ ")]);
        let enumerator = FakeEnumerator { panes: vec![] };
        let status = ScriptedStatus::new(vec![]);
        let messages = ScriptedMessages::none();
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let repo = tempfile::tempdir().expect("temp repo root");
        let outcome = send_reengagement(
            &mut deps,
            repo.path(),
            "paw-test",
            2,
            "feat-a",
            "C-c",
            Duration::ZERO,
        )
        .expect("dispatch succeeds");
        assert_eq!(outcome, NudgeOutcome::Delivered);
        assert_eq!(
            dispatcher.literal_sends,
            vec![(2, "C-c".to_string())],
            "a key-name-shaped payload is sent literally, not as a key"
        );
        assert_eq!(
            dispatcher.events,
            vec![(2, "C-c".to_string()), (2, "Enter".to_string())],
            "text then a SEPARATE Enter (D6)"
        );
    }

    // --- nudge settle delay + verify-then-recover (GP-15) -------------------

    /// Runs [`send_reengagement`] against pane 2 with `capturer` and
    /// `settle_delay`, returning the outcome, the recorded dispatcher, the
    /// recorded alerts, and how much simulated clock time elapsed.
    fn call_send_reengagement(
        capturer: &dyn PaneCapture,
        text: &str,
        settle_delay: Duration,
    ) -> (NudgeOutcome, RecordingDispatcher, RecordingAlerts, Duration) {
        let mut dispatcher = RecordingDispatcher::default();
        let enumerator = FakeEnumerator { panes: vec![] };
        let status = ScriptedStatus::new(vec![]);
        let messages = ScriptedMessages::none();
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let before = clock.now();
        let repo = tempfile::tempdir().expect("temp repo root");
        let outcome = {
            let mut deps = DriveDeps {
                enumerator: &enumerator,
                capturer,
                dispatcher: &mut dispatcher,
                status: &status,
                messages: &messages,
                clock: &clock,
                merge_check: &FakeMergeCheck::default(),
                alerts: &mut alerts,
                learnings: &mut learnings,
                manual_decisions: &mut manual_decisions,
                refresher: &mut refresher,
                wedged: Vec::new(),
            };
            send_reengagement(
                &mut deps,
                repo.path(),
                "paw-test",
                2,
                "feat-a",
                text,
                settle_delay,
            )
            .expect("dispatch succeeds")
        };
        let elapsed = clock.now().duration_since(before);
        (outcome, dispatcher, alerts, elapsed)
    }

    /// Spec scenario "Nudge applies the per-CLI settle delay between text and
    /// Enter": the loop sleeps the settle delay between sending the text and
    /// the submitting `Enter`, never relying on a single combined text+Enter.
    #[test]
    fn nudge_waits_the_settle_delay_before_enter() {
        let capturer = FakeCapturer::new(&[(2, "standing by...\n$ ")]);
        let settle_delay = Duration::from_millis(750);
        let (outcome, dispatcher, _alerts, elapsed) =
            call_send_reengagement(&capturer, "please continue", settle_delay);
        assert_eq!(outcome, NudgeOutcome::Delivered);
        assert!(
            elapsed >= settle_delay,
            "expected at least the settle delay ({settle_delay:?}) to elapse \
             between the text send and Enter, got {elapsed:?}"
        );
        assert_eq!(
            dispatcher.events,
            vec![(2, "please continue".to_string()), (2, "Enter".to_string())],
            "text then a separate Enter, with the settle delay slept in between"
        );
    }

    /// Spec scenario "A stale input line is recovered with clear + re-type +
    /// Enter": a nudge whose text is still on the pane's input line after the
    /// follow-up `Enter` is recovered by clearing the line (`C-u`),
    /// re-sending the text, and sending `Enter` again — never just a second
    /// `Enter`.
    #[test]
    fn stale_input_line_triggers_clear_retype_enter_recovery() {
        let stuck = idle_input_box("please continue");
        // Ready-check sees an idle pane; the post-Enter verify sees the text
        // still stuck on the input line; the post-recovery verify sees it
        // cleared.
        let capturer = SequencedCapturer::new(&[IDLE_PANE, &stuck, IDLE_PANE]);
        let (outcome, dispatcher, alerts, _elapsed) =
            call_send_reengagement(&capturer, "please continue", Duration::ZERO);
        assert_eq!(outcome, NudgeOutcome::Delivered);
        assert_eq!(
            dispatcher.events,
            vec![
                (2, "please continue".to_string()),
                (2, "Enter".to_string()),
                (2, "C-u".to_string()),
                (2, "please continue".to_string()),
                (2, "Enter".to_string()),
            ],
            "a stale line is recovered by clearing (C-u), re-typing, then \
             Enter — never just a second Enter"
        );
        assert!(
            alerts.escalations.is_empty(),
            "recovered on the first attempt — no escalation"
        );
    }

    /// Spec: "a bare `Enter`/`C-m` alone SHALL NOT be treated as sufficient
    /// recovery" — a pane whose input line never clears, no matter how many
    /// clear+re-type+Enter rounds are sent, is reported
    /// [`NudgeOutcome::Wedged`] (never misreported as delivered) and
    /// escalated to the broker, after a bounded number of recovery attempts.
    #[test]
    fn a_persistently_stale_line_is_reported_wedged_and_escalated() {
        let stuck = idle_input_box("please continue");
        let capturer = SequencedCapturer::new(&[IDLE_PANE, &stuck, &stuck, &stuck]);
        let (outcome, dispatcher, alerts, _elapsed) =
            call_send_reengagement(&capturer, "please continue", Duration::ZERO);
        assert_eq!(outcome, NudgeOutcome::Wedged);
        let expected_len = 2 + usize::from(NUDGE_RECOVERY_ATTEMPTS) * 3;
        assert_eq!(
            dispatcher.events.len(),
            expected_len,
            "bounded recovery: one initial send + {NUDGE_RECOVERY_ATTEMPTS} \
             clear+retype+Enter rounds, events were {:?}",
            dispatcher.events
        );
        assert_eq!(alerts.escalations.len(), 1, "escalated exactly once");
        assert_eq!(alerts.escalations[0].verdict, NUDGE_WEDGED_VERDICT);
        assert_eq!(alerts.escalations[0].agent_id, "feat-a");
    }

    /// Spec scenario "A wrapped nudge still on the input line is recognised
    /// as unsubmitted": the post-`Enter` capture shows only the sigil row's
    /// prefix of a longer send, plus a continuation row with no sigil — still
    /// PRESENT, not delivered, because the full text starts with that prefix.
    #[test]
    fn wrapped_nudge_still_present_triggers_recovery_and_delivers() {
        let text = "please review the changes and continue with the next task";
        let wrapped = wrapped_input_box(
            "please review the changes and",
            "continue with the next task",
        );
        let capturer = SequencedCapturer::new(&[IDLE_PANE, &wrapped, IDLE_PANE]);
        let (outcome, dispatcher, alerts, _elapsed) =
            call_send_reengagement(&capturer, text, Duration::ZERO);
        assert_eq!(outcome, NudgeOutcome::Delivered);
        assert_eq!(
            dispatcher.events,
            vec![
                (2, text.to_string()),
                (2, "Enter".to_string()),
                (2, "C-u".to_string()),
                (2, text.to_string()),
                (2, "Enter".to_string()),
            ],
            "a wrapped rendering is recognised as still present and \
             recovered, never misreported delivered merely because the \
             sigil row differs from the full text"
        );
        assert!(alerts.escalations.is_empty());
    }

    /// Spec scenario "A paste-buffer placeholder is recognised as
    /// unsubmitted".
    #[test]
    fn placeholder_in_input_box_triggers_recovery() {
        let text = "please continue";
        let placeholder = idle_input_box("[Pasted text #1 +5 lines]");
        let capturer = SequencedCapturer::new(&[IDLE_PANE, &placeholder, IDLE_PANE]);
        let (outcome, dispatcher, _alerts, _elapsed) =
            call_send_reengagement(&capturer, text, Duration::ZERO);
        assert_eq!(outcome, NudgeOutcome::Delivered);
        assert_eq!(
            dispatcher.events,
            vec![
                (2, text.to_string()),
                (2, "Enter".to_string()),
                (2, "C-u".to_string()),
                (2, text.to_string()),
                (2, "Enter".to_string()),
            ],
            "a paste-buffer placeholder is recognised as still present and \
             recovered"
        );
    }

    /// Spec scenario "An empty input box means the nudge was submitted": an
    /// actual empty input box (a sigil row with no text), not merely the
    /// absence of a box, is delivered with no extra keystrokes.
    #[test]
    fn empty_input_box_delivers_with_no_extra_keys() {
        let capturer = FakeCapturer::new(&[(2, &idle_input_box(""))]);
        let (outcome, dispatcher, _alerts, _elapsed) =
            call_send_reengagement(&capturer, "please continue", Duration::ZERO);
        assert_eq!(outcome, NudgeOutcome::Delivered);
        assert_eq!(
            dispatcher.events,
            vec![(2, "please continue".to_string()), (2, "Enter".to_string())],
            "an already-empty box needs no recovery keystrokes"
        );
    }

    /// Spec scenario "Verification waits for the pane to redraw": the verify
    /// capture is taken only after sleeping `settle_delay` again following
    /// the `Enter`, so the injected clock records two sleeps per attempt —
    /// one for the text→Enter wait, one for the Enter→verify wait.
    #[test]
    fn verify_sleeps_the_settle_delay_after_enter_too() {
        let capturer = FakeCapturer::new(&[(2, "standing by...\n$ ")]);
        let settle_delay = Duration::from_millis(200);
        let (outcome, _dispatcher, _alerts, elapsed) =
            call_send_reengagement(&capturer, "please continue", settle_delay);
        assert_eq!(outcome, NudgeOutcome::Delivered);
        assert_eq!(
            elapsed,
            (settle_delay + Duration::from_millis(1)) * 2,
            "one settle sleep before Enter and one more before the verify capture"
        );
    }

    /// Spec scenario "Disabled loopback preserves today's behavior": with
    /// `auto_loopback` off (the default, and every config with no
    /// `[supervisor.correction]` table) the same gate failure sends nothing.
    #[test]
    fn disabled_loopback_sends_no_reengagement_keystrokes() {
        let agents = correction_agents();
        let enumerator = correction_panes();
        let capturer = FakeCapturer::new(&[(2, "standing by...\n$ ")]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![
            vec![row("feat-a", "working")],
            vec![row("supervisor", "done")],
        ]);
        let messages = ScriptedMessages::new(vec![vec![gate_feedback(
            "feat-a",
            "testing",
            "3 unit tests fail",
        )]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        // `DriveConfig::default()` carries `CorrectionConfig::default()`, i.e.
        // `auto_loopback = false` — the absent-table resolution.
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        assert!(
            dispatcher.events.is_empty(),
            "no re-engagement keystrokes when auto_loopback is off"
        );
        assert!(summary.exhausted.is_empty(), "no correction cycle ran");
    }

    /// Task 3.2 (TOCTOU): the fresh re-confirm capture taken immediately
    /// before the send shows a live permission prompt — the worker is
    /// mid-action, not blocked awaiting correction — so nothing is typed. The
    /// correction stays pending and lands on the NEXT sweep, so the failure is
    /// deferred, never dropped.
    #[test]
    fn reengagement_defers_when_the_pane_is_no_longer_idle() {
        let agents = correction_agents();
        let enumerator = correction_panes();
        let idle = "standing by...\n$ ";
        let live = "Bash command\n  frobnicate\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n(esc to cancel)";
        // sweep1 (idle) → re-confirm1 (live, blocks) → sweep2 (idle) →
        // re-confirm2 (idle, sends). The last capture repeats thereafter.
        let capturer = SequencedCapturer::new(&[idle, live, idle, idle]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![
            vec![row("feat-a", "working")],
            vec![row("feat-a", "working")],
            vec![row("supervisor", "done")],
        ]);
        let messages = ScriptedMessages::new(vec![vec![gate_feedback(
            "feat-a",
            "doc audit",
            "mdbook build fails",
        )]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            correction: CorrectionConfig {
                auto_loopback: true,
                ..CorrectionConfig::default()
            },
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        assert_eq!(
            dispatcher.events.len(),
            2,
            "exactly one re-engagement, delivered on the sweep after the guard fired"
        );
        assert!(dispatcher.events[0].1.contains("doc audit"));
    }

    /// Builds the fixture for the bounded-cycle tests: a branch that fails its
    /// gate on every one of `rounds` sweeps and never completes, so the loop
    /// exits on its heartbeat.
    fn run_always_failing(
        rounds: usize,
        correction: CorrectionConfig,
        learnings_enabled: bool,
        status_rows: Vec<AgentStatusRow>,
    ) -> (
        DriveSummary,
        RecordingDispatcher,
        RecordingAlerts,
        RecordingLearnings,
    ) {
        let agents = correction_agents();
        let enumerator = correction_panes();
        let capturer = FakeCapturer::new(&[(2, "standing by...\n$ ")]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![status_rows]);
        let messages = ScriptedMessages::new(
            (0..rounds)
                .map(|_| vec![gate_feedback("feat-a", "testing", "still failing")])
                .collect(),
        );
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let summary = {
            let mut deps = DriveDeps {
                enumerator: &enumerator,
                capturer: &capturer,
                dispatcher: &mut dispatcher,
                status: &status,
                messages: &messages,
                clock: &clock,
                merge_check: &FakeMergeCheck::default(),
                alerts: &mut alerts,
                learnings: &mut learnings,
                manual_decisions: &mut manual_decisions,
                refresher: &mut refresher,
                wedged: Vec::new(),
            };
            let config = DriveConfig {
                poll_interval: Duration::from_secs(1),
                // Exits after `rounds` sweeps: the fake clock advances one
                // interval (plus a tick) per sleep. `feat-a`'s empty CLI is
                // pinned to a zero settle delay so a re-engagement's send
                // costs no simulated time either, keeping the round count
                // exact.
                heartbeat: Duration::from_secs(rounds as u64 - 1),
                correction,
                learnings_enabled,
                clis: zero_settle_delay_clis(),
                ..DriveConfig::default()
            };
            drive_loop_in_tmp("paw-test", &agents, &mut deps, &config)
        };
        (summary, dispatcher, alerts, learnings)
    }

    /// Spec scenario "Cycles are bounded by `max_cycles`": a branch failing
    /// every attempt is re-engaged exactly `max_cycles` times and never again,
    /// and the exhaustion policy is applied once.
    #[test]
    fn correction_cycles_are_bounded_by_max_cycles() {
        let (summary, dispatcher, _alerts, _learnings) = run_always_failing(
            6,
            CorrectionConfig {
                auto_loopback: true,
                max_cycles: 3,
                ..CorrectionConfig::default()
            },
            false,
            vec![row("feat-a", "working")],
        );
        assert_eq!(
            dispatcher.events.len(),
            6,
            "exactly max_cycles (3) re-engagements, each a text + Enter pair"
        );
        assert_eq!(
            summary.exhausted.len(),
            1,
            "exhaustion applied exactly once"
        );
        assert_eq!(summary.exhausted[0].agent_id, "feat-a");
        assert_eq!(summary.exhausted[0].cycles, 3);
    }

    /// Spec scenario "`escalate` flags the branch".
    #[test]
    fn exhaustion_escalate_flags_the_branch() {
        let (summary, _dispatcher, alerts, _learnings) = run_always_failing(
            6,
            CorrectionConfig {
                auto_loopback: true,
                max_cycles: 3,
                on_exhausted: OnExhausted::Escalate,
                ..CorrectionConfig::default()
            },
            false,
            vec![row("feat-a", "working")],
        );
        assert_eq!(summary.exhausted[0].policy, OnExhausted::Escalate);
        let flagged: Vec<&Escalation> = alerts
            .escalations
            .iter()
            .filter(|e| e.verdict == CORRECTION_EXHAUSTED_VERDICT)
            .collect();
        assert_eq!(
            flagged.len(),
            1,
            "the branch is flagged once as unrecoverable"
        );
        assert_eq!(flagged[0].agent_id, "feat-a");
        // `escalate_after_cycles` (3) is NOT less than `max_cycles` (3), so no
        // distinct early flag fires.
        assert!(
            !alerts
                .escalations
                .iter()
                .any(|e| e.verdict == CORRECTION_SLOW_VERDICT),
            "no early flag when escalate_after_cycles >= max_cycles"
        );
    }

    /// Spec scenario "`abandon` stops correcting the branch": the branch is
    /// marked failed in the summary and NO escalation-as-recoverable is sent.
    #[test]
    fn exhaustion_abandon_marks_failed_without_escalating() {
        let (summary, dispatcher, alerts, _learnings) = run_always_failing(
            6,
            CorrectionConfig {
                auto_loopback: true,
                max_cycles: 2,
                on_exhausted: OnExhausted::Abandon,
                ..CorrectionConfig::default()
            },
            false,
            vec![row("feat-a", "working")],
        );
        assert_eq!(summary.exhausted[0].policy, OnExhausted::Abandon);
        assert!(
            alerts.escalations.is_empty(),
            "abandon sends no escalation-as-recoverable"
        );
        assert_eq!(
            dispatcher.events.len(),
            4,
            "re-engaged exactly max_cycles (2) times"
        );
        assert!(
            summary.render().contains("abandoned (marked failed)"),
            "the summary marks the branch failed"
        );
    }

    /// Spec scenario "Early flag at `escalate_after_cycles`": the heads-up
    /// fires exactly once, and re-engagement continues until `max_cycles`.
    #[test]
    fn early_flag_fires_once_and_correction_continues() {
        let (summary, dispatcher, alerts, _learnings) = run_always_failing(
            8,
            CorrectionConfig {
                auto_loopback: true,
                max_cycles: 5,
                escalate_after_cycles: 2,
                ..CorrectionConfig::default()
            },
            false,
            vec![row("feat-a", "working")],
        );
        let early: Vec<&Escalation> = alerts
            .escalations
            .iter()
            .filter(|e| e.verdict == CORRECTION_SLOW_VERDICT)
            .collect();
        assert_eq!(early.len(), 1, "the early heads-up fires exactly once");
        assert_eq!(early[0].agent_id, "feat-a");
        assert_eq!(
            dispatcher.events.len(),
            10,
            "re-engagement continues past the early flag until max_cycles (5)"
        );
        assert_eq!(summary.exhausted[0].cycles, 5);
    }

    /// Spec scenario "Learning emitted when learnings enabled": an exhausted
    /// branch produces a `correction_exhausted` record carrying the branch, the
    /// worker CLI, and the cycle count.
    #[test]
    fn exhaustion_emits_learning_when_learnings_enabled() {
        let (_summary, _dispatcher, _alerts, learnings) = run_always_failing(
            5,
            CorrectionConfig {
                auto_loopback: true,
                max_cycles: 2,
                ..CorrectionConfig::default()
            },
            true,
            vec![row_with_cli("feat-a", "working", "claude")],
        );
        let bodies = learnings.bodies(crate::broker::learnings::CATEGORY_CORRECTION_EXHAUSTED);
        assert_eq!(bodies.len(), 1, "one exhaustion learning");
        let body: serde_json::Value = serde_json::from_str(bodies[0]).expect("body is JSON");
        assert_eq!(body["branch"], "feat-a");
        assert_eq!(body["cli"], "claude");
        assert_eq!(body["cycles"], 2);
    }

    /// Spec scenario "No learning when learnings disabled": no telemetry
    /// without consent.
    #[test]
    fn exhaustion_emits_no_learning_when_learnings_disabled() {
        let (summary, _dispatcher, _alerts, learnings) = run_always_failing(
            5,
            CorrectionConfig {
                auto_loopback: true,
                max_cycles: 2,
                ..CorrectionConfig::default()
            },
            false,
            vec![row_with_cli("feat-a", "working", "claude")],
        );
        assert_eq!(summary.exhausted.len(), 1, "the branch still exhausted");
        assert!(
            learnings
                .bodies(crate::broker::learnings::CATEGORY_CORRECTION_EXHAUSTED)
                .is_empty(),
            "no correction_exhausted learning when [learnings] is disabled"
        );
    }

    /// Spec scenario "Passing the gate exits the cycle early", end to end: a
    /// branch that fails once and then passes is re-engaged once and never
    /// reaches the exhaustion policy.
    #[test]
    fn passing_the_gate_exits_the_correction_cycle() {
        let agents = correction_agents();
        let enumerator = correction_panes();
        let capturer = FakeCapturer::new(&[(2, "standing by...\n$ ")]);
        let mut dispatcher = RecordingDispatcher::default();
        // Fails on the first sweep, then reaches a terminal PASS.
        let status = ScriptedStatus::new(vec![
            vec![row("feat-a", "working")],
            vec![row("feat-a", "verified")],
        ]);
        let messages = ScriptedMessages::new(vec![vec![gate_feedback(
            "feat-a",
            "spec audit",
            "scenario without a test",
        )]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            correction: CorrectionConfig {
                auto_loopback: true,
                max_cycles: 1,
                ..CorrectionConfig::default()
            },
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        assert_eq!(dispatcher.events.len(), 2, "re-engaged once");
        assert!(
            summary.exhausted.is_empty(),
            "a branch that passes never reaches the exhaustion policy"
        );
    }

    // --- autonomous orchestrator (supervisor-autonomous-orchestrator) --------

    /// A pane sitting at its input box: no live permission prompt,
    /// not mid-turn.
    const IDLE_PANE: &str = "? for shortcuts";

    /// A pane actively producing a response. Injecting here would
    /// land a task in the middle of the model's own turn.
    const MID_RESPONSE_PANE: &str = "Boondoggling… (esc to interrupt)";

    /// Two coding agents on distinct worktrees, the roster every orchestrator
    /// test below shares.
    fn two_agents() -> Vec<AgentPane> {
        vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
                branch: "feat/a".to_string(),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
                branch: "feat/b".to_string(),
            },
        ]
    }

    fn pane(pane_index: usize, path: &str) -> PaneInfo {
        PaneInfo {
            pane_index,
            owner: path.to_string(),
        }
    }

    /// Runs the loop with fakes and hands back what it dispatched, escalated, and
    /// summarised — the boilerplate every orchestrator test shares.
    fn run_loop(
        agents: &[AgentPane],
        panes: Vec<PaneInfo>,
        pane_text: &[(usize, &str)],
        statuses: Vec<Vec<AgentStatusRow>>,
        messages: &ScriptedMessages,
        config: &DriveConfig,
    ) -> (RecordingDispatcher, RecordingAlerts) {
        let enumerator = FakeEnumerator { panes };
        let capturer = FakeCapturer::new(pane_text);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(statuses);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        drive_loop_in_tmp("paw-test", agents, &mut deps, config);
        (dispatcher, alerts)
    }

    /// Like [`run_loop`] but for `drive-loop-exit-summary-truth` scenarios:
    /// takes the [`MergeCheck`] fake directly and hands back the full
    /// [`DriveSummary`] alongside the dispatcher, so a test can assert on the
    /// ref-derived outcome/escalations AND on keystrokes in one place.
    fn run_loop_with_merge_check(
        agents: &[AgentPane],
        panes: Vec<PaneInfo>,
        pane_text: &[(usize, &str)],
        statuses: Vec<Vec<AgentStatusRow>>,
        merge_check: &dyn MergeCheck,
        config: &DriveConfig,
    ) -> (DriveSummary, RecordingDispatcher) {
        let enumerator = FakeEnumerator { panes };
        let capturer = FakeCapturer::new(pane_text);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(statuses);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check,
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let summary = drive_loop_in_tmp("paw-test", agents, &mut deps, config);
        (summary, dispatcher)
    }

    /// Spec scenario: two `done` workers, neither branch actually merged — the
    /// exact 0.16.0 dogfood shape (`drive-loop-exit-summary-truth`). The
    /// summary must read `escalated-for-review` / `0/2 branches merged` with
    /// one `[unmerged]` entry per branch, and no surface
    /// must receive no keystrokes for those entries (they are pushed directly
    /// onto `escalations`, never through `record_escalation`).
    #[test]
    fn two_done_agents_none_merged_reports_escalated_for_review() {
        let agents = two_agents();
        let merge_check = FakeMergeCheck::default()
            .with_unmerged("feat/a", 3, "aaa1111")
            .with_unmerged("feat/b", 5, "bbb2222");
        let (summary, dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![
                pane(0, "/repo"),
                pane(2, "/repo-feat-a"),
                pane(3, "/repo-feat-b"),
            ],
            &[(0, IDLE_PANE), (2, IDLE_PANE), (3, IDLE_PANE)],
            vec![vec![row("feat-a", "done"), row("feat-b", "done")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        assert_eq!(
            summary.merged_summary.as_deref(),
            Some("0/2 branches merged")
        );
        assert_eq!(summary.escalations.len(), 2, "{:?}", summary.escalations);
        for e in &summary.escalations {
            assert_eq!(e.verdict, "unmerged");
        }
        assert!(
            summary
                .escalations
                .iter()
                .any(|e| e.agent_id == "feat-a" && e.command.contains("aaa1111")),
            "{:?}",
            summary.escalations
        );
        assert!(
            summary
                .escalations
                .iter()
                .any(|e| e.agent_id == "feat-b" && e.command.contains("bbb2222")),
            "{:?}",
            summary.escalations
        );
        assert!(
            dispatcher.events.is_empty(),
            "no surface may receive keystrokes for unmerged entries: {:?}",
            dispatcher.events
        );
    }

    /// The all-merged counterpart: both branches actually merged reports
    /// `completed` / `2/2 branches merged` with no escalations.
    #[test]
    fn two_done_agents_all_merged_reports_completed() {
        let agents = two_agents();
        let merge_check = FakeMergeCheck::default();
        let (summary, _dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![
                pane(0, "/repo"),
                pane(2, "/repo-feat-a"),
                pane(3, "/repo-feat-b"),
            ],
            &[(0, IDLE_PANE), (2, IDLE_PANE), (3, IDLE_PANE)],
            vec![vec![row("feat-a", "done"), row("feat-b", "done")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert_eq!(summary.outcome, DriveOutcome::Completed);
        assert_eq!(
            summary.merged_summary.as_deref(),
            Some("2/2 branches merged")
        );
        assert!(summary.escalations.is_empty(), "{:?}", summary.escalations);
    }

    /// D2 (fail-safe): a comparison git cannot make (a deleted branch, a bad
    /// ref) counts as unmerged, with the reason surfaced on the escalation.
    #[test]
    fn comparison_error_counts_as_unmerged_with_reason() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let merge_check =
            FakeMergeCheck::default().with_error("feat/a", "fatal: not a valid object name");
        let (summary, _dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![pane(0, "/repo"), pane(2, "/repo-feat-a")],
            &[(0, IDLE_PANE), (2, IDLE_PANE)],
            vec![vec![row("feat-a", "done")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        assert_eq!(summary.escalations.len(), 1, "{:?}", summary.escalations);
        assert_eq!(summary.escalations[0].verdict, "unmerged");
        assert!(
            summary.escalations[0]
                .command
                .contains("fatal: not a valid object name"),
            "{:?}",
            summary.escalations[0]
        );
    }

    /// A `heartbeat` exit still gets the ref-derived merge truth in its
    /// summary text, even though it is not a completion outcome (D3): the
    /// roll-up line benefits from the same truth, but the outcome/exit status
    /// is unchanged from before this feature.
    #[test]
    fn heartbeat_exit_reports_ref_derived_merged_summary() {
        let agents = two_agents();
        let merge_check = FakeMergeCheck::default().with_unmerged("feat/a", 1, "ccc3333");
        let (summary, _dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![
                pane(0, "/repo"),
                pane(2, "/repo-feat-a"),
                pane(3, "/repo-feat-b"),
            ],
            &[(0, IDLE_PANE), (2, IDLE_PANE), (3, IDLE_PANE)],
            vec![vec![row("feat-a", "working"), row("feat-b", "working")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_millis(0),
                heartbeat: Duration::from_millis(0),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert_eq!(summary.outcome, DriveOutcome::Heartbeat);
        assert_eq!(
            summary.merged_summary.as_deref(),
            Some("1/2 branches merged")
        );
    }

    // --- CLI-unavailability detection (drive-loop-exit-summary-truth, D4) ---

    /// A CLI-unavailability marker on a CODING pane is a deduped escalation —
    /// the loop keeps driving the rest of the wave, and the final outcome is
    /// never `completed` while it stands.
    #[test]
    fn coding_pane_marker_escalates_and_keeps_driving() {
        let agents = two_agents();
        let merge_check = FakeMergeCheck::default();
        let (summary, dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![
                pane(0, "/repo"),
                pane(2, "/repo-feat-a"),
                pane(3, "/repo-feat-b"),
            ],
            &[
                (0, IDLE_PANE),
                (2, "You've hit your session limit"),
                (3, &live_safe_capture("cargo test --lib")),
            ],
            vec![vec![row("feat-a", "done"), row("feat-b", "done")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert_ne!(
            summary.outcome,
            DriveOutcome::Completed,
            "a live cli-unavailable escalation must never read completed"
        );
        assert!(
            summary
                .escalations
                .iter()
                .any(|e| e.agent_id == "feat-a" && e.verdict == "cli-unavailable"),
            "{:?}",
            summary.escalations
        );
        // feat-b's prompt is escalated too — the wave keeps moving, and
        // neither pane is typed into.
        assert!(
            summary
                .escalations
                .iter()
                .any(|e| e.agent_id == "feat-b" && e.verdict == PERMISSION_PROMPT_VERDICT),
            "feat-b's prompt must still be escalated: {:?}",
            summary.escalations
        );
        // The escalations are broker records only — no surface receives a
        // keystroke.
        assert!(
            dispatcher.events.is_empty(),
            "no surface may be typed into: {:?}",
            dispatcher.events
        );
    }

    /// A marker separated from the end of the capture by more than the
    /// bounded trailing region ([`anchor_on_trailing_region`]'s
    /// `LIVE_PROMPT_TRAILING_BOUND`) is inert — the same "narration, not a
    /// live signal" rule [`is_live_prompt`] follows (`cli-unavailable-
    /// detection-gaps`, task 2.2). 14 filler lines — one past the maximum
    /// offset the bounded scan tolerates for a 4-line tail window — keeps
    /// this scenario genuinely scrolled away under the new bounded-region
    /// scan, rather than relying on the narrower pre-fix window.
    #[test]
    fn scrolled_away_marker_is_not_acted_on() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let merge_check = FakeMergeCheck::default();
        let filler: String = (1..=14)
            .map(|i| format!("filler{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let scrolled_away = format!("You've hit your session limit\n{filler}");
        let (summary, _dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![pane(2, "/repo-feat-a")],
            &[(2, scrolled_away.as_str())],
            vec![vec![row("feat-a", "done")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert_eq!(summary.outcome, DriveOutcome::Completed);
        assert!(summary.escalations.is_empty(), "{:?}", summary.escalations);
    }

    // --- `cli-unavailable-detection-gaps` gap closures ----------------------

    /// Spec scenario "A banner above a task-list footer is still detected":
    /// current Claude Code renders a persistent multi-line task list under
    /// the composer, which used to push the banner out of the fixed 4-line
    /// tail. The bounded-trailing-region scan ([`anchor_on_trailing_region`])
    /// must still find it.
    #[test]
    fn banner_above_task_list_footer_is_detected_on_coding_pane() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let merge_check = FakeMergeCheck::default();
        let banner_above_chrome = "You've hit your session limit\n\
             Tasks\n☐ Implement feature\n☐ Write tests\n☐ Update docs\n\
             main* ⇡2\nSonnet 5.1 · 42% context used";
        let (summary, _dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![pane(2, "/repo-feat-a")],
            &[(2, banner_above_chrome)],
            vec![vec![row("feat-a", "done")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert!(
            summary
                .escalations
                .iter()
                .any(|e| e.agent_id == "feat-a" && e.verdict == "cli-unavailable"),
            "{:?}",
            summary.escalations
        );
    }

    /// Spec scenario "A per-CLI marker override is honoured by the loop": a
    /// CLI whose `[clis.<name>].prompt_profile` overrides
    /// `cli_unavailable_markers` with `out of tokens` is detected when that
    /// CLI's pane tail shows the override text — not the embedded Claude
    /// default, which does not contain it.
    #[test]
    fn per_cli_marker_override_is_honoured_by_the_loop() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let mut clis: HashMap<String, CustomCli> = HashMap::new();
        clis.insert(
            "mycli".to_string(),
            CustomCli {
                command: "mycli".to_string(),
                display_name: None,
                submit_delay_ms: None,
                settings_path: None,
                approval_args: HashMap::new(),
                prompt_profile: Some(crate::config::CliPromptProfileOverride {
                    cli_unavailable_markers: Some(vec!["out of tokens".to_string()]),
                    ..Default::default()
                }),
                session_id_args: None,
                resume_args: None,
                mcp: None,
            },
        );
        let merge_check = FakeMergeCheck::default();
        let (summary, _dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![pane(2, "/repo-feat-a")],
            &[(2, "out of tokens")],
            vec![vec![row_with_cli("feat-a", "working", "mycli")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                clis,
                ..DriveConfig::default()
            },
        );
        assert!(
            summary.escalations.iter().any(|e| e.agent_id == "feat-a"
                && e.verdict == "cli-unavailable"
                && e.command.contains("out of tokens")),
            "{:?}",
            summary.escalations
        );
    }

    /// Spec scenario "The same banner across sweeps is one escalation": the
    /// dedup key is `(agent_id, matched marker)`, never incidental transcript
    /// content — so a banner that stays while EARLIER lines in the capture
    /// change between sweeps still collapses to one escalation.
    #[test]
    fn same_banner_across_sweeps_whose_earlier_lines_change_is_one_escalation() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        // Distinct leading content each sweep, same trailing banner — a
        // same-marker dedup key must still collapse these to one escalation.
        let capturer = SequencedCapturer::new(&[
            "turn 1 of the transcript\nYou've hit your session limit",
            "turn 2, different leading content\nYou've hit your session limit",
        ]);
        let status = ScriptedStatus::new(vec![
            vec![row("feat-a", "working")], // tick 1: not yet complete
            vec![row("feat-a", "done")],    // tick 2: complete
        ]);
        let enumerator = FakeEnumerator {
            panes: vec![pane(2, "/repo-feat-a")],
        };
        let mut dispatcher = RecordingDispatcher::default();
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let merge_check = FakeMergeCheck::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &ScriptedMessages::default(),
            clock: &clock,
            merge_check: &merge_check,
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let summary = drive_loop_in_tmp(
            "paw-test",
            &agents,
            &mut deps,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert_eq!(
            summary
                .escalations
                .iter()
                .filter(|e| e.agent_id == "feat-a" && e.verdict == "cli-unavailable")
                .count(),
            1,
            "deduped across two sweeps despite changing leading content: {:?}",
            summary.escalations
        );
    }

    /// Spec scenario "Two different banners on one agent are two
    /// escalations": the dedup key includes the matched MARKER, so a
    /// session-limit banner followed by an authentication-failure banner on
    /// the same agent records two distinct escalations.
    #[test]
    fn two_different_banners_on_one_agent_are_two_escalations() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let capturer =
            SequencedCapturer::new(&["You've hit your session limit", "Invalid API key"]);
        let status = ScriptedStatus::new(vec![
            vec![row("feat-a", "working")],
            vec![row("feat-a", "done")],
        ]);
        let enumerator = FakeEnumerator {
            panes: vec![pane(2, "/repo-feat-a")],
        };
        let mut dispatcher = RecordingDispatcher::default();
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let merge_check = FakeMergeCheck::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &ScriptedMessages::default(),
            clock: &clock,
            merge_check: &merge_check,
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let summary = drive_loop_in_tmp(
            "paw-test",
            &agents,
            &mut deps,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        let cli_unavailable: Vec<_> = summary
            .escalations
            .iter()
            .filter(|e| e.agent_id == "feat-a" && e.verdict == "cli-unavailable")
            .collect();
        assert_eq!(
            cli_unavailable.len(),
            2,
            "two distinct banners must yield two escalations: {:?}",
            summary.escalations
        );
    }

    /// Spec scenario "A banner quoted by the dashboard does not trigger": the
    /// dashboard's broker-log panel can display a worker's session-limit line
    /// at the tail of its capture. The dashboard is not a coding surface, so
    /// it is never scanned — no escalation, and the wave completes.
    #[test]
    fn a_banner_quoted_by_the_dashboard_does_not_trigger() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let merge_check = FakeMergeCheck::default();
        let (summary, dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![pane(0, "/repo"), pane(1, "/repo-feat-a")],
            &[
                (0, "broker log\nfeat-a: You've hit your session limit"),
                (1, IDLE_PANE),
            ],
            vec![vec![row("feat-a", "done")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert!(
            summary.escalations.is_empty(),
            "nothing may be attributed to the dashboard: {:?}",
            summary.escalations
        );
        assert_eq!(summary.outcome, DriveOutcome::Completed);
        assert!(dispatcher.events.is_empty(), "{:?}", dispatcher.events);
    }

    /// Spec scenario (GP-50) "The embedded profile recognises an organisation
    /// spend limit": the full spend-limit banner matches
    /// `cli_unavailable_markers` and the loop escalates a coding pane showing
    /// it; it must NOT match `stream_error_markers`, which is reserved for
    /// transient mid-turn drops.
    #[test]
    fn spend_limit_banner_matches_cli_unavailable_not_stream_error() {
        let spend_limit_banner = "You've hit your org's monthly spend limit · run /usage-credits to ask your admin for a higher limit";
        let profile = claude_prompt_profile();
        assert!(
            profile
                .cli_unavailable_markers
                .iter()
                .any(|m| spend_limit_banner
                    .to_ascii_lowercase()
                    .contains(&m.to_ascii_lowercase())),
            "the embedded profile must recognise the spend-limit banner"
        );
        assert!(
            !profile
                .stream_error_markers
                .iter()
                .any(|m| spend_limit_banner
                    .to_ascii_lowercase()
                    .contains(&m.to_ascii_lowercase())),
            "the spend-limit banner must not also read as a transient stream error"
        );

        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let merge_check = FakeMergeCheck::default();
        let (summary, _dispatcher) = run_loop_with_merge_check(
            &agents,
            vec![pane(2, "/repo-feat-a")],
            &[(2, spend_limit_banner)],
            vec![vec![row("feat-a", "done")]],
            &merge_check,
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                default_branch: "main".to_string(),
                ..DriveConfig::default()
            },
        );
        assert!(
            summary
                .escalations
                .iter()
                .any(|e| e.agent_id == "feat-a" && e.verdict == "cli-unavailable"),
            "{:?}",
            summary.escalations
        );
    }

    /// Asserts a payload containing `needle` was typed into `pane_index`
    /// LITERALLY (`send-keys -l`) and submitted by a SEPARATE `Enter` keystroke
    /// immediately after — the D6 nudge discipline.
    fn assert_text_then_separate_enter(d: &RecordingDispatcher, pane_index: usize, needle: &str) {
        let pos = d
            .events
            .iter()
            .position(|(p, t)| *p == pane_index && t.contains(needle))
            .unwrap_or_else(|| {
                panic!(
                    "expected a send containing {needle:?} to pane {pane_index}; \
                     events were {:?}",
                    d.events
                )
            });
        assert!(
            d.literal_sends
                .iter()
                .any(|(p, t)| *p == pane_index && t.contains(needle)),
            "the body must go out literally (send-keys -l), never through tmux's key table"
        );
        assert_eq!(
            d.events.get(pos + 1),
            Some(&(pane_index, "Enter".to_string())),
            "the submitting Enter must be its own keystroke, right after the text"
        );
    }

    /// Whether any literal send to `pane_index` contains `needle`.
    fn injected(d: &RecordingDispatcher, pane_index: usize, needle: &str) -> bool {
        d.literal_sends
            .iter()
            .any(|(p, t)| *p == pane_index && t.contains(needle))
    }

    // --- Spec scenario "Behavior is unchanged": drive-loop parity ----------

    /// Everything one parity run of the loop did, keyed by surface.
    struct LoopTrace {
        /// Pane indices the loop captured (i.e. swept), in index order.
        swept: Vec<usize>,
        /// `(agent_id, verdict)` per escalation, in order.
        escalated: Vec<(String, String)>,
        /// Pane indices the loop sent keystrokes to, deduped in index order.
        keyed: Vec<usize>,
        /// The terminal outcome.
        outcome: DriveOutcome,
    }

    /// A capturer that records which pane indices were captured, so a parity
    /// test can assert the SET of surfaces the loop swept.
    struct TracingCapturer {
        by_pane: HashMap<usize, String>,
        seen: RefCell<Vec<usize>>,
    }
    impl PaneCapture for TracingCapturer {
        fn capture(&self, _session: &str, pane_index: usize) -> String {
            self.seen.borrow_mut().push(pane_index);
            self.by_pane.get(&pane_index).cloned().unwrap_or_default()
        }
    }

    /// Drives one sweep of the loop under `layout` and reports what it touched.
    ///
    /// The heartbeat is one poll interval, so the loop exits after a single
    /// sweep and the trace is that sweep's decisions with nothing accumulated
    /// on top.
    fn trace_one_sweep(
        layout: SessionLayout,
        agents: &[AgentPane],
        panes: Vec<PaneInfo>,
        pane_text: &[(usize, &str)],
    ) -> LoopTrace {
        let enumerator = FakeEnumerator { panes };
        let capturer = TracingCapturer {
            by_pane: pane_text
                .iter()
                .map(|(i, t)| (*i, (*t).to_string()))
                .collect(),
            seen: RefCell::new(Vec::new()),
        };
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![Vec::new()]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_secs(1),
            layout,
            ..DriveConfig::default()
        };
        let summary = {
            let mut deps = DriveDeps {
                enumerator: &enumerator,
                capturer: &capturer,
                dispatcher: &mut dispatcher,
                status: &status,
                messages: &messages,
                clock: &clock,
                merge_check: &FakeMergeCheck::default(),
                alerts: &mut alerts,
                learnings: &mut learnings,
                manual_decisions: &mut manual_decisions,
                refresher: &mut refresher,
                wedged: Vec::new(),
            };
            drive_loop_in_tmp("paw-test", agents, &mut deps, &config)
        };

        let mut swept = capturer.seen.borrow().clone();
        swept.sort_unstable();
        swept.dedup();
        let mut keyed: Vec<usize> = dispatcher.events.iter().map(|(p, _)| *p).collect();
        keyed.sort_unstable();
        keyed.dedup();
        LoopTrace {
            swept,
            escalated: alerts
                .escalations
                .iter()
                .map(|e| (e.agent_id.clone(), e.verdict.clone()))
                .collect(),
            keyed,
            outcome: summary.outcome,
        }
    }

    /// `approval-auto` scenario "Keystrokes reach only coding-agent
    /// surfaces", supervised half (GP-46): pane 0 is the dashboard — never
    /// captured, never escalated, never typed into, even with a prompt drawn
    /// on it — the coding surfaces escalate under their own ids, and the
    /// escalations are broker records only.
    #[test]
    fn supervised_layout_sweeps_only_coding_surfaces_and_types_nowhere() {
        let agents = two_agents();
        let trace = trace_one_sweep(
            supervised_layout(),
            &agents,
            vec![
                pane(0, "/repo"),
                pane(1, "/repo-feat-a"),
                pane(2, "/repo-feat-b"),
            ],
            &[
                (0, &live_safe_capture("should never be swept")),
                (1, &live_safe_capture("frobnicate --all")),
                (2, &live_safe_capture("cargo build")),
            ],
        );

        assert_eq!(
            trace.swept,
            vec![1, 2],
            "the dashboard surface is never swept"
        );
        assert_eq!(
            trace.escalated,
            vec![
                ("feat-a".to_string(), PERMISSION_PROMPT_VERDICT.to_string()),
                ("feat-b".to_string(), PERMISSION_PROMPT_VERDICT.to_string()),
            ],
            "only the prompting coding surfaces escalate, under their own ids"
        );
        assert!(
            trace.keyed.is_empty(),
            "no surface is typed into: {:?}",
            trace.keyed
        );
        assert_eq!(trace.outcome, DriveOutcome::Heartbeat);
    }

    /// `approval-auto` scenarios "Dashboard surface is never nudged in a
    /// supervised session", "Unresolved surface is refused" and "Coding agent
    /// surfaces are reached": buffered text on an idle input box is submitted
    /// on the coding surface only — the dashboard and a surface no worktree
    /// owns receive nothing.
    #[test]
    fn buffered_input_is_submitted_on_coding_surfaces_only() {
        let agents = two_agents();
        let buffered = idle_input_box("/opsx:apply my-change");
        let trace = trace_one_sweep(
            supervised_layout(),
            &agents,
            vec![
                pane(0, "/repo"),
                pane(1, "/repo-feat-a"),
                pane(3, "/elsewhere"),
            ],
            &[(0, &buffered), (1, &buffered), (3, &buffered)],
        );

        assert_eq!(trace.swept, vec![1], "only the coding surface is captured");
        assert_eq!(
            trace.keyed,
            vec![1],
            "the buffered submit reaches the coding surface and nothing else"
        );
    }

    /// Scenario "A prompt drawn on the dashboard is not escalated"
    /// (`supervisor-unattended-operation`): text on the dashboard — which can
    /// quote a worker's prompt — is not a coding surface, so the loop never
    /// captures it, escalates it, or types into it.
    #[test]
    fn a_prompt_drawn_on_the_dashboard_is_not_escalated() {
        let agents = two_agents();
        let trace = trace_one_sweep(
            supervised_layout(),
            &agents,
            vec![pane(0, "/repo")],
            &[(0, &live_safe_capture("git status"))],
        );

        assert!(trace.swept.is_empty(), "swept {:?}", trace.swept);
        assert!(
            trace.escalated.is_empty(),
            "escalated {:?}",
            trace.escalated
        );
        assert!(trace.keyed.is_empty(), "keyed {:?}", trace.keyed);
    }

    /// Scenario "A pre-relocation live session keeps its agent offset"
    /// (`session-state`): a session an earlier binary saved with
    /// `dashboard_pane = Some(1)` still runs its old orchestrator CLI at pane
    /// 0 and its dashboard at pane 1. Neither is a coding surface, so the
    /// loop acts on panes 2.. only — even when both reserved panes show a
    /// live prompt.
    #[test]
    fn a_pre_relocation_session_acts_on_neither_reserved_pane() {
        let agents = two_agents();
        let legacy = crate::session::Session {
            session_name: "paw-test".to_string(),
            repo_path: PathBuf::from("/repo"),
            project_name: "test".to_string(),
            created_at: SystemTime::UNIX_EPOCH,
            status: crate::session::SessionStatus::Active,
            worktrees: Vec::new(),
            broker_port: Some(9119),
            broker_bind: Some("127.0.0.1".to_string()),
            broker_log_path: None,
            mode: crate::session::SessionMode::Supervisor,
            dashboard_pane: Some(1),
            layout: None,
        };
        let trace = trace_one_sweep(
            SessionLayout::of(&legacy),
            &agents,
            vec![
                pane(0, "/repo"),
                pane(1, "/repo"),
                pane(2, "/repo-feat-a"),
                pane(3, "/repo-feat-b"),
            ],
            &[
                (0, &live_safe_capture("old orchestrator prompt")),
                (1, &live_safe_capture("dashboard text")),
                (2, &live_safe_capture("frobnicate --all")),
                (3, IDLE_PANE),
            ],
        );

        assert_eq!(trace.swept, vec![2, 3], "panes 0 and 1 are never swept");
        assert_eq!(
            trace.escalated,
            vec![("feat-a".to_string(), PERMISSION_PROMPT_VERDICT.to_string())]
        );
        assert!(trace.keyed.is_empty(), "keyed {:?}", trace.keyed);
    }

    /// `approval-auto` scenario "Keystrokes reach only coding-agent
    /// surfaces", bare half: index 0 is the bare layout's dashboard — never
    /// swept, never escalated — the coding surfaces escalate under their own
    /// ids, and nothing is typed anywhere.
    #[test]
    fn bare_layout_sweeps_only_coding_surfaces_and_types_nowhere() {
        let agents = two_agents();
        let trace = trace_one_sweep(
            SessionLayout::planned(crate::session::SessionMode::Bare, true),
            &agents,
            vec![
                pane(0, "/repo"),
                pane(1, "/repo-feat-a"),
                pane(2, "/repo-feat-b"),
            ],
            &[
                (0, &live_safe_capture("should never be swept")),
                (1, &live_safe_capture("frobnicate --all")),
                (2, &live_safe_capture("cargo build")),
            ],
        );

        assert_eq!(
            trace.swept,
            vec![1, 2],
            "index 0 is the bare layout's dashboard surface"
        );
        assert_eq!(
            trace.escalated,
            vec![
                ("feat-a".to_string(), PERMISSION_PROMPT_VERDICT.to_string()),
                ("feat-b".to_string(), PERMISSION_PROMPT_VERDICT.to_string()),
            ],
            "no surface escalates under the supervisor wire identity"
        );
        assert!(
            trace.keyed.is_empty(),
            "nothing is typed into: {:?}",
            trace.keyed
        );
        assert_eq!(trace.outcome, DriveOutcome::Heartbeat);
    }

    /// A CODING branch whose slug collides with the supervisor wire identity
    /// is what it is — a coding surface — so its CLI-unavailability banner is
    /// an ordinary non-blocking escalation under its own (colliding) id and
    /// the wave keeps driving.
    #[test]
    fn a_coding_branch_slugged_supervisor_is_an_ordinary_coding_surface() {
        let agents = vec![AgentPane {
            agent_id: SUPERVISOR_AGENT_ID.to_string(),
            worktree_path: PathBuf::from("/repo-supervisor"),
            branch: "feat/supervisor".to_string(),
        }];
        let trace = trace_one_sweep(
            supervised_layout(),
            &agents,
            vec![pane(0, "/repo"), pane(1, "/repo-supervisor")],
            &[
                (0, IDLE_PANE),
                (1, "working…\nYou've hit your session limit"),
            ],
        );

        assert_eq!(
            trace.outcome,
            DriveOutcome::Heartbeat,
            "a worker's banner must not end the wave"
        );
        assert_eq!(
            trace.escalated,
            vec![(
                SUPERVISOR_AGENT_ID.to_string(),
                "cli-unavailable".to_string()
            )],
            "it is escalated as a coding pane, under its own (colliding) id"
        );
        assert_eq!(trace.swept, vec![1], "the dashboard is not swept");
    }

    /// Spec scenario "Coding agents resolve by owner, not index", loop half: a
    /// coding worktree whose surface sits at an index the layout reserves is
    /// still swept and escalated as that agent, never as the dashboard.
    #[test]
    fn a_coding_surface_at_a_reserved_index_escalates_under_its_own_id() {
        let agents = two_agents();
        let trace = trace_one_sweep(
            supervised_layout(),
            &agents,
            vec![pane(0, "/repo-feat-a")],
            &[(0, &live_safe_capture("frobnicate --all"))],
        );

        assert_eq!(trace.swept, vec![0]);
        assert_eq!(
            trace.escalated,
            vec![("feat-a".to_string(), PERMISSION_PROMPT_VERDICT.to_string())],
            "the owning worktree wins over the reserved index"
        );
    }

    /// Scenarios "Escalation is recorded as a broker review item only"
    /// (`supervisor-autonomous-orchestrator`) and "A permission-prompt
    /// escalation is never typed into a surface"
    /// (`supervisor-unattended-operation`): both prompting workers escalate,
    /// each escalation is a broker review item, and no surface — the
    /// dashboard included — receives a keystroke, while the wave keeps
    /// moving to completion.
    #[test]
    fn escalation_is_recorded_as_a_broker_review_item_only() {
        let agents = two_agents();
        let (dispatcher, alerts) = run_loop(
            &agents,
            vec![
                pane(0, "/repo"),
                pane(1, "/repo-feat-a"),
                pane(2, "/repo-feat-b"),
            ],
            &[
                (0, IDLE_PANE),
                (1, &live_safe_capture("frobnicate --all")),
                (2, &live_safe_capture("cargo build")),
            ],
            vec![
                vec![row("feat-a", "working"), row("feat-b", "working")],
                vec![row("supervisor", "done")],
            ],
            &ScriptedMessages::none(),
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                ..DriveConfig::default()
            },
        );

        assert_eq!(alerts.escalations.len(), 2, "recorded on the broker");
        assert!(
            alerts
                .escalations
                .iter()
                .all(|e| e.verdict == PERMISSION_PROMPT_VERDICT),
            "{:?}",
            alerts.escalations
        );
        assert!(
            dispatcher.events.is_empty(),
            "no surface receives a keystroke; events were {:?}",
            dispatcher.events
        );
    }

    // --- the drive loop follows the live agent roster -----------------------
    // `supervisor-unattended-operation`: "The drive loop follows the live
    // agent roster". An agent attached with `git paw add` while the loop runs
    // is driven; one detached with `git paw remove` stops being driven; a
    // sweep whose roster read fails keeps the roster it last read.

    /// A [`RosterReader`] that yields one scripted answer per sweep, repeating
    /// its last answer once the script runs out. `None` stands for a sweep
    /// whose session state could not be read.
    #[derive(Debug)]
    struct ScriptedRoster {
        rounds: Mutex<std::collections::VecDeque<Option<Vec<AgentPane>>>>,
        last: Mutex<Option<Vec<AgentPane>>>,
    }

    impl ScriptedRoster {
        fn new(rounds: Vec<Option<Vec<AgentPane>>>) -> Arc<Self> {
            Arc::new(Self {
                rounds: Mutex::new(rounds.into()),
                last: Mutex::new(None),
            })
        }
    }

    impl RosterReader for ScriptedRoster {
        fn read_roster(&self) -> Option<Vec<AgentPane>> {
            let mut rounds = self.rounds.lock().ok()?;
            let mut last = self.last.lock().ok()?;
            match rounds.pop_front() {
                Some(answer) => {
                    if answer.is_some() {
                        last.clone_from(&answer);
                    }
                    answer
                }
                // Script exhausted: hold the last answer rather than snapping
                // back to the launch-time roster.
                None => last.clone(),
            }
        }
    }

    fn agent(id: &str, worktree: &str, branch: &str) -> AgentPane {
        AgentPane {
            agent_id: id.to_string(),
            worktree_path: PathBuf::from(worktree),
            branch: branch.to_string(),
        }
    }

    /// [`run_loop`], plus a scripted roster and the full [`DriveSummary`], so a
    /// test can assert on what the loop drove AND on what the summary says.
    fn run_loop_with_roster(
        agents: &[AgentPane],
        roster: Arc<dyn RosterReader + Send + Sync>,
        panes: Vec<PaneInfo>,
        pane_text: &[(usize, &str)],
        statuses: Vec<Vec<AgentStatusRow>>,
        merge_check: &dyn MergeCheck,
    ) -> (DriveSummary, RecordingDispatcher, RecordingAlerts) {
        let enumerator = FakeEnumerator { panes };
        let capturer = FakeCapturer::new(pane_text);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(statuses);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check,
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            roster: Some(roster),
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", agents, &mut deps, &config);
        (summary, dispatcher, alerts)
    }

    /// Scenario "An added agent is driven by a running loop": the loop starts
    /// with one agent; a second appears in the roster mid-run and its live
    /// permission prompt is escalated by name on a subsequent sweep — with no
    /// keystroke answering it.
    #[test]
    fn an_agent_added_mid_run_is_swept_and_escalated_by_the_running_loop() {
        let started_with = vec![agent("feat-a", "/repo-feat-a", "feat/a")];
        let after_add = vec![
            agent("feat-a", "/repo-feat-a", "feat/a"),
            agent("feat-b", "/repo-feat-b", "feat/b"),
        ];
        let (_, dispatcher, alerts) = run_loop_with_roster(
            &started_with,
            ScriptedRoster::new(vec![Some(started_with.clone()), Some(after_add)]),
            vec![
                pane(0, "/repo"),
                pane(1, "/repo-feat-a"),
                pane(2, "/repo-feat-b"),
            ],
            &[
                (0, IDLE_PANE),
                (1, IDLE_PANE),
                (2, &live_safe_capture("cargo build")),
            ],
            vec![
                vec![row("feat-a", "working")],
                vec![row("supervisor", "done")],
            ],
            &FakeMergeCheck::default(),
        );

        assert_eq!(
            alerts
                .escalations
                .iter()
                .map(|e| (e.agent_id.as_str(), e.verdict.as_str()))
                .collect::<Vec<_>>(),
            vec![("feat-b", PERMISSION_PROMPT_VERDICT)],
            "the added agent's prompt is escalated by name once the roster \
             carries it; escalations were {:?}",
            alerts.escalations
        );
        assert!(
            dispatcher.events.is_empty(),
            "no keystroke answers the escalated prompt; events were {:?}",
            dispatcher.events
        );
    }

    /// Scenario "A removed agent is no longer driven": once the roster drops an
    /// agent, no later sweep resolves its surface, and the exit summary counts
    /// and lists only the agents that remain.
    #[test]
    fn an_agent_removed_mid_run_leaves_the_roster_and_the_summary() {
        let started_with = vec![
            agent("feat-a", "/repo-feat-a", "feat/a"),
            agent("feat-x", "/repo-feat-x", "feat/x"),
        ];
        let after_remove = vec![agent("feat-a", "/repo-feat-a", "feat/a")];
        // Both branches are unmerged, so a roster still carrying `feat/x`
        // would put an `[unmerged]` entry for it in the summary.
        let merge_check = FakeMergeCheck::default()
            .with_unmerged("feat/a", 1, "aaa1111")
            .with_unmerged("feat/x", 2, "xxx2222");
        let (summary, dispatcher, _) = run_loop_with_roster(
            &started_with,
            ScriptedRoster::new(vec![Some(after_remove)]),
            vec![
                pane(0, "/repo"),
                pane(1, "/repo-feat-a"),
                pane(2, "/repo-feat-x"),
            ],
            &[
                (0, IDLE_PANE),
                (1, IDLE_PANE),
                (2, &live_safe_capture("rm -rf build")),
            ],
            vec![vec![row("supervisor", "done")]],
            &merge_check,
        );

        let rendered = format!("{summary:?}");
        assert!(
            !rendered.contains("feat-x") && !rendered.contains("feat/x"),
            "the removed agent must not appear in the exit summary; summary: {rendered}"
        );
        assert!(
            dispatcher.events.is_empty(),
            "no keystroke is attributed to the removed agent; events were {:?}",
            dispatcher.events
        );
    }

    /// Scenario "An unreadable roster keeps the previous one": a sweep whose
    /// session state cannot be read keeps driving the agents the loop already
    /// knows about rather than driving none.
    #[test]
    fn a_sweep_whose_roster_read_fails_keeps_the_previous_roster() {
        let agents = two_agents();
        let panes = vec![
            pane(0, "/repo"),
            pane(1, "/repo-feat-a"),
            pane(2, "/repo-feat-b"),
        ];
        let text: &[(usize, &str)] = &[
            (0, IDLE_PANE),
            (1, &live_safe_capture("frobnicate --all")),
            (2, &live_safe_capture("cargo build")),
        ];
        let statuses = vec![
            vec![row("feat-a", "working"), row("feat-b", "working")],
            vec![row("supervisor", "done")],
        ];

        // Every read fails.
        let (_, _, kept) = run_loop_with_roster(
            &agents,
            ScriptedRoster::new(vec![None, None]),
            panes.clone(),
            text,
            statuses.clone(),
            &FakeMergeCheck::default(),
        );
        assert_eq!(
            kept.escalations.len(),
            2,
            "an unreadable roster keeps both agents in play; escalations were {:?}",
            kept.escalations
        );

        // Control: a roster that reads back EMPTY really does stop the loop
        // driving anyone, so the assertion above is about the read failing —
        // not about the refresh being inert.
        let (_, _, emptied) = run_loop_with_roster(
            &agents,
            ScriptedRoster::new(vec![Some(Vec::new())]),
            panes,
            text,
            statuses,
            &FakeMergeCheck::default(),
        );
        assert!(
            emptied.escalations.is_empty(),
            "an empty roster drives nobody; escalations were {:?}",
            emptied.escalations
        );
    }

    /// A worker's ambiguous `agent.question` is a judgment call, but its own
    /// broker message is the review item: the loop publishes no copy of it and
    /// types it into no surface.
    #[test]
    fn a_worker_question_stays_on_the_broker_and_is_typed_nowhere() {
        let agents = two_agents();
        let question = BrokerMessage::Question {
            agent_id: "feat-a".to_string(),
            payload: crate::broker::messages::QuestionPayload {
                question: "spec says both A and B own this file — which wins?".to_string(),
            },
        };
        let (dispatcher, alerts) = run_loop(
            &agents,
            vec![pane(0, "/repo"), pane(1, "/repo-feat-a")],
            &[(0, IDLE_PANE), (1, IDLE_PANE)],
            vec![
                vec![row("feat-a", "working"), row("feat-b", "working")],
                vec![row("supervisor", "done")],
            ],
            &ScriptedMessages::new(vec![vec![question]]),
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                ..DriveConfig::default()
            },
        );

        assert!(dispatcher.events.is_empty(), "{:?}", dispatcher.events);
        assert!(alerts.escalations.is_empty(), "{:?}", alerts.escalations);
    }

    // --- wind-down suppresses nudge dispatch (GP-16) ------------------------

    /// Spec scenario "Wind-down does not re-accumulate unsubmitted text": a
    /// gate failure observed on the same tick the wave completes leaves a
    /// pending correction that the correction pass never gets a chance to
    /// dispatch — the loop breaks before it runs, so the worker's idle pane
    /// gains no re-engagement text on its way out.
    #[test]
    fn a_pending_correction_is_not_dispatched_once_the_wave_completes() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
            branch: "feat/a".to_string(),
        }];
        let (dispatcher, _) = run_loop(
            &agents,
            vec![pane(0, "/repo"), pane(2, "/repo-feat-a")],
            &[(0, IDLE_PANE), (2, IDLE_PANE)],
            vec![vec![row("supervisor", "done")]],
            &ScriptedMessages::new(vec![vec![gate_feedback(
                "feat-a",
                "testing",
                "still failing",
            )]]),
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                correction: CorrectionConfig {
                    auto_loopback: true,
                    ..CorrectionConfig::default()
                },
                ..DriveConfig::default()
            },
        );
        assert!(
            dispatcher.events.is_empty(),
            "no re-engagement keystrokes once the wave completes on the same \
             tick the gate failure was observed; events were {:?}",
            dispatcher.events
        );
    }

    /// A worker `agent.artifact` at `agent_status`, for the merge-decision arm.
    fn artifact(agent_status: &str) -> BrokerMessage {
        BrokerMessage::Artifact {
            agent_id: "feat-a".to_string(),
            payload: crate::broker::messages::ArtifactPayload {
                status: agent_status.to_string(),
                exports: Vec::new(),
                modified_files: vec!["src/lib.rs".to_string()],
            },
        }
    }

    /// Scenario "Merge decision is recorded, not injected"
    /// (`supervisor-autonomous-orchestrator`): a worker's `committed` (or
    /// `done`) artifact is the point a merge decision becomes live. The
    /// worker's own artifact message is the broker record of it; the loop
    /// publishes no copy and types into no surface.
    #[test]
    fn merge_decision_is_recorded_not_injected() {
        for status in ["committed", "done"] {
            let agents = two_agents();
            let (dispatcher, alerts) = run_loop(
                &agents,
                vec![pane(0, "/repo"), pane(1, "/repo-feat-a")],
                &[(0, IDLE_PANE), (1, IDLE_PANE)],
                vec![
                    vec![row("feat-a", status), row("feat-b", "working")],
                    vec![row("supervisor", "done")],
                ],
                &ScriptedMessages::new(vec![vec![artifact(status)]]),
                &DriveConfig {
                    poll_interval: Duration::from_secs(1),
                    heartbeat: Duration::from_hours(1),
                    ..DriveConfig::default()
                },
            );
            assert!(
                dispatcher.events.is_empty(),
                "{status}: no surface may be typed into; events were {:?}",
                dispatcher.events
            );
            assert!(
                alerts.escalations.is_empty(),
                "{status}: the artifact itself is the record; got {:?}",
                alerts.escalations
            );
        }
    }

    /// The escalation context a headless orchestrator invocation reads for a
    /// merge decision names the agent, the status that made it live, and the
    /// sequencing ask — a pure renderer, kept for that hand-off.
    #[test]
    fn merge_handoff_text_names_the_agent_status_and_sequencing() {
        let text = merge_handoff_text("feat-a", "committed");
        assert!(
            text.contains("Merge decision handed to you: feat-a"),
            "{text}"
        );
        assert!(text.contains("committed"), "{text}");
        assert!(text.contains("merge sequence"), "{text}");
    }

    // === `supervisor-branch-refresh`: post-merge trigger + gates (group 5/6) ===

    /// Builds an `agent.advanced-main` event with `base` as the default
    /// branch — the drive loop's only trigger for evaluating branch refresh
    /// (design D4).
    fn advanced_main(base: &str) -> BrokerMessage {
        BrokerMessage::AdvancedMain {
            payload: crate::broker::messages::AdvancedMainPayload {
                from: SUPERVISOR_AGENT_ID.to_string(),
                merged_branch: "feat/other".to_string(),
                new_main_sha: "abcdef123456".to_string(),
                base: base.to_string(),
                merged_at: chrono::Utc::now(),
                summary: None,
            },
        }
    }

    fn one_agent_at(worktree: &str) -> Vec<AgentPane> {
        vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from(worktree),
            branch: "feat/a".to_string(),
        }]
    }

    /// Bundles a single-agent branch-refresh scenario so each test only
    /// states what differs: the pane capture, the two status ticks, the
    /// observed messages, and whether the capability is enabled.
    #[allow(clippy::too_many_arguments)]
    fn run_branch_refresh_scenario(
        capture: &str,
        first_tick_status: Vec<AgentStatusRow>,
        messages_batches: Vec<Vec<BrokerMessage>>,
        branch_refresh_enabled: bool,
        refresher: RecordingRefresher,
    ) -> RecordingRefresher {
        let agents = one_agent_at("/repo-feat-a");
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                owner: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(2, capture)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![first_tick_status, vec![row("supervisor", "done")]]);
        let messages = ScriptedMessages::new(messages_batches);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = refresher;
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            branch_refresh_enabled,
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        refresher
    }

    /// Spec scenario "Clean, idle, claimed, conflict-free, behind branch is
    /// refreshed": every gate passes, so the branch is rebased and the
    /// worker is notified with the distinguishing tag — never
    /// `agent.advanced-main` (tasks 6.1/6.3).
    #[test]
    fn branch_refresh_happy_path_rebases_and_notifies() {
        let refresher = run_branch_refresh_scenario(
            IDLE_PANE,
            vec![row_with_modified_files("feat-a", "working", &[])],
            vec![vec![advanced_main("main")]],
            true,
            RecordingRefresher::default(),
        );
        assert_eq!(
            refresher.rebase_calls,
            vec![PathBuf::from("/repo-feat-a")],
            "the passing branch must be rebased exactly once"
        );
        assert_eq!(
            refresher.notify_calls,
            vec![("feat-a".to_string(), "main".to_string())],
            "the worker must be notified with the merge's base branch"
        );
    }

    /// Task 6.1/6.3, at the message-shape level: the notification text
    /// carries the `[branch-refresh]` tag and is delivered as `agent.feedback`
    /// (via [`GitBranchRefresher::notify`]'s production wiring), never as
    /// `agent.advanced-main` — asserted directly against the text builder
    /// the production notifier calls.
    #[test]
    fn branch_refresh_notification_is_tagged_feedback_not_advanced_main() {
        let text = super::super::branch_refresh::refreshed_notification_text("main");
        assert!(text.starts_with(super::super::branch_refresh::BRANCH_REFRESH_TAG));
        // The text is plugged into a `BrokerMessage::Feedback`, not
        // `BrokerMessage::AdvancedMain` — see `GitBranchRefresher::notify`.
        let msg = BrokerMessage::Feedback {
            agent_id: "feat-a".to_string(),
            payload: crate::broker::messages::FeedbackPayload {
                from: SUPERVISOR_AGENT_ID.to_string(),
                errors: vec![text],
            },
        };
        assert!(!matches!(msg, BrokerMessage::AdvancedMain { .. }));
    }

    /// Spec scenario "No refresh without a merge": with the capability
    /// enabled but no `agent.advanced-main` ever observed, no branch is
    /// rebased even as several sweeps pass (task 5.2).
    #[test]
    fn branch_refresh_never_fires_without_an_observed_merge() {
        let refresher = run_branch_refresh_scenario(
            IDLE_PANE,
            vec![row_with_modified_files("feat-a", "working", &[])],
            Vec::new(), // ScriptedMessages::new(vec![]) — never any messages
            true,
            RecordingRefresher::default(),
        );
        assert!(
            refresher.rebase_calls.is_empty(),
            "no merge was observed; the branch must never be rebased"
        );
        assert!(refresher.notify_calls.is_empty());
    }

    /// Spec scenario "Refresh disabled by default" / task 7.3: with the
    /// capability disabled, an observed merge event is a complete no-op —
    /// behaviour identical to before the capability existed.
    #[test]
    fn branch_refresh_disabled_ignores_an_observed_merge() {
        let refresher = run_branch_refresh_scenario(
            IDLE_PANE,
            vec![row_with_modified_files("feat-a", "working", &[])],
            vec![vec![advanced_main("main")]],
            false, // branch_refresh_enabled
            RecordingRefresher::default(),
        );
        assert!(
            refresher.rebase_calls.is_empty(),
            "disabled means byte-identical to pre-capability behaviour: no rebase"
        );
        assert!(refresher.notify_calls.is_empty());
    }

    /// Spec scenario "Dirty working tree is never rebased".
    #[test]
    fn branch_refresh_skips_a_dirty_worker() {
        let refresher = run_branch_refresh_scenario(
            IDLE_PANE,
            vec![row_with_modified_files(
                "feat-a",
                "working",
                &["src/lib.rs"],
            )],
            vec![vec![advanced_main("main")]],
            true,
            RecordingRefresher::default(),
        );
        assert!(
            refresher.rebase_calls.is_empty(),
            "a worker with uncommitted changes must never be rebased"
        );
    }

    /// Spec scenario "Mid-response pane is not rebased".
    #[test]
    fn branch_refresh_skips_a_mid_response_pane() {
        let refresher = run_branch_refresh_scenario(
            MID_RESPONSE_PANE,
            vec![row_with_modified_files("feat-a", "working", &[])],
            vec![vec![advanced_main("main")]],
            true,
            RecordingRefresher::default(),
        );
        assert!(
            refresher.rebase_calls.is_empty(),
            "a mid-response worker must never be rebased"
        );
    }

    /// Task 3.2 at the orchestration level: a branch predicted to conflict is
    /// skipped and the rebase entry point is never invoked.
    #[test]
    fn branch_refresh_skips_a_predicted_conflict() {
        let refresher = run_branch_refresh_scenario(
            IDLE_PANE,
            vec![row_with_modified_files("feat-a", "working", &[])],
            vec![vec![advanced_main("main")]],
            true,
            RecordingRefresher::default().with_conflict(Path::new("/repo-feat-a")),
        );
        assert!(
            refresher.rebase_calls.is_empty(),
            "a predicted conflict must never invoke the rebase entry point"
        );
    }

    /// Spec scenario "Branch already current is not rebased".
    #[test]
    fn branch_refresh_skips_a_branch_already_current() {
        let refresher = run_branch_refresh_scenario(
            IDLE_PANE,
            vec![row_with_modified_files("feat-a", "working", &[])],
            vec![vec![advanced_main("main")]],
            true,
            RecordingRefresher::default().with_not_behind(Path::new("/repo-feat-a")),
        );
        assert!(
            refresher.rebase_calls.is_empty(),
            "a branch already current must never be rebased"
        );
    }

    /// Spec scenario "Verified branch is excluded from refresh".
    #[test]
    fn branch_refresh_skips_a_verified_awaiting_merge_branch() {
        let refresher = run_branch_refresh_scenario(
            IDLE_PANE,
            vec![row_with_modified_files("feat-a", "verified", &[])],
            vec![vec![advanced_main("main")]],
            true,
            RecordingRefresher::default(),
        );
        assert!(
            refresher.rebase_calls.is_empty(),
            "a verified-awaiting-merge branch must never be refreshed"
        );
    }

    /// A rebase that fails once invoked must never notify the worker —
    /// notification only follows a SUCCESSFUL refresh.
    #[test]
    fn branch_refresh_does_not_notify_on_a_failed_rebase() {
        let refresher = run_branch_refresh_scenario(
            IDLE_PANE,
            vec![row_with_modified_files("feat-a", "working", &[])],
            vec![vec![advanced_main("main")]],
            true,
            RecordingRefresher::default().with_failing_rebase(),
        );
        assert_eq!(refresher.rebase_calls.len(), 1, "the rebase was attempted");
        assert!(
            refresher.notify_calls.is_empty(),
            "a failed rebase must not notify the worker"
        );
    }

    // === Task 8: cross-module E2E against a REAL git repo (real worktrees, ===
    // === real rebase; only tmux/pane capture are faked) ======================

    fn run_real_git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("run git command");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn capture_real_git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("run git command");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Builds a real repo on `main` at one commit, with `feat/example`
    /// branched off it, then advances `main` by one more commit — so
    /// `feat/example` is genuinely behind. No `origin` remote: a single local
    /// repo resolves its default branch via the local-`main` fallback
    /// ([`crate::git::default_branch`]).
    fn real_repo_with_a_behind_branch() -> tempfile::TempDir {
        let sandbox = tempfile::tempdir().expect("tempdir");
        let repo_root = sandbox.path();
        run_real_git(repo_root, &["init", "-q", "-b", "main"]);
        run_real_git(repo_root, &["config", "user.email", "test@test.com"]);
        run_real_git(repo_root, &["config", "user.name", "Test"]);
        std::fs::write(repo_root.join("a.txt"), "one\n").unwrap();
        run_real_git(repo_root, &["add", "."]);
        run_real_git(repo_root, &["commit", "-q", "-m", "init"]);
        run_real_git(repo_root, &["branch", "feat/example"]);
        std::fs::write(repo_root.join("main-only.txt"), "x\n").unwrap();
        run_real_git(repo_root, &["add", "."]);
        run_real_git(repo_root, &["commit", "-q", "-m", "main advances"]);
        sandbox
    }

    /// Task 8.1: a real, end-to-end pass through the orchestration function
    /// against a real git repo — a real worktree for a behind branch is
    /// actually rebased onto main, with only the tmux pane capture faked
    /// (idle, so gate 2 passes) and the pane claim taken for real against the
    /// repo's own `.git-paw/tmp/`.
    #[test]
    fn branch_refresh_end_to_end_rebases_a_real_worktree() {
        let sandbox = real_repo_with_a_behind_branch();
        let repo_root = sandbox.path().to_path_buf();
        let creation = crate::git::create_worktree(
            &repo_root,
            "feat/example",
            false,
            crate::config::WorktreePlacement::Sibling,
        )
        .expect("create a real worktree for feat/example");
        let worktree_path = creation.path;

        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: worktree_path.clone(),
            branch: "feat/example".to_string(),
        }];
        let mut pane_by_agent = HashMap::new();
        pane_by_agent.insert("feat-a".to_string(), 2usize);
        let latest_status = vec![row_with_modified_files("feat-a", "working", &[])];
        let ctx = SweepContext {
            session: "paw-test",
            repo_root: &repo_root,
        };

        let enumerator = FakeEnumerator { panes: vec![] };
        let capturer = FakeCapturer::new(&[(2, IDLE_PANE)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![]);
        let messages = ScriptedMessages::none();
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = GitBranchRefresher {
            repo_root: repo_root.clone(),
            broker_url: None,
        };
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };

        refresh_branches_after_merge(
            ctx,
            &agents,
            &pane_by_agent,
            &latest_status,
            "main",
            &mut deps,
        );

        let main_head = capture_real_git(&repo_root, &["rev-parse", "main"]);
        let feat_head = capture_real_git(&worktree_path, &["rev-parse", "feat/example"]);
        assert_eq!(
            main_head, feat_head,
            "feat/example must be rebased onto main's latest commit"
        );
    }

    /// Task 8.2: a dirty worker is skipped and its uncommitted changes are
    /// PROVABLY untouched afterwards — real file content, real git status,
    /// checked after the orchestration function runs against a real repo.
    #[test]
    fn branch_refresh_end_to_end_leaves_a_dirty_worktree_provably_untouched() {
        let sandbox = real_repo_with_a_behind_branch();
        let repo_root = sandbox.path().to_path_buf();
        let creation = crate::git::create_worktree(
            &repo_root,
            "feat/example",
            false,
            crate::config::WorktreePlacement::Sibling,
        )
        .expect("create a real worktree for feat/example");
        let worktree_path = creation.path;

        // The agent has uncommitted work: an edited tracked file.
        let uncommitted_content = "uncommitted local edit\n";
        std::fs::write(worktree_path.join("a.txt"), uncommitted_content).unwrap();

        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: worktree_path.clone(),
            branch: "feat/example".to_string(),
        }];
        let mut pane_by_agent = HashMap::new();
        pane_by_agent.insert("feat-a".to_string(), 2usize);
        // Cleanliness gate reads the watcher's tracked modified_files, which
        // reports the dirty file — the same signal a real watcher would have
        // published.
        let latest_status = vec![row_with_modified_files("feat-a", "working", &["a.txt"])];
        let ctx = SweepContext {
            session: "paw-test",
            repo_root: &repo_root,
        };

        let enumerator = FakeEnumerator { panes: vec![] };
        let capturer = FakeCapturer::new(&[(2, IDLE_PANE)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![]);
        let messages = ScriptedMessages::none();
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut manual_decisions = NoopManualDecisions;
        let mut refresher = GitBranchRefresher {
            repo_root: repo_root.clone(),
            broker_url: None,
        };
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            merge_check: &FakeMergeCheck::default(),
            alerts: &mut alerts,
            learnings: &mut learnings,
            manual_decisions: &mut manual_decisions,
            refresher: &mut refresher,
            wedged: Vec::new(),
        };

        let pre_head = capture_real_git(&worktree_path, &["rev-parse", "feat/example"]);
        refresh_branches_after_merge(
            ctx,
            &agents,
            &pane_by_agent,
            &latest_status,
            "main",
            &mut deps,
        );
        let post_head = capture_real_git(&worktree_path, &["rev-parse", "feat/example"]);

        assert_eq!(
            pre_head, post_head,
            "a dirty worker's branch must never be rewritten"
        );
        let on_disk = std::fs::read_to_string(worktree_path.join("a.txt")).unwrap();
        assert_eq!(
            on_disk, uncommitted_content,
            "the uncommitted edit must be provably untouched"
        );
        let status = capture_real_git(&worktree_path, &["status", "--porcelain"]);
        assert!(
            !status.is_empty(),
            "the worktree must still show the same uncommitted change, not a clean tree"
        );
    }

    /// The loop publishes its OWN escalations as `agent.question` from the
    /// supervisor. Re-observing those must not inject anything, or the loop feeds
    /// itself its own tail — one hand-off per escalation, forever.
    #[test]
    fn the_loops_own_escalation_question_is_never_handed_back() {
        let agents = two_agents();
        let own = BrokerMessage::Question {
            agent_id: SUPERVISOR_AGENT_ID.to_string(),
            payload: crate::broker::messages::QuestionPayload {
                question: "feat-a is stalled on a danger permission prompt".to_string(),
            },
        };
        let (dispatcher, _) = run_loop(
            &agents,
            vec![pane(0, "/repo"), pane(2, "/repo-feat-a")],
            &[(0, IDLE_PANE), (2, IDLE_PANE)],
            vec![
                vec![row("feat-a", "working"), row("feat-b", "working")],
                vec![row("supervisor", "done")],
            ],
            &ScriptedMessages::new(vec![vec![own]]),
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                ..DriveConfig::default()
            },
        );

        assert!(
            dispatcher.literal_sends.is_empty(),
            "a supervisor-authored question is not a worker judgment call; sends were {:?}",
            dispatcher.literal_sends
        );
    }

    /// Spec: "Each status string maps to its phase" — one mapping owns the
    /// status→phase derivation, and anything outside the vocabulary the loop
    /// reacts to lands on `Other` rather than on a phase it acts upon. The
    /// mapping is exact, so a differently-cased status is unrecognized too.
    #[test]
    fn from_status_maps_each_broker_status_to_its_phase() {
        for (status, expected) in [
            ("working", WorkerPhase::Working),
            ("idle", WorkerPhase::Idle),
            ("blocked", WorkerPhase::Blocked),
            ("committed", WorkerPhase::Committed),
            ("verified", WorkerPhase::Verified),
            ("done", WorkerPhase::Done),
            ("booting", WorkerPhase::Other),
            ("DONE", WorkerPhase::Other),
            ("", WorkerPhase::Other),
        ] {
            assert_eq!(
                WorkerPhase::from_status(status),
                expected,
                "status {status:?} maps to the wrong phase"
            );
        }
    }

    /// Spec: "The completed and merge-candidate predicates equal the old
    /// arrays" — the decision sites now read the predicates, so each must answer
    /// exactly what its authoritative status array answers, for every status
    /// either array lists plus the ones neither does.
    #[test]
    fn phase_predicates_agree_with_the_status_arrays() {
        let statuses = AGENT_COMPLETE_STATUSES
            .iter()
            .chain(MERGE_CANDIDATE_STATUSES)
            .copied()
            .chain(["working", "idle", "blocked", "booting", ""]);
        for status in statuses {
            let phase = WorkerPhase::from_status(status);
            assert_eq!(
                phase.is_completed(),
                AGENT_COMPLETE_STATUSES.contains(&status),
                "is_completed disagrees with AGENT_COMPLETE_STATUSES for {status:?}"
            );
            assert_eq!(
                phase.is_merge_candidate(),
                MERGE_CANDIDATE_STATUSES.contains(&status),
                "is_merge_candidate disagrees with MERGE_CANDIDATE_STATUSES for {status:?}"
            );
        }
    }

    /// "Finished" is `done`/`verified` only, read from broker status rows. The
    /// statuses a worker occupies while AWAITING correction — `blocked`,
    /// `committed` — are deliberately NOT finished, or the correction loop could
    /// never reach the workers it exists to re-engage.
    #[test]
    fn finished_worker_is_decided_by_the_complete_status_set() {
        for status in AGENT_COMPLETE_STATUSES {
            let rows = vec![row("feat-a", status)];
            assert!(
                is_finished_worker(&rows, "feat-a"),
                "{status} means the worker has finished"
            );
        }
        for status in ["blocked", "committed", "working"] {
            assert!(
                !is_finished_worker(&[row("feat-a", status)], "feat-a"),
                "{status} is quiet, not finished — it stays nudge-eligible"
            );
        }
        assert!(
            !is_finished_worker(&[], "feat-a"),
            "a pane that has not published yet stays nudge-eligible"
        );
    }

    /// The correction-pass config every nudge-gate test below shares.
    fn correcting_config() -> DriveConfig {
        DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_secs(1),
            correction: CorrectionConfig {
                auto_loopback: true,
                ..CorrectionConfig::default()
            },
            ..DriveConfig::default()
        }
    }

    /// Spec: "A verified worker is never nudged" / "A working worker remains
    /// eligible for a nudge" — both decided from broker status, with both panes'
    /// captures held identical and unchanging across ticks so a pane-content diff
    /// could not possibly tell them apart. Both workers carry the same pending
    /// gate failure, so the ONLY discriminator is the broker status.
    ///
    /// This asserts the *observable* rule end-to-end, but note it cannot isolate
    /// which mechanism delivers it: `clear_completed` already drops a finished
    /// branch's correction state earlier in the same sweep, so the outcome holds
    /// even without the [`is_finished_worker`] gate. The gate is the explicit,
    /// order-independent encoding of the requirement; the discriminating test for
    /// the gate itself is [`finished_worker_is_decided_by_the_complete_status_set`].
    #[test]
    fn finished_worker_is_never_nudged_while_a_working_peer_still_is() {
        let agents = two_agents();
        let messages = ScriptedMessages::new(vec![vec![
            gate_feedback("feat-a", "testing", "two tests fail"),
            gate_feedback("feat-b", "testing", "two tests fail"),
        ]]);
        let (dispatcher, _) = run_loop(
            &agents,
            vec![pane(2, "/repo-feat-a"), pane(3, "/repo-feat-b")],
            // Identical, unchanging captures: only the broker status differs.
            &[(2, IDLE_PANE), (3, IDLE_PANE)],
            vec![vec![row("feat-a", "verified"), row("feat-b", "working")]],
            &messages,
            &correcting_config(),
        );

        assert!(
            !injected(&dispatcher, 2, "Supervisor gate"),
            "a finished (verified) worker receives no nudge; sends were {:?}",
            dispatcher.literal_sends
        );
        assert_text_then_separate_enter(&dispatcher, 3, "Supervisor gate 'testing' failed");
    }

    /// Spec: "A committed or blocked worker awaiting correction is still nudged".
    ///
    /// This is the case that separates *finished* from merely *quiet*. A worker
    /// that committed and is standing by, or one blocked and not polling its
    /// inbox, is exactly who `supervisor-correction-loop` exists to re-engage —
    /// gating those out would silently disable it for its dominant path.
    #[test]
    fn worker_awaiting_correction_is_nudged_even_when_committed_or_blocked() {
        let agents = two_agents();
        let messages = ScriptedMessages::new(vec![vec![
            gate_feedback("feat-a", "testing", "two tests fail"),
            gate_feedback("feat-b", "testing", "two tests fail"),
        ]]);
        let (dispatcher, _) = run_loop(
            &agents,
            vec![pane(2, "/repo-feat-a"), pane(3, "/repo-feat-b")],
            &[(2, IDLE_PANE), (3, IDLE_PANE)],
            vec![vec![row("feat-a", "committed"), row("feat-b", "blocked")]],
            &messages,
            &correcting_config(),
        );

        assert_text_then_separate_enter(&dispatcher, 2, "Supervisor gate 'testing' failed");
        assert_text_then_separate_enter(&dispatcher, 3, "Supervisor gate 'testing' failed");
    }
}
