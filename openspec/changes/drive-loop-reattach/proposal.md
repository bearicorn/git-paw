## Why

An unattended wave cannot be re-driven once its drive loop exits (PP-24, HIGH, peer-4-poker on
0.17.0). The loop runs only at the end of a fresh launch: re-running `git paw start --unattended`
against the repository's live session hits the GP-13 fork guard, which refuses a non-interactive
caller (`reattach_active_session_or_refuse`, `src/main.rs:342`), and none of the Active / Paused /
Stopped branches in `cmd_supervisor` (`src/commands/supervisor.rs:415-436`) takes `unattended`.
So every unattended wave gets one loop's worth of pumping (about 25 minutes until
`unattended-heartbeat-progress`), then none, and the only remedy is tearing the session down. After
the relocation the drive loop is the actuator an external orchestrator (re)starts on a live
session; it has to be restartable without a relaunch.

## What Changes

- **Re-running `git paw start --unattended` binds a new drive loop to the repository's live
  supervised session** instead of refusing or forking:
  - **Active:** the loop runs against the existing session as is. No pane is created or
    rebuilt, nothing is injected, the terminal is not attached.
  - **Paused:** the session is revived as `git paw start` revives it today (dashboard and broker
    back, pending prompts submitted behind the readiness gate), then the loop runs.
  - **Stopped:** the session is recovered as `git paw start` recovers it today, then the loop runs.
  - The loop drives the session's own roster and broker endpoint; launch-selection flags
    (`--branches`, `--specs`, `--from-all-specs`, `--cli`) are not applied to a live session, so
    re-running the original launch command re-binds.
- **At most one drive loop per session instance.** Every loop, fresh or re-bound, holds a
  drive-loop binding for its session instance. A second `start --unattended` while a live loop
  holds it refuses before changing anything, naming the session and the holding process. A binding
  that stopped being refreshed (a killed loop) lapses after a bounded window.
- **A loop of a torn-down instance never drives the next one (GP-06).** `git paw stop` revokes the
  binding, and a re-attach on a stopped session supersedes any binding left over. A loop that finds
  its binding revoked or taken over exits on that tick as `session-torn-down`, before acting.
  Recovery keeps the receipt's `created_at`, so without this a loop that missed a quick
  stop-then-recover would go on driving the recovered session.
- A **bare** session (started without supervisor mode) is refused with an actionable error: the
  loop would submit the boot prompts a bare session leaves for the human to send.
- Without `--unattended` the GP-13 behavior is unchanged (interactive attach, non-interactive
  refusal). The re-bound loop is the same loop: it approves nothing.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `supervisor-unattended-operation`: a drive loop can be bound to a live supervised session; at most
  one drive loop drives a session instance, and stop revokes its binding.
- `tmux-orchestration`: *Start reattaches to an existing live session instead of forking* —
  with `--unattended`, start binds a drive loop to the live session instead of attaching or
  refusing; it still never forks.

## Impact

- **Predecessors (implementation order):** archive position 10.
  - After `supervisor-pane-removal` (6): there is no orchestrator pane, so a re-attach has nothing
    to re-inject; the loop sweeps coding surfaces only.
  - After `unattended-heartbeat-progress` (7): a progressing wave rarely exits on the heartbeat,
    so a re-attach is the remedy for a stalled or interrupted one, not a 25-minute chore.
  - After `pane-less-session-parity` (9): the revived-from-pause path submits pending prompts
    behind the readiness gate (a gate failure fails the re-attach before any loop runs), and the
    loop re-reads the live roster every sweep, so agents added with `git paw add` are driven.
  - No earlier v0.18 change modifies *Start reattaches to an existing live session instead of
    forking*; the MODIFIED delta is written on the released v0.17.0 text. The ADDED requirements
    sit beside *The drive loop is bound to a specific session instance*, which stays unchanged.
- **Successors:**
  - `broker-host-separation` (13): the broker host survives the dashboard, so an Active re-attach
    finds the broker running; the revive-from-pause path recreates the host and the dashboard. The
    loop still runs in the foreground `--unattended` process (its D1).
  - `drive-loop-functional-core` (14): the binding check stays a shell observation made before
    each tick's decisions.
  - `cmd-library-surface` (15): the start use case moves into the library and reports "already
    running" in its outcome; the entry layer then attaches, refuses, or (with `--unattended`) runs
    the drive loop, and the binding is taken where the loop is run. The requirements here are
    phrased by behavior, not by function name, so they hold across that move.
- **Code:** `src/commands/supervisor.rs` (the existing-session branch: Active / Paused / Stopped
  take `unattended`; bare-session refusal; the drive-loop entry builds its roster and broker URL
  from the live session), `src/commands/start.rs` and `src/commands/recover.rs` (revive / recover
  without attaching the terminal when a loop follows), `src/supervisor/loop_binding.rs` (new: the
  drive-loop binding's acquire, refresh and release, modelled on `claim.rs`),
  `src/supervisor/drive.rs` (`FileSessionInstanceGuard` also checks the binding),
  `src/commands/stop.rs` (revokes the binding), `src/cli.rs` (`start` help).
- **Persisted state:** one gitignored binding file per repository under `.git-paw/tmp/`; no
  session-receipt field changes.
- **CLI:** no new flag or subcommand; `start --unattended` on a live supervised session binds a
  loop instead of refusing.
- **Tests:** `tests/unattended_drive_e2e.rs` (re-bind after a heartbeat exit; concurrent second
  loop refused; Paused and Stopped re-attach; bare session refused),
  `tests/supervisor_reattach_integration.rs` (non-`--unattended` refusal unchanged),
  `tests/e2e_supervisor_stop.rs` (stop revokes the binding), binding unit tests (refusal, lapse,
  other instance, supersede), drive-loop unit test (a lost binding ends the loop before it acts).
- **Docs:** `docs/src/user-guide/supervisor.md` (what to run after a heartbeat exit; one loop per
  session; stop revokes), `docs/src/cli-reference.md` (`start --unattended` on a live session),
  README unattended paragraph, `start --help`.
- **Enum-variant ripple:** none (`DriveOutcome` unchanged; a lost binding reuses
  `session-torn-down`; `BrokerMessage`, `SpecBackendKind` untouched).
- **Security / safety:** no new process, pane or keystroke path; the re-bound loop approves nothing
  (`classifier-retirement`). The single-loop guard removes a doubled actuator (two loops nudging or
  submitting into the same panes). The binding file is advisory, holds no secret, and is built
  from the repository root and a fixed name; tampering can only end a loop or delay a re-attach,
  both fail-safe. See design.
- **Dependencies:** none.
