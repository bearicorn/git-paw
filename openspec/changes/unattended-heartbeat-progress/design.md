## Context

`src/supervisor/drive.rs` (released v0.17.0):

- `HEARTBEAT_INTERVAL = Duration::from_mins(25)` (`:82`); `run_drive_loop` lets
  `GIT_PAW_DRIVE_HEARTBEAT_MS` override it for the e2e harness (`:3104`).
- `drive_loop` records `let start = deps.clock.now();` (`:1251`) and, at the bottom of every tick,
  `if deps.clock.now().duration_since(start) >= config.heartbeat { break (DriveOutcome::Heartbeat, …) }`
  (`:1417`).
- `run_drive_loop` prints the summary and returns `Ok` for every outcome except `CliUnavailable`.
- `MergeCheck::merge_state` (`GitMergeCheck`, `:2970`) answers the exit summary's merge question:
  for a merged branch it returns early with an empty `tip`; otherwise it runs three git commands
  (ancestor check, ahead count, short tip).

The heartbeat therefore fires 25 minutes after launch, whatever the workers are doing. The
dogfood symptom (v0.15.0, `nohup git paw start … --unattended &`: "sweeps + auto-approves for a
while, then `exited: heartbeat`, worker stalls at the next prompt for 79 minutes") is exactly that
budget expiring on a progressing wave. There is no TTY dependency to fix.

Since v0.17.0 (`classifier-retirement`) the loop approves nothing: it escalates every live prompt
once per dedup window (broker, summary, manual-decision log, a `tooling_friction` learning) and
keeps the rest of the wave moving. The symptom changes shape but not cause: after the early exit no
prompt is escalated, no buffered input is submitted and completion is never reported. The design origin
(roadmap appendix: the operator loop "exits to re-engage the human on … a ~25-min heartbeat";
"no progress across a heartbeat — checkbox AND commit count unchanged over ~25 min") was an
attended loop where a human is present to relaunch.

## Goals / Non-Goals

**Goals:**

- A detached unattended run that keeps making progress runs until it completes.
- A wave that makes no progress still re-engages the human after the same interval, with the same
  summary and outcome label.
- The heartbeat decision is pure and tested with the fake clock.

**Non-Goals:**

- A daemon or session host for the drive loop (`broker-host-separation` D1 keeps the loop in the
  foreground `--unattended` process; a host for it waits for the tmux/PTY spike).
- Surviving the tmux server's death or the `--unattended` process being killed.
- A user-facing setting for the interval (`GIT_PAW_DRIVE_HEARTBEAT_MS` stays a test override).
- Changing the exit status for `heartbeat` — that is v0.24 `error-exit-code-contract`.
- Re-binding a loop to a live session after it exits — that is `drive-loop-reattach` (archive
  position 10).

## Decisions

### D1 — Measure from the last progress, not from start

`drive_loop` keeps `last_progress: Instant`, initialised to the loop's start and advanced
whenever a tick observes progress. The exit check becomes
`heartbeat_due(last_progress, now, config.heartbeat)`, a pure function. This matches the
requirement's title and the roadmap's own definition of "no progress across a heartbeat".
`last_progress` is in-memory loop state: a loop that `drive-loop-reattach` binds to a live session
later starts its own interval at its own start.

*Alternatives considered:*
- **Exit only when detached** (no TTY on stdin/stdout). Rejected: it makes behaviour depend on how
  the process was launched, which is untestable in CI and surprising to a human who attaches later.
- **Surface status and keep running on heartbeat.** Rejected: a wave stalled on an escalated prompt
  would then run forever. The requirement forbids running indefinitely without re-engaging, and
  exiting is how an attended operator notices.
- **Raise the interval.** Rejected: any fixed budget strands a long enough wave.

### D2 — What counts as progress

A tick counts as progress when either of these is observed for a coding agent:

1. its branch tip differs from the tip observed on the previous tick (read through a narrow tip
   query on the existing `MergeCheck` port, backed by `git::branch_tip_short`; `MergeState.tip` is
   not reused because it is empty for a merged branch). A branch seen for the first time has no
   previous tip: its first read is recorded, not counted;
2. its resolved `WorkerPhase` rank rose (the loop already tracks `resolved_phase`).

Deliberately excluded:

- raw pane-output changes (spinners and status-line clocks change while an agent is wedged);
- arbitrary broker messages (status heartbeats are republished while idle);
- anything the loop does itself: an escalation, the re-escalation of a prompt still unanswered after
  its dedup window, the `tooling_friction` learning it records for each, a nudge or a buffered-input
  submit. Counting them would let one unanswered prompt keep the loop alive forever, because the
  loop re-escalates it every dedup window.

A worker that edits for 25 minutes with no commit and no phase change is treated as not
progressing — the same definition `sweep.sh detect-stuck` uses for no-progress, and the definition
`orchestrator-read-surface` (archive position 3, merged before this change) gives the broker's
`wave` stall tracking (a phase advancing or a branch tip moving, never the drive loop's own
escalations or learnings).

### D3 — Read tips cheaply

One tip read per coding branch per tick (15 s): one `git rev-parse --short` each. A failed read is
"no observation", never progress and never an error — the summary's ref-derived merge truth is
unaffected.

### D4 — The functional-core shape, first

`heartbeat_due` and `tick_made_progress` are free functions over plain values (instants, tip maps,
a phase-advance flag) and read no port. The shell gathers the tips and phase ranks and applies the
result. `drive-loop-functional-core` (archive position 14) lists both as existing pure decisions
and moves them into its core module unchanged.

## Risks / Trade-offs

- [Safety: the drive loop now runs for as long as a wave progresses, not at most 25 minutes] →
  Since v0.17.0 the loop approves nothing; the longer run only extends detection, escalation, nudges
  and buffered-input submits. Its keystrokes stay behind the live-prompt gate and the exclusive
  per-pane claim, and whether an agent's command runs is still decided by the agent CLI's own
  permission system or a human. The heartbeat was never a security control; it was a re-engagement
  timer. Stated in the user guide so an operator who relied on the 25-minute cap knows it is gone
  for progressing waves.
- [A worker that commits less often than every 25 minutes and publishes no phase change ends the
  run on `heartbeat`] → Accepted: that is the same outcome as today for such a wave, and the
  summary says so.
- [A wave whose agents all wait on unanswered prompts ends on `heartbeat`] → Intended. Nobody is
  answering; exiting with the escalation list in the summary is how the human is re-engaged. An
  earlier exit when every agent is blocked on a prompt is a separate follow-up and not part of this
  change.
- [Tip reads add git calls] → N `rev-parse`-class calls every 15 s; negligible beside the per-pane
  captures the loop already makes.

## Migration Plan

Behavioural, no data migration. A wave shorter than 25 minutes behaves identically. A longer,
progressing wave runs to completion instead of exiting on `heartbeat`.

## Open Questions

- If the tmux/PTY spike (after v0.18.0) adopts a session-host daemon that owns the drive loop,
  re-check that this decision moved with the loop unchanged. The cause here is the interval's
  reference point, which no host change fixes, so the change is needed either way.
