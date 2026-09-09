//! The `--unattended` drive loop (`unattended-operation` capability).
//!
//! `cmd_supervisor` runs this loop in-process (in the foreground
//! `git paw start --unattended` process) after the tmux session is built. The
//! loop keeps a multi-agent supervisor wave moving with no human in the seat:
//! it polls on a ~15-second cadence, sweeps the supervisor pane (pane 0) and
//! every coding-agent pane, auto-approves classifier-safe permission prompts,
//! escalates risky/unknown prompts for later human review WITHOUT blocking the
//! rest of the wave, detects completion, and exits with a summary.
//!
//! The loop is the *sole* auto-approver for an unattended session — the
//! dashboard's auto-approve thread is disabled (see `main.rs`) so two approvers
//! never race on the same pane.
//!
//! # Battle-tested heuristics encoded here
//!
//! The v0.6.0 dogfood proved a handful of operator-loop rules are load-bearing;
//! they are encoded as normative behaviour so they survive in-tool:
//!
//! - **Act only on a LIVE prompt** — the prompt's structural markers (option
//!   glyphs / `Do you want to …` / `Esc to cancel`) at the capture's tail,
//!   over a window spanning a full multi-option prompt block
//!   ([`crate::supervisor::auto_approve::is_live_prompt`]); prompt-like text
//!   scrolled into history is ignored (D4).
//! - **Explicit per-pane capture** — one `tmux capture-pane` per pane, never a
//!   `for p in …` shell loop (D3).
//! - **Pane→agent resolution via `pane_current_path`** — never pane index or
//!   CLI-argument order ([`resolve_pane_agent`], D2).
//! - **Cover pane 0 but never pollute it** — the supervisor's own pane is swept
//!   and its safe prompts approved with the minimal option-digit + `Enter`
//!   keystrokes only; nothing is typed when pane 0 shows no live prompt
//!   (W15-3 / W15-13, D5).
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
//! fakes — no tmux, no real LLM, no interactive terminal. The production entry
//! point [`run_drive_loop`] wires the real implementations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use crate::broker::messages::{BrokerMessage, FeedbackPayload};
use crate::config::{CorrectionConfig, CustomCli, OnExhausted, resolve_submit_delay_ms};
use crate::error::PawError;
use crate::session::{self, SessionStatus};

use super::approval_gate::{approval_dedup_key, live_prompt_in_tail};
use super::approve::{KeyDispatcher, TmuxKeyDispatcher, approval_keystrokes};
use super::auto_approve::{
    ProtectedPaths, detect_prompt_shape, extract_command_slice, extract_path_from_file_prompt,
    is_dangerous, is_git_dir_write, is_live_prompt, is_managed_script_invocation,
    is_protected_path_violation, is_safe_command, is_scratch_rm, is_worktree_dev_test_op,
    is_worktree_file_op, is_worktree_git_op, normalize_command, select_option_index,
};
use super::claim::PaneClaim;
use super::poll::{AgentStatusRow, fetch_status_over_http};

/// Poll cadence: the loop re-sweeps every pane on approximately this interval.
pub const POLL_INTERVAL: Duration = Duration::from_secs(15);

/// Heartbeat window: after approximately this long with no completion the loop
/// re-engages the human by exiting with a status summary rather than running
/// forever silently.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_mins(25);

/// Alert dedup window: a repeated `(agent_id, shape)` escalation collapses to a
/// single alert within this window (W15-19).
pub const DEDUP_WINDOW: Duration = Duration::from_mins(5);

/// Orchestration-sweep cadence: the loop nudges the orchestrator pane to run an
/// orchestration sweep on approximately this interval — deliberately a multiple
/// of [`POLL_INTERVAL`] (20×), so the fast approval sweep is unchanged and the
/// orchestrator is not prompt-stormed by a per-tick nudge.
pub const ORCHESTRATION_NUDGE_INTERVAL: Duration = Duration::from_mins(5);

/// The `agent_id` under which the supervisor's own pane (pane 0) is tracked.
pub const SUPERVISOR_AGENT_ID: &str = "supervisor";

/// Pane index of the supervisor's own pane in the supervisor layout.
const SUPERVISOR_PANE_INDEX: usize = 0;

/// Pane index of the dashboard TUI in the supervisor layout. Never swept for
/// approval — it is a `git-paw __dashboard` process, not an agent CLI.
const DASHBOARD_PANE_INDEX: usize = 1;

/// Learning category recorded when the loop absorbs friction it could not
/// auto-approve (per `learnings-supervisor-observation-channel`).
const LEARNING_CATEGORY: &str = "tooling_friction";

// ---------------------------------------------------------------------------
// Roster + pane resolution
// ---------------------------------------------------------------------------

/// A coding agent in the session, used to resolve a pane to its agent by
/// working directory. The supervisor (pane 0) and dashboard (pane 1) are not
/// listed here — they resolve to the repo root by pane index.
#[derive(Debug, Clone)]
pub struct AgentPane {
    /// Broker agent id (slugified branch name).
    pub agent_id: String,
    /// Absolute worktree root the agent's pane runs in (`pane_current_path`).
    pub worktree_path: PathBuf,
}

/// The role a swept pane resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneRole {
    /// Pane 0 — the supervisor's own pane.
    Supervisor,
    /// Pane 1 — the dashboard TUI; never swept for approval.
    Dashboard,
    /// A coding-agent pane, resolved by `pane_current_path`. Carries the
    /// resolved agent id.
    Coding(String),
    /// The pane's path matched no known agent and it is not pane 0/1.
    Unknown,
}

impl PaneRole {
    /// The `agent_id` used for broker alerts and the summary, or `None` for the
    /// dashboard pane (which is never acted on).
    #[must_use]
    pub fn agent_id(&self) -> Option<&str> {
        match self {
            PaneRole::Supervisor => Some(SUPERVISOR_AGENT_ID),
            PaneRole::Coding(id) => Some(id.as_str()),
            PaneRole::Unknown => Some("unknown"),
            PaneRole::Dashboard => None,
        }
    }
}

/// Resolves a pane to its agent role.
///
/// Coding agents resolve by matching `pane_current_path` against a known
/// `worktree_path` (canonicalised where possible) — NEVER by pane index or
/// CLI-argument order, because pane indices are neither alphabetical nor
/// argument-ordered and drift on layout changes (D2). Pane 0 resolves to the
/// supervisor and pane 1 to the dashboard; both run at the repo root, so the
/// index disambiguates the two only for panes that did not match a coding
/// agent's distinct worktree path.
#[must_use]
pub fn resolve_pane_agent(
    pane_index: usize,
    pane_current_path: &str,
    agents: &[AgentPane],
) -> PaneRole {
    // Coding agents first, by working directory — index-independent so a
    // renumbered layout still attributes the pane correctly.
    let candidate = canonical_or_owned(Path::new(pane_current_path));
    for agent in agents {
        if paths_match(&candidate, &agent.worktree_path) {
            return PaneRole::Coding(agent.agent_id.clone());
        }
    }
    // Not a coding worktree: pane 0 is the supervisor, pane 1 the dashboard.
    match pane_index {
        SUPERVISOR_PANE_INDEX => PaneRole::Supervisor,
        DASHBOARD_PANE_INDEX => PaneRole::Dashboard,
        _ => PaneRole::Unknown,
    }
}

/// Returns the pane index of the orchestrator (supervisor CLI) pane, or `None`
/// when the session has no supervisor pane.
///
/// Resolution reuses [`resolve_pane_agent`], so the orchestrator is identified
/// the same structural way every other pane is. There is deliberately no
/// LLM-liveness probe and no broker heartbeat: the session layout already fixes
/// pane 0 as the supervisor, so pane **presence** is the signal.
#[must_use]
pub fn orchestrator_pane_index(panes: &[PaneInfo], agents: &[AgentPane]) -> Option<usize> {
    panes
        .iter()
        .find(|p| {
            resolve_pane_agent(p.pane_index, &p.pane_current_path, agents) == PaneRole::Supervisor
        })
        .map(|p| p.pane_index)
}

/// Whether an orchestrator (supervisor CLI) pane is present — the predicate that
/// selects between the hand-to-orchestrator path and the broker-only fallback.
///
/// A pure `--unattended` run with no supervisor pane returns `false`, and the
/// loop's escalation behaviour is then identical to the pre-orchestrator
/// version: a uniform broker review item and no pane injection at all.
#[must_use]
pub fn orchestrator_present(panes: &[PaneInfo], agents: &[AgentPane]) -> bool {
    orchestrator_pane_index(panes, agents).is_some()
}

/// Canonicalises `p`, falling back to the path as-given when it cannot be
/// resolved (e.g. it does not exist in a unit test).
fn canonical_or_owned(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Compares a (possibly canonicalised) pane path against a worktree root,
/// tolerating symlink differences by canonicalising the root too.
fn paths_match(pane_path: &Path, worktree_root: &Path) -> bool {
    if pane_path == worktree_root {
        return true;
    }
    canonical_or_owned(worktree_root) == *pane_path
}

// ---------------------------------------------------------------------------
// Prompt classification (the auto-approve-classifier the loop consumes)
// ---------------------------------------------------------------------------

/// A prompt's three-way safety verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptVerdict {
    /// Safe to auto-approve. Carries the 1-based option index to select and the
    /// name of the rule that matched (for the audit log).
    Safe {
        /// 1-based option index to select at the prompt.
        option_index: u8,
        /// Human-readable name of the classifier rule that matched.
        matched: String,
    },
    /// A curated danger-list match — escalate, never auto-approve.
    Danger,
    /// An approval prompt whose command class is unrecognised — escalate.
    Unknown,
}

impl PromptVerdict {
    /// Short label for the exit summary / broker alert.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            PromptVerdict::Safe { .. } => "safe",
            PromptVerdict::Danger => "danger",
            PromptVerdict::Unknown => "unknown",
        }
    }
}

/// Classifies a live prompt capture into a [`PromptVerdict`], mirroring the
/// danger-first decision order the dashboard poll loop uses
/// ([`crate::supervisor::poll`]): danger-list plus the `.git/`-write rule
/// first (terminal escalate), then the scratch-`rm` exception, an invocation
/// of one of git-paw's own managed helper scripts, the worktree-confined
/// `git add`/`git commit` pre-approval, the worktree-confined dev-test
/// shapes, the shell whitelist, and finally the worktree file-op boundary.
/// Anything unmatched is [`PromptVerdict::Unknown`].
///
/// `worktree_root` is `None` for panes without a known worktree (the supervisor
/// pane), which suppresses the worktree-scoped rules for that pane.
///
/// The slice is [`normalize_command`]-normalised first, so a routine command
/// wrapped in a gate-reporting exit-code probe classifies as the bare command it
/// is. Every rule below — danger-list included — runs against that normalised
/// slice, so the wrapper can never downgrade an escalation.
///
/// The option index is resolved only once a safe rule has matched, so a prompt
/// offering a durable "don't ask again" grant takes it for any safe /
/// worktree-confined command rather than only a read-mostly verb.
#[must_use]
pub fn classify_prompt(
    captured: &str,
    whitelist: &[String],
    worktree_root: Option<&Path>,
    approve_worktree_writes: bool,
    protected: &ProtectedPaths,
) -> PromptVerdict {
    let slice = prompt_command_slice(captured);

    // Danger-first precedence: a curated danger-list match — or a write
    // targeting the operator's protected config/memory territory
    // (`agent-memory-isolation`) — is a terminal escalate that overrides any
    // whitelist / safe-by-pattern match.
    if is_dangerous(&slice)
        || is_protected_path_violation(captured, &slice, protected, worktree_root)
        || is_git_dir_write(captured, &slice, worktree_root)
    {
        return PromptVerdict::Danger;
    }

    // The safe rules, in the poll loop's precedence order:
    // - the scratch-path exception (an `rm -rf` whose every target is repo/OS
    //   scratch);
    // - an invocation of one of git-paw's own bundled helper scripts
    //   (GP-02b);
    // - worktree-confined `git add` / `git commit` pre-approval;
    // - worktree-confined dev-test shapes (`bash -n`, non-recursive chmod,
    //   mktemp, interpreter-of-worktree-script);
    // - the shell whitelist (read-mostly verbs + configured safe commands);
    // - a write/edit/create prompt whose target resolves inside the worktree.
    //
    // The worktree-scoped rules are suppressed for panes without a known
    // worktree (the supervisor pane).
    let matched = if is_scratch_rm(&slice) {
        Some("scratch-rm".to_string())
    } else if is_managed_script_invocation(&slice, worktree_root) {
        Some("managed-script".to_string())
    } else if worktree_root.is_some_and(|root| is_worktree_git_op(&slice, root)) {
        Some("worktree-git".to_string())
    } else if worktree_root.is_some_and(|root| is_worktree_dev_test_op(&slice, root)) {
        Some("worktree-dev-test".to_string())
    } else if let Some(entry) = first_whitelist_match(&slice, whitelist) {
        Some(entry)
    } else if worktree_root
        .is_some_and(|root| is_worktree_file_op(captured, root, approve_worktree_writes))
    {
        Some("worktree-file-op".to_string())
    } else {
        None
    };

    match matched {
        Some(matched) => PromptVerdict::Safe {
            option_index: select_option_index(detect_prompt_shape(captured), &slice, true),
            matched,
        },
        None => PromptVerdict::Unknown,
    }
}

/// Returns the normalised command slice a capture is classified against: the
/// prompted command text ([`extract_command_slice`], falling back to the whole
/// capture when no command header is present), with the gate-reporting wrappers
/// stripped by [`normalize_command`].
///
/// [`classify_prompt`] derives its slice through this function, so exposing it
/// lets the hidden `git paw __classify` subcommand resolve the option index for
/// a non-[`PromptVerdict::Safe`] verdict from the exact same slice the
/// classification saw, instead of re-deriving it.
#[must_use]
pub fn prompt_command_slice(captured: &str) -> String {
    normalize_command(&extract_command_slice(captured).unwrap_or_else(|| captured.to_string()))
}

/// Returns the first whitelist entry that matches any line of `captured`, using
/// the shared prefix/word-boundary semantics of
/// [`is_safe_command`]. Mirrors the poll loop's private helper.
fn first_whitelist_match(captured: &str, whitelist: &[String]) -> Option<String> {
    for line in captured.lines() {
        for entry in whitelist {
            if is_safe_command(line, std::slice::from_ref(entry)) {
                return Some(entry.clone());
            }
        }
    }
    None
}

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

/// Renders the orchestration-sweep nudge injected into the orchestrator's pane
/// on the [`ORCHESTRATION_NUDGE_INTERVAL`] cadence.
///
/// The nudge carries no per-agent detail on purpose: it asks the orchestrator to
/// re-derive the whole picture from the broker (spawn order, merge sequencing,
/// blocked workers) rather than pushing the loop's mechanical view of it, which
/// is what keeps judgment in the smart model and mere triggering in the loop.
#[must_use]
pub fn orchestration_nudge_text() -> String {
    one_line_capped(
        "Run an orchestration sweep now: re-read the broker state and reconsider \
         dependency-aware spawn order, merge sequencing, and any blocked or \
         non-converging worker. Act on what you find; escalate to the human only \
         what you genuinely cannot decide.",
    )
}

/// Renders the task text injected into the orchestrator's pane when a worker
/// asks an ambiguous question.
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

/// Renders the task text injected into the orchestrator's pane when a worker
/// publishes an artifact at a [`MERGE_CANDIDATE_STATUSES`] status — the point at
/// which a **merge decision** becomes live.
///
/// A committed or done branch is what the supervisor's verify-then-merge
/// sequencing keys off, so this is the loop triggering that decision rather than
/// waiting for the orchestrator to notice the artifact on its own.
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
    /// The framing follows the verdict, because the two kinds ask the human for
    /// different things. A classifier verdict means a pane is sitting on a
    /// permission prompt that could not be auto-approved — review the pane. A
    /// correction-loop verdict means a branch is not converging; there is no
    /// prompt to review, so the text must not claim one.
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
            _ => format!(
                "{} is stalled on a {} permission prompt the unattended loop could not \
                 auto-approve; please review the pane and decide manually. Command: {}",
                self.agent_id, self.verdict, self.command
            ),
        }
    }

    /// Renders the task text injected into the orchestrator's pane for this
    /// escalation.
    ///
    /// Where [`Self::question`] asks a *human* to review a pane, this asks the
    /// *orchestrator* to decide and act — it can approve the escalated pane,
    /// publish an answer, or declare the branch unrecoverable itself. The human
    /// framing is deliberately not reused: telling a model to "review the pane
    /// and decide manually" invites it to wait for someone else.
    #[must_use]
    pub fn handoff_text(&self) -> String {
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

/// One pane in the session, as reported by `tmux list-panes`.
#[derive(Debug, Clone)]
pub struct PaneInfo {
    /// The pane's tmux index within window 0.
    pub pane_index: usize,
    /// The pane's `pane_current_path` (working directory).
    pub pane_current_path: String,
}

/// Enumerates the session's panes (one explicit `tmux list-panes` per sweep).
pub trait PaneEnumerator {
    /// Returns every pane in `session`.
    fn list_panes(&self, session: &str) -> Vec<PaneInfo>;
}

/// Captures a single pane's content (one explicit `tmux capture-pane` per
/// pane — never a shell `for` loop).
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
    fn log_approval(&mut self, agent_id: &str, matched: &str);
    /// Surfaces a risky/unknown prompt for later human review.
    fn escalate(&mut self, escalation: &Escalation);
}

/// Records qualitative learnings via `sweep.sh learn` (never raw curl).
pub trait LearningSink {
    /// Records a learning. `body` is a JSON object string.
    fn record(&mut self, category: &str, title: &str, body: &str);
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
    /// Publishes alerts.
    pub alerts: &'a mut dyn AlertSink,
    /// Records learnings.
    pub learnings: &'a mut dyn LearningSink,
    /// Performs branch-refresh git actions and worker notification.
    pub refresher: &'a mut dyn BranchRefresher,
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
    /// Cadence of the orchestration-sweep nudge injected into the orchestrator
    /// pane — deliberately longer than `poll_interval`.
    pub orchestration_nudge_interval: Duration,
    /// Effective safe-command whitelist.
    pub whitelist: Vec<String>,
    /// Whether in-worktree write/edit/create prompts auto-approve.
    pub approve_worktree_writes: bool,
    /// Protected-path set for the operator config/memory danger rule
    /// (`agent-memory-isolation`).
    pub protected_paths: ProtectedPaths,
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
    /// CLI running in the orchestrator's pane (pane 0), used to resolve its
    /// nudge settle delay. Empty when unknown, which resolves to
    /// [`crate::DEFAULT_SUBMIT_DELAY_MS`] via [`resolve_submit_delay_ms`].
    pub supervisor_cli: String,
}

impl Default for DriveConfig {
    fn default() -> Self {
        Self {
            poll_interval: POLL_INTERVAL,
            heartbeat: HEARTBEAT_INTERVAL,
            dedup_window: DEDUP_WINDOW,
            orchestration_nudge_interval: ORCHESTRATION_NUDGE_INTERVAL,
            whitelist: Vec::new(),
            approve_worktree_writes: true,
            protected_paths: ProtectedPaths::default(),
            correction: CorrectionConfig::default(),
            learnings_enabled: false,
            broker_log_hint: None,
            learnings_hint: None,
            branch_refresh_enabled: false,
            clis: HashMap::new(),
            supervisor_cli: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

/// Runs the drive loop against the injected dependencies until an exit
/// condition (completion or heartbeat) is reached, then returns the summary.
///
/// Each poll iteration:
/// 1. Enumerates panes (one `list-panes`), resolving each to its agent by
///    `pane_current_path`.
/// 2. Captures each pane explicitly (one `capture-pane` per pane) and acts only
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
    let coding_ids: Vec<String> = agents.iter().map(|a| a.agent_id.clone()).collect();
    let worktree_by_id: HashMap<String, PathBuf> = agents
        .iter()
        .map(|a| (a.agent_id.clone(), a.worktree_path.clone()))
        .collect();

    let mut dedup = DedupWindow::new(config.dedup_window);
    let mut escalations: Vec<Escalation> = Vec::new();
    let mut correction = CorrectionState::new();
    let mut exhausted: Vec<ExhaustedBranch> = Vec::new();
    // Highest-progressed `WorkerPhase` ever observed per agent (GP-05), so a
    // generic `working` heartbeat republished after a `verified`/`done`
    // artifact event can never regress the exit summary's resolved state.
    let mut resolved_phase: HashMap<String, WorkerPhase> = HashMap::new();

    let start = deps.clock.now();
    // Nudge timer starts at the loop's start, so the first orchestration nudge
    // lands one full cadence in rather than on the opening tick.
    let mut last_orchestration_nudge = start;
    // The orchestrator's nudge settle delay never changes mid-run, so it is
    // resolved once here rather than on every tick.
    let orchestrator_settle_delay = Duration::from_millis(resolve_submit_delay_ms(
        &config.supervisor_cli,
        &config.clis,
    ));

    // The loop is an expression that breaks with the terminal `(outcome,
    // latest_status)`: every exit path assigns both, so there are no
    // pre-initialised placeholders to leave dead.
    let (outcome, latest_status) = loop {
        // --- Session-instance check (GP-06), before any action this tick -----
        if !instance.is_current() {
            break (DriveOutcome::SessionTornDown, Vec::new());
        }

        // --- Sweep every pane (pane-keyed, explicit per-pane capture) --------
        // The pane list is bound before the sweep because the orchestrator's
        // presence is read from it: pane presence IS the orchestrator signal,
        // and it is re-resolved every sweep so a supervisor pane that dies
        // mid-wave falls back to broker-only escalation from then on.
        let panes = deps.enumerator.list_panes(session);
        let ctx = SweepContext {
            session,
            repo_root,
            orchestrator: orchestrator_pane_index(&panes, agents),
            orchestrator_settle_delay,
        };
        let mut pane_by_agent: HashMap<String, usize> = HashMap::new();
        for pane in panes {
            let role = resolve_pane_agent(pane.pane_index, &pane.pane_current_path, agents);
            let Some(agent_id) = role.agent_id() else {
                continue; // dashboard pane — never acted on
            };
            let agent_id = agent_id.to_string();
            pane_by_agent.insert(agent_id.clone(), pane.pane_index);

            let worktree_root = worktree_by_id.get(&agent_id).map(PathBuf::as_path);
            if let Some(escalation) = sweep_pane(
                ctx,
                &pane,
                &agent_id,
                worktree_root,
                deps,
                config,
                &mut dedup,
            ) {
                escalations.push(escalation);
            }
        }

        // --- Fetch status once this tick, BEFORE dispatching any nudge -------
        // (GP-16): the freshest possible completion signal gates every nudge
        // dispatched below, including `observe_worker_messages`' hand-offs,
        // which used to run on a stale (pre-fetch) view.
        let latest_status = deps.status.fetch();
        // A branch that reached a terminal PASS leaves the correction cycle,
        // so an unrelated later failure starts from a fresh budget.
        correction.clear_completed(&latest_status);

        // Track the most-progressed phase ever seen per agent (GP-05): only
        // ever raises the recorded phase, never lowers it, so a later generic
        // `working` heartbeat cannot erase an earlier `verified`/`done` signal.
        for row in &latest_status {
            let phase = WorkerPhase::from_status(&row.status);
            resolved_phase
                .entry(row.agent_id.clone())
                .and_modify(|best| {
                    if phase.rank() > best.rank() {
                        *best = phase;
                    }
                })
                .or_insert(phase);
        }

        // Wind-down guard (GP-16): once this tick's own fetch shows the wave
        // complete, the loop is about to exit — no nudge is dispatched below,
        // on this tick or any later one (there is no later one; the loop
        // breaks at the bottom of this same tick).
        let winding_down = detect_completion(&latest_status, &coding_ids).is_some();

        // --- Observe newly published worker messages -------------------------
        let merged_bases = observe_worker_messages(
            ctx,
            &coding_ids,
            deps,
            &mut correction,
            &config.correction.gate_tags,
            winding_down,
        );

        // --- Branch refresh (post-merge, opt-in) ------------------------------
        trigger_branch_refresh(
            config.branch_refresh_enabled,
            ctx,
            agents,
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

        // --- Orchestration-sweep nudge (longer cadence) ----------------------
        // Placed after the completion check so a finished wave is never nudged.
        if let Some(pane_index) = ctx.orchestrator
            && deps.clock.now().duration_since(last_orchestration_nudge)
                >= config.orchestration_nudge_interval
            // Suppressed while the orchestrator is mid-response: the timer is
            // advanced only on a dispatched nudge, so a busy orchestrator has
            // its nudge deferred to the next tick rather than skipped for a
            // whole cadence.
            && hand_to_orchestrator(deps, ctx, pane_index, &orchestration_nudge_text())
        {
            last_orchestration_nudge = deps.clock.now();
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

        // --- Heartbeat check -------------------------------------------------
        if deps.clock.now().duration_since(start) >= config.heartbeat {
            break (DriveOutcome::Heartbeat, latest_status);
        }

        deps.clock.sleep(config.poll_interval);
    };

    // --- Wind-down synthesis learning (deduped against in-session friction) --
    if !escalations.is_empty() {
        deps.learnings.record(
            LEARNING_CATEGORY,
            "unattended wave wind-down synthesis",
            &winddown_learning_body(outcome, &escalations),
        );
    }

    // Wave-level "N/M branches merged" line (GP-05): printed for a completed
    // wave in addition to the per-agent list — the fallback when a specific
    // agent's own resolution stayed sparse (e.g. its only signal was the
    // supervisor's own verdict, never its own `verified`/`done` status), and a
    // useful roll-up otherwise.
    let merged_summary =
        (outcome == DriveOutcome::Completed && !coding_ids.is_empty()).then(|| {
            let merged = coding_ids
                .iter()
                .filter(|id| resolved_phase.get(*id).is_some_and(|p| p.rank() == 2))
                .count();
            format!("{merged}/{} branches merged", coding_ids.len())
        });

    DriveSummary {
        outcome,
        agent_states: resolved_agent_states(&coding_ids, &latest_status, &resolved_phase),
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
/// Three worker signals matter here, and each is a different kind of live
/// judgment call:
/// - **`agent.feedback`** carrying a `[<gate>]` tag is a FAILING gate verdict; it
///   marks the branch for the correction pass. (A passing gate publishes
///   `agent.verified`, which the `/status` sweep handles instead.)
/// - **`agent.question`** is an ambiguous question. The worker published it
///   precisely *because* it could not decide, so the message itself is the
///   ambiguity signal — the loop adds no classifier of its own — and it is handed
///   to the orchestrator to answer.
/// - **`agent.artifact`** at a [`MERGE_CANDIDATE_STATUSES`] status is the point a
///   **merge decision** becomes live, so it is handed over for verification and
///   merge sequencing.
///
/// Every arm filters on `coding_ids` FIRST. The loop's own escalations go out as
/// `agent.question` from the supervisor, so an unfiltered question arm would feed
/// the loop its own tail — injecting each escalation twice and, worse, once per
/// message it generated. Handing off is fire-and-forget throughout: no arm blocks
/// the wave on the orchestrator's response.
///
/// `winding_down` (GP-16) suppresses only the hand-off keystrokes (the
/// `Question`/`Artifact` arms) once this tick's own status fetch has already
/// shown the wave complete — the loop is about to exit and typing a fresh
/// task prompt into the orchestrator's pane on the way out would just leave
/// unsubmitted text behind. The `Feedback` bookkeeping and the
/// `AdvancedMain` merge-base collection below are unaffected: neither sends a
/// keystroke, and both stay useful even on the final tick (a correction
/// record left pending is harmless once the wave is done, and a merge base
/// observed on the final tick is still a real merge that happened).
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
    ctx: SweepContext<'_>,
    coding_ids: &[String],
    deps: &mut DriveDeps<'_>,
    correction: &mut CorrectionState,
    gate_tags: &[String],
    winding_down: bool,
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
        match &msg {
            BrokerMessage::Feedback { payload, .. } => {
                if let Some(gate) = gate_failure(payload, gate_tags) {
                    correction.mark_gate_failure(
                        agent_id,
                        reengagement_text(&gate, &payload.errors.join("; ")),
                    );
                }
            }
            BrokerMessage::Question { payload, .. } => {
                if !winding_down && let Some(pane_index) = ctx.orchestrator {
                    let text = question_handoff_text(agent_id, &payload.question);
                    hand_to_orchestrator(deps, ctx, pane_index, &text);
                }
            }
            BrokerMessage::Artifact { payload, .. } => {
                if !winding_down
                    && let Some(pane_index) = ctx.orchestrator
                    && WorkerPhase::from_status(&payload.status).is_merge_candidate()
                {
                    let text = merge_handoff_text(agent_id, &payload.status);
                    hand_to_orchestrator(deps, ctx, pane_index, &text);
                }
            }
            _ => {}
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
) {
    if !enabled {
        return;
    }
    for base in merged_bases {
        refresh_branches_after_merge(ctx, agents, pane_by_agent, latest_status, base, deps);
    }
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
fn refresh_branches_after_merge(
    ctx: SweepContext<'_>,
    agents: &[AgentPane],
    pane_by_agent: &HashMap<String, usize>,
    latest_status: &[AgentStatusRow],
    base: &str,
    deps: &mut DriveDeps<'_>,
) {
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
            deps.refresher.notify(&agent.agent_id, base);
        }
    }
}

/// The tmux session, the repository root, and the orchestrator pane resolved
/// for the current sweep.
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
    /// Orchestrator (supervisor CLI) pane index, or `None` when the session has
    /// no supervisor pane — the broker-only fallback.
    orchestrator: Option<usize>,
    /// The orchestrator pane's resolved per-CLI nudge settle delay
    /// ([`resolve_submit_delay_ms`]), bundled here so every
    /// [`hand_to_orchestrator`] call site resolves it from one place.
    orchestrator_settle_delay: Duration,
}

/// Records `escalation` on the broker — **uniformly**, whether or not an
/// orchestrator is running — and, when an orchestrator pane is present, ALSO
/// hands it to that pane as a task prompt.
///
/// The split matters: the broker record is the durable, drainable review item a
/// human reads in a no-supervisor run, while the injection actively *triggers* a
/// present orchestrator instead of relying on it to poll an inbox it never
/// reads. With no orchestrator pane the second step does not happen at all, so
/// the escalation path is identical to the pre-orchestrator version.
fn record_escalation(deps: &mut DriveDeps<'_>, ctx: SweepContext<'_>, escalation: &Escalation) {
    deps.alerts.escalate(escalation);
    if let Some(pane_index) = ctx.orchestrator {
        hand_to_orchestrator(deps, ctx, pane_index, &escalation.handoff_text());
    }
}

/// Sweeps one pane: captures it, and when a LIVE prompt is in the tail either
/// approves it (audit-logged first, then the option digit and `Enter` as two
/// separate keystrokes, gated on a fresh re-confirm) or records a deduped
/// escalation without blocking the rest of the wave.
///
/// Returns the escalation when one was newly recorded this sweep, so the caller
/// accumulates it for the exit summary. A pane with no live prompt — mere
/// narration, or a resolved prompt scrolled into history — is left untouched;
/// that is also what keeps pane 0 quiet. The one exception is an IDLE pane whose
/// input box still holds an unsubmitted directive: that gets a follow-up `Enter`
/// (and nothing else) via [`submit_buffered_input`], because no other part of the
/// loop can see a stall that never became a prompt.
fn sweep_pane(
    ctx: SweepContext<'_>,
    pane: &PaneInfo,
    agent_id: &str,
    worktree_root: Option<&Path>,
    deps: &mut DriveDeps<'_>,
    config: &DriveConfig,
    dedup: &mut DedupWindow,
) -> Option<Escalation> {
    let session = ctx.session;
    let capture = deps.capturer.capture(session, pane.pane_index);
    if !is_live_prompt(&capture) {
        let _ = submit_buffered_input(
            deps.capturer,
            deps.dispatcher,
            session,
            pane.pane_index,
            &capture,
        );
        return None;
    }

    let verdict = classify_prompt(
        &capture,
        &config.whitelist,
        worktree_root,
        config.approve_worktree_writes,
        &config.protected_paths,
    );

    match verdict {
        PromptVerdict::Safe {
            option_index,
            matched,
        } => {
            // Log the approval BEFORE the keystrokes go out.
            deps.alerts.log_approval(agent_id, &matched);
            // This minimal sequence is what makes pane-0 approval safe: it
            // consumes only the prompt, never landing free text or a stray
            // newline in the supervisor's prompt box (W15-13).
            let _ = send_approval(
                deps.capturer,
                deps.dispatcher,
                ctx.repo_root,
                session,
                pane.pane_index,
                option_index,
            );
            None
        }
        PromptVerdict::Danger | PromptVerdict::Unknown => {
            // Non-blocking escalation, deduped on (agent_id, shape).
            let key = approval_dedup_key(agent_id, &capture);
            if !dedup.should_emit(&key, deps.clock.now()) {
                return None;
            }
            let escalation = Escalation {
                agent_id: agent_id.to_string(),
                verdict: verdict.label().to_string(),
                command: dedup_shape(&capture),
            };
            record_escalation(deps, ctx, &escalation);
            // Opportunistic learning: the loop absorbed friction it could not
            // auto-approve.
            deps.learnings.record(
                LEARNING_CATEGORY,
                "unattended loop escalated a prompt",
                &friction_learning_body(&escalation),
            );
            Some(escalation)
        }
    }
}

/// The mutable slice of one sweep the correction pass operates on, bundled so
/// [`run_correction_pass`] keeps a small signature.
struct CorrectionPass<'a> {
    /// tmux session + orchestrator pane for this sweep.
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
            record_escalation(deps, pass.ctx, &escalation);
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
        record_escalation(deps, pass.ctx, &escalation);
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

/// Takes the pane's exclusive approval claim, re-confirms a live prompt with a
/// fresh capture immediately before the send, then dispatches the option digit
/// followed by a separate `Enter`.
///
/// Returns `Ok(true)` when the keystrokes were sent and `Ok(false)` when the
/// send was skipped — either another approver holds the pane's claim, or the
/// prompt cleared between the sweep and the send (no stray input either way).
/// This gate applies to EVERY pane including pane 0 — the drive loop is the
/// sole approver of classifier-safe prompts for an unattended session and is
/// explicitly permitted to clear the supervisor's own safe prompts, but only
/// with these minimal keystrokes.
///
/// The claim is what makes "sole approver" enforced rather than conventional: a
/// pane already being driven by the orchestrator's `sweep.sh approve` (a
/// separate process) is skipped this tick and retried on a later one, so the
/// two can never both land a keystroke. It is held across the re-confirm AND
/// the dispatch, and released by [`PaneClaim`]'s `Drop` on every return path
/// including an error or a panic.
fn send_approval(
    capturer: &dyn PaneCapture,
    dispatcher: &mut dyn KeyDispatcher,
    repo_root: &Path,
    session: &str,
    pane_index: usize,
    option_index: u8,
) -> Result<bool, PawError> {
    let Some(_claim) = PaneClaim::try_acquire(repo_root, pane_index) else {
        return Ok(false); // another approver owns this pane — skip, never wait
    };
    let capture = capturer.capture(session, pane_index);
    if !live_prompt_in_tail(&capture) {
        return Ok(false);
    }
    for key in approval_keystrokes(option_index) {
        dispatcher
            .send_key(session, pane_index, &key)
            .map_err(|e| PawError::TmuxError(format!("send-keys {key} failed: {e}")))?;
    }
    Ok(true)
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
/// stale input line — the drive loop's only actuator, made reliable (GP-15).
///
/// Gated on a fresh capture taken immediately before the send satisfying
/// `ready` — the TOCTOU guard every free-text path shares, so no caller can
/// skip it. Callers differ only in how strict `ready` is: the worker
/// re-engagement needs the pane merely to be free of a live permission
/// prompt, while an orchestrator hand-off additionally requires it not to be
/// mid-response ([`orchestrator_ready`]).
///
/// The whole operation — initial send through every recovery attempt — runs
/// under `pane_index`'s exclusive [`PaneClaim`], so a `C-u` this function
/// sends is always clearing a line THIS call itself just typed, never
/// agent-authored input another approver is mid-way through (task 1.3): a
/// pane already claimed by a concurrent approver is skipped this tick
/// ([`NudgeOutcome::Skipped`]) rather than waited on, matching every other
/// claim-gated send in this module.
///
/// The send-verify-recover sequence:
/// 1. Send the text ([`KeyDispatcher::send_text`], literal), wait
///    `settle_delay` (the per-CLI settle delay [`resolve_submit_delay_ms`]
///    resolves, the same resolver the boot prompt uses), then send a
///    SEPARATE `Enter` — never a single combined text+Enter.
/// 2. Capture the pane and check whether `text` is still sitting on the input
///    box ([`input_box_text`]). If it is not, the nudge is
///    [`NudgeOutcome::Delivered`].
/// 3. Otherwise recover: clear the line (`C-u`), re-send the text, wait
///    `settle_delay`, send `Enter` again — up to [`NUDGE_RECOVERY_ATTEMPTS`]
///    times. A bare second `Enter` alone is a proven no-op against a stale
///    input line, so recovery always re-establishes the input rather than
///    repeating just the `Enter`.
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
    for attempt in 0..=NUDGE_RECOVERY_ATTEMPTS {
        if attempt > 0 {
            // Recovery: clear the stale line THIS call put there (never
            // reached on agent-authored input — the claim above is what
            // makes that guarantee hold) before re-typing.
            deps.dispatcher
                .send_key(session, pane_index, "C-u")
                .map_err(|e| PawError::TmuxError(format!("send-keys C-u failed: {e}")))?;
        }
        deps.dispatcher
            .send_text(session, pane_index, &body)
            .map_err(|e| PawError::TmuxError(format!("send-keys -l failed: {e}")))?;
        deps.clock.sleep(settle_delay);
        deps.dispatcher
            .send_key(session, pane_index, &submit)
            .map_err(|e| PawError::TmuxError(format!("send-keys {submit} failed: {e}")))?;

        let verify = deps.capturer.capture(session, pane_index);
        if input_box_text(&verify).as_deref() != Some(text) {
            return Ok(NudgeOutcome::Delivered);
        }
    }

    deps.alerts.escalate(&Escalation {
        agent_id: agent_id.to_string(),
        verdict: NUDGE_WEDGED_VERDICT.to_string(),
        command: one_line_capped(text),
    });
    Ok(NudgeOutcome::Wedged)
}

/// Extracts the text sitting in a pane's input box, or `None` when the capture
/// shows no input box or the box is empty.
///
/// The input box is the bordered line carrying the CLI's prompt sigil (`│ > …`)
/// — the same landmark [`crate::tmux::readiness::CLI_READY_MARKERS`] uses to
/// recognise a launched CLI. The LAST such line wins, because a pane's live
/// input box is always its most recent one. A numbered option line is never
/// read as buffered input, so a boxed prompt option cannot be mistaken for a
/// stranded directive.
fn input_box_text(capture: &str) -> Option<String> {
    let text = capture.lines().rev().find_map(|raw| {
        let inner = raw.trim().strip_prefix('│')?.trim_end_matches('│').trim();
        let text = inner
            .strip_prefix('>')
            .or_else(|| inner.strip_prefix('❯'))?
            .trim();
        Some(text.to_string())
    })?;
    let mut chars = text.chars();
    let is_option = matches!(chars.next(), Some(c) if c.is_ascii_digit())
        && matches!(chars.next(), Some('.' | ')'));
    (!text.is_empty() && !is_option).then_some(text)
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
    !live_prompt_in_tail(capture)
        && !pane_is_mid_response(capture)
        && input_box_text(capture).is_some()
}

/// Submits a directive stranded in an idle pane's input box by sending a
/// follow-up `Enter`, the same keystroke [`nudge_keystrokes`] uses to flush a
/// paste buffer.
///
/// Returns `Ok(true)` when the `Enter` went out and `Ok(false)` otherwise. The
/// shape is checked twice: once on `swept`, the capture the sweep already took
/// (so a pane with nothing buffered costs no extra `capture-pane`), and again on
/// a fresh capture taken immediately before the send — the same TOCTOU
/// discipline [`send_approval`] applies, so a pane that started responding or
/// raised a prompt in between receives no stray keystroke. No text is ever
/// typed, so the pane-0 no-pollution rule holds for the supervisor's own pane
/// too.
fn submit_buffered_input(
    capturer: &dyn PaneCapture,
    dispatcher: &mut dyn KeyDispatcher,
    session: &str,
    pane_index: usize,
    swept: &str,
) -> Result<bool, PawError> {
    if !pane_has_buffered_input(swept)
        || !pane_has_buffered_input(&capturer.capture(session, pane_index))
    {
        return Ok(false);
    }
    dispatcher
        .send_key(session, pane_index, "Enter")
        .map_err(|e| PawError::TmuxError(format!("send-keys Enter failed: {e}")))?;
    Ok(true)
}

/// Capture markers that identify a pane actively producing a response — the CLI
/// is mid-turn with its interrupt footer showing, rather than sitting at its
/// input box.
///
/// The set is deliberately conservative: an unrecognised CLI matches nothing and
/// is treated as idle, which at worst reproduces the injection behaviour of a
/// pane whose state the loop cannot read.
const MID_RESPONSE_MARKERS: &[&str] = &["esc to interrupt", "ctrl+c to interrupt"];

/// Whether `capture` shows a pane mid-response (see [`MID_RESPONSE_MARKERS`]).
fn pane_is_mid_response(capture: &str) -> bool {
    let lowered = capture.to_ascii_lowercase();
    MID_RESPONSE_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
}

/// Whether the orchestrator's pane is ready to receive an injected task.
///
/// Two conditions, each guarding a distinct hazard:
/// - **No live permission prompt.** The approval path owns that pane state on
///   this or a prior tick, and free text typed at a prompt lands *in* the prompt.
///   This is what keeps the hand-off from colliding with the pane-0 approval
///   no-pollution rule, which stays unmodified.
/// - **Not mid-response.** A task injected into the middle of the orchestrator's
///   own turn pollutes its context instead of queueing a job.
fn orchestrator_ready(capture: &str) -> bool {
    !live_prompt_in_tail(capture) && !pane_is_mid_response(capture)
}

/// Injects `text` into the orchestrator's pane as a task prompt, returning
/// whether any keystroke was actually dispatched.
///
/// Fire-and-forget by design: a refusal (the pane was busy), a dispatch
/// failure, or a wedged pane (escalated by [`send_nudge`] itself) is
/// swallowed here, because the broker record is the durable channel and the
/// injection is only the trigger. Nothing here blocks the wave — the loop
/// moves straight on, and the next orchestration nudge re-triggers the
/// orchestrator anyway. The returned flag lets a *cadenced* caller
/// distinguish "dispatched" from "suppressed" so it can retry rather than
/// swallow the whole interval.
fn hand_to_orchestrator(
    deps: &mut DriveDeps<'_>,
    ctx: SweepContext<'_>,
    pane_index: usize,
    text: &str,
) -> bool {
    send_nudge(
        deps,
        ctx.repo_root,
        ctx.session,
        pane_index,
        SUPERVISOR_AGENT_ID,
        text,
        ctx.orchestrator_settle_delay,
        orchestrator_ready,
    )
    .is_ok_and(NudgeOutcome::was_sent)
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
/// resolved phase outranks it (GP-05).
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
            (id.clone(), status)
        })
        .collect()
}

/// JSON body for the opportunistic friction learning.
fn friction_learning_body(e: &Escalation) -> String {
    serde_json::json!({
        "observation": format!(
            "unattended drive loop escalated a {} prompt from {} it could not auto-approve",
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

/// Production [`PaneEnumerator`]: `tmux list-panes -t <session> -F ...`.
struct TmuxPaneEnumerator;

impl PaneEnumerator for TmuxPaneEnumerator {
    fn list_panes(&self, session: &str) -> Vec<PaneInfo> {
        let target = format!("{session}:0");
        let output = Command::new("tmux")
            .args([
                "list-panes",
                "-t",
                &target,
                "-F",
                "#{pane_index} #{pane_current_path}",
            ])
            .output();
        let Ok(output) = output else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        let text = String::from_utf8_lossy(&output.stdout);
        parse_list_panes(&text)
    }
}

/// Parses `tmux list-panes -F '#{pane_index} #{pane_current_path}'` output.
fn parse_list_panes(text: &str) -> Vec<PaneInfo> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (idx, path) = line.split_once(' ')?;
            let pane_index = idx.trim().parse::<usize>().ok()?;
            Some(PaneInfo {
                pane_index,
                pane_current_path: path.trim().to_string(),
            })
        })
        .collect()
}

/// Production [`PaneCapture`]: one `tmux capture-pane` per pane.
struct TmuxPaneCapture;

impl PaneCapture for TmuxPaneCapture {
    fn capture(&self, session: &str, pane_index: usize) -> String {
        super::permission_prompt::capture_pane(session, pane_index).unwrap_or_default()
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
    /// Highest sequence number already observed. `Cell` because the trait takes
    /// `&self` (the loop is single-threaded).
    cursor: std::cell::Cell<u64>,
}

impl MessageObserver for HttpLogObserver {
    fn poll_new(&self) -> Vec<BrokerMessage> {
        let Some(url) = &self.broker_url else {
            return Vec::new();
        };
        let Ok(entries) = crate::broker::publish::fetch_log_entries_over_http(url) else {
            return Vec::new();
        };
        let since = self.cursor.get();
        let mut out = Vec::new();
        for entry in entries {
            if entry.seq > since {
                self.cursor.set(self.cursor.get().max(entry.seq));
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
    fn log_approval(&mut self, agent_id: &str, matched: &str) {
        let Some(url) = &self.broker_url else {
            return;
        };
        let summary = format!("auto_approved: matched {matched}");
        let msg = crate::broker::publish::build_status_message(
            agent_id,
            "auto_approved",
            Some(summary),
            None,
        );
        if let Err(e) = crate::broker::publish::publish_to_broker_http(url, &msg) {
            eprintln!("drive: failed to publish auto-approve status for {agent_id}: {e}");
        }
    }

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

/// Inputs for [`run_drive_loop`] beyond the session name, repo root, and agent
/// roster — bundled so the production entry point stays under the
/// argument-count lint and so `cmd_supervisor` builds them in one place.
pub struct DriveRunOptions {
    /// Broker `/status` + publish endpoint, or `None` when the broker is off.
    pub broker_url: Option<String>,
    /// Effective safe-command whitelist the classifier consumes.
    pub whitelist: Vec<String>,
    /// Whether in-worktree write/edit/create prompts auto-approve.
    pub approve_worktree_writes: bool,
    /// Protected-path set for the operator config/memory danger rule
    /// (`agent-memory-isolation`).
    pub protected_paths: ProtectedPaths,
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
    /// CLI running in the orchestrator's pane (pane 0), for its nudge settle
    /// delay. Empty when unknown.
    pub supervisor_cli: String,
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
/// Returns an error only for an unrecoverable setup failure; the loop itself
/// swallows transient tmux/broker errors and keeps polling.
pub fn run_drive_loop(
    session: &str,
    repo_root: &Path,
    agents: &[AgentPane],
    options: DriveRunOptions,
) -> Result<DriveSummary, PawError> {
    let DriveRunOptions {
        broker_url,
        whitelist,
        approve_worktree_writes,
        protected_paths,
        correction,
        learnings_enabled,
        broker_log_hint,
        learnings_hint,
        branch_refresh_enabled,
        session_created_at,
        clis,
        supervisor_cli,
    } = options;

    let instance = FileSessionInstanceGuard {
        repo_root: repo_root.to_path_buf(),
        session_name: session.to_string(),
        bound_created_at: session_created_at,
    };

    let enumerator = TmuxPaneEnumerator;
    let capturer = TmuxPaneCapture;
    let mut dispatcher = TmuxKeyDispatcher;
    let status = HttpStatusFetcher {
        broker_url: broker_url.clone(),
    };
    let messages = HttpLogObserver {
        broker_url: broker_url.clone(),
        cursor: std::cell::Cell::new(0),
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
        poll_interval: duration_from_env_ms("GIT_PAW_DRIVE_POLL_MS", POLL_INTERVAL),
        heartbeat: duration_from_env_ms("GIT_PAW_DRIVE_HEARTBEAT_MS", HEARTBEAT_INTERVAL),
        orchestration_nudge_interval: duration_from_env_ms(
            "GIT_PAW_DRIVE_ORCHESTRATION_NUDGE_MS",
            ORCHESTRATION_NUDGE_INTERVAL,
        ),
        whitelist,
        approve_worktree_writes,
        protected_paths,
        correction,
        learnings_enabled,
        broker_log_hint,
        learnings_hint,
        branch_refresh_enabled,
        clis,
        supervisor_cli,
        ..DriveConfig::default()
    };

    let mut deps = DriveDeps {
        enumerator: &enumerator,
        capturer: &capturer,
        dispatcher: &mut dispatcher,
        status: &status,
        messages: &messages,
        clock: &clock,
        alerts: &mut alerts,
        learnings: &mut learnings,
        refresher: &mut refresher,
    };

    let summary = drive_loop(session, repo_root, agents, &mut deps, &config, &instance);
    println!("{}", summary.render());
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

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
        approvals: Vec<(String, String)>,
        escalations: Vec<Escalation>,
    }
    impl AlertSink for RecordingAlerts {
        fn log_approval(&mut self, agent_id: &str, matched: &str) {
            self.approvals
                .push((agent_id.to_string(), matched.to_string()));
        }
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
    /// re-engagement or orchestrator hand-off costs no simulated time,
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
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
            },
        ];
        // Pane index 4 holds feat/b's worktree; index 2 holds feat/a's.
        assert_eq!(
            resolve_pane_agent(4, "/repo-feat-b", &agents),
            PaneRole::Coding("feat-b".to_string())
        );
        assert_eq!(
            resolve_pane_agent(2, "/repo-feat-a", &agents),
            PaneRole::Coding("feat-a".to_string())
        );
    }

    #[test]
    fn resolves_pane_zero_and_one_to_supervisor_and_dashboard() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        assert_eq!(
            resolve_pane_agent(0, "/repo", &agents),
            PaneRole::Supervisor
        );
        assert_eq!(resolve_pane_agent(1, "/repo", &agents), PaneRole::Dashboard);
    }

    // --- classify_prompt ----------------------------------------------------

    #[test]
    fn classifies_safe_cargo_test() {
        let whitelist = vec!["cargo test".to_string()];
        let cap = live_safe_capture("cargo test --workspace");
        let v = classify_prompt(&cap, &whitelist, None, false, &ProtectedPaths::default());
        assert!(matches!(
            v,
            PromptVerdict::Safe {
                option_index: 1,
                ..
            }
        ));
    }

    #[test]
    fn classifies_danger_git_push() {
        let cap = "Bash command\n  git push origin main\nDo you want to proceed?\n(esc to cancel)";
        assert_eq!(
            classify_prompt(cap, &[], None, false, &ProtectedPaths::default()),
            PromptVerdict::Danger
        );
    }

    /// Spec scenario "Write to operator memory escalates as danger" through
    /// the drive-loop classifier: a write targeting the protected set is
    /// [`PromptVerdict::Danger`] — terminal, never auto-approved — at the
    /// same precedence as the curated danger-list.
    #[test]
    fn classifies_protected_path_write_as_danger() {
        let op_home = tempfile::tempdir().unwrap();
        let mut config = crate::config::PawConfig::default();
        config.clis.insert(
            "myvariant".to_string(),
            crate::config::CustomCli {
                command: "myvariant".to_string(),
                display_name: None,
                submit_delay_ms: None,
                approval_args: std::collections::HashMap::new(),
                settings_path: Some(
                    op_home
                        .path()
                        .join(".myvariant/settings.json")
                        .to_string_lossy()
                        .into_owned(),
                ),
            },
        );
        let protected = ProtectedPaths::derive(&config, None);
        let worktree = tempfile::tempdir().unwrap();
        // File-prompt path: a write into the operator's config dir.
        let cap = format!(
            "Do you want to allow this write to {}/settings.json?\n(esc to cancel)",
            op_home.path().join(".myvariant").to_string_lossy()
        );
        assert_eq!(
            classify_prompt(&cap, &[], Some(worktree.path()), true, &protected),
            PromptVerdict::Danger
        );
        // Shell write target: an append via a whitelisted verb still escalates.
        let cap = format!(
            "Bash command\n  echo x >> {}/settings.json\nDo you want to proceed?\n(esc to cancel)",
            op_home.path().join(".myvariant").to_string_lossy()
        );
        assert_eq!(
            classify_prompt(
                &cap,
                &["echo".to_string()],
                Some(worktree.path()),
                true,
                &protected
            ),
            PromptVerdict::Danger
        );
        // In-worktree writes are unaffected: same protected set, target
        // inside the agent's own worktree.
        let cap = "Do you want to allow this write to notes/memory.md?\n(esc to cancel)";
        assert!(matches!(
            classify_prompt(cap, &[], Some(worktree.path()), true, &protected),
            PromptVerdict::Safe { .. }
        ));
    }

    #[test]
    fn classifies_unknown_when_no_rule_matches() {
        let cap = "Bash command\n  frobnicate --all\nDo you want to proceed?\n(esc to cancel)";
        assert_eq!(
            classify_prompt(cap, &[], None, false, &ProtectedPaths::default()),
            PromptVerdict::Unknown
        );
    }

    /// GP-02b: an invocation of one of git-paw's own bundled helper scripts
    /// classifies safe through the drive-loop classifier.
    #[test]
    fn classifies_managed_script_invocation_as_safe() {
        let cap = live_safe_capture(".git-paw/scripts/broker.sh --agent feat-x status booting");
        assert!(matches!(
            classify_prompt(&cap, &[], None, false, &ProtectedPaths::default()),
            PromptVerdict::Safe { .. }
        ));
    }

    /// GP-04b: a write into a repository `.git/` directory is a terminal
    /// danger escalation through the drive-loop classifier, even when the
    /// verb (`echo`) is whitelisted.
    #[test]
    fn classifies_git_dir_write_as_danger() {
        let tmp = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(tmp.path())
            .status()
            .expect("git init");
        let cap = live_safe_capture("echo x >> .git/info/exclude");
        assert_eq!(
            classify_prompt(
                &cap,
                &["echo".to_string()],
                Some(tmp.path()),
                false,
                &ProtectedPaths::default()
            ),
            PromptVerdict::Danger
        );
    }

    /// Rider scenario: a worktree-confined dev-test shape classifies safe for
    /// an agent pane (known worktree root)…
    #[test]
    fn classifies_worktree_dev_test_shape_safe_for_agent_pane() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("scripts")).unwrap();
        std::fs::write(tmp.path().join("scripts/helper.sh"), "echo hi\n").unwrap();
        let cap = live_safe_capture("bash -n scripts/helper.sh");
        match classify_prompt(
            &cap,
            &[],
            Some(tmp.path()),
            false,
            &ProtectedPaths::default(),
        ) {
            PromptVerdict::Safe { matched, .. } => assert_eq!(matched, "worktree-dev-test"),
            other => panic!("expected Safe worktree-dev-test, got {other:?}"),
        }
    }

    /// …and the supervisor pane, which has no worktree root, is unaffected:
    /// the same capture stays Unknown (spec scenario "supervisor pane
    /// unaffected").
    #[test]
    fn worktree_dev_test_shape_stays_unknown_without_worktree_root() {
        let cap = live_safe_capture("bash -n scripts/helper.sh");
        assert_eq!(
            classify_prompt(&cap, &[], None, false, &ProtectedPaths::default()),
            PromptVerdict::Unknown
        );
    }

    // --- exit-probe normalization (A) ---------------------------------------

    /// Spec scenario "A safe command with a trailing exit-code probe classifies
    /// safe": through the loop's own classifier, the wrapped command reaches the
    /// same verdict as the bare one.
    #[test]
    fn classifies_safe_command_wrapped_in_an_exit_code_probe() {
        let whitelist = vec!["cargo test".to_string()];
        let bare = classify_prompt(
            &live_safe_capture("cargo test --lib"),
            &whitelist,
            None,
            false,
            &ProtectedPaths::default(),
        );
        let wrapped = classify_prompt(
            &live_safe_capture("cargo test --lib; echo test-exit=$?"),
            &whitelist,
            None,
            false,
            &ProtectedPaths::default(),
        );
        assert!(matches!(bare, PromptVerdict::Safe { .. }));
        assert_eq!(wrapped, bare, "the probe must not change the verdict");
    }

    /// Spec scenario "A trailing redirect is normalized away".
    #[test]
    fn classifies_safe_command_wrapped_in_a_discard_redirect() {
        let whitelist = vec!["mdbook build".to_string()];
        let bare = classify_prompt(
            &live_safe_capture("mdbook build docs/"),
            &whitelist,
            None,
            false,
            &ProtectedPaths::default(),
        );
        let wrapped = classify_prompt(
            &live_safe_capture("mdbook build docs/ >/dev/null 2>&1"),
            &whitelist,
            None,
            false,
            &ProtectedPaths::default(),
        );
        assert!(matches!(bare, PromptVerdict::Safe { .. }));
        assert_eq!(wrapped, bare, "the redirect must not change the verdict");
    }

    /// Spec scenario "Normalization does not rescue a danger command": the
    /// danger-list runs on the normalized slice, so the wrapper buys nothing.
    #[test]
    fn exit_probe_does_not_rescue_a_danger_command() {
        let cap = live_safe_capture("git push --force origin main; echo $?");
        assert_eq!(
            classify_prompt(
                &cap,
                &["git".to_string()],
                None,
                false,
                &ProtectedPaths::default()
            ),
            PromptVerdict::Danger
        );
    }

    // --- durable-grant preference (C) ---------------------------------------

    /// A 3-option capture: `Yes` / the durable "don't ask again" grant / `No`.
    fn live_durable_capture(cmd: &str) -> String {
        format!(
            "Bash command\n  {cmd}\nDo you want to proceed?\n❯ 1. Yes\n  2. Yes, and don't ask again for: {cmd}\n  3. No\n(esc to cancel)"
        )
    }

    /// Spec scenario "A safe prompt offering a durable option takes it": a
    /// stack command safe only through the resolved allowlist — `cargo` is not a
    /// read-mostly verb — still takes the durable grant, so the identical prompt
    /// stops re-appearing.
    #[test]
    fn safe_prompt_with_a_durable_option_takes_it() {
        let cap = live_durable_capture("cargo test --lib");
        match classify_prompt(
            &cap,
            &["cargo test".to_string()],
            None,
            false,
            &ProtectedPaths::default(),
        ) {
            PromptVerdict::Safe { option_index, .. } => assert_eq!(option_index, 2),
            other => panic!("expected Safe, got {other:?}"),
        }
    }

    /// Spec scenario "A safe prompt with no durable option uses the once-only
    /// option".
    #[test]
    fn safe_prompt_without_a_durable_option_uses_option_one() {
        let cap = live_safe_capture("cargo test --lib");
        match classify_prompt(
            &cap,
            &["cargo test".to_string()],
            None,
            false,
            &ProtectedPaths::default(),
        ) {
            PromptVerdict::Safe { option_index, .. } => assert_eq!(option_index, 1),
            other => panic!("expected Safe, got {other:?}"),
        }
    }

    /// Spec scenario "A non-safe prompt is never granted durably": an unmatched
    /// command escalates, so no option — durable or otherwise — is selected.
    #[test]
    fn non_safe_prompt_is_never_granted_durably() {
        let cap = live_durable_capture("frobnicate --all");
        assert_eq!(
            classify_prompt(&cap, &[], None, false, &ProtectedPaths::default()),
            PromptVerdict::Unknown,
            "an unclassified command escalates instead of taking a durable grant"
        );
    }

    /// End-to-end through the loop: the durable option's digit (`2`) is what
    /// actually goes out on the wire for a safe 3-option prompt.
    #[test]
    fn loop_dispatches_the_durable_option_digit() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                pane_current_path: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(2, &live_durable_capture("cargo test --lib"))]);
        let mut dispatcher = RecordingDispatcher::default();
        // Complete on the first poll so exactly one sweep runs.
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        assert_eq!(
            dispatcher.events,
            vec![(2, "2".to_string()), (2, "Enter".to_string())],
            "the durable grant's digit, then a separate Enter"
        );
    }

    // --- idle pane with buffered input (D) ----------------------------------

    /// An idle pane sitting at its input box with `text` unsubmitted.
    fn idle_input_box(text: &str) -> String {
        format!(
            "● Ran tool\n╭────────────────────────────────╮\n│ > {text}                       │\n╰────────────────────────────────╯\n  ? for shortcuts"
        )
    }

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

    /// End-to-end through the loop: an idle pane holding buffered text receives
    /// exactly one `Enter` and nothing else — no free text, so pane 0's
    /// no-pollution rule is untouched.
    #[test]
    fn loop_submits_a_stranded_directive_on_an_idle_pane() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                pane_current_path: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(2, &idle_input_box("/opsx:apply my-change"))]);
        let mut dispatcher = RecordingDispatcher::default();
        // Complete on the first poll so exactly one sweep runs.
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        assert_eq!(
            dispatcher.events,
            vec![(2, "Enter".to_string())],
            "only the submitting Enter, never free text"
        );
        assert!(
            dispatcher.literal_sends.is_empty(),
            "nothing is typed into the pane"
        );
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

    // --- GP-05: exit summary resolves terminal state (unattended-wave-lifecycle) --

    #[test]
    fn completed_wave_resolves_terminal_state_not_working() {
        let agents = vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
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
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            poll_interval: Duration::from_millis(0),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };

        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        assert_eq!(summary.outcome, DriveOutcome::Completed);
        assert!(
            summary.agent_states.iter().all(|(_, s)| s != "working"),
            "resolved state must never regress to working: {:?}",
            summary.agent_states
        );
        assert!(
            summary
                .agent_states
                .contains(&("feat-a".to_string(), "verified".to_string())),
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
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 1,
                pane_current_path: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(1, &live_safe_capture("cargo test"))]);
        let mut dispatcher = RecordingDispatcher::default();
        // Never completes on its own — the instance teardown is what ends it.
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "working")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
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
            dispatcher.events,
            vec![(1, "1".to_string()), (1, "Enter".to_string())],
            "only the first (pre-teardown) sweep's approval is sent — the loop \
             must never act on a subsequently-created same-named session's panes"
        );
    }

    #[test]
    fn instance_guard_true_lets_a_live_session_keep_driving() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let enumerator = FakeEnumerator { panes: vec![] };
        let capturer = FakeCapturer::new(&[]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "verified")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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

    // --- parse_list_panes (task 3.4) ---------------------------------------

    #[test]
    fn parses_list_panes_output() {
        let text = "0 /repo\n2 /repo-feat-a\n3 /repo-feat-b\n";
        let panes = parse_list_panes(text);
        assert_eq!(panes.len(), 3);
        assert_eq!(panes[0].pane_index, 0);
        assert_eq!(panes[1].pane_current_path, "/repo-feat-a");
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
    /// per pane, never a single shell `for` loop — and skips the dashboard pane
    /// entirely (it is never captured). With no live prompt there is no
    /// approval re-capture, so the call count equals the number of acted panes.
    #[test]
    fn sweep_captures_each_pane_exactly_once() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![
                PaneInfo {
                    pane_index: 0,
                    pane_current_path: "/repo".to_string(),
                },
                PaneInfo {
                    pane_index: 1,
                    pane_current_path: "/repo".to_string(),
                },
                PaneInfo {
                    pane_index: 2,
                    pane_current_path: "/repo-feat-a".to_string(),
                },
            ],
        };
        let capturer = FakeCapturer::new(&[(0, "supervisor thinking\n$ "), (2, "working...\n$ ")]);
        let mut dispatcher = RecordingDispatcher::default();
        // Complete on the first poll so exactly one sweep runs.
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        // Panes 0 and 2 captured once each; pane 1 (dashboard) never captured.
        assert_eq!(
            capturer.calls.get(),
            2,
            "one explicit capture per acted pane, dashboard pane skipped"
        );
    }

    // --- full loop: safe approval + completion (task 8.1 in-memory) --------

    #[test]
    fn loop_approves_safe_prompt_then_exits_on_completion() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![
                PaneInfo {
                    pane_index: 0,
                    pane_current_path: "/repo".to_string(),
                },
                PaneInfo {
                    pane_index: 2,
                    pane_current_path: "/repo-feat-a".to_string(),
                },
            ],
        };
        let capturer = FakeCapturer::new(&[
            (0, ""), // supervisor pane: no live prompt
            (2, &live_safe_capture("cargo test")),
        ]);
        let mut dispatcher = RecordingDispatcher::default();
        // The agent's task is verified by the first status poll, so the loop
        // approves the live safe prompt once, detects completion, and exits.
        // (The fake capture does not clear after an approval, so completing on
        // the first sweep keeps the approval count deterministic — mirroring
        // the single-poll pane-0 approval test.)
        let status = ScriptedStatus::new(vec![vec![row("feat-a", "verified")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        assert_eq!(summary.outcome, DriveOutcome::Completed);
        // The safe prompt on the coding pane was approved with `1` then Enter.
        assert_eq!(
            dispatcher.events,
            vec![(2, "1".to_string()), (2, "Enter".to_string())]
        );
        assert_eq!(alerts.approvals.len(), 1, "one approval logged");
        assert!(alerts.escalations.is_empty(), "no escalations");
    }

    /// Spec scenario "The Rust loop and the shell helper are mutually
    /// exclusive", reverse direction: another approver (here `sweep.sh`, which
    /// creates the very same claim file from its own process) already holds
    /// pane 2, so the loop dispatches NO keystroke for a prompt it classified
    /// safe — and pane 3, unclaimed, is approved in the same sweep, proving the
    /// skip is per-pane and does not block the wave.
    #[test]
    fn a_pane_claimed_by_another_approver_receives_no_keystroke() {
        let agents = vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
            },
        ];
        let enumerator = FakeEnumerator {
            panes: vec![
                PaneInfo {
                    pane_index: 2,
                    pane_current_path: "/repo-feat-a".to_string(),
                },
                PaneInfo {
                    pane_index: 3,
                    pane_current_path: "/repo-feat-b".to_string(),
                },
            ],
        };
        let capturer = FakeCapturer::new(&[
            (2, &live_safe_capture("cargo test")),
            (3, &live_safe_capture("cargo test")),
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
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
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

        assert_eq!(
            dispatcher.events,
            vec![(3, "1".to_string()), (3, "Enter".to_string())],
            "the claimed pane must receive nothing; the free pane is still approved"
        );
        assert!(
            claimed.exists(),
            "the loop must leave the other approver's claim in place"
        );
    }

    // --- full loop: danger escalation is non-blocking (task 8.2 in-memory) --

    #[test]
    fn loop_escalates_danger_without_blocking_other_agent() {
        let agents = vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
            },
        ];
        let enumerator = FakeEnumerator {
            panes: vec![
                PaneInfo {
                    pane_index: 2,
                    pane_current_path: "/repo-feat-a".to_string(),
                },
                PaneInfo {
                    pane_index: 3,
                    pane_current_path: "/repo-feat-b".to_string(),
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
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo build".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        let summary = drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);

        // The danger prompt was escalated, NOT approved.
        assert_eq!(alerts.escalations.len(), 1);
        assert_eq!(alerts.escalations[0].agent_id, "feat-a");
        assert_eq!(alerts.escalations[0].verdict, "danger");
        // feat-b's safe prompt still got approved — the wave kept progressing.
        assert!(
            dispatcher.events.iter().any(|(p, _)| *p == 3),
            "the other agent's safe prompt was still approved"
        );
        // Never sent keystrokes to the danger pane.
        assert!(
            !dispatcher.events.iter().any(|(p, _)| *p == 2),
            "no keystrokes to the danger pane"
        );
        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        // A friction learning was recorded opportunistically + at wind-down.
        assert!(!learnings.records.is_empty(), "friction learning recorded");
    }

    // --- pane 0 coverage (task 4.3 / W15-3 / W15-13) -----------------------

    #[test]
    fn loop_approves_supervisor_pane_safe_prompt() {
        let agents: Vec<AgentPane> = Vec::new();
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 0,
                pane_current_path: "/repo".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(0, &live_safe_capture("cargo test"))]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        // Pane 0 IS approved (W15-3) but only with the minimal digit+Enter
        // (W15-13) — no free text.
        assert_eq!(
            dispatcher.events,
            vec![(0, "1".to_string()), (0, "Enter".to_string())]
        );
    }

    #[test]
    fn loop_leaves_supervisor_pane_untouched_without_prompt() {
        let agents: Vec<AgentPane> = Vec::new();
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 0,
                pane_current_path: "/repo".to_string(),
            }],
        };
        // Supervisor pane is mid-conversation, no live prompt footer.
        let capturer = FakeCapturer::new(&[(0, "supervisor is thinking about the plan\n$ ")]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        assert!(
            dispatcher.events.is_empty(),
            "no keystrokes to pane 0 without a live prompt"
        );
    }

    // --- heartbeat exit (task 6.2) -----------------------------------------

    #[test]
    fn loop_exits_on_heartbeat_when_never_completing() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                pane_current_path: "/repo-feat-a".to_string(),
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
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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

    // --- pane-keyed sweep: a pane with no broker record is still swept -------

    #[test]
    fn sweeps_pane_with_no_broker_record() {
        // feat-a has NOT published any status (empty status snapshot), but its
        // pane still shows a live safe prompt — it must be swept and approved.
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                pane_current_path: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(2, &live_safe_capture("cargo test"))]);
        let mut dispatcher = RecordingDispatcher::default();
        // First poll: empty roster (no broker record). Second: supervisor done.
        let status = ScriptedStatus::new(vec![vec![], vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        assert!(
            dispatcher.events.iter().any(|(p, _)| *p == 2),
            "a pane with no broker record was still swept and approved"
        );
    }

    // --- scrollback prompt is ignored (task 3.3 / D4) ----------------------

    #[test]
    fn scrollback_prompt_is_not_acted_on() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                pane_current_path: "/repo-feat-a".to_string(),
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
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
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
        }]
    }

    fn correction_panes() -> FakeEnumerator {
        FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                pane_current_path: "/repo-feat-a".to_string(),
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
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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

        // A classifier verdict keeps the prompt framing — accurate for it.
        for verdict in ["danger", "unknown"] {
            let prompt = Escalation {
                agent_id: "feat-a".to_string(),
                verdict: verdict.to_string(),
                command: "git push --force origin main".to_string(),
            };
            let text = prompt.question();
            assert!(
                text.contains("permission prompt") && text.contains(verdict),
                "a {verdict} prompt still asks the human to review the pane, got: {text}"
            );
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
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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
                alerts: &mut alerts,
                learnings: &mut learnings,
                refresher: &mut refresher,
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
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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
        let mut refresher = RecordingRefresher::default();
        let summary = {
            let mut deps = DriveDeps {
                enumerator: &enumerator,
                capturer: &capturer,
                dispatcher: &mut dispatcher,
                status: &status,
                messages: &messages,
                clock: &clock,
                alerts: &mut alerts,
                learnings: &mut learnings,
                refresher: &mut refresher,
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
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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

    /// Spec scenario "Prompt cleared before send sends nothing"
    /// (`approve-send-gate-hardening`): the sweep capture shows a live safe
    /// prompt, but the FRESH re-confirm capture taken immediately before
    /// sending shows it cleared — zero keystrokes are dispatched, so no
    /// approval digit lands as stray chat input.
    #[test]
    fn prompt_cleared_between_decision_and_send_sends_nothing() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let enumerator = FakeEnumerator {
            panes: vec![PaneInfo {
                pane_index: 2,
                pane_current_path: "/repo-feat-a".to_string(),
            }],
        };
        let live = "Bash command\n  cargo test --workspace\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n(esc to cancel)";
        let cleared = "$ cargo test --workspace\nrunning 5 tests\nall passed\n$ ";
        let capturer = SequencedCapturer::new(&[live, cleared]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![vec![row("supervisor", "done")]]);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = RecordingRefresher::default();
        let messages = ScriptedMessages::none();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop_in_tmp("paw-test", &agents, &mut deps, &config);
        assert!(
            dispatcher.events.is_empty(),
            "a prompt that cleared between decision and send must dispatch zero keystrokes"
        );
        assert_eq!(
            alerts.approvals.len(),
            1,
            "the safe decision was made (and audit-logged) before the prompt cleared"
        );
    }

    // --- autonomous orchestrator (supervisor-autonomous-orchestrator) --------

    /// An orchestrator pane sitting at its input box: no live permission prompt,
    /// not mid-turn — the state a hand-off may be injected into.
    const IDLE_PANE: &str = "? for shortcuts";

    /// An orchestrator pane actively producing a response. Injecting here would
    /// land a task in the middle of the model's own turn.
    const MID_RESPONSE_PANE: &str = "Boondoggling… (esc to interrupt)";

    /// Two coding agents on distinct worktrees, the roster every orchestrator
    /// test below shares.
    fn two_agents() -> Vec<AgentPane> {
        vec![
            AgentPane {
                agent_id: "feat-a".to_string(),
                worktree_path: PathBuf::from("/repo-feat-a"),
            },
            AgentPane {
                agent_id: "feat-b".to_string(),
                worktree_path: PathBuf::from("/repo-feat-b"),
            },
        ]
    }

    fn pane(pane_index: usize, path: &str) -> PaneInfo {
        PaneInfo {
            pane_index,
            pane_current_path: path.to_string(),
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
        let mut refresher = RecordingRefresher::default();
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
        };
        drive_loop_in_tmp("paw-test", agents, &mut deps, config);
        (dispatcher, alerts)
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

    /// Presence is structural — a pane resolving to the supervisor IS the
    /// orchestrator, and a session with no such pane has none.
    #[test]
    fn orchestrator_presence_is_read_from_the_pane_layout() {
        let agents = two_agents();
        let with_supervisor = vec![pane(0, "/repo"), pane(2, "/repo-feat-a")];
        assert_eq!(
            orchestrator_pane_index(&with_supervisor, &agents),
            Some(0),
            "pane 0 at the repo root is the orchestrator"
        );
        assert!(orchestrator_present(&with_supervisor, &agents));

        let workers_only = vec![pane(2, "/repo-feat-a"), pane(3, "/repo-feat-b")];
        assert_eq!(orchestrator_pane_index(&workers_only, &agents), None);
        assert!(
            !orchestrator_present(&workers_only, &agents),
            "a pure --unattended run with no supervisor pane has no orchestrator"
        );
    }

    /// Spec: "An unknown prompt is injected into the orchestrator pane" — the
    /// escalation is recorded on the broker AND handed to the orchestrator, while
    /// the other worker keeps progressing (the wave never blocks on the hand-off).
    #[test]
    fn unknown_prompt_is_handed_to_the_orchestrator_and_recorded_on_the_broker() {
        let agents = two_agents();
        let (dispatcher, alerts) = run_loop(
            &agents,
            vec![
                pane(0, "/repo"),
                pane(2, "/repo-feat-a"),
                pane(3, "/repo-feat-b"),
            ],
            &[
                (0, IDLE_PANE),
                (2, &live_safe_capture("frobnicate --all")),
                (3, &live_safe_capture("cargo build")),
            ],
            vec![
                vec![row("feat-a", "working"), row("feat-b", "working")],
                vec![row("supervisor", "done")],
            ],
            &ScriptedMessages::none(),
            &DriveConfig {
                whitelist: vec!["cargo build".to_string()],
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                ..DriveConfig::default()
            },
        );

        // Recorded uniformly on the broker …
        assert_eq!(alerts.escalations.len(), 1);
        assert_eq!(alerts.escalations[0].agent_id, "feat-a");
        assert_eq!(alerts.escalations[0].verdict, "unknown");
        // … AND handed to the orchestrator's pane as a task prompt.
        assert_text_then_separate_enter(&dispatcher, 0, "Judgment call handed to you");
        assert!(
            injected(&dispatcher, 0, "feat-a"),
            "the hand-off names the agent whose judgment call it is"
        );
        // The other worker's safe prompt was still approved this same sweep.
        assert!(
            dispatcher.events.contains(&(3, "1".to_string()))
                && dispatcher.events.contains(&(3, "Enter".to_string())),
            "the wave kept progressing; events were {:?}",
            dispatcher.events
        );
    }

    /// Spec: "No orchestrator present falls back to broker-only escalation" —
    /// the same input records the escalation and injects into NO pane. This is
    /// also the back-compat guarantee for a pure `--unattended` run.
    #[test]
    fn unknown_prompt_without_an_orchestrator_records_only_the_broker_escalation() {
        let agents = two_agents();
        let (dispatcher, alerts) = run_loop(
            &agents,
            vec![pane(2, "/repo-feat-a"), pane(3, "/repo-feat-b")],
            &[
                (2, &live_safe_capture("frobnicate --all")),
                (3, &live_safe_capture("cargo build")),
            ],
            vec![
                vec![row("feat-a", "working"), row("feat-b", "working")],
                vec![row("supervisor", "done")],
            ],
            &ScriptedMessages::none(),
            &DriveConfig {
                whitelist: vec!["cargo build".to_string()],
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                ..DriveConfig::default()
            },
        );

        assert_eq!(alerts.escalations.len(), 1, "still recorded on the broker");
        assert_eq!(alerts.escalations[0].verdict, "unknown");
        assert!(
            dispatcher.literal_sends.is_empty(),
            "no pane injection at all without an orchestrator; literal sends were {:?}",
            dispatcher.literal_sends
        );
        // Only the safe pane's approval keystrokes went out — the escalated pane
        // was never typed into, which is the prior behaviour exactly.
        assert!(
            dispatcher
                .events
                .iter()
                .all(|(p, key)| *p == 3 && matches!(key.as_str(), "1" | "Enter")),
            "only pane 3's approval keystrokes are expected; events were {:?}",
            dispatcher.events
        );
    }

    /// Spec: "An ambiguous question is injected into the orchestrator pane" — a
    /// worker's `agent.question` IS the ambiguity signal, so it goes straight to
    /// the orchestrator for the smart model to answer.
    #[test]
    fn worker_question_is_handed_to_the_orchestrator_pane() {
        let agents = two_agents();
        let question = BrokerMessage::Question {
            agent_id: "feat-a".to_string(),
            payload: crate::broker::messages::QuestionPayload {
                question: "spec says both A and B own this file — which wins?".to_string(),
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
            &ScriptedMessages::new(vec![vec![question]]),
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                ..DriveConfig::default()
            },
        );

        assert_text_then_separate_enter(&dispatcher, 0, "which wins?");
        assert!(
            injected(&dispatcher, 0, "Judgment call handed to you: feat-a"),
            "the hand-off names the waiting worker; literal sends were {:?}",
            dispatcher.literal_sends
        );
    }

    // --- wind-down suppresses nudge dispatch (GP-16) ------------------------

    /// Spec scenario "No nudge is sent after wind-down begins": a `Question`
    /// message that arrives on the SAME tick the wave's own status fetch
    /// already shows it complete is never handed to the orchestrator — the
    /// loop's wind-down state is checked before dispatching a hand-off, not
    /// just before the cadenced orchestration nudge.
    #[test]
    fn no_hand_off_is_dispatched_on_the_tick_the_wave_completes() {
        let agents = vec![AgentPane {
            agent_id: "feat-a".to_string(),
            worktree_path: PathBuf::from("/repo-feat-a"),
        }];
        let question = BrokerMessage::Question {
            agent_id: "feat-a".to_string(),
            payload: crate::broker::messages::QuestionPayload {
                question: "which wins?".to_string(),
            },
        };
        let (dispatcher, _) = run_loop(
            &agents,
            vec![pane(0, "/repo"), pane(2, "/repo-feat-a")],
            &[(0, IDLE_PANE), (2, IDLE_PANE)],
            // A single tick: the status fetch already shows the wave
            // complete (a supervisor verdict) at the same moment the
            // Question message is observed.
            vec![vec![row("supervisor", "done")]],
            &ScriptedMessages::new(vec![vec![question]]),
            &DriveConfig {
                poll_interval: Duration::from_secs(1),
                heartbeat: Duration::from_hours(1),
                ..DriveConfig::default()
            },
        );
        assert!(
            dispatcher.events.is_empty(),
            "no nudge keystrokes on the tick the wave completes; events were {:?}",
            dispatcher.events
        );
    }

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

    /// Drives one wave whose only message is a `feat-a` artifact at `status`, and
    /// reports what reached the orchestrator pane.
    fn run_artifact_wave(status: &str) -> RecordingDispatcher {
        let agents = two_agents();
        let (dispatcher, _) = run_loop(
            &agents,
            vec![pane(0, "/repo"), pane(2, "/repo-feat-a")],
            &[(0, IDLE_PANE), (2, IDLE_PANE)],
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
        dispatcher
    }

    /// Spec: "A merge decision is injected into the orchestrator pane" — a
    /// `committed` or `done` artifact is the point a merge decision becomes live,
    /// so the loop triggers the orchestrator rather than waiting to be noticed.
    #[test]
    fn merge_candidate_artifact_hands_the_merge_decision_to_the_orchestrator() {
        for status in MERGE_CANDIDATE_STATUSES {
            let dispatcher = run_artifact_wave(status);
            assert_text_then_separate_enter(&dispatcher, 0, "Merge decision handed to you: feat-a");
            assert!(
                injected(&dispatcher, 0, "merge sequence"),
                "the {status} hand-off asks for merge sequencing"
            );
            assert!(
                injected(&dispatcher, 0, status),
                "the hand-off names the status that made the decision live"
            );
        }
    }

    /// …and no other artifact status does. `blocked` and `verified` never arrive
    /// as an artifact status (they are `agent.blocked` / `agent.verified`), and a
    /// merge-sequencing prompt for a branch in either state would be misleading —
    /// there is no merge to sequence.
    #[test]
    fn non_merge_candidate_artifact_hands_over_nothing() {
        for status in ["blocked", "verified", "working"] {
            let dispatcher = run_artifact_wave(status);
            assert!(
                !injected(&dispatcher, 0, "Merge decision handed to you"),
                "a {status} artifact is not a merge candidate; sends were {:?}",
                dispatcher.literal_sends
            );
        }
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
                pane_current_path: "/repo-feat-a".to_string(),
            }],
        };
        let capturer = FakeCapturer::new(&[(2, capture)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![first_tick_status, vec![row("supervisor", "done")]]);
        let messages = ScriptedMessages::new(messages_batches);
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
        let mut refresher = refresher;
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            messages: &messages,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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
        }];
        let mut pane_by_agent = HashMap::new();
        pane_by_agent.insert("feat-a".to_string(), 2usize);
        let latest_status = vec![row_with_modified_files("feat-a", "working", &[])];
        let ctx = SweepContext {
            session: "paw-test",
            repo_root: &repo_root,
            orchestrator: None,
            orchestrator_settle_delay: Duration::ZERO,
        };

        let enumerator = FakeEnumerator { panes: vec![] };
        let capturer = FakeCapturer::new(&[(2, IDLE_PANE)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![]);
        let messages = ScriptedMessages::none();
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
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
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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
            orchestrator: None,
            orchestrator_settle_delay: Duration::ZERO,
        };

        let enumerator = FakeEnumerator { panes: vec![] };
        let capturer = FakeCapturer::new(&[(2, IDLE_PANE)]);
        let mut dispatcher = RecordingDispatcher::default();
        let status = ScriptedStatus::new(vec![]);
        let messages = ScriptedMessages::none();
        let clock = FakeClock::new();
        let mut alerts = RecordingAlerts::default();
        let mut learnings = RecordingLearnings::default();
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
            alerts: &mut alerts,
            learnings: &mut learnings,
            refresher: &mut refresher,
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

    /// A task injected mid-turn pollutes the orchestrator's context, so the
    /// hand-off is suppressed while its pane is producing output — the broker
    /// record still stands.
    #[test]
    fn handoff_is_suppressed_while_the_orchestrator_is_mid_response() {
        let agents = two_agents();
        let (dispatcher, alerts) = run_loop(
            &agents,
            vec![pane(0, "/repo"), pane(2, "/repo-feat-a")],
            &[
                (0, MID_RESPONSE_PANE),
                (2, &live_safe_capture("frobnicate --all")),
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

        assert_eq!(
            alerts.escalations.len(),
            1,
            "the broker record still stands"
        );
        assert!(
            dispatcher.literal_sends.is_empty(),
            "nothing is typed into a mid-response orchestrator; sends were {:?}",
            dispatcher.literal_sends
        );
    }

    /// Spec: "Orchestration nudge fires on the longer cadence" — once the
    /// orchestration interval elapses with no completion, the nudge goes out as
    /// text plus a separate `Enter`.
    #[test]
    fn orchestration_nudge_fires_on_the_longer_cadence() {
        let agents = two_agents();
        let (dispatcher, _) = run_loop(
            &agents,
            vec![pane(0, "/repo"), pane(2, "/repo-feat-a")],
            &[(0, IDLE_PANE), (2, IDLE_PANE)],
            vec![vec![row("feat-a", "working"), row("feat-b", "working")]],
            &ScriptedMessages::none(),
            &DriveConfig {
                poll_interval: Duration::from_secs(15),
                orchestration_nudge_interval: Duration::from_secs(10),
                heartbeat: Duration::from_secs(20),
                ..DriveConfig::default()
            },
        );

        assert_text_then_separate_enter(&dispatcher, 0, "Run an orchestration sweep now");
        assert!(
            injected(&dispatcher, 0, "merge sequencing"),
            "the nudge asks for spawn order, merge sequencing, and blocked workers"
        );
    }

    /// …and within a single approval tick nothing is nudged: the orchestration
    /// cadence is a multiple of the poll interval, so the fast sweep does not
    /// prompt-storm the orchestrator.
    #[test]
    fn no_orchestration_nudge_within_a_single_approval_tick() {
        let agents = two_agents();
        let (dispatcher, _) = run_loop(
            &agents,
            vec![pane(0, "/repo"), pane(2, "/repo-feat-a")],
            &[(0, IDLE_PANE), (2, IDLE_PANE)],
            vec![vec![row("feat-a", "working"), row("feat-b", "working")]],
            &ScriptedMessages::none(),
            &DriveConfig {
                poll_interval: POLL_INTERVAL,
                orchestration_nudge_interval: ORCHESTRATION_NUDGE_INTERVAL,
                heartbeat: POLL_INTERVAL + Duration::from_secs(1),
                ..DriveConfig::default()
            },
        );

        assert!(
            dispatcher.literal_sends.is_empty(),
            "no orchestration nudge inside one approval tick; sends were {:?}",
            dispatcher.literal_sends
        );
    }

    /// The orchestration nudge is suppressed while the orchestrator is
    /// mid-response, and — because the cadence timer only advances on a
    /// delivered nudge — it is deferred rather than swallowed.
    #[test]
    fn orchestration_nudge_is_suppressed_while_the_orchestrator_is_mid_response() {
        let agents = two_agents();
        let (dispatcher, _) = run_loop(
            &agents,
            vec![pane(0, "/repo"), pane(2, "/repo-feat-a")],
            &[(0, MID_RESPONSE_PANE), (2, IDLE_PANE)],
            vec![vec![row("feat-a", "working"), row("feat-b", "working")]],
            &ScriptedMessages::none(),
            &DriveConfig {
                poll_interval: Duration::from_secs(15),
                orchestration_nudge_interval: Duration::from_secs(10),
                heartbeat: Duration::from_secs(20),
                ..DriveConfig::default()
            },
        );

        assert!(
            dispatcher.literal_sends.is_empty(),
            "a busy orchestrator is not nudged; sends were {:?}",
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
