//! The drive loop's functional core (`code-architecture`: "Drive-loop
//! decisions are pure functions with effects returned as data").
//!
//! The loop is `observe → decide → apply`. The shell in [`super`] observes —
//! it lists surfaces, captures their tails, fetches `/status`, polls new broker
//! messages, asks git for merge states and reads the clock — and hands those
//! observations to the decisions here. Each decision is a pure function: it
//! takes the current decision state and the observed inputs (the time among
//! them) and returns the next state with an ordered list of [`DriveEffect`]s.
//! The shell applies each effect in order through its ports and appends it,
//! with its [`EffectOutcome`], to the run's effect log.
//!
//! Nothing here reads a port, the clock, git or the broker, and no decision
//! returns a keystroke that answers a permission prompt: a live prompt only
//! ever yields escalation effects.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::broker::messages::BrokerMessage;
use crate::config::{
    CliPromptProfile, CorrectionConfig, CustomCli, OnExhausted, resolve_submit_delay_ms,
};
use crate::error::PawError;
use crate::surface::SurfaceRole;

use super::{
    AgentPane, AgentStatusRow, CORRECTION_EXHAUSTED_VERDICT, CORRECTION_SLOW_VERDICT,
    CorrectionState, DedupWindow, DriveOutcome, DriveSummary, Escalation, ExhaustedBranch,
    LEARNING_CATEGORY, MergeState, NUDGE_RECOVERY_ATTEMPTS, NUDGE_WEDGED_VERDICT,
    PERMISSION_PROMPT_VERDICT, WorkerPhase, approval_dedup_key, cli_unavailable_dedup_key,
    cli_unavailable_line, dedup_shape, exhaustion_learning_body, friction_learning_body,
    gate_failure, is_finished_worker, is_live_prompt, nudge_submitted, one_line_capped,
    pane_has_buffered_input, reengagement_text, worker_cli,
};

/// One action the drive loop takes, returned by a decision as data and
/// applied by the shell.
///
/// Closed: the shell must handle every case. There is no approval variant —
/// no git-paw process answers a permission prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveEffect {
    /// Record a forwarded permission prompt in the session's manual-decision
    /// log (through the `ManualDecisionSink` port).
    RecordManualDecision {
        /// The agent whose surface shows the prompt.
        agent_id: String,
        /// The captured surface content the prompt was read from.
        captured: String,
    },
    /// Publish an escalation on the broker for human review. Every published
    /// escalation also joins the exit summary.
    PublishEscalation(Escalation),
    /// Record a qualitative learning.
    RecordLearning {
        /// The learning's category.
        category: &'static str,
        /// The learning's title.
        title: &'static str,
        /// The learning's JSON body.
        body: String,
    },
    /// Submit a directive stranded in an idle coding surface's input box: a
    /// follow-up `Enter`, then — if the text survives — the bounded
    /// clear + re-type + `Enter` recovery under the surface's claim.
    SubmitBufferedInput {
        /// The coding agent that owns the surface.
        agent_id: String,
        /// The surface's position in the session's layout.
        pane_index: usize,
        /// The per-CLI settle delay between keystrokes.
        settle_delay: Duration,
    },
    /// Re-engage a gate-failed worker: type `text`, settle, send a separate
    /// `Enter`, verify and recover — under the surface's claim and gated on a
    /// fresh capture free of a live permission prompt.
    Reengage {
        /// The coding agent that owns the surface.
        agent_id: String,
        /// The surface's position in the session's layout.
        pane_index: usize,
        /// The re-engagement text.
        text: String,
        /// The per-CLI settle delay between keystrokes.
        settle_delay: Duration,
    },
    /// Refresh the live worker branches after an observed merge onto `base`:
    /// the existing gated rebase + notify, per branch that passes every gate.
    RefreshBranches {
        /// The default branch the merge landed on.
        base: String,
    },
    /// Record a branch whose correction budget is spent, for the exit summary.
    RecordExhausted(ExhaustedBranch),
    /// Note a judgment call a coding agent raised. Effect-log only: the
    /// worker's own broker message is the review item, so applying it changes
    /// no surface, broker message or file.
    NoteJudgmentCall {
        /// The coding agent that raised it.
        agent_id: String,
        /// What the call is about.
        call: JudgmentCall,
    },
}

/// What a judgment call is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JudgmentCall {
    /// The worker asked a question it could not decide (`agent.question`).
    Question {
        /// The question text.
        question: String,
    },
    /// The worker's artifact reached a merge-candidate status, so a merge
    /// decision is live (`agent.artifact`).
    MergeCandidate {
        /// The artifact's status.
        status: String,
    },
}

/// What happened when the shell applied one [`DriveEffect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectOutcome {
    /// Recorded through its port (a manual decision, a learning, an exhausted
    /// branch).
    Recorded,
    /// Published on the broker.
    Published,
    /// Keystrokes went out and the text was submitted.
    Sent,
    /// No keystroke went out: another approver held the surface's claim, the
    /// fresh capture no longer called for it, or nothing was buffered.
    Skipped,
    /// Keystrokes went out but the text was still on the input line after
    /// every recovery attempt; escalated as wedged.
    Wedged,
    /// A buffered-input submit's `Enter` went out, but another approver held
    /// the surface's claim, so its recovery is left to the next sweep.
    Deferred,
    /// The keystroke dispatch failed.
    Failed(String),
    /// The branch refresh ran; carries the branches it rebased.
    Refreshed(Vec<String>),
    /// Noted in the effect log only.
    Noted,
}

impl EffectOutcome {
    /// Whether any keystroke reached the surface — a correction cycle is spent
    /// only when one did.
    #[must_use]
    pub fn was_sent(&self) -> bool {
        matches!(self, Self::Sent | Self::Wedged | Self::Deferred)
    }
}

/// One entry of the drive loop's effect log: an effect the shell applied and
/// its outcome, in application order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedEffect {
    /// The effect.
    pub effect: DriveEffect,
    /// What applying it did.
    pub outcome: EffectOutcome,
}

// ---------------------------------------------------------------------------
// Prompt decision
// ---------------------------------------------------------------------------

/// What the shell observed about one surface this sweep.
pub(super) struct SurfaceObservation<'a> {
    /// The surface's resolved role; only a coding agent's surface is acted on.
    pub(super) role: &'a SurfaceRole,
    /// The surface's position in the session's layout.
    pub(super) pane_index: usize,
    /// The surface's captured screen.
    pub(super) capture: &'a str,
    /// The worker's resolved prompt profile (its CLI-unavailability markers).
    pub(super) profile: &'a CliPromptProfile,
    /// The worker's resolved settle delay between keystrokes.
    pub(super) settle_delay: Duration,
}

/// Decides what one sweep does about a coding surface, from its captured
/// tail, returning the next dedup state and the effects.
///
/// In order:
/// 1. A CLI-unavailability marker in the tail (D4) → a `cli-unavailable`
///    escalation, deduped on `(agent_id, matched marker)` — never on the
///    banner line, which may carry a changing reset time — so a persistent
///    banner is escalated once per window and two distinct banners stay
///    distinct (`cli-unavailable-detection-gaps`).
/// 2. No live prompt → submit a directive stranded in an idle input box, if
///    one is buffered; otherwise nothing.
/// 3. A live prompt → it is escalated for a human, deduped on
///    `(agent_id, command shape)`: the manual-decision record first, then the
///    `permission-prompt` escalation, then the `tooling_friction` learning. A
///    prompt still unanswered after the window is escalated again.
///
/// It answers NO prompt, whatever the command text says: whether the prompted
/// command may run is the agent CLI's decision or the human's, so no effect
/// returned for a live prompt types into the surface. A surface that is not a
/// coding agent's yields nothing.
pub(super) fn decide_prompt(
    state: &DedupWindow,
    obs: &SurfaceObservation<'_>,
    now: Instant,
) -> (DedupWindow, Vec<DriveEffect>) {
    let mut dedup = state.clone();
    let Some(agent_id) = obs.role.coding_agent() else {
        return (dedup, Vec::new());
    };

    if let Some(m) = cli_unavailable_line(obs.capture, &obs.profile.cli_unavailable_markers) {
        if !dedup.should_emit(&cli_unavailable_dedup_key(agent_id, &m.marker), now) {
            return (dedup, Vec::new());
        }
        let escalation = Escalation {
            agent_id: agent_id.to_string(),
            verdict: "cli-unavailable".to_string(),
            command: m.line,
        };
        return (dedup, vec![DriveEffect::PublishEscalation(escalation)]);
    }

    if !is_live_prompt(obs.capture) {
        let effects = if pane_has_buffered_input(obs.capture) {
            vec![DriveEffect::SubmitBufferedInput {
                agent_id: agent_id.to_string(),
                pane_index: obs.pane_index,
                settle_delay: obs.settle_delay,
            }]
        } else {
            Vec::new()
        };
        return (dedup, effects);
    }

    if !dedup.should_emit(&approval_dedup_key(agent_id, obs.capture), now) {
        return (dedup, Vec::new());
    }
    let escalation = Escalation {
        agent_id: agent_id.to_string(),
        verdict: PERMISSION_PROMPT_VERDICT.to_string(),
        command: dedup_shape(obs.capture),
    };
    // The forwarded decision is recorded BEFORE the escalation goes out, and
    // only on this arm: after the dedup admitted it, so one emitted escalation
    // is one log line, and never for a banner, a wedged nudge or a
    // correction-loop signal — the log `git paw approvals` reads as "commands
    // that needed a human". The learning notes that a prompt the CLI's own
    // permission rules did not cover cost the wave a human round-trip.
    let learning = DriveEffect::RecordLearning {
        category: LEARNING_CATEGORY,
        title: "unattended loop escalated a prompt",
        body: friction_learning_body(&escalation),
    };
    let effects = vec![
        DriveEffect::RecordManualDecision {
            agent_id: agent_id.to_string(),
            captured: obs.capture.to_string(),
        },
        DriveEffect::PublishEscalation(escalation),
        learning,
    ];
    (dedup, effects)
}

// ---------------------------------------------------------------------------
// Message decision
// ---------------------------------------------------------------------------

/// The loop's configuration and state the message decision reads.
pub(super) struct MessagePolicy<'a> {
    /// The session's coding agents, as last read.
    pub(super) coding_ids: &'a [String],
    /// `[supervisor.correction] gate_tags`: the tags that mark a failing gate.
    pub(super) gate_tags: &'a [String],
    /// Whether this tick's `/status` shows the wave complete.
    pub(super) winding_down: bool,
    /// Whether `supervisor-branch-refresh` is enabled.
    pub(super) branch_refresh_enabled: bool,
}

/// Decides what the broker messages published since the previous sweep mean
/// for the loop, returning the next correction state and the effects in
/// message order.
///
/// - **`agent.advanced-main`** → a branch refresh onto the payload's `base`,
///   when the refresh is enabled. Matched FIRST: its sender is the payload's
///   `from` (typically `supervisor`), never a coding agent, so the sender gate
///   below would otherwise drop every merge event. This is the post-merge
///   trigger of `supervisor-branch-refresh` (design D4): a refresh is
///   evaluated only in reaction to an observed merge, never on a timer.
/// - Every other message is considered only when its agent id is one of the
///   session's coding agents, so the loop's own `supervisor` traffic never
///   feeds back into itself.
/// - **`agent.feedback`** carrying a `[<gate>]` tag is a FAILING gate verdict:
///   it marks the branch for the correction pass. (A passing gate publishes
///   `agent.verified`, which the `/status` sweep handles instead.)
/// - A worker's **`agent.question`**, and its **`agent.artifact`** at a merge-
///   candidate status, outside wind-down → a judgment call naming the agent
///   and the question or status. No orchestrator runs inside the session: the
///   worker's own broker message is the review item, so the call is only
///   noted, never republished or typed anywhere.
pub(super) fn decide_message(
    state: &CorrectionState,
    messages: &[BrokerMessage],
    policy: &MessagePolicy<'_>,
) -> (CorrectionState, Vec<DriveEffect>) {
    let mut correction = state.clone();
    let mut effects = Vec::new();
    for msg in messages {
        if let BrokerMessage::AdvancedMain { payload } = msg {
            if policy.branch_refresh_enabled {
                effects.push(DriveEffect::RefreshBranches {
                    base: payload.base.clone(),
                });
            }
            continue;
        }
        let Some(agent_id) = policy.coding_ids.iter().find(|id| *id == msg.agent_id()) else {
            continue;
        };
        let call = match msg {
            BrokerMessage::Feedback { payload, .. } => {
                if let Some(gate) = gate_failure(payload, policy.gate_tags) {
                    correction.mark_gate_failure(
                        agent_id,
                        reengagement_text(&gate, &payload.errors.join("; ")),
                    );
                }
                None
            }
            BrokerMessage::Question { payload, .. } => Some(JudgmentCall::Question {
                question: payload.question.clone(),
            }),
            BrokerMessage::Artifact { payload, .. }
                if WorkerPhase::from_status(&payload.status).is_merge_candidate() =>
            {
                Some(JudgmentCall::MergeCandidate {
                    status: payload.status.clone(),
                })
            }
            _ => None,
        };
        if let Some(call) = call
            && !policy.winding_down
        {
            effects.push(DriveEffect::NoteJudgmentCall {
                agent_id: agent_id.clone(),
                call,
            });
        }
    }
    (correction, effects)
}

// ---------------------------------------------------------------------------
// Nudge decision (`supervisor-correction-loop`, `idle-pane-nudge-submission`)
// ---------------------------------------------------------------------------

/// What the shell observed this tick that the nudge decision reads.
pub(super) struct NudgeObservation<'a> {
    /// Pane index per coding agent, resolved this sweep.
    pub(super) panes: &'a HashMap<String, usize>,
    /// This tick's `/status` snapshot (the worker's phase and CLI).
    pub(super) status: &'a [AgentStatusRow],
}

/// The configuration the nudge decision reads.
pub(super) struct NudgePolicy<'a> {
    /// `[supervisor.correction]`.
    pub(super) correction: &'a CorrectionConfig,
    /// Whether `[supervisor] learnings` is enabled — gates the exhaustion
    /// learning (no telemetry without consent).
    pub(super) learnings_enabled: bool,
    /// `[clis.<name>]`, for the worker's settle delay.
    pub(super) clis: &'a HashMap<String, CustomCli>,
}

/// Decides one sweep of the correction loop over every branch marked by a
/// failing gate verdict, returning the next correction state and the effects
/// in agent-id order.
///
/// When `auto_loopback` is off (the default, and every config with no
/// `[supervisor.correction]` table) nothing happens: no keystroke, no cycle,
/// no exhaustion — the gate feedback is published exactly as it was before the
/// correction loop existed. Otherwise, per pending branch:
/// - Budget already spent → the `on_exhausted` policy, once, never a
///   re-engagement again. It is applied when a gate failure arrives for a
///   branch whose budget is ALREADY spent, not the instant the last
///   re-engagement goes out, so the final fix attempt gets its chance to pass
///   (a branch that passes clears its state and never exhausts).
/// - No surface resolved this sweep → kept pending, retried next sweep.
/// - The worker has FINISHED by its broker status → kept pending, never
///   nudged. The signal is the broker status, never a pane-content diff: a
///   finished worker's pane is just as unchanging as a stuck one's.
/// - Otherwise → a re-engagement carrying the gate feedback, with the
///   worker's own CLI's settle delay (an agent with no status row yet falls
///   back to the agnostic default).
///
/// The cycle is not spent here: whether the re-engagement reached the surface
/// is only known once the shell applied it, so [`decide_reengaged`] spends it.
pub(super) fn decide_nudge(
    state: &CorrectionState,
    obs: &NudgeObservation<'_>,
    policy: &NudgePolicy<'_>,
) -> (CorrectionState, Vec<DriveEffect>) {
    let mut correction = state.clone();
    let mut effects = Vec::new();
    if !policy.correction.auto_loopback {
        return (correction, effects);
    }
    for (agent_id, feedback) in correction.pending() {
        if correction.cycles(&agent_id) >= policy.correction.max_cycles {
            if correction.take_exhaustion(&agent_id) {
                effects.extend(exhaustion_effects(
                    &correction,
                    obs.status,
                    policy,
                    &agent_id,
                ));
            }
            continue;
        }
        let Some(&pane_index) = obs.panes.get(&agent_id) else {
            continue;
        };
        if is_finished_worker(obs.status, &agent_id) {
            continue;
        }
        let cli = obs
            .status
            .iter()
            .find(|r| r.agent_id == agent_id)
            .map_or("", |r| r.cli.as_str());
        effects.push(DriveEffect::Reengage {
            settle_delay: Duration::from_millis(resolve_submit_delay_ms(cli, policy.clis)),
            agent_id,
            pane_index,
            text: feedback,
        });
    }
    (correction, effects)
}

/// Decides what follows a re-engagement that reached `agent_id`'s surface:
/// the cycle is spent, and the one-time early heads-up for a slow-converging
/// worker is escalated when it lands strictly before the budget is spent.
pub(super) fn decide_reengaged(
    state: &CorrectionState,
    agent_id: &str,
    policy: &CorrectionConfig,
) -> (CorrectionState, Vec<DriveEffect>) {
    let mut correction = state.clone();
    let cycles = correction.record_reengagement(agent_id);
    let mut effects = Vec::new();
    if cycles == policy.escalate_after_cycles
        && policy.escalate_after_cycles < policy.max_cycles
        && correction.take_early_flag(agent_id)
    {
        effects.push(DriveEffect::PublishEscalation(Escalation {
            agent_id: agent_id.to_string(),
            verdict: CORRECTION_SLOW_VERDICT.to_string(),
            command: format!(
                "correction cycle {cycles} of {} — worker is converging slowly",
                policy.max_cycles
            ),
        }));
    }
    (correction, effects)
}

/// The effects of the configured `on_exhausted` action for a branch whose
/// correction budget is spent, which also record it in the exit summary.
///
/// `escalate` flags the branch to the orchestrator/human through the loop's
/// escalation path; `abandon` marks it failed without an escalation. Neither
/// re-engages the branch again.
fn exhaustion_effects(
    correction: &CorrectionState,
    status: &[AgentStatusRow],
    policy: &NudgePolicy<'_>,
    agent_id: &str,
) -> Vec<DriveEffect> {
    let cycles = correction.cycles(agent_id);
    let on_exhausted = policy.correction.on_exhausted;
    let mut effects = Vec::new();
    if on_exhausted == OnExhausted::Escalate {
        effects.push(DriveEffect::PublishEscalation(Escalation {
            agent_id: agent_id.to_string(),
            verdict: CORRECTION_EXHAUSTED_VERDICT.to_string(),
            command: format!(
                "branch is unrecoverable after {cycles} correction cycle(s); needs a human"
            ),
        }));
    }
    effects.push(DriveEffect::RecordExhausted(ExhaustedBranch {
        agent_id: agent_id.to_string(),
        cycles,
        policy: on_exhausted,
    }));
    if policy.learnings_enabled {
        effects.push(DriveEffect::RecordLearning {
            category: crate::broker::learnings::CATEGORY_CORRECTION_EXHAUSTED,
            title: "correction budget exhausted",
            body: exhaustion_learning_body(agent_id, worker_cli(status, agent_id), cycles),
        });
    }
    effects
}

/// The next step of the send-verify-recover sequence, decided from the
/// verify capture taken after a send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecoveryStep {
    /// The text was submitted.
    Delivered,
    /// The text is still on the input line and a recovery attempt remains:
    /// clear (`C-u`), re-type, `Enter`.
    Retry,
    /// The text is still on the input line after every recovery attempt:
    /// escalate it as wedged rather than retry further.
    Wedged,
}

/// Decides the next step after `attempts_made` recovery attempts (0 for the
/// initial send) from `verify`, the capture taken after the last `Enter`.
///
/// Judged by the input box's content ([`nudge_submitted`]), never by exact
/// equality with `text`, so a wrapped rendering is recognised as still
/// present. At most [`NUDGE_RECOVERY_ATTEMPTS`] recoveries are made: a bare
/// second `Enter` is a proven no-op against a stale line, so each recovery
/// re-establishes the input, and a truly wedged surface fails fast.
pub(super) fn decide_recovery_step(verify: &str, text: &str, attempts_made: u8) -> RecoveryStep {
    if nudge_submitted(verify, text) {
        RecoveryStep::Delivered
    } else if attempts_made < NUDGE_RECOVERY_ATTEMPTS {
        RecoveryStep::Retry
    } else {
        RecoveryStep::Wedged
    }
}

/// The `nudge-wedged` escalation for `text` stuck on `agent_id`'s input line,
/// flattened and capped for the broker.
pub(super) fn wedged_escalation(agent_id: &str, text: &str) -> Escalation {
    Escalation {
        agent_id: agent_id.to_string(),
        verdict: NUDGE_WEDGED_VERDICT.to_string(),
        command: one_line_capped(text),
    }
}

// ---------------------------------------------------------------------------
// Exit summary
// ---------------------------------------------------------------------------

/// The drive loop's final state, as the shell hands it to [`summarize`].
pub(super) struct LoopState<'a> {
    /// Why the loop exited.
    pub(super) outcome: DriveOutcome,
    /// Every escalation the run published, then the wedged nudges, in order.
    pub(super) escalations: Vec<Escalation>,
    /// Branches whose correction budget was spent.
    pub(super) exhausted: Vec<ExhaustedBranch>,
    /// The coding-agent roster as last read.
    pub(super) agents: &'a [AgentPane],
    /// The final `/status` snapshot.
    pub(super) latest_status: &'a [AgentStatusRow],
    /// The most-progressed phase ever observed per agent (GP-05).
    pub(super) resolved_phase: &'a HashMap<String, WorkerPhase>,
    /// The repository's default branch name, for the merge text.
    pub(super) default_branch: &'a str,
    /// Broker log pointer for the summary.
    pub(super) broker_log_hint: Option<String>,
    /// Learnings file pointer for the summary.
    pub(super) learnings_hint: Option<String>,
}

/// Decides the exit summary from the loop's final state and each coding
/// branch's merge state as git reported it (`drive-loop-exit-summary-truth`),
/// returning the summary and the wind-down learning effect, if any.
///
/// Merged-ness is an input, never inferred from loop or phase state. A
/// comparison git could not make (an `Err`) counts as unmerged — the fail-safe
/// direction (D2), because this summary is the input to a destructive `purge`.
/// A wave the loop reported finished (`Completed` / `EscalatedForReview`) is
/// only `Completed` when every branch is merged (D3); each unmerged branch
/// becomes an `[unmerged]` entry in the summary's escalations. It is never
/// published: nothing is typed or sent during wind-down (GP-16).
pub(super) fn summarize(
    state: LoopState<'_>,
    merge_states: &HashMap<String, Result<MergeState, PawError>>,
) -> (DriveSummary, Vec<DriveEffect>) {
    let LoopState {
        mut outcome,
        mut escalations,
        exhausted,
        agents,
        latest_status,
        resolved_phase,
        default_branch,
        broker_log_hint,
        learnings_hint,
    } = state;
    let coding_ids: Vec<String> = agents.iter().map(|a| a.agent_id.clone()).collect();

    if matches!(
        outcome,
        DriveOutcome::Completed | DriveOutcome::EscalatedForReview
    ) {
        for agent in agents {
            let detail = match merge_states.get(&agent.agent_id) {
                Some(Ok(state)) if state.merged => None,
                Some(Ok(state)) => Some(format!(
                    "{}: {} commit(s) not in {default_branch} (tip {})",
                    agent.branch, state.ahead, state.tip
                )),
                Some(Err(e)) => Some(format!(
                    "{}: could not compare against {default_branch}: {e}",
                    agent.branch
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

    let mut effects = Vec::new();
    if !escalations.is_empty() {
        effects.push(DriveEffect::RecordLearning {
            category: LEARNING_CATEGORY,
            title: "unattended wave wind-down synthesis",
            body: winddown_learning_body(outcome, &escalations),
        });
    }

    // Wave-level "N/M branches merged" line: the ref-derived count, printed
    // for every outcome with coding agents (not only `Completed`) — a
    // `heartbeat` exit benefits from the same truth as a finished one.
    let merged_summary = (!coding_ids.is_empty()).then(|| {
        let merged = merge_states
            .values()
            .filter(|r| matches!(r, Ok(s) if s.merged))
            .count();
        format!("{merged}/{} branches merged", coding_ids.len())
    });

    let summary = DriveSummary {
        outcome,
        agent_states: resolved_agent_states(
            &coding_ids,
            latest_status,
            resolved_phase,
            merge_states,
            default_branch,
        ),
        escalations,
        exhausted,
        broker_log_hint,
        learnings_hint,
        merged_summary,
    };
    (summary, effects)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &str) -> AgentPane {
        AgentPane {
            agent_id: id.to_string(),
            worktree_path: format!("/repo-{id}").into(),
            branch: format!("feat/{id}"),
        }
    }

    fn row(id: &str, status: &str) -> AgentStatusRow {
        AgentStatusRow {
            agent_id: id.to_string(),
            status: status.to_string(),
            last_seen_seconds: 0,
            cli: String::new(),
            modified_files: Vec::new(),
        }
    }

    fn merged() -> MergeState {
        MergeState {
            merged: true,
            ahead: 0,
            tip: String::new(),
        }
    }

    fn unmerged(ahead: usize) -> MergeState {
        MergeState {
            merged: false,
            ahead,
            tip: "abc1234".to_string(),
        }
    }

    // --- source audit -----------------------------------------------------

    /// Spec scenario "Decisions perform no I/O": the decision module's
    /// production code names no terminal port, broker client, process, file
    /// system or clock read.
    #[test]
    fn decisions_perform_no_io() {
        let source = include_str!("decide.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("the module has production code before its tests");
        for forbidden in [
            "TerminalBackend",
            "PaneEnumerator",
            "PaneCapture",
            "KeyDispatcher",
            "DriveDeps",
            "BrokerClient",
            "broker::client",
            "broker::publish",
            "fetch_status_over_http",
            "StatusFetcher",
            "MessageObserver",
            "MergeCheck",
            "std::process",
            "Command::new",
            "std::fs",
            "std::net",
            "Instant::now",
            "SystemTime::now",
            "Clock",
            "std::env",
            "duration_from_env_ms",
            "effective_poll_interval",
            "PaneClaim",
            "find_session_for_repo",
            "session::",
        ] {
            assert!(
                !production.contains(forbidden),
                "the drive-loop decision module must not use `{forbidden}`"
            );
        }

        // The text scan above cannot see I/O hidden behind a helper the
        // decisions import from the shell, so that import list is pinned: a
        // new import fails here until it is reviewed for purity.
        let imports = production
            .split("use super::{")
            .nth(1)
            .and_then(|rest| rest.split("};").next())
            .expect("the decisions import their helpers through `use super::{...}`");
        let mut imported: Vec<&str> = imports
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .collect();
        imported.sort_unstable();
        let mut allowed = [
            "AgentPane",
            "AgentStatusRow",
            "CORRECTION_EXHAUSTED_VERDICT",
            "CORRECTION_SLOW_VERDICT",
            "CorrectionState",
            "DedupWindow",
            "DriveOutcome",
            "DriveSummary",
            "Escalation",
            "ExhaustedBranch",
            "LEARNING_CATEGORY",
            "MergeState",
            "NUDGE_RECOVERY_ATTEMPTS",
            "NUDGE_WEDGED_VERDICT",
            "PERMISSION_PROMPT_VERDICT",
            "WorkerPhase",
            "approval_dedup_key",
            "cli_unavailable_dedup_key",
            "cli_unavailable_line",
            "dedup_shape",
            "exhaustion_learning_body",
            "friction_learning_body",
            "gate_failure",
            "is_finished_worker",
            "is_live_prompt",
            "nudge_submitted",
            "one_line_capped",
            "pane_has_buffered_input",
            "reengagement_text",
            "worker_cli",
        ];
        allowed.sort_unstable();
        assert_eq!(
            imported, allowed,
            "the decisions' helper imports changed: every helper a decision calls must stay \
             pure (no port, clock, env, file, process or broker access) — review each new \
             import for I/O, then update this allowlist"
        );
    }

    // --- decide_prompt ----------------------------------------------------

    const LIVE_LS_PROMPT: &str =
        "Bash command\n  ls -la\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n(esc to cancel)";
    const SESSION_LIMIT: &str = "working…\nYou've hit your session limit · resets 3pm";

    fn coding(id: &str) -> SurfaceRole {
        SurfaceRole::Coding(id.to_string())
    }

    /// Calls the prompt decision on `capture` for `role` with the embedded
    /// profile — data in, state + effects out.
    fn prompt(
        state: &DedupWindow,
        role: &SurfaceRole,
        capture: &str,
        now: Instant,
    ) -> (DedupWindow, Vec<DriveEffect>) {
        let profile = crate::config::claude_prompt_profile().clone();
        decide_prompt(
            state,
            &SurfaceObservation {
                role,
                pane_index: 1,
                capture,
                profile: &profile,
                settle_delay: Duration::from_millis(250),
            },
            now,
        )
    }

    fn fresh_dedup() -> DedupWindow {
        DedupWindow::new(Duration::from_mins(5))
    }

    /// Spec scenario "A prompt decision is tested with data alone".
    #[test]
    fn a_live_prompt_yields_escalation_effects_and_no_keystroke() {
        let (_, effects) = prompt(
            &fresh_dedup(),
            &coding("feat-a"),
            LIVE_LS_PROMPT,
            Instant::now(),
        );
        let [
            DriveEffect::RecordManualDecision { agent_id, captured },
            DriveEffect::PublishEscalation(escalation),
            DriveEffect::RecordLearning { category, .. },
        ] = effects.as_slice()
        else {
            panic!("manual decision, escalation, learning — in that order: {effects:?}");
        };
        assert_eq!(agent_id, "feat-a");
        assert_eq!(captured, LIVE_LS_PROMPT);
        assert_eq!(escalation.agent_id, "feat-a");
        assert_eq!(escalation.verdict, "permission-prompt");
        assert_eq!(escalation.command, "ls -la");
        assert_eq!(*category, "tooling_friction");
        assert!(
            !effects.iter().any(|e| matches!(
                e,
                DriveEffect::SubmitBufferedInput { .. } | DriveEffect::Reengage { .. }
            )),
            "no keystroke effect for a live prompt"
        );
    }

    /// Spec scenario "A repeated prompt within the dedup window yields no
    /// effect".
    #[test]
    fn a_repeated_prompt_inside_the_window_yields_nothing() {
        let first = Instant::now();
        let (seen, _) = prompt(&fresh_dedup(), &coding("feat-a"), LIVE_LS_PROMPT, first);
        let (after, effects) = prompt(
            &seen,
            &coding("feat-a"),
            LIVE_LS_PROMPT,
            first + Duration::from_mins(1),
        );
        assert!(effects.is_empty(), "{effects:?}");
        assert_eq!(after, seen, "dedup state unchanged");
    }

    #[test]
    fn a_prompt_still_unanswered_after_the_window_is_escalated_again() {
        let first = Instant::now();
        let (seen, _) = prompt(&fresh_dedup(), &coding("feat-a"), LIVE_LS_PROMPT, first);
        let (_, effects) = prompt(
            &seen,
            &coding("feat-a"),
            LIVE_LS_PROMPT,
            first + Duration::from_mins(6),
        );
        assert_eq!(effects.len(), 3, "{effects:?}");
    }

    #[test]
    fn buffered_input_in_an_idle_box_is_submitted() {
        let capture = "● Ran tool\n╭────────────────────────────────╮\n│ > /opsx:apply my-change        │\n╰────────────────────────────────╯\n  ? for shortcuts";
        let (_, effects) = prompt(&fresh_dedup(), &coding("feat-a"), capture, Instant::now());
        assert_eq!(
            effects,
            vec![DriveEffect::SubmitBufferedInput {
                agent_id: "feat-a".to_string(),
                pane_index: 1,
                settle_delay: Duration::from_millis(250),
            }]
        );
    }

    #[test]
    fn an_idle_surface_with_nothing_buffered_yields_nothing() {
        let (after, effects) = prompt(
            &fresh_dedup(),
            &coding("feat-a"),
            "● Done.\n  ? for shortcuts",
            Instant::now(),
        );
        assert!(effects.is_empty(), "{effects:?}");
        assert_eq!(after, fresh_dedup());
    }

    #[test]
    fn a_cli_unavailable_banner_is_escalated_verbatim() {
        let (_, effects) = prompt(
            &fresh_dedup(),
            &coding("feat-a"),
            SESSION_LIMIT,
            Instant::now(),
        );
        assert_eq!(
            effects,
            vec![DriveEffect::PublishEscalation(Escalation {
                agent_id: "feat-a".to_string(),
                verdict: "cli-unavailable".to_string(),
                command: "You've hit your session limit · resets 3pm".to_string(),
            })]
        );
    }

    /// Spec scenario "A repeated CLI-unavailable banner within the dedup
    /// window yields no effect" — keyed on agent + marker, so a reset time
    /// that changed between sweeps is still the same banner.
    #[test]
    fn a_repeated_cli_unavailable_banner_inside_the_window_yields_nothing() {
        let first = Instant::now();
        let (seen, _) = prompt(&fresh_dedup(), &coding("feat-a"), SESSION_LIMIT, first);
        let (after, effects) = prompt(
            &seen,
            &coding("feat-a"),
            "working…\nYou've hit your session limit · resets 4pm",
            first + Duration::from_mins(1),
        );
        assert!(effects.is_empty(), "{effects:?}");
        assert_eq!(after, seen, "dedup state unchanged");
    }

    #[test]
    fn a_non_coding_surface_yields_nothing() {
        for role in [SurfaceRole::Dashboard, SurfaceRole::Unknown] {
            for capture in [LIVE_LS_PROMPT, SESSION_LIMIT] {
                let (after, effects) = prompt(&fresh_dedup(), &role, capture, Instant::now());
                assert!(effects.is_empty(), "{role:?}: {effects:?}");
                assert_eq!(after, fresh_dedup());
            }
        }
    }

    // --- decide_message ---------------------------------------------------

    use crate::broker::messages::{
        AdvancedMainPayload, ArtifactPayload, FeedbackPayload, QuestionPayload,
    };

    fn question(from: &str, text: &str) -> BrokerMessage {
        BrokerMessage::Question {
            agent_id: from.to_string(),
            payload: QuestionPayload {
                question: text.to_string(),
            },
        }
    }

    fn artifact(from: &str, status: &str) -> BrokerMessage {
        BrokerMessage::Artifact {
            agent_id: from.to_string(),
            payload: ArtifactPayload {
                status: status.to_string(),
                exports: Vec::new(),
                modified_files: Vec::new(),
            },
        }
    }

    fn gate_feedback(to: &str, error: &str) -> BrokerMessage {
        BrokerMessage::Feedback {
            agent_id: to.to_string(),
            payload: FeedbackPayload {
                from: "supervisor".to_string(),
                errors: vec![error.to_string()],
            },
        }
    }

    fn advanced_main(base: &str) -> BrokerMessage {
        BrokerMessage::AdvancedMain {
            payload: AdvancedMainPayload {
                from: "supervisor".to_string(),
                merged_branch: "feat/feat-a".to_string(),
                new_main_sha: "0123abc".to_string(),
                base: base.to_string(),
                merged_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
                summary: None,
            },
        }
    }

    /// Calls the message decision for coding agents `feat-a` / `feat-b`, gate
    /// tag `test`, branch refresh on.
    fn message(
        state: &CorrectionState,
        messages: &[BrokerMessage],
        winding_down: bool,
    ) -> (CorrectionState, Vec<DriveEffect>) {
        let coding_ids = ["feat-a".to_string(), "feat-b".to_string()];
        let gate_tags = ["test".to_string()];
        decide_message(
            state,
            messages,
            &MessagePolicy {
                coding_ids: &coding_ids,
                gate_tags: &gate_tags,
                winding_down,
                branch_refresh_enabled: true,
            },
        )
    }

    /// Spec scenario "A question and a merge candidate are judgment calls in
    /// the message decision".
    #[test]
    fn a_question_and_a_merge_candidate_are_judgment_calls() {
        let (after, effects) = message(
            &CorrectionState::new(),
            &[
                question("feat-a", "which config file?"),
                artifact("feat-b", "committed"),
            ],
            false,
        );
        assert_eq!(
            effects,
            vec![
                DriveEffect::NoteJudgmentCall {
                    agent_id: "feat-a".to_string(),
                    call: JudgmentCall::Question {
                        question: "which config file?".to_string(),
                    },
                },
                DriveEffect::NoteJudgmentCall {
                    agent_id: "feat-b".to_string(),
                    call: JudgmentCall::MergeCandidate {
                        status: "committed".to_string(),
                    },
                },
            ],
            "judgment calls only: no keystroke and no broker publish"
        );
        assert_eq!(after, CorrectionState::new());
    }

    #[test]
    fn a_done_artifact_is_a_merge_candidate_and_other_statuses_are_not() {
        let (_, done) = message(
            &CorrectionState::new(),
            &[artifact("feat-a", "done")],
            false,
        );
        assert_eq!(done.len(), 1, "{done:?}");
        for status in ["working", "verified", "blocked", "idle"] {
            let (_, effects) = message(
                &CorrectionState::new(),
                &[artifact("feat-a", status)],
                false,
            );
            assert!(effects.is_empty(), "{status}: {effects:?}");
        }
    }

    #[test]
    fn judgment_calls_are_not_noted_during_wind_down() {
        let (_, effects) = message(
            &CorrectionState::new(),
            &[
                question("feat-a", "which config file?"),
                artifact("feat-b", "committed"),
            ],
            true,
        );
        assert!(effects.is_empty(), "{effects:?}");
    }

    /// Spec scenario "Messages from non-coding senders yield nothing".
    #[test]
    fn messages_from_non_coding_senders_yield_nothing() {
        let (after, effects) = message(
            &CorrectionState::new(),
            &[
                question("supervisor", "anyone there?"),
                artifact("stranger", "committed"),
                gate_feedback("stranger", "[test] 3 failed"),
            ],
            false,
        );
        assert!(effects.is_empty(), "{effects:?}");
        assert_eq!(after, CorrectionState::new(), "correction state unchanged");
    }

    #[test]
    fn gate_failure_feedback_marks_the_branch_for_correction() {
        let (after, effects) = message(
            &CorrectionState::new(),
            &[gate_feedback("feat-a", "[test] 3 failed")],
            false,
        );
        assert!(effects.is_empty(), "{effects:?}");
        assert_eq!(
            after.pending(),
            vec![(
                "feat-a".to_string(),
                reengagement_text("test", "[test] 3 failed")
            )]
        );
    }

    #[test]
    fn untagged_feedback_does_not_mark_the_branch() {
        let (after, _) = message(
            &CorrectionState::new(),
            &[gate_feedback("feat-a", "please tidy the docs")],
            false,
        );
        assert_eq!(after, CorrectionState::new());
    }

    #[test]
    fn an_observed_merge_triggers_a_refresh_even_though_its_sender_is_not_a_coding_agent() {
        let (_, effects) = message(&CorrectionState::new(), &[advanced_main("main")], false);
        assert_eq!(
            effects,
            vec![DriveEffect::RefreshBranches {
                base: "main".to_string(),
            }]
        );
    }

    #[test]
    fn an_observed_merge_yields_no_refresh_while_refresh_is_disabled() {
        let coding_ids = ["feat-a".to_string()];
        let (_, effects) = decide_message(
            &CorrectionState::new(),
            &[advanced_main("main")],
            &MessagePolicy {
                coding_ids: &coding_ids,
                gate_tags: &[],
                winding_down: false,
                branch_refresh_enabled: false,
            },
        );
        assert!(effects.is_empty(), "{effects:?}");
    }

    // --- decide_nudge / decide_reengaged / decide_recovery_step -----------

    fn loopback(max_cycles: u32, escalate_after_cycles: u32) -> CorrectionConfig {
        CorrectionConfig {
            auto_loopback: true,
            max_cycles,
            escalate_after_cycles,
            on_exhausted: OnExhausted::Escalate,
            ..CorrectionConfig::default()
        }
    }

    /// A correction state with `feat-a` marked by a failing `test` gate after
    /// `cycles` re-engagements already sent.
    fn marked(cycles: u32) -> CorrectionState {
        let mut state = CorrectionState::new();
        for _ in 0..cycles {
            state.mark_gate_failure("feat-a", "earlier".to_string());
            state.record_reengagement("feat-a");
        }
        state.mark_gate_failure("feat-a", "fix the tests".to_string());
        state
    }

    fn nudge(
        state: &CorrectionState,
        status: &[AgentStatusRow],
        correction: &CorrectionConfig,
        learnings_enabled: bool,
    ) -> (CorrectionState, Vec<DriveEffect>) {
        let panes = HashMap::from([("feat-a".to_string(), 1)]);
        decide_nudge(
            state,
            &NudgeObservation {
                panes: &panes,
                status,
            },
            &NudgePolicy {
                correction,
                learnings_enabled,
                clis: &HashMap::new(),
            },
        )
    }

    #[test]
    fn a_marked_branch_is_reengaged_with_its_feedback() {
        let (after, effects) = nudge(
            &marked(0),
            &[row("feat-a", "committed")],
            &loopback(3, 2),
            false,
        );
        assert_eq!(
            effects,
            vec![DriveEffect::Reengage {
                agent_id: "feat-a".to_string(),
                pane_index: 1,
                text: "fix the tests".to_string(),
                settle_delay: Duration::from_millis(crate::DEFAULT_SUBMIT_DELAY_MS),
            }]
        );
        assert_eq!(after, marked(0), "no cycle is spent before the send");
    }

    #[test]
    fn nothing_is_reengaged_while_auto_loopback_is_off() {
        let (after, effects) = nudge(
            &marked(0),
            &[row("feat-a", "committed")],
            &CorrectionConfig::default(),
            true,
        );
        assert!(effects.is_empty(), "{effects:?}");
        assert_eq!(after, marked(0));
    }

    #[test]
    fn a_finished_worker_or_an_unresolved_surface_is_kept_pending_not_nudged() {
        for status in ["verified", "done"] {
            let (after, effects) =
                nudge(&marked(0), &[row("feat-a", status)], &loopback(3, 2), false);
            assert!(effects.is_empty(), "{status}: {effects:?}");
            assert_eq!(after.pending().len(), 1, "{status}: still pending");
        }
        let mut other = CorrectionState::new();
        other.mark_gate_failure("feat-b", "fix".to_string());
        let (after, effects) = nudge(&other, &[], &loopback(3, 2), false);
        assert!(
            effects.is_empty(),
            "no surface resolved for feat-b: {effects:?}"
        );
        assert_eq!(after, other);
    }

    #[test]
    fn a_spent_budget_applies_the_exhaustion_policy_once() {
        let status = [row("feat-a", "committed")];
        let (after, effects) = nudge(&marked(3), &status, &loopback(3, 2), true);
        let [
            DriveEffect::PublishEscalation(escalation),
            DriveEffect::RecordExhausted(branch),
            DriveEffect::RecordLearning { category, .. },
        ] = effects.as_slice()
        else {
            panic!("escalation, exhausted record, learning: {effects:?}");
        };
        assert_eq!(escalation.verdict, "correction-exhausted");
        assert_eq!(
            *branch,
            ExhaustedBranch {
                agent_id: "feat-a".to_string(),
                cycles: 3,
                policy: OnExhausted::Escalate,
            }
        );
        assert_eq!(*category, "correction_exhausted");

        let mut again = after.clone();
        again.mark_gate_failure("feat-a", "still failing".to_string());
        let (_, effects) = nudge(&again, &status, &loopback(3, 2), true);
        assert!(
            effects.is_empty(),
            "never re-engaged or re-exhausted: {effects:?}"
        );
    }

    #[test]
    fn abandon_records_the_branch_without_an_escalation_or_unconsented_learning() {
        let policy = CorrectionConfig {
            on_exhausted: OnExhausted::Abandon,
            ..loopback(1, 1)
        };
        let (_, effects) = nudge(&marked(1), &[row("feat-a", "committed")], &policy, false);
        assert!(
            matches!(effects.as_slice(), [DriveEffect::RecordExhausted(_)]),
            "{effects:?}"
        );
    }

    #[test]
    fn a_sent_reengagement_spends_a_cycle_and_heads_up_once_before_the_budget() {
        let policy = loopback(3, 2);
        let (one, effects) = decide_reengaged(&marked(0), "feat-a", &policy);
        assert_eq!(one.cycles("feat-a"), 1);
        assert!(one.pending().is_empty(), "the feedback was delivered");
        assert!(effects.is_empty(), "{effects:?}");

        let (two, effects) = decide_reengaged(&marked(1), "feat-a", &policy);
        assert_eq!(two.cycles("feat-a"), 2);
        let [DriveEffect::PublishEscalation(heads_up)] = effects.as_slice() else {
            panic!("one early heads-up: {effects:?}");
        };
        assert_eq!(heads_up.verdict, "correction-slow");

        let (_, effects) = decide_reengaged(&two, "feat-a", &loopback(4, 2));
        assert!(effects.is_empty(), "the heads-up fires once: {effects:?}");
    }

    #[test]
    fn no_heads_up_when_it_would_coincide_with_the_spent_budget() {
        let (_, effects) = decide_reengaged(&marked(1), "feat-a", &loopback(2, 2));
        assert!(effects.is_empty(), "{effects:?}");
    }

    #[test]
    fn recovery_retries_a_stale_line_then_declares_it_wedged() {
        let stale = "╭──────────────╮\n│ > please continue │\n╰──────────────╯";
        assert_eq!(
            decide_recovery_step(stale, "please continue", 0),
            RecoveryStep::Retry
        );
        assert_eq!(
            decide_recovery_step(stale, "please continue", 1),
            RecoveryStep::Retry
        );
        assert_eq!(
            decide_recovery_step(stale, "please continue", 2),
            RecoveryStep::Wedged
        );
        let submitted = "╭──────────────╮\n│ >              │\n╰──────────────╯";
        assert_eq!(
            decide_recovery_step(submitted, "please continue", 2),
            RecoveryStep::Delivered
        );
    }

    #[test]
    fn a_wedged_escalation_is_one_line_and_capped() {
        let escalation =
            wedged_escalation("feat-a", &format!("line one\nline two {}", "x".repeat(700)));
        assert_eq!(escalation.verdict, "nudge-wedged");
        assert!(!escalation.command.contains('\n'));
        assert!(escalation.command.starts_with("line one line two "));
        assert!(escalation.command.ends_with(" ..."));
    }

    // --- summarize --------------------------------------------------------

    /// Final loop state for a wave whose two workers both reached `verified`.
    fn summary_of(
        outcome: DriveOutcome,
        merge_states: &HashMap<String, Result<MergeState, PawError>>,
    ) -> (DriveSummary, Vec<DriveEffect>) {
        let agents = [agent("feat-a"), agent("feat-b")];
        let status = [row("feat-a", "verified"), row("feat-b", "verified")];
        let resolved = HashMap::new();
        summarize(
            LoopState {
                outcome,
                escalations: Vec::new(),
                exhausted: Vec::new(),
                agents: &agents,
                latest_status: &status,
                resolved_phase: &resolved,
                default_branch: "main",
                broker_log_hint: None,
                learnings_hint: None,
            },
            merge_states,
        )
    }

    /// Spec scenario "The exit summary takes merge truth as an input".
    #[test]
    fn summary_takes_merge_truth_as_an_input() {
        let merge_states = HashMap::from([
            ("feat-a".to_string(), Ok(merged())),
            ("feat-b".to_string(), Ok(unmerged(2))),
        ]);
        let (summary, effects) = summary_of(DriveOutcome::Completed, &merge_states);
        assert_ne!(summary.outcome, DriveOutcome::Completed);
        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        assert_eq!(
            summary.escalations,
            vec![Escalation {
                agent_id: "feat-b".to_string(),
                verdict: "unmerged".to_string(),
                command: "feat/feat-b: 2 commit(s) not in main (tip abc1234)".to_string(),
            }]
        );
        assert_eq!(
            summary.merged_summary.as_deref(),
            Some("1/2 branches merged")
        );
        assert!(
            matches!(
                effects.as_slice(),
                [DriveEffect::RecordLearning {
                    title: "unattended wave wind-down synthesis",
                    ..
                }]
            ),
            "an escalated wind-down records one synthesis learning: {effects:?}"
        );
    }

    #[test]
    fn summary_completes_only_when_every_branch_is_merged() {
        let merge_states = HashMap::from([
            ("feat-a".to_string(), Ok(merged())),
            ("feat-b".to_string(), Ok(merged())),
        ]);
        let (summary, effects) = summary_of(DriveOutcome::Completed, &merge_states);
        assert_eq!(summary.outcome, DriveOutcome::Completed);
        assert!(summary.escalations.is_empty());
        assert!(effects.is_empty(), "nothing escalated, no learning");
        assert_eq!(
            summary.agent_states,
            vec![
                ("feat-a".to_string(), "verified · merged".to_string()),
                ("feat-b".to_string(), "verified · merged".to_string()),
            ]
        );
    }

    #[test]
    fn summary_counts_an_uncomparable_branch_as_unmerged() {
        let merge_states = HashMap::from([
            ("feat-a".to_string(), Ok(merged())),
            (
                "feat-b".to_string(),
                Err(PawError::BranchError("bad ref".to_string())),
            ),
        ]);
        let (summary, _) = summary_of(DriveOutcome::Completed, &merge_states);
        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        assert_eq!(summary.escalations.len(), 1);
        assert_eq!(summary.escalations[0].verdict, "unmerged");
        assert!(
            summary.escalations[0]
                .command
                .starts_with("feat/feat-b: could not compare against main:"),
            "{:?}",
            summary.escalations[0]
        );
    }

    /// A wave that finished with escalations already pending still gains an
    /// `[unmerged]` entry per unmerged branch.
    #[test]
    fn an_escalated_wave_still_reports_its_unmerged_branches() {
        let merge_states = HashMap::from([
            ("feat-a".to_string(), Ok(merged())),
            ("feat-b".to_string(), Ok(unmerged(1))),
        ]);
        let (summary, _) = summary_of(DriveOutcome::EscalatedForReview, &merge_states);
        assert_eq!(summary.outcome, DriveOutcome::EscalatedForReview);
        assert_eq!(summary.escalations.len(), 1, "{:?}", summary.escalations);
        assert_eq!(summary.escalations[0].agent_id, "feat-b");
        assert_eq!(summary.escalations[0].verdict, "unmerged");
    }

    #[test]
    fn summary_keeps_a_heartbeat_outcome_and_still_reports_merge_truth() {
        let merge_states = HashMap::from([
            ("feat-a".to_string(), Ok(unmerged(1))),
            ("feat-b".to_string(), Ok(unmerged(3))),
        ]);
        let (summary, effects) = summary_of(DriveOutcome::Heartbeat, &merge_states);
        assert_eq!(summary.outcome, DriveOutcome::Heartbeat);
        assert!(
            summary.escalations.is_empty(),
            "only a finished wave gains [unmerged] entries"
        );
        assert!(effects.is_empty());
        assert_eq!(
            summary.merged_summary.as_deref(),
            Some("0/2 branches merged")
        );
    }
}
