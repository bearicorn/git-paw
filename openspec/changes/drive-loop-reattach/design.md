## Context

Released v0.17.0:

- `git paw start --unattended` always dispatches to `cmd_supervisor` (`--unattended` implies
  supervisor mode, `src/main.rs:82-89`). Its first step looks the repository's session up
  (`commands/supervisor.rs:408-437`) and, for a live one, branches on the effective status:
  Paused → `restart_from_pause`, Active → `reattach_active_session_or_refuse` (GP-13: attach on a
  TTY, refuse otherwise, `main.rs:342`), Stopped → `recover_session`. None of the three receives
  `unattended`; each ends in `attach_or_print_hint` or the refusal.
- The drive loop runs only at the end of a fresh launch: `drive_unattended_loop`
  (`commands/supervisor.rs:964`) builds the roster from a `Session`, re-reads the receipt's
  `created_at` (whole seconds, the frozen serializer) and calls `drive::run_drive_loop`. It takes
  the broker URL from the current `[broker]` config.
- GP-06's guard, `FileSessionInstanceGuard` (`drive.rs:2664`), re-reads the receipt every tick and
  stays current while the name and `created_at` match and the status is not `Stopped`. Recovery and
  restart-from-pause clone the receipt, so both keep `created_at`.
- No drive-loop lock exists. The repo has two advisory-file patterns: `.git-paw/.add-remove.lock`
  (`src/lock.rs`, atomic `create_new`, manual removal when stale) and the per-pane approval claim
  (`src/supervisor/claim.rs`, atomic `create_new`, mtime TTL steal).
- A bare session pre-fills its boot prompts without submitting them; the drive loop submits an idle
  pane's buffered input (*Idle pane with buffered input is submitted*).

By archive position 10, `supervisor-pane-removal` (6) has removed the orchestrator pane,
`unattended-heartbeat-progress` (7) measures the heartbeat from the last progress, and
`pane-less-session-parity` (9) gates restart-from-pause's pending-prompt submit and makes the loop
re-read its roster every sweep.

## Goals / Non-Goals

**Goals:**

- Re-running `git paw start --unattended` on the repository's live supervised session drives it
  again, in every effective status, without rebuilding an Active session.
- At most one drive loop acts on a session instance; a loop of a torn-down instance never acts on
  the next one, including after a quick stop-then-recover.
- No new subcommand, flag or `DriveOutcome` variant.

**Non-Goals:**

- Applying `--branches` / `--specs` / `--cli` to a live session (that is `git paw add`).
- Driving a bare session unattended.
- Rewriting the orchestrator context in the root sidecar on re-attach.
- A detached or daemonised loop (`broker-host-separation` D1 keeps it in the foreground process).
- Printing the re-bind command in the exit summary (that would modify *Exit summary*; docs only).
- Showing the bound loop in `git paw status`.

## Decisions

### D1 — Re-run `start --unattended`; no `git paw drive` verb

`--unattended` on a live session means "drive this session". It is the command an operator (or an
external orchestrator, or the PP-24 peer) runs first, and the GP-13 requirement already allows "an
explicit opt-in flag" in place of the non-interactive refusal. Re-running the exact launch command
re-binds, because selection flags are not applied to a live session (already true of the three
branches today).

*Alternatives:* a `git paw drive` subcommand — a second entry to the same loop, plus help, docs and
completions, for no behavior `start` cannot express; an `--attach-loop` flag — redundant with
`--unattended`.

### D2 — One branch per status, no terminal attach

| Effective status | Without `--unattended` (unchanged) | With `--unattended` |
|---|---|---|
| Active | attach (TTY) / GP-13 refusal | bind, run the loop |
| Paused | revive, attach or hint | bind, revive, run the loop |
| Stopped | recover, attach or hint | bind (superseding), recover, run the loop |

The revive and recover paths are reused as they are, minus the final attach: they return once the
session is up and the caller attaches or runs the loop. That is also the shape
`cmd-library-surface` (15) gives the library (the use case reports "already running" or
"launched"; the entry layer attaches, refuses or runs the loop). A fresh `--unattended` launch
never attaches either, so a TTY makes no difference here; the command prints the session name and
`git paw attach`.

A bare session is refused before the binding is taken or anything changes, decided on the recorded
`mode`. The loop's buffered-input submit would otherwise send the prompts a bare session
deliberately leaves for the human.

### D3 — Roster, broker endpoint and instance token come from the live session

The roster is the receipt's worktrees (and, after `pane-less-session-parity`, re-read every sweep).
The broker URL is built from the receipt's `broker_bind` / `broker_port`, as recovery already does,
so a `[broker] port` edited after launch cannot point the loop at no broker or at another
repository's broker. Supervisor policy (correction, learnings, branch refresh, manual-decision log)
comes from the current config, as on a fresh launch. The instance token is the receipt's
`created_at` as read back from disk.

### D4 — The binding: a refreshed lease file

A new `src/supervisor/loop_binding.rs`, modelled on `claim.rs`, owns
`<repo>/.git-paw/tmp/drive-loop.binding` (gitignored by `git paw init`, beside the approval
claims): JSON with the session name, the instance token, the holder's pid and a per-loop id (pid
plus loop start time). Created atomically with `create_new`; replaced atomically (write a sibling temp file, rename over).
Refreshed every tick by updating its mtime; stale when the mtime is older than
max(4 × poll interval, 2 min). Released on exit (RAII) only if it still names this loop.

| Existing binding | Active / Paused / fresh launch | Stopped (recover) |
|---|---|---|
| none | create | create |
| names another instance | replace | replace |
| this instance, stale | replace | replace |
| this instance, fresh | refuse | replace |
| unreadable or unparsable | replace | replace |

The binding is taken before revive or recovery, so a refused caller changes nothing and a failed
revive releases it. Two concurrent first binders race on `create_new` (one wins, the other reads a
fresh binding and refuses). Two concurrent replacers race on the rename; the loser's first tick sees
another loop id and exits before acting.

Writes are atomic renames, so an unparsable binding was not written by a loop; a live holder that
finds its binding overwritten exits on its next tick, so replacing it strands no loop. The refusal
names the session, the holder's pid and the binding file, and says to wait for that loop to exit
or stop that process, then re-run.

*Alternatives:* pid liveness (`kill -0`) — no `libc` dependency, pid reuse, and pids are not
comparable across PID namespaces; `src/lock.rs`'s create-only lock — a killed loop would block
re-attach until a human removes the file, the opposite of what an unattended run needs; a field in
the session receipt — every receipt writer (start, add, remove, pause, recover) would have to
preserve it, and the receipt serializer is frozen.

### D5 — The tick checks the binding; stop revokes it

`FileSessionInstanceGuard::is_current` additionally requires the binding to name this loop's id,
and refreshes it when it does. A missing or foreign binding ends the loop with the existing
`session-torn-down` outcome, checked before any sweep (the GP-06 position), with one stderr line
saying the binding was revoked or taken over. `git paw stop` removes the binding after it records
`Stopped`. Purge needs nothing: the receipt disappears (the existing guard exits the loop), and a
later fresh launch replaces the binding as another instance's.

*Alternative:* a new `superseded` outcome — an enum ripple into the summary, docs and tests for a
rare case whose summary is otherwise the same.

### D6 — Nothing is injected on re-attach

There is no orchestrator pane after `supervisor-pane-removal`, and coding panes keep their
conversations; binding sends no keystroke. The root sidecar's orchestrator context is not
rewritten: since the pane removal, escalations reach the broker, the summary and the manual-decision
log whether or not a loop runs, so the context stays accurate.

## Risks / Trade-offs

- [A suspended loop (SIGSTOP, laptop sleep) lets its binding lapse; a new loop binds; the old one
  resumes] → its next tick sees a foreign binding and exits before acting. A tick already in flight
  when the binding is replaced can finish, the same granularity as the GP-06 record check.
- [Deleting `.git-paw/tmp/` ends a running loop] → fail-safe (the loop approves nothing); the user
  guide says so, and re-running `start --unattended` re-binds.
- [A loop started by an older binary holds no binding, so a re-attach is not refused while it runs]
  → upgrade note in the user guide: stop the old `--unattended` process before re-attaching.
- [Safety: a rogue agent edits or deletes the binding] → it can end a loop or delay a re-attach,
  both fail-safe: the loop approves nothing, and prompts keep waiting for a human. Keeping a forged
  binding fresh needs continuous writes outside the agent's worktree, which worktree confinement
  (or the sandbox) already contains; the refusal names the file and pid for inspection.
- [Security: process spawning and identity] → no new process, tmux command or keystroke path: the
  loop runs in the invoking `git paw start`, and revive and recover are the existing paths. The
  binding path is the repository root plus a fixed name; its contents are parsed as data, never
  executed; the pid is displayed, never signalled by git-paw. Nothing secret is stored.
- [Two loops on one session would each nudge and submit] → removed by the single-binding rule; the
  per-pane claim and the live-prompt gate stay as the second line.

## Migration Plan

Additive. The binding file appears with the first loop of a new binary. Without `--unattended`
nothing changes. Rollback: an older binary ignores the binding file; `git paw stop` or deleting the
file clears it.

## Open Questions

- Should `orchestrator-skill-export` (18) teach the external orchestrator to re-run
  `start --unattended` after a heartbeat or interrupted loop? Recommendation: yes, one line, in that
  change's content pass; this change documents it in the user guide.
