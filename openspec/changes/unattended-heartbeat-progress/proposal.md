## Why

A detached `git paw start --unattended` (under `nohup`, no TTY) sweeps for a while, prints
`Unattended drive loop exited: heartbeat`, and exits; the workers then run on with no drive loop:
no live prompt is escalated, no buffered input is submitted, and the run never reports
completion. (In v0.15.0 dogfood the workers stalled at their next approval prompt; since v0.17.0
the loop approves nothing either way, but it is the only thing that escalates and keeps the wave
moving.) The roadmap records this as a heartbeat check
that misbehaves without a terminal. The code says otherwise: nothing in the loop looks at the
terminal. `drive_loop` breaks with `DriveOutcome::Heartbeat` once
`clock.now() - start >= HEARTBEAT_INTERVAL` (25 minutes) — measured from the **loop's start**, not
from the last sign of progress. Every wave that takes longer than 25 minutes ends there, attended
or not. Attended, a human sees the summary and relaunches; detached, nobody does. The peer-4-poker
run on 0.17.0 hit the same wall (PP-24, HIGH): every unattended wave got about 25 minutes of
pumping, then none.

No other v0.18.0 change fixes it. `broker-host-separation` moves the broker into its own process
but keeps the drive loop in the foreground `--unattended` process (its D1 option A), and no other
v0.18 change touches the heartbeat. The requirement's own title already says the heartbeat fires
"after prolonged **inactivity**"; the code implements prolonged runtime.

## What Changes

- The heartbeat measures time since the wave last made **progress**, not time since the loop
  started. Progress is a coding branch's tip moving or a coding agent's phase advancing, and
  nothing else. Nothing the loop does itself counts: its escalations, its re-escalations of a prompt
  still unanswered, and the learnings it records. A wave that keeps progressing is never cut off; a
  wave that makes no progress for the heartbeat interval (including one whose agents all wait on
  unanswered prompts) still exits with a summary, exactly as today.
- The decision becomes a pure function of (last progress, now, interval). It is the first drive-loop
  decision extracted in the functional-core shape; `drive-loop-functional-core` (archive position
  14) lists it as an existing pure decision and extracts the loop's other decisions the same way.
- A detached run is tested end to end: no TTY, workers that keep committing past the heartbeat
  interval, and the run reaches `completed`.

The exit set and the summary format are unchanged. The production constants
(`HEARTBEAT_INTERVAL` = 25 min, `POLL_INTERVAL` = 15 s) are unchanged.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `supervisor-unattended-operation`: *Heartbeat re-engages the human after prolonged inactivity*
  — the interval is measured from the last observed progress, and progress is defined.

## Impact

- **Predecessors (implementation order):** archive position 7.
  - After `orchestrator-read-surface` (3): same progress definition as its broker `wave` stall
    tracking (a phase advancing or a branch tip moving, never the drive loop's own escalations or
    learnings), so `git paw status` and the heartbeat agree on what "no progress" means.
  - After `supervisor-pane-removal` (6): it removes the orchestrator-pane paths from the same loop
    (`orchestrator_pane_index`, `hand_to_orchestrator`, the orchestration nudge), so the progress
    tracker observes coding agents only. The heartbeat check stays at the bottom of the tick, after
    the correction pass.
  - No earlier v0.18 change modifies *Heartbeat re-engages the human after prolonged inactivity*;
    the MODIFIED delta is written on the released v0.17.0 text.
  - Builds on v0.17 `classifier-retirement`: the loop sends no approval keystrokes, so "an approval
    the loop dispatched" is not a progress kind. Every live prompt is escalated once per dedup
    window, and *Every live permission prompt is escalated without blocking the wave* already lists
    the heartbeat as an exit for a wave whose agents all wait on prompts; this change keeps that
    exit by not counting re-escalations or the learnings they record.
- **Successors:**
  - `drive-loop-reattach` (10): re-running `start --unattended` binds a new loop to the live session
    after a heartbeat exit; this change makes such exits rare on a progressing wave.
  - `drive-loop-functional-core` (14): lists `heartbeat_due` and `tick_made_progress` as existing
    pure decisions and moves them into its core module unchanged.
  - `orchestrator-escalation-handoff` (20): adds an exit-summary requirement; touches neither this
    requirement nor its decision.
- **Code:** `src/supervisor/drive.rs` — `drive_loop` tracks the last-progress instant; a pure
  `heartbeat_due(last_progress, now, interval)` decision replacing the
  `clock.now().duration_since(start) >= config.heartbeat` check; a per-tick branch-tip read through
  a narrow tip query on the existing `MergeCheck` port (one `git rev-parse --short` per branch,
  `git::branch_tip_short`). `MergeState.tip` is not reused: `GitMergeCheck::merge_state` returns
  an empty tip for a merged branch and runs three git commands per call. Escalations,
  re-escalations and learnings do not feed the tracker. `run_drive_loop` wiring is unchanged
  (`GIT_PAW_DRIVE_HEARTBEAT_MS` stays the test override).
- **Tests:** drive-loop unit tests with the fake clock (progress resets the heartbeat; no progress
  fires it; only the two progress kinds count; re-escalating an unanswered prompt does not defer
  it); `tests/unattended_drive_e2e.rs` gains a detached run-to-completion test; the existing
  `unattended_heartbeat_exits_with_summary` (idle dummies) still passes unchanged.
- **Docs:** `docs/src/user-guide/supervisor.md` (unattended heartbeat wording),
  `docs/src/cli-reference.md` and `docs/src/user-guide/tiered-model-workflow.md` ("~25-minute
  heartbeat"), README unattended paragraph; `start --unattended --help` does not mention the
  heartbeat today.
- **Enum-variant ripple:** none (`DriveOutcome` unchanged; `BrokerMessage` untouched).
- **Security / safety:** see design — the drive loop is no longer capped at 25 minutes of
  wall-clock time on a progressing wave. Since v0.17.0 it approves nothing, so the longer run only
  extends detection, escalation, nudges and buffered-input submits; its keystrokes stay behind the
  live-prompt gate and the per-pane claim.
- **Dependencies:** none.
- **Cross-version:**
  - v0.24 `error-exit-code-contract` gives the `heartbeat` outcome a distinct non-zero exit status,
    so a detached wrapper can tell an exit for inactivity from completion. Until then a heartbeat
    exit returns `0`, as today.
  - The v0.19 roadmap section lists this pump exit as an item "to fix alongside", and the roadmap
    routes "the heartbeat exit" to v0.24.0; this release now owns it and both entries drop it.

## Rebase note (2026-10-25): audit against released v0.17.0

- Pulled forward from the v0.24 spec branch into v0.18 at the user's request because of PP-24
  (peer-4-poker on 0.17.0: unattended waves get about 25 minutes of pumping, then none). Archive
  position 7: after `orchestrator-read-surface` (3) and `supervisor-pane-removal` (6), before
  `drive-loop-reattach` (10), `drive-loop-functional-core` (14) and
  `orchestrator-escalation-handoff` (20). Ordering prose rewritten to match.
- `heartbeat_due` is now the first pure decision extracted in the functional-core shape;
  `drive-loop-functional-core` lists it as existing rather than this change rebasing over it.
- Code premises re-checked against v0.17.0: `HEARTBEAT_INTERVAL` (`drive.rs:82`), the
  `duration_since(start)` check (`:1417`), `DriveOutcome::Heartbeat`, `GIT_PAW_DRIVE_HEARTBEAT_MS`
  (`:3104`) and `run_drive_loop` returning `Ok` for every outcome but `cli-unavailable` all hold.
- Correction: `MergeState.tip` is empty when the branch is merged (`GitMergeCheck`, `:2970-2989`),
  so the tip read is a narrow tip query on the same port, not `merge_state`.
- The MODIFIED header matches the released *Heartbeat re-engages the human after prolonged
  inactivity* (`supervisor-unattended-operation/spec.md:211`); no earlier v0.18 change modifies it.
- Docs list widened to every page that says "~25-minute heartbeat" (`cli-reference.md:132`,
  `tiered-model-workflow.md:117`, README).
- `error-exit-code-contract` stays in v0.24 and is now a forward reference.
