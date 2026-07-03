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
//! - **Act only on a LIVE prompt** — a recognized footer within the last ~4
//!   non-blank lines ([`crate::supervisor::auto_approve::is_live_prompt`]);
//!   prompt-like text scrolled into history is ignored (D4).
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
use std::time::{Duration, Instant};

use crate::error::PawError;

use super::approval_gate::{approval_dedup_key, live_prompt_in_tail};
use super::approve::{KeyDispatcher, TmuxKeyDispatcher, approval_keystrokes};
use super::auto_approve::{
    detect_prompt_shape, extract_command_slice, extract_path_from_file_prompt, is_dangerous,
    is_live_prompt, is_safe_command, is_scratch_rm, is_worktree_file_op, is_worktree_git_op,
    select_option_index,
};
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
/// ([`crate::supervisor::poll`]): danger-list first (terminal escalate), then
/// the scratch-`rm` exception, the worktree-confined `git add`/`git commit`
/// pre-approval, the shell whitelist, and finally the worktree file-op
/// boundary. Anything unmatched is [`PromptVerdict::Unknown`].
///
/// `worktree_root` is `None` for panes without a known worktree (the supervisor
/// pane), which suppresses the worktree-scoped rules for that pane.
#[must_use]
pub fn classify_prompt(
    captured: &str,
    whitelist: &[String],
    worktree_root: Option<&Path>,
    approve_worktree_writes: bool,
) -> PromptVerdict {
    let slice = extract_command_slice(captured).unwrap_or_else(|| captured.to_string());
    let option_index = select_option_index(detect_prompt_shape(captured), &slice);

    // Danger-first precedence: a curated danger-list match is a terminal
    // escalate that overrides any whitelist / safe-by-pattern match.
    if is_dangerous(&slice) {
        return PromptVerdict::Danger;
    }
    // Scratch-path exception: an `rm -rf` whose every target is repo/OS scratch.
    if is_scratch_rm(&slice) {
        return PromptVerdict::Safe {
            option_index,
            matched: "scratch-rm".to_string(),
        };
    }
    // Worktree-confined `git add` / `git commit` pre-approval.
    if let Some(root) = worktree_root
        && is_worktree_git_op(&slice, root)
    {
        return PromptVerdict::Safe {
            option_index,
            matched: "worktree-git".to_string(),
        };
    }
    // Shell whitelist (read-mostly verbs + configured safe commands).
    if let Some(entry) = first_whitelist_match(&slice, whitelist) {
        return PromptVerdict::Safe {
            option_index,
            matched: entry,
        };
    }
    // A write/edit/create prompt whose target resolves inside the worktree.
    if let Some(root) = worktree_root
        && is_worktree_file_op(captured, root, approve_worktree_writes)
    {
        return PromptVerdict::Safe {
            option_index,
            matched: "worktree-file-op".to_string(),
        };
    }
    PromptVerdict::Unknown
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
// Completion detection
// ---------------------------------------------------------------------------

/// Agent statuses that count as "task complete" for the all-agents-checked
/// completion rule. `committed` is deliberately excluded — a commit is not yet
/// a verified completion.
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
                .any(|r| &r.agent_id == id && AGENT_COMPLETE_STATUSES.contains(&r.status.as_str()))
        })
    {
        return Some(CompletionReason::AllTasksChecked);
    }
    None
}

// ---------------------------------------------------------------------------
// Escalation + summary
// ---------------------------------------------------------------------------

/// A risky/unknown prompt escalated for later human review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Escalation {
    /// Agent whose pane showed the prompt.
    pub agent_id: String,
    /// Classifier verdict label (`danger` / `unknown`).
    pub verdict: String,
    /// The prompted command (or a short description) for the summary.
    pub command: String,
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
    /// Pointer to the broker log (path or URL), if known.
    pub broker_log_hint: Option<String>,
    /// Pointer to the captured learnings file, if known.
    pub learnings_hint: Option<String>,
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
    /// The clock.
    pub clock: &'a dyn Clock,
    /// Publishes alerts.
    pub alerts: &'a mut dyn AlertSink,
    /// Records learnings.
    pub learnings: &'a mut dyn LearningSink,
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
    /// Effective safe-command whitelist.
    pub whitelist: Vec<String>,
    /// Whether in-worktree write/edit/create prompts auto-approve.
    pub approve_worktree_writes: bool,
    /// Broker log pointer for the summary.
    pub broker_log_hint: Option<String>,
    /// Learnings file pointer for the summary.
    pub learnings_hint: Option<String>,
}

impl Default for DriveConfig {
    fn default() -> Self {
        Self {
            poll_interval: POLL_INTERVAL,
            heartbeat: HEARTBEAT_INTERVAL,
            dedup_window: DEDUP_WINDOW,
            whitelist: Vec::new(),
            approve_worktree_writes: true,
            broker_log_hint: None,
            learnings_hint: None,
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
///    fresh re-confirm), risky/unknown prompts are escalated non-blocking and
///    deduped on `(agent_id, shape)`.
/// 4. Fetches `/status`; on completion the loop exits, otherwise it checks the
///    heartbeat and sleeps for the poll interval.
///
/// The pane sweep is **pane-keyed**: every pane returned by the enumerator is
/// evaluated, including a pane that has booted but not yet published any
/// `agent.status` (W15-7). The loop never treats a feedback→fix→re-verify cycle
/// as stuck — there is no cycle counter, only the completion and heartbeat exit
/// conditions.
pub fn drive_loop(
    session: &str,
    agents: &[AgentPane],
    deps: &mut DriveDeps<'_>,
    config: &DriveConfig,
) -> DriveSummary {
    let coding_ids: Vec<String> = agents.iter().map(|a| a.agent_id.clone()).collect();
    let worktree_by_id: HashMap<String, PathBuf> = agents
        .iter()
        .map(|a| (a.agent_id.clone(), a.worktree_path.clone()))
        .collect();

    let mut dedup = DedupWindow::new(config.dedup_window);
    let mut escalations: Vec<Escalation> = Vec::new();

    let start = deps.clock.now();

    // The loop is an expression that breaks with the terminal `(outcome,
    // latest_status)`: every exit path assigns both, so there are no
    // pre-initialised placeholders to leave dead.
    let (outcome, latest_status) = loop {
        // --- Sweep every pane (pane-keyed, explicit per-pane capture) --------
        for pane in deps.enumerator.list_panes(session) {
            let role = resolve_pane_agent(pane.pane_index, &pane.pane_current_path, agents);
            let Some(agent_id) = role.agent_id() else {
                continue; // dashboard pane — never acted on
            };
            let agent_id = agent_id.to_string();

            let capture = deps.capturer.capture(session, pane.pane_index);
            // Act only on a LIVE prompt in the tail; a non-live pane (mere
            // narration, or a resolved prompt scrolled away) is left untouched.
            // This is also what keeps pane 0 quiet when it has no live prompt.
            if !is_live_prompt(&capture) {
                continue;
            }

            let worktree_root = worktree_by_id.get(&agent_id).map(PathBuf::as_path);
            let verdict = classify_prompt(
                &capture,
                &config.whitelist,
                worktree_root,
                config.approve_worktree_writes,
            );

            match verdict {
                PromptVerdict::Safe {
                    option_index,
                    matched,
                } => {
                    // Log the approval BEFORE the keystrokes go out.
                    deps.alerts.log_approval(&agent_id, &matched);
                    // Re-confirm a live prompt with a fresh capture, then send
                    // the option digit and `Enter` as two separate keystrokes.
                    // This minimal sequence is what makes pane-0 approval safe:
                    // it consumes only the prompt, never landing free text or a
                    // stray newline in the supervisor's prompt box (W15-13).
                    let _ = send_approval(
                        deps.capturer,
                        deps.dispatcher,
                        session,
                        pane.pane_index,
                        option_index,
                    );
                }
                PromptVerdict::Danger | PromptVerdict::Unknown => {
                    // Non-blocking escalation, deduped on (agent_id, shape).
                    let key = approval_dedup_key(&agent_id, &capture);
                    if dedup.should_emit(&key, deps.clock.now()) {
                        let escalation = Escalation {
                            agent_id: agent_id.clone(),
                            verdict: verdict.label().to_string(),
                            command: dedup_shape(&capture),
                        };
                        deps.alerts.escalate(&escalation);
                        // Opportunistic learning: the loop absorbed friction it
                        // could not auto-approve.
                        deps.learnings.record(
                            LEARNING_CATEGORY,
                            "unattended loop escalated a prompt",
                            &friction_learning_body(&escalation),
                        );
                        escalations.push(escalation);
                    }
                    // Keep progressing the rest of the wave.
                }
            }
        }

        // --- Completion check ------------------------------------------------
        let latest_status = deps.status.fetch();
        if detect_completion(&latest_status, &coding_ids).is_some() {
            let outcome = if escalations.is_empty() {
                DriveOutcome::Completed
            } else {
                DriveOutcome::EscalatedForReview
            };
            break (outcome, latest_status);
        }

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

    DriveSummary {
        outcome,
        agent_states: agent_states_from_status(&latest_status),
        escalations,
        broker_log_hint: config.broker_log_hint.clone(),
        learnings_hint: config.learnings_hint.clone(),
    }
}

/// Re-confirms a live prompt with a fresh capture immediately before the send,
/// then dispatches the option digit followed by a separate `Enter`.
///
/// Returns `Ok(true)` when the keystrokes were sent, `Ok(false)` when the
/// prompt cleared between the sweep and the send (no stray input). This gate
/// applies to EVERY pane including pane 0 — the drive loop is the sole approver
/// for an unattended session and is explicitly permitted to clear the
/// supervisor's own safe prompts, but only with these minimal keystrokes.
fn send_approval(
    capturer: &dyn PaneCapture,
    dispatcher: &mut dyn KeyDispatcher,
    session: &str,
    pane_index: usize,
    option_index: u8,
) -> Result<bool, PawError> {
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
#[must_use]
pub fn nudge_keystrokes(text: &str) -> Vec<String> {
    vec![text.to_string(), "Enter".to_string()]
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

/// Builds the per-agent final-state list from a `/status` snapshot.
fn agent_states_from_status(rows: &[AgentStatusRow]) -> Vec<(String, String)> {
    rows.iter()
        .map(|r| (r.agent_id.clone(), r.status.clone()))
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
        let question = format!(
            "{} is stalled on a {} permission prompt the unattended loop could not \
             auto-approve; please review the pane and decide manually. Command: {}",
            escalation.agent_id, escalation.verdict, escalation.command
        );
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
    /// Broker-log pointer for the exit summary.
    pub broker_log_hint: Option<String>,
    /// Learnings-file pointer for the exit summary.
    pub learnings_hint: Option<String>,
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
        broker_log_hint,
        learnings_hint,
    } = options;

    let enumerator = TmuxPaneEnumerator;
    let capturer = TmuxPaneCapture;
    let mut dispatcher = TmuxKeyDispatcher;
    let status = HttpStatusFetcher {
        broker_url: broker_url.clone(),
    };
    let clock = SystemClock;
    let mut alerts = BrokerAlertSink { broker_url };
    let mut learnings = SweepLearningSink {
        repo_root: repo_root.to_path_buf(),
        seen: std::collections::HashSet::new(),
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
        whitelist,
        approve_worktree_writes,
        broker_log_hint,
        learnings_hint,
        ..DriveConfig::default()
    };

    let mut deps = DriveDeps {
        enumerator: &enumerator,
        capturer: &capturer,
        dispatcher: &mut dispatcher,
        status: &status,
        clock: &clock,
        alerts: &mut alerts,
        learnings: &mut learnings,
    };

    let summary = drive_loop(session, agents, &mut deps, &config);
    println!("{}", summary.render());
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

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

    #[derive(Default)]
    struct RecordingDispatcher {
        events: Vec<(usize, String)>,
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
        records: Vec<(String, String)>,
    }
    impl LearningSink for RecordingLearnings {
        fn record(&mut self, category: &str, title: &str, _body: &str) {
            self.records.push((category.to_string(), title.to_string()));
        }
    }

    fn row(agent: &str, status: &str) -> AgentStatusRow {
        AgentStatusRow {
            agent_id: agent.to_string(),
            status: status.to_string(),
            last_seen_seconds: 0,
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
        let v = classify_prompt(&cap, &whitelist, None, false);
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
            classify_prompt(cap, &[], None, false),
            PromptVerdict::Danger
        );
    }

    #[test]
    fn classifies_unknown_when_no_rule_matches() {
        let cap = "Bash command\n  frobnicate --all\nDo you want to proceed?\n(esc to cancel)";
        assert_eq!(
            classify_prompt(cap, &[], None, false),
            PromptVerdict::Unknown
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
            broker_log_hint: Some("/tmp/broker.log".to_string()),
            learnings_hint: Some(".git-paw/session-learnings.md".to_string()),
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
            vec!["please continue".to_string(), "Enter".to_string()],
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
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop("paw-test", &agents, &mut deps, &config);
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
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        let summary = drive_loop("paw-test", &agents, &mut deps, &config);

        assert_eq!(summary.outcome, DriveOutcome::Completed);
        // The safe prompt on the coding pane was approved with `1` then Enter.
        assert_eq!(
            dispatcher.events,
            vec![(2, "1".to_string()), (2, "Enter".to_string())]
        );
        assert_eq!(alerts.approvals.len(), 1, "one approval logged");
        assert!(alerts.escalations.is_empty(), "no escalations");
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
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo build".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        let summary = drive_loop("paw-test", &agents, &mut deps, &config);

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
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop("paw-test", &agents, &mut deps, &config);
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
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop("paw-test", &agents, &mut deps, &config);
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
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
        };
        let config = DriveConfig {
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_secs(5),
            ..DriveConfig::default()
        };
        let summary = drive_loop("paw-test", &agents, &mut deps, &config);
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
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop("paw-test", &agents, &mut deps, &config);
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
        let mut deps = DriveDeps {
            enumerator: &enumerator,
            capturer: &capturer,
            dispatcher: &mut dispatcher,
            status: &status,
            clock: &clock,
            alerts: &mut alerts,
            learnings: &mut learnings,
        };
        let config = DriveConfig {
            whitelist: vec!["cargo test".to_string()],
            poll_interval: Duration::from_secs(1),
            heartbeat: Duration::from_hours(1),
            ..DriveConfig::default()
        };
        drive_loop("paw-test", &agents, &mut deps, &config);
        assert!(
            dispatcher.events.is_empty(),
            "a scrollback prompt must not trigger keystrokes"
        );
    }
}
