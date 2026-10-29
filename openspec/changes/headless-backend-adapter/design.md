## Context

After v0.18.0 every orchestration-path terminal operation goes through `TerminalBackend`
(`src/terminal/mod.rs`): `open_session`, `session_alive`, `close_session`, `set_session_env`,
`spawn_surface`, `kill`, `list_surfaces`, `label_surface`, `send_text`, `send_key`, `capture_tail`,
`stream_to`, `liveness`, `attach`, `detach_clients`, an optional `layout`, and (from
`broker-host-separation`) `spawn_service` / `stop_service` / `service_alive`. `TmuxBackend` is the
only production adapter and `FakeBackend` serves the logic tests. The port's own review question is
"how would a headless backend answer this?". This change answers it in code.

Facts this design relies on, at the code this cycle builds on:

- **Launch commands are shell strings.** The pane command is typed into a shell
  (`tmux/command.rs`). From v0.19.0 one builder (`CliProfile::launch_command`) composes it from
  verbatim config strings: `args`, `approval_args`, the session-id and resume templates, and
  `headless_args`. The executable is shell-quoted (`domain::shell_quote`).
- **A worker's session id is assigned at launch.** `WorktreeEntry.cli_session_id` is set from
  `session_id_args`, and recovery resumes with `resume_args`. Only the `claude` family has
  built-in rows.
- **The headless one-shot form exists.** It is v0.18.0's `headless_args` (`claude` → `-p`,
  `codex` → `exec`), reading the prompt from stdin.
- **Detached processes already exist.** The v0.18.0 escalation one-shot spawns in its own process
  group with stdin/redirect through `CommandRunner`.
- **`unsafe extern "C"` is the established way to reach libc.** `dashboard.rs` declares `getppid`
  this way, and `main.rs` declares `signal`. There is no `libc` crate.
- **The `__broker` host exits on SIGHUP, on reparent-to-init and on tty hang-up**
  (`broker-host-separation`).
- **std facilities at MSRV 1.96:** `CommandExt::{process_group, pre_exec}`, `std::io::pipe` (1.87),
  `File::{lock, try_lock}` (1.89), `OpenOptionsExt::mode`, `DirBuilderExt::mode`. Files are opened
  close-on-exec by default.

The word "headless" already appears in `session-state` (*Headless canvas fits multi-agent
supervisor layouts*), where it means a *detached tmux session*. That requirement is unrelated to
this backend.

## Goals / Non-Goals

**Goals:**
- A second production adapter that answers every port operation in git-paw's terms.
- Workers that survive the launching shell and terminal with no daemon.
- Message delivery that is ordered and exactly-once, with no keystroke timing anywhere.
- Owner-only run state outside every repository; zero new dependencies.
- tmux behavior byte-identical (the port additions answer "unknown" / "interactive" for tmux).

**Non-Goals:**
- Making the backend selectable, the per-CLI eligibility check, and the permission-policy gate
  (`headless-backend-selection`).
- Drive-loop and inbox-wake behavior (`headless-drive-adaptation`).
- A PTY-owning interactive backend, other multiplexers, native Windows (see Open Questions).
- Using a CLI's streaming-input protocol to keep one long-lived process per worker.

## Decisions

### D1 — A headless surface is a conversation driven turn by turn

A surface is not a process. It is a worker's conversation. Each submitted message runs one *turn*:
one invocation of the CLI's non-interactive form, which resumes the conversation, reads the
message on stdin, works until it ends its turn, and exits. Between turns no process exists, and
the surface is *awaiting input*. The launch flow gives `spawn_surface` a **turn plan** of two
command strings, both from the one builder:

- *first* — with the session-id arguments, used for the surface's first turn;
- *continuation* — with the resume arguments, used for every later turn.

*Alternatives rejected:*
- One long-lived CLI process fed through a stdin pipe, using the CLI's streaming-input mode. That
  needs a third-party protocol the code does not encode (to verify by the v0.23.0 audit), and a
  process that holds the pipe open is a daemon.
- A PTY-owning interactive backend. It is out of scope, and it re-creates the interactive bug
  classes this backend avoids.

### D2 — Run state in the user data directory, owner-only, keyed by validated newtypes

```text
<session_state_dir>/<session>.headless/            0700
  run.json                                         env overlay, opened_at (the open marker)
  surfaces/<slug>/                                 0700, <slug> = BranchSlug
    surface.json   owner, label, turn plan, stream destination
    draft          unsubmitted text
    inbox/<seq>.msg, <seq>.claimed
    turns.jsonl    one record per turn: seq, started_at, ended_at, exit (code | signal)
    output.log     the surface's output stream (append-only)
    runner.log     the runner's own diagnostics
    lock           held by the live runner
  services/<name>/ lock, output.log, runner.log
```

Files are created `0600`. Every path is built from `SessionName` and `BranchSlug`, which already
reject traversal. *Why not under `.git-paw/`:* keeping the state outside the repository keeps it
out of every worktree's view and out of `git status`. It needs no `.gitignore` change. And
`purge` already owns the session-state directory. The v0.22.0 session record (inside the repo)
receives a copy through `stream_to` (D6).

### D3 — Draft, submit and clear mirror an input line; delivery is queued

- `send_text` appends to `draft`.
- `send_key(Enter)` atomically moves the draft into `inbox/<seq>.msg` (temp file, then rename),
  then *ensures a runner*.
- `send_key(CtrlU)` truncates the draft.
- Every other `Key` fails with `PawError::Unsupported { backend, operation }`. In particular the
  digit keys fail: there is no prompt to answer.

This keeps v0.18.0's rule that text and submit are separate operations. It also makes delivery
cross-process: the boot prompt held unsubmitted on an add to a paused session is just a draft.

*Ensure a runner:* probe the surface lock with `try_lock`. When it is free, spawn the runner. Then
wait, bounded at 5 s, until the lock is held. Submit therefore returns only when a runner owns the
surface, or it returns an error.

The runner claims a message by renaming it to `.claimed`, and deletes it when the turn ends.
Before it exits it releases the lock and re-checks the inbox. A message that arrived in that
window is picked up by whichever runner wins the lock, so no wake-up is lost.

A submitted message byte-identical to one still unclaimed is dropped. A wake and a nudge in the
same tick therefore do not cost two turns.

A message that was claimed but never finished (the runner died) is **not re-run automatically**.
A turn has side effects, so recovery sends a continuation message instead
(`headless-drive-adaptation`, `headless-backend-selection`).

### D4 — The turn runner is the worker, not a daemon

`git paw __headless-runner <run-dir> <surface>` is a hidden subcommand. That is the decided
hidden-subcommand IPC pattern.

- **Spawned** with `Command::new(current_exe())`, `pre_exec(setsid)`, stdin `/dev/null`, and
  stdout/stderr to `runner.log`. It has no controlling terminal and is the leader of its own
  session and process group.
- **Each turn** runs `sh -c 'exec <command>'` in the owner worktree:
  - the environment is the runner's own plus the session overlay;
  - stdin is a pipe that receives the message and is then closed;
  - stdout and stderr share one `io::pipe`, whose bytes the runner appends to `output.log` and to
    the `stream_to` destination;
  - a record is appended to `turns.jsonl` at start and at end.
- **It exits** when the inbox is empty.

*Why `sh -c`:* the command string is exactly what the tmux backend types into a shell, so the
verbatim config strings mean the same on both backends, with no word-splitter (and no new crate).
The message never appears on a command line. *Why `exec`:* the CLI is then the child, so the
recorded exit status is the CLI's own. *Why a runner at all:* a detached child's exit status is
lost once its parent exits. Something must also deliver messages queued during a turn and copy
the output. The runner exists exactly while there is work.

### D5 — Activity and liveness are one pure function of observed facts

`surface_activity(lock_held, last_turn, pending_message) -> SurfaceActivity`, first matching row
(a killed surface no longer exists and is absent from `list_surfaces`):

| Observation | Activity |
|---|---|
| lock held | `Busy { last_output }` (the output stream's modification time) |
| a turn started with no end record, or a message is pending with no runner | `Interrupted` |
| the last turn ended with a non-zero code or a signal | `Failed { exit }` |
| otherwise (including never messaged) | `AwaitingInput` |

`liveness` is *alive* for `Busy` and `AwaitingInput`, and *not alive* otherwise. That matches
tmux's meaning: a crashed or failed CLI reads as not alive. tmux answers `activity` with
`Unknown`, because its activity is read from the screen by the drive loop's detectors, which
stay as they are.

### D6 — `stream_to` is a second destination of every later turn

The destination is persisted in `surface.json`. The runner opens it for each turn in append mode,
creating it `0600` when absent and refusing it (with a warning in `runner.log`) when it is a
symbolic link. Launch flows call `stream_to` before the first message, so a headless capture
holds every turn from its first byte, where tmux's `pipe-pane` misses output written before it
attaches. A failing destination never fails the turn; `output.log` is unaffected.

### D7 — kill and close stop process groups, guarded by the lock

The runner writes its pid after taking the lock. `kill(surface)`:

1. When the lock is held, send `SIGTERM` to the process group (`kill(-pgid, …)`).
2. Wait up to 5 s for the lock to free.
3. Then send `SIGKILL` to the group.
4. Remove the surface directory.

Signalling only a group whose leader currently holds the lock means the pid cannot have been
reused while it is held.

`close_session` kills every surface, stops every service and removes the run directory.
`stop` (recoverable) is still recoverable: the conversation lives in the CLI's own transcript,
keyed by `cli_session_id`, which is in the session file. Unsubmitted drafts are lost, as typed
text is when tmux kills a session.

### D8 — Services run under the runner, too

`spawn_service(session, name, command)` starts `__headless-runner --service <name>`. The runner
runs the command as its child (same group), holds the service lock, and records the exit.

- `stop_service` sends `SIGHUP` to the group. The runner forwards nothing and waits. The broker
  host already treats SIGHUP as its graceful shutdown with a final flush.
- Because the host's parent is the live runner, the host's reparent-to-init exit never fires, and
  it has no controlling terminal to lose. So `__broker` needs **no change**.
- In service mode the runner checks every 5 s that the run directory still exists. If it does
  not, it hangs up its child, so state removed by hand cannot leave a service running forever.

*Alternative rejected:* an environment flag that disables the host's orphan check. It couples the
host to one backend and leaves no lock to probe for `service_alive`.

### D9 — `attach` follows outputs; it never sends input

`attach` tails every surface's `output.log` by polling every 250 ms (no file-watch dependency),
prints each line prefixed with the surface label, and returns on interrupt. The runners are in
other sessions, so the interrupt cannot reach them. It writes no escape sequences.
`detach_clients` and `layout` are no-ops for a backend with no clients and no tiles.

### D10 — Port additions are answered by tmux without behavior change

- `surfaces_are_interactive()`: tmux returns `true`, headless `false`.
- `activity(surface)`: tmux returns `Unknown`.
- `SurfaceSpec` gains an optional non-interactive turn plan. tmux ignores it. Headless refuses a
  spec without one.
- `PawError::Unsupported` is new.

The fake gains scripted activity and a non-interactive mode, so the drive-loop work in
`headless-drive-adaptation` is tested once against it.

### D11 — The launching environment is never written to disk

`set_session_env` stores only the variables git-paw sets, such as `GIT_PAW_BROKER_URL`. A turn's
environment is the environment of the process that started its runner — the user's shell for
`start` and `add`, or the broker host for inbox wakes — plus that overlay. Persisting the caller's
environment would put API tokens on disk.

## Risks / Trade-offs

Threat analysis (`security-and-safety-review`). The policy gate that decides *whether* a CLI may
run headless is in `headless-backend-selection`. These are the adapter's own surfaces.

- **T1 — Injection through a message, label or path.** Messages go only to stdin. Labels are data
  in JSON. Paths come from `SessionName` / `BranchSlug`. The only shell string is the builder's
  output, with its existing quoting (`safe-process-invocation`).
- **T2 — A worker tampers with run state.** A worker could inject a message into a peer's inbox,
  forge a turn record, or swap `output.log` or the stream destination for a symlink. → The state
  is outside every worktree and owner-only, and the runner refuses symlinks when it opens a file.
  A worker that can write anywhere the user can (unsandboxed full-auto) can still do this, which
  is one reason the selection gate refuses unsandboxed full-auto. Residual: there is no
  `O_NOFOLLOW` without libc, so a check-then-open window remains, as in v0.22.0.
- **T3 — Signalling the wrong process.** → Only a group whose leader holds the lock is signalled.
  A narrow TOCTOU window between the probe and `kill` remains.
- **T4 — Persistence beyond the session.** A worker can start a process that calls `setsid`
  itself and so escapes the group kill. This is no worse than tmux's `kill-session`. The sandbox
  profiles are the mitigation, and it is documented.
- **T5 — Secrets.** The environment is never persisted (D11). The turn plan holds the session id,
  stored `0600` as the session file already stores it.
- **T6 — Unbounded output growth.** `output.log` is append-only plain text, removed at close.
  There is no rotation in this change.
- **T7 — A turn that never ends holds the lock forever.** → It reads `Busy`.
  `headless-drive-adaptation` escalates no-progress, and `kill` always works.
- **T8 — `unsafe`.** Two `extern "C"` declarations: `setsid` (async-signal-safe, called only
  inside `pre_exec`) and `kill` (used with a negative pid, which addresses the runner's process
  group: `setsid` makes the runner's pid its group id). Each has a SAFETY comment and is reviewed
  under Gate 5. There is no new crate.

## Migration Plan

Additive and not reachable by users until `headless-backend-selection`:

1. Port additions and the fake (tmux answers `Unknown` / interactive); suite green.
2. Pure core: `surface_activity`, inbox/draft state transitions, turn-log parsing — table tests.
3. The runner subcommand (turn and service modes) and `HeadlessBackend`.
4. Integration tests with a stub CLI script; source audit; architecture docs.

Rollback: revert. The run directory is ignored by older binaries; `purge` of a headless session
needs this release.

## Open Questions

- **A first turn cut short.** Can each CLI resume a conversation whose first turn was killed
  before it finished, or does re-running the first-turn command collide with the assigned id? To
  verify per CLI by the v0.23.0 audit, through the `headless-worker` column added by
  `headless-backend-selection`. Until then such a surface reads `Interrupted` and is escalated,
  never silently retried.
- **Output rotation.** Should there be a size cap on `output.log`? Not in this change. Revisit if
  real runs grow it beyond what `capture_tail` and `attach` handle comfortably.
- **Native Windows** would additionally need:
  - Job Objects in place of process groups and `setsid`;
  - `CREATE_NEW_PROCESS_GROUP` / `DETACHED_PROCESS` spawning;
  - `TerminateJobObject` in place of group signals;
  - a non-`sh` turn invocation, with a word-splitting rule for the verbatim config strings;
  - a graceful-stop signal in place of `SIGHUP` for the broker host;
  - replacements for the bash helper scripts (`broker.sh`, `sweep.sh`);
  - the platform data directory in `dirs.rs`.

  `File::lock` and the message-on-stdin design carry over unchanged. Out of scope here.
