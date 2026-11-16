## Why

Since v0.17.0 a recovered worker relaunches with its recorded CLI session id, so its conversation
comes back. Three gaps showed up in the v0.20.0 runs:

- **#26 — resuming a conversation that never began.** `attach_agent` assigns and persists the id
  *before* the CLI starts, so recovery can resume a pane that died early. If the session is stopped
  before the worker's first turn, the CLI never created the conversation. `stop` then
  `start --unattended` relaunched both workers with `--resume <id>`; the CLI printed "No conversation
  found" and exited, leaving a bare shell. git-paw has no idea a recorded id may name nothing.
- **#32 — resumed workers sit idle.** Recovery relaunches the panes and types nothing. A resumed CLI
  waits at its prompt, so the orchestrator has to type "continue" into every pane by hand.
- **DF-26 / DV-11 — host sleep.** A laptop sleep kills in-flight worker turns or leaves a worker's
  test shell hung, and nothing re-engages the session afterwards. In one run a six-hour sleep left both
  workers stuck until a human noticed.

## What Changes

- **Resume only a conversation that exists.** Recovery decides per worker between *resume*, *fresh
  with a new id*, and *fresh without an id*. It asks the CLI family whether the recorded conversation
  exists (Claude: its transcript file, which git-paw already locates for usage). A family that cannot
  tell falls back to a git-paw marker, `conversation_started`, recorded when git-paw submits the
  worker's first prompt. An unknown answer resumes, as today.
- **A failed resume falls back once.** When a resumed pane is a bare shell at the readiness gate (the
  CLI exited), git-paw relaunches it once with a new session id and records that id.
- **A worker relaunched fresh gets its launch prompt** (boot block, task prose, worktree rule), as a
  started worker does, instead of the resume message.
- **A resumed worker gets a resume nudge.** Once its pane is ready, git-paw submits a short bundled
  "you were resumed, continue" message. It is on by default and can be turned off or reworded
  (`[workers] resume_nudge`, `[workers] resume_message`). This applies to attended `start` and to
  `start --unattended` recovery. It does not apply to a paused restart or a reattach, where the CLIs
  never stopped.
- **The drive loop detects a host sleep and re-engages.** Each tick compares wall-clock time with
  monotonic time. When the wall clock advanced by more than `[drive] wake_gap_seconds` (default 120)
  beyond the monotonic clock, the loop logs the wake and, per worker, relaunches an exited CLI under
  the same resume rules, nudges an idle one, and escalates a pane that has been busy since before the
  sleep to the orchestrator instead of interrupting it.
- **Attended sessions after a sleep are out of scope.** No git-paw process runs a loop in an attended
  session; see the design's open question.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `session-state`:
  - MODIFIED *Recovery relaunches a worker with its recorded session id* (resume only a conversation
    not known to be absent).
  - ADDED: the `conversation_started` marker; how recovery decides whether a conversation exists; the
    fresh relaunch with a new recorded id; the one-time fallback after a failed resume; the launch
    prompt for a fresh relaunch; the resume nudge and its configuration; the nudge only reaching a
    ready pane.
- `cli-provider`: ADDED a CLI family may report whether a session's conversation exists.
- `supervisor-unattended-operation`: ADDED wake detection by wall-clock gap; re-engaging idle and
  exited workers after a wake; reporting busy workers after a wake.

## Impact

- **Code:**
  - `src/agent_cli/` (provider `conversation_exists`; the Claude row reuses `usage.rs`'s transcript
    lookup);
  - `src/session.rs` (`WorktreeEntry.conversation_started`);
  - `src/commands/recover.rs` (a pure `resume_decision`, the post-launch gate, fallback, launch prompt
    and nudge);
  - `src/commands/launch.rs` (marker set when the first prompt is submitted);
  - `src/commands/start.rs` (`submit_held_boot_prompts` sets the marker);
  - `src/supervisor/drive.rs` (`Clock` gains a wall clock; the wake check and re-engage effects);
  - `src/config/` (`[workers] resume_nudge` / `resume_message`, `[drive] wake_gap_seconds`);
  - `assets/resume-nudge.md` and `assets/wake-nudge.md` (bundled texts).
- **Persisted state:** one optional field on `WorktreeEntry`. Old files load with it absent, and an
  absent marker reads as "unknown", so their behaviour is unchanged.
- **Tests:** pure decision table; recovery e2e with a fake CLI that exits on an unknown resume id;
  nudge e2e; drive-loop wake test through the fake clock.
- **Docs:** `docs/src/user-guide/session-lifecycle.md` (recovery, the nudge, sleep), the
  configuration reference (three keys), and the orchestrator skill (no need to type "continue").
- **Enum-variant ripple:** a new `DriveEffect` variant for wake re-engagement and a new escalation
  kind; matches in `drive.rs`/`decide.rs` only.
- **Cross-change sequencing:**
  - `start-resume-summary` (this release) prints `resumes session <id>` / `starts fresh` from a
    `resume_mode(entry, cli_profile)` extracted from `cli_command_for_recovery`. This change replaces
    that decision with `resume_decision`, which can also say "fresh, conversation never started".
    Whichever lands second rebases. The summary renders the decision this change makes, and it is
    printed after the post-launch fallback so it reports what actually ran.
  - `supervisor-vocabulary-split` creates `[workers]` and `[drive]`. If this change lands first, the
    keys sit under `[supervisor]` and move with the split's migration table.
  - `session-state-locking`: recording a new session id or the marker is a read-modify-write through
    its store, taken after the readiness gates, never held across them.
  - `session-state-reconciliation` owns repairs (missing worktrees, dead brokers). This change does
    not touch them.
