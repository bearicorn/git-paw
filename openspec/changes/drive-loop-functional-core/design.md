## Context

`drive_loop` already injects its effects (`DriveDeps`), so the loop is testable — but only by scripting
fakes and a fake clock, because each decision is computed in the middle of the I/O that feeds and
executes it. Some pieces are already pure (`live_prompt_in_tail`, `approval_dedup_key`,
`detect_completion`, and — from `unattended-heartbeat-progress`, archive position 7 —
`heartbeat_due(last_progress, now, interval)` and `tick_made_progress(prev_tips, tips,
phase_advanced)`, which the shell keeps calling as they are); the dashboard's MVU `update` is the
in-repo precedent.
`docs/src/architecture-direction.md` shows the target shape with a worked example, but that example
(`decide_approval` returning `Approve { option }`) predates v0.17.0 `classifier-retirement`, after
which the loop answers no prompt; this change rewrites it. The concept-decoupling skill fixes the
rules: inputs are observed facts passed in, effects may be returned as data, migrate one decision at
a time starting with the nearly pure ones.

Where the four decisions live today (after v0.17.0; `supervisor-pane-removal` has removed the
orchestrator-pane hand-off from the message arms by the time this lands):

| Decision | Today |
|---|---|
| Prompt | `sweep_pane` (capture → `cli_unavailable_line` → `is_live_prompt` / `submit_buffered_input` → dedup (`approval_dedup_key`) → `ManualDecisionSink::record_forwarded` → `record_escalation` → `tooling_friction` learning) |
| Message | `observe_worker_messages` (`drive.rs:1575-1620`): `agent.advanced-main` → merged base for `trigger_branch_refresh`; then, only for a coding agent's id, gate-failure `agent.feedback` → `CorrectionState::mark_gate_failure`, `agent.question` and merge-candidate `agent.artifact` → (v0.17) orchestrator-pane hand-off outside wind-down |
| Nudge | `run_correction_pass` → `send_reengagement` → `send_nudge` (`NudgeOutcome`, recovery attempts, wedged verdict), plus `submit_buffered_input` |
| Exit summary | the tail of `drive_loop` (merge truth from `MergeCheck`, unmerged escalations, wind-down learning, `resolved_agent_states`, the `N/M merged` line) |

## Goals / Non-Goals

**Goals:**
- Four pure decisions with captured-data tests; effects as a closed enum applied by the shell.
- An ordered effect log of what the loop did, carried in the run result.
- Byte-identical behavior: same keystrokes, same order, same manual-decision records, same broker
  messages, same summary text.

**Non-Goals:**
- Persisting the effect log or defining its file format (v0.22's replay work).
- Any behavior change (the headless hand-off is `orchestrator-escalation-handoff`, a behavior change
  with its own spec; manual-decision recording is v0.17.0's and is carried over as an effect
  unchanged).
- Renaming the existing ports.

## Decisions

### D1 — `decide(state, observation) → (state, effects)` per decision

`decide_prompt(&PromptState, &SurfaceObservation, now) -> (PromptState, Vec<DriveEffect>)`,
`decide_message(&CorrectionState, &[BrokerMessage], &MessagePolicy) -> (CorrectionState,
Vec<DriveEffect>)` (the policy carries the coding agents' ids, the gate tags and the wind-down flag),
`decide_nudge(&CorrectionState, &NudgeObservation, &NudgePolicy) -> (CorrectionState,
Vec<DriveEffect>)`, `summarize(&LoopState, &[(AgentId, Result<MergeState, _>)]) -> DriveSummary`.
Time is an input (`now`), never read inside. For a live permission prompt `decide_prompt` runs the
dedup and, when the key is new, returns `RecordManualDecision`, `PublishEscalation` and
`RecordLearning` in v0.17.0's order; it never returns a keystroke for that surface. The per-pane claim
stays in the shell: it is I/O whose *purpose* is to observe the world at the last moment, so the
keystroke effects (buffered-input submit, nudge) carry the surface and the shell executes them behind
the existing claim. The CLI-unavailable arm dedups on `(agent, matched marker)` exactly as today
(`drive.rs:1871`), so a banner that persists across sweeps yields one escalation per dedup window and
no effect in between.

`decide_message` keeps today's sender gate: `agent.advanced-main` is matched first (its sender is the
payload's `from`, never a coding agent), every other message is considered only when its agent id is
one of the session's coding agents, so the loop's own escalations never feed back. A question or a
merge candidate (an `agent.artifact` whose status is a merge candidate) observed outside wind-down is
returned as a `NoteJudgmentCall { agent, kind }` effect — the data `orchestrator-escalation-handoff`
extends with its hand-off effect. Applying it only appends to the effect log: after
`supervisor-pane-removal` the worker's broker message is itself the review item, so no surface, broker
or file write changes.

### D2 — A closed `DriveEffect` enum

Variants mirror what the loop does today: record a manual decision (through the `ManualDecisionSink`
port), publish an escalation, record a learning, submit text with a settle delay, send a key, refresh
branches after an observed merge (the existing rebase + notify path, still gated by its enablement
flag), rebase + notify a branch, mark a correction cycle, and note a judgment call (questions and merge
candidates; effect-log only in this change). There is no approval variant: no git-paw process answers a
permission prompt. An enum (not a trait) because the set is closed and the shell must handle each case
(the concept-decoupling enum-vs-trait rule).

### D3 — Effect log

The shell appends each effect after applying it, with its outcome (sent / recorded / skipped by
claim / failed). The log is in memory and returned with the summary; tests assert on it. Persisting it is left
to v0.22 (see Open Questions), so this change adds no file and no format.

### D4 — Order of extraction

Exit summary first (already nearly pure; merge truth is already an injected input), then the prompt
decision (the live gate and dedup key are pure; the rest is sequencing), then the message decision
(one loop over polled messages; it writes the correction state the nudge reads), then nudge (most
stateful). Each step lands behind the full drive-loop suite. `heartbeat_due` / `tick_made_progress`
are not touched.

### D5 — Observations gathered by the shell

The shell lists surfaces and owners (terminal port), resolves roles, captures tails, fetches `/status`,
reads new log entries by cursor, asks git for merge states, and reads the clock, then calls the
decisions. No decision holds a port.

## Risks / Trade-offs

- [Subtle reordering of keystrokes, manual-decision records or broker messages] → the effect log makes
  order assertable; e2e suites unchanged; argv fixtures from `terminal-backend-seam` catch tmux-level
  drift.
- [Decision signatures grow wide] → bundle inputs into observation/policy structs; the existing
  `SweepContext` / `CorrectionPass` bundles are the starting point.
- [Effect log memory on a long run] → bounded: one entry per action, and runs exit on the heartbeat.

## Migration Plan

1. `DriveEffect`, the shell's `apply`, the effect log; route existing calls through `apply` unchanged.
2. Extract `summarize`; table tests; suite green.
3. Extract `decide_prompt`; table tests over captured fixtures; suite green.
4. Extract `decide_message`; table tests; suite green.
5. Extract `decide_nudge`; table tests; suite green.
6. Docs, including the rewritten `architecture-direction.md` worked example.

Rollback: each step is a plain commit that compiles and passes `just check`.

## Open Questions

- Should the effect log be written to the session's log directory now (behind `[logging]`) so v0.22 has
  data from real runs? Recommendation: no — the format is v0.22's decision; an in-memory log costs
  nothing and keeps this change a refactor.
