## Why

The drive loop (`src/supervisor/drive.rs`, 8,660 lines in v0.17.0, about 3,225 production) is the actuator the
relocation keeps. Its side effects already sit behind traits (`PaneEnumerator`, `PaneCapture`,
`KeyDispatcher`, `StatusFetcher`, `MessageObserver`, `Clock`, `AlertSink`, `LearningSink`,
`BranchRefresher`, `MergeCheck`, `SessionInstanceGuard`, and v0.17.0's `ManualDecisionSink`), but its
*decisions* are interleaved with the calls that capture and type: `sweep_pane` captures, scans for
CLI unavailability, checks liveness, submits buffered input, dedups, records the manual decision,
escalates and records a learning in one body; `observe_worker_messages` polls new broker messages
and, in the same loop, marks gate failures on the correction state, collects merge events for the
branch refresh and routes questions and merge candidates (`drive.rs:1575-1620`);
`send_reengagement` / `send_nudge` / `submit_buffered_input` decide and act together; the exit
summary is assembled inside `drive_loop`
from loop state and `MergeCheck` calls. So every decision is tested through fakes and a fake clock, and
nothing records *what the loop decided* except the side effects themselves.

The target architecture's functional core / imperative shell applies here: `observe → decide → apply`,
with only `decide(state, observation) → (state, effects)` pure and effects returned as data. Effects as
data also give an ordered record of every action the loop took — the audit trail v0.22's replay
timeline is planned to project. Archive position 14.

## What Changes

- Extract four decisions — the three the roadmap names plus the message decision the escalation
  hand-off needs — one at a time, each behavior-preserving:
  - **Prompt** — live gate → dedup → escalate: a pure function of the surface's role, its captured
    tail and the worker's resolved prompt profile plus the dedup state, returning the next dedup state
    and the effects (record the manual decision, publish the `permission-prompt` escalation, record
    the friction learning; record a CLI-unavailable escalation; submit buffered input; or nothing).
    Since v0.17.0 `classifier-retirement` there is no approval decision: no classification, no option
    selection, no approval keystroke.
  - **Nudge** — correction-loop re-engagement, buffered-input submission and nudge recovery: a pure
    function of the correction state, the worker's broker status, the captured tail and the wind-down
    flag.
  - **Message** (`decide_message`) — over the broker messages observed since the cursor: a gate-failure
    `agent.feedback` marks the correction state, an `agent.advanced-main` yields a branch-refresh
    effect, and a coding agent's `agent.question` or merge-candidate `agent.artifact` (outside wind-down)
    is returned as a **judgment call** — data naming the agent and the question or merge status. In this
    change a judgment call is only noted in the effect log (after `supervisor-pane-removal` it stays a
    broker review item); messages from senders that are not the session's coding agents yield nothing.
  - **Exit summary** — a pure function of the loop's final state and the *observed* merge state of each
    branch (passed in, never inferred from loop state).
- **Existing pure decisions stay where they are:** `unattended-heartbeat-progress` (archive position 7)
  already extracted `heartbeat_due(last_progress, now, interval)` and `tick_made_progress(prev_tips,
  tips, phase_advanced)`; the shell keeps calling them and this change does not re-extract them.
- **Effects as data.** A closed `DriveEffect` enum; the shell applies effects in order through the
  ports and appends each applied effect to an ordered in-memory effect log carried in the run result.
- **Observations as inputs.** The shell gathers observations (surfaces and owners via the terminal
  port, captures, `/status`, new log entries by cursor, merge states, the clock) and passes them in; no
  decision reads a port.
- No CLI, config, wire, persisted-state or behavior change; existing drive-loop suites pass unchanged.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `code-architecture`: drive-loop decisions are pure functions over observed inputs with effects
  returned as data, applied in order by the shell and recorded in an effect log.

## Impact

- **Predecessors (implementation order):** archive position 14, after `orchestrator-read-surface` (3,
  messages by cursor), `supervisor-pane-removal` (6, no orchestrator branches left in the sweep or in
  `observe_worker_messages`), `unattended-heartbeat-progress` (7, `heartbeat_due` /
  `tick_made_progress` already pure), `pane-less-session-parity` (9, live roster),
  `drive-loop-reattach` (10, the loop bound to a live session instance), `terminal-backend-seam` (12,
  observations and actions through the port) and `broker-host-separation` (13).
  v0.17 changes whose requirements become these decisions' contracts and must land first:
  `live-prompt-tail-window` (*Live-prompt gate*), `classifier-retirement` (*Every live permission
  prompt is escalated without blocking the wave*, *Approval keystrokes are sent only by an explicit
  approve action*, *Manual-decision log file* — the drive loop's escalation records, absorbed from
  `drive-loop-manual-decision-log`), `idle-pane-nudge-submission` (*send-keys nudges send a follow-up
  Enter*, *Idle pane with buffered input is submitted*), `cli-unavailable-detection-gaps` (*CLI
  unavailability…*); and the archived `drive-loop-exit-summary-truth` (ref-derived merge truth, now an
  input). This change modifies none of their requirements.
- **Successor:** `orchestrator-escalation-handoff` (20) attaches its headless hand-off effect to the
  message decision's judgment calls (questions and merge decisions) and to the nudge decision's
  correction-exhausted branch — not to the prompt decision (a live permission prompt is never handed
  off) and not to the early heads-up.
- **Code:** a core module for the drive loop's pure decisions and `DriveEffect` (e.g.
  `src/supervisor/decide.rs`), `src/supervisor/drive.rs` shrinking to the observe/apply shell; the
  existing side-effect traits stay as ports.
- **Tests:** decision tests become table tests over captured fixtures (no fakes, no clock); existing
  drive-loop unit tests and `tests/unattended_drive_e2e.rs` unchanged; an effect-log ordering test.
- **Docs:** `docs/src/architecture.md` (drive loop structure), `docs/src/architecture-direction.md`
  "Where the code is today" Drive loop row and its worked example, which still shows a
  `decide_approval` returning `Approve { option }`; it is rewritten to the escalate-only prompt
  decision.
- **Enum-variant ripple:** `BrokerMessage` unchanged (the decisions match on variants they already
  match on, co-located). `DriveEffect` is new and closed; its consumers are the shell and the effect log.
- **Security / safety:** none changed — the per-pane claim runs in the shell exactly where it runs
  today, and no decision can produce a keystroke that answers a permission prompt.

- **Dependencies:** none.

## Rebase note (2026-10-20)

v0.17.0 `classifier-retirement` removed the approval path this change used to extract
(`classify_prompt`, option selection, `send_approval`, `AlertSink::log_approval`) and dropped
`broad-grant-scope-safety`. The approval decision became the prompt decision (live gate → dedup →
manual-decision record → escalation → friction learning, in v0.17.0's order), and
`ManualDecisionSink` joins the ports.

## Rebase note (2026-10-25): audit against released v0.17.0

- Fixed the successor sentence: `orchestrator-escalation-handoff` never attaches to `decide_prompt`
  (its own D10/D11 say the prompt decision never returns a hand-off); it attaches to the new message
  decision and to correction-exhausted (design call 5).
- Added a fourth pure decision, `decide_message`, over the messages `observe_worker_messages`
  (`drive.rs:1575-1620`) handles today — gate-failure feedback, merge events, questions and merge
  candidates — so the hand-off has a decision to extend. Requirement, scenarios, design D1/D2/D4 and
  a task section added.
- Added a "repeated CLI-unavailable banner → no effect" fixture: that arm dedups too
  (`drive.rs:1871`, keyed on agent + marker).
- `heartbeat_due` / `tick_made_progress` are listed as existing pure decisions from
  `unattended-heartbeat-progress` (7); not re-extracted.
- Line counts corrected (8,660 total, about 3,225 production); ordering prose updated to archive
  position 14.
