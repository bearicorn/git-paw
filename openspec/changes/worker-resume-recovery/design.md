## Context

**Session ids.** `attach_agent` assigns a UUIDv4 when the worker's CLI resolves a `session_id_args`
template. It does this before the CLI starts, "so recovery can resume it even if the pane dies
moments later" (`src/commands/launch.rs:333-344`). The id is stored in `WorktreeEntry.cli_session_id`
(`src/session.rs:260`). `launch.rs:336-367` is its only production writer, and nothing clears or
rotates it. `stop` kills tmux and marks the session stopped but keeps the id
(`src/commands/stop.rs:46-62`). `LaunchSession` is `None | Assign(id) | Resume(id)`
(`src/agent_cli/profile.rs:17-24`), rendered by `launch_command` (`profile.rs:517-522`). The Claude
row renders `--session-id abc` on a fresh launch and `--resume u1` on recovery
(`src/agent_cli/tests.rs:522`, `:420`; resume template `src/agent_cli/mod.rs:433-434`).

**Recovery.** `cli_command_for_recovery` (`src/commands/recover.rs:36-63`) resumes whenever an id is
recorded (`recover.rs:45-48`). `recover_session_detached` (`recover.rs:102-281`) rebuilds the panes,
saves the session as active, starts the broker host and rewrites the discovery file. It types nothing
into a worker pane: no readiness gate, no boot prompt, no message. A boot prompt held by `add` into a
paused session (`src/commands/add.rs:379-385`) is not submitted on this path either. The paused
restart (`src/commands/start.rs:1254`) keeps the live panes, whose CLIs "keep their conversations"
(`start.rs:1237-1240`). It submits held prompts through `submit_held_boot_prompts`
(`start.rs:1460-1580`): resolve the pane live (`start.rs:1501`), gate it
(`gate_pane_or_fail_on_dialog`, `start.rs:1531`), `take()` and submit (`start.rs:1540-1542`), and
warn rather than fail on a blocked pane. `--unattended` on an existing session goes through
`main.rs:605-626` (`drive_existing`: bind, `revive_for_drive`, run the loop). `revive_for_drive`
(`src/commands/supervisor.rs:1070-1095`) calls the same `restart_from_pause_detached` /
`recover_session_detached` as attended `start`.

**Readiness and sending.** `gate_pane_or_fail_on_dialog` (`launch.rs:473`) wraps
`tmux::gate_pane_for_injection` (`src/tmux/readiness.rs:279`). The gate classifies a pane as ready,
at a dialog, or a bare shell (`readiness.rs:144`), relaunches a bare shell with the command it was
given (`readiness.rs:183`, `:367`), and answers known dialogs. `submit_prompt_to_pane`
(`launch.rs:519`) sends text, waits the settle delay, then presses Enter.

**Transcripts.** `usage.rs` already finds a Claude session's transcript. `claude_project_slug`
(`src/agent_cli/usage.rs:387`) and `claude_config_dirs` (`usage.rs:397`: the entry's relocated dir,
`$CLAUDE_CONFIG_DIR`, `~/.claude`) give `claude_transcripts` (`usage.rs:425`):
`<config>/projects/<slug(worktree)>/<id>.jsonl`, falling back to any `projects/*/<id>.jsonl`.

**Drive loop.** `drive_loop` (`src/supervisor/drive.rs:1324`, loop at `:1424-1607`) sleeps
`POLL_INTERVAL` (15 s, `drive.rs:90`) at the end of each tick (`drive.rs:1606`). Its `Clock`
(`drive.rs:959-964`) returns only a monotonic `Instant`. The loop keeps `start` and `last_progress`
(`drive.rs:1380-1384`) but not the previous tick's time, so a host sleep between ticks goes
unnoticed. Its only re-engagement is the gate-failure loopback (`src/supervisor/drive/decide.rs:436`)
and buffered-input submission (`decide.rs:67`), sent through `send_reengagement` / `send_nudge` /
`submit_and_verify` (`drive.rs:2213`, `:2288`, `:2365`). The heartbeat exit after 25 minutes without
progress (`drive.rs:107`) is measured on the monotonic clock.

## Goals / Non-Goals

**Goals:**
- Never relaunch a worker into a resume its CLI will refuse.
- A recovered worker carries on without anyone typing into its pane.
- An unattended wave survives a host sleep without a human.

**Non-Goals:**
- Attended sessions after a sleep (open question).
- Killing or interrupting a hung tool call.
- Resuming conversations of CLIs without a resume template (unchanged: they start fresh).
- The resume summary's wording (`start-resume-summary`).

## Decisions

### D1 — How git-paw knows the conversation exists: ask the CLI, fall back to a marker

`resume_decision(entry, profile, probe) -> Resume(id) | FreshAssign | FreshNone` is pure over three
facts:

1. **The CLI's own answer.** `CliProvider::conversation_exists(query) -> Option<bool>`.
   - The Claude row returns `Some(true)` when `claude_transcripts` finds the main `<id>.jsonl`, and
     `Some(false)` when every config dir was searched and none has it.
   - Rows without a transcript source return `None`.
   - This is the same file `claude --resume` looks up, so it is the most direct evidence there is.
2. **The git-paw marker.** `WorktreeEntry.conversation_started: Option<bool>`.
   - It is `Some(false)` when the id is assigned (`launch.rs:336`).
   - It becomes `Some(true)` when git-paw submits that worker's first prompt into a ready pane: the
     start and `add` boot prompt, a held prompt (`start.rs:1540`), or the prompt after a fresh
     relaunch.
   - Session files written before this change have no marker, which reads as `None`.
3. **Rule.**
   - Probe `Some(true)` → resume.
   - Probe `Some(false)` → fresh with a new id.
   - Probe `None` and marker `Some(false)` → fresh with a new id.
   - Otherwise → resume (today's behaviour).
   - No id, or no resume template → fresh without resume arguments, as today.

*Why both:* the probe is exact but only Claude has one today. The marker works for every CLI but
cannot see a CLI that crashed after git-paw typed the prompt. *Alternatives rejected:*
- Marker only: wrong when the prompt was typed but never reached the CLI, as in a dialog
  (feedback #25 and #30).
- Probe only: does nothing for Codex or custom CLIs.
- Parsing "No conversation found" from the pane: CLI- and locale-specific text, and only seen after
  the failure. D3 covers that case without reading the text.

### D2 — A fresh relaunch always gets a new id

A fresh relaunch renders `LaunchSession::Assign(new uuid)` and records the new id and
`conversation_started = Some(false)` in a single session-store update after the panes are built.
Reusing the old id with `--session-id` was rejected: if the CLI did create something under that id
(the probe said `None`), Claude refuses an id already in use.

### D3 — A failed resume falls back once, through the gate's relaunch

After recovery, each resumed pane goes through the readiness gate. The gate is given the *fresh*
command as its relaunch command, so a pane that is a bare shell (the CLI exited on an unknown
conversation) is relaunched once with a new id. The id is recorded and a warning names the branch.
A second bare shell is reported as the gate reports it today. This catches the cases the probe and
marker get wrong without parsing CLI output.

### D4 — What a recovered worker is sent

- **Resumed:** the resume nudge, a bundled asset (`assets/resume-nudge.md`) overridable with
  `[workers] resume_message`. It says git-paw restarted the session, the previous turn may have been
  cut off, and the worker should re-read its assignment sidecar and continue.
  `[workers] resume_nudge = false` turns it off.
- **Fresh:** the launch prompt, rebuilt with `compose_launch_prompt` (`launch.rs:110-121`) from the
  boot block, the task prose of the entry's `spec_id` when it still resolves (otherwise the no-spec
  prose), and the worktree rule. A held `pending_boot_prompt` is used as-is when present. A fresh CLI
  has no context, so "continue" would mean nothing to it. The marker is set on submission.
- Both go through the `submit_held_boot_prompts` pattern: live pane resolution, gate, submit, and a
  warning instead of a failure when blocked.
- The paused restart and the reattach send nothing new, because their CLIs never stopped.

This runs inside recovery, so attended `start` and `revive_for_drive` share it. Attended `start`
waits for the gates before it attaches. This is bounded by the readiness budget, and the gates run
for all panes before the attach.

### D5 — Wake detection: wall clock against monotonic clock

`Clock` gains `wall_now() -> SystemTime`. Each tick stores both readings. A host sleep shows up as
`wall_delta - mono_delta > wake_gap`, where `wake_gap` is `[drive] wake_gap_seconds` (default 120,
`0` disables). This works because `Instant` excludes suspend on macOS and Linux, and `SystemTime`
does not. A long tick (slow `git`, a blocked broker) grows both clocks, so it is never mistaken for a
sleep. A clock step (NTP, a manual change) of more than the gap is a known false positive and only
costs one idle-worker nudge. The fake clock in tests drives both clocks.

### D6 — Re-engaging after a wake

Decided purely from the tick's pane views, then applied by the shell:

- **CLI exited (bare shell):** relaunch with the D1 decision and D3 fallback, then send the D4
  message.
- **CLI idle at its prompt:** submit the wake message (`assets/wake-nudge.md`: the host slept, an
  interrupted turn or a hung command should be re-run), unless `resume_nudge = false`.
- **CLI busy:** do not touch it. Publish one escalation to the orchestrator naming the pane and the
  sleep length, since a hung test shell (DV-11) looks busy. Interrupting it with Ctrl-C was rejected:
  git-paw cannot tell a hung command from a long one.

The loop prints one `host woke after <duration>` line. The heartbeat is not reset: it runs on the
monotonic clock, which did not count the sleep.

## Risks / Trade-offs

- [The Claude transcript layout changes] → the probe returns `None` when no projects dir exists at
  all, so the marker and D3 still apply. The layout is already a dependency of `usage.rs`.
- [A nudge typed while the human is typing into an attended pane after recovery] → recovery panes
  are freshly launched, and the gate waits for an idle prompt. No nudge is ever sent to an attended
  session outside recovery.
- [Attended `start` waits longer before attaching] → one readiness gate per worker, as the paused
  restart already does for held prompts.

## Resolved Questions (2026-11-17)

- **Waking an attended session:** no nudge command and no `start` prompt in v0.21.0.
- **Missing recorded conversations:** left to `session-state-reconciliation`.
