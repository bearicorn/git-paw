## Why

v0.18.0 made the terminal a port (`TerminalBackend`, keyed by `SurfaceId`) with tmux as its only
adapter, so that tmux / PTY / headless became a choice between backends instead of a bet. The
roadmap's drop-tmux analysis found that the one capability tmux uniquely provides — workers that
keep going while nobody is attached — needs no daemon when a worker is a non-interactive process
writing to a log: `setsid` plus redirection keeps it alive from the OS. A worker driven by
messages instead of keystrokes also has none of tmux's interactive bug classes (`send-keys`
reliability, paste buffers, follow-up `Enter`, pane-index mapping). The user decided on
2026-10-19 to ship this as an additive, opt-in backend after the 1.0 freeze (v1.3.0).

This change adds the adapter only. Nothing selects it yet — selection, the safety policy and the
docs land in `headless-backend-selection` — so it changes nothing a user can observe.

## What Changes

- **`HeadlessBackend`** (`src/terminal/headless.rs`) implements every `TerminalBackend`
  operation. A headless *surface* is a worker conversation driven turn by turn. Each surface has
  owner-only state in the user data directory, beside the session file: its owner, label and turn
  plan, a draft, an inbox, a turn log, an output stream and a lock.
  - `spawn_surface` records the surface; it starts no process.
  - `send_text` appends to the draft. `send_key(Enter)` submits the draft as one message.
    `send_key(CtrlU)` clears the draft. Any other key fails with a typed "unsupported by this
    backend" error.
  - Submitting queues the message and makes sure a **turn runner** is running.
  - `capture_tail` reads the end of the output stream. `stream_to` adds a second destination that
    every later turn's output is also written to.
  - `liveness` and the new `activity` query come from the lock and the turn log. `kill` stops the
    surface's process group and removes its state.
  - `attach` follows every surface's output read-only, each line prefixed with its label.
    `detach_clients` and `layout` do nothing.
  - `set_session_env` stores git-paw's own session variables, never the caller's environment.
  - Services run detached and are stopped gracefully. `close_session` stops everything and
    removes the session's run state.
- **The turn runner** is a hidden `git paw __headless-runner`. It is started in its own session
  (`setsid`) with no controlling terminal and holds the surface lock. It runs one turn per queued
  message, in order. Each turn runs the CLI in the worktree, gives it the message on stdin,
  appends its output to the stream, and records start, end and exit status. The runner exits when
  the inbox is empty. No daemon outlives the work.
- **Port additions** (additive to v0.18.0's trait): whether a backend's surfaces are interactive;
  a per-surface `activity` (busy, awaiting input, failed, interrupted, unknown — tmux answers
  unknown); an optional non-interactive turn plan on `SurfaceSpec`; the typed unsupported error.
  The fake backend gains scripted activity and a non-interactive mode.
- **Services stay alive without a parent.** A service the headless backend starts (the broker
  host) does not exit when it is reparented to init. It shuts down gracefully when stopped or when
  its session's run state is gone.
- Zero new dependencies. The change uses std only (`process_group`, `pre_exec`, `io::pipe`,
  `File::try_lock`), plus one documented `unsafe extern "C"` declaration of `setsid`, following the
  existing `getppid` pattern. MSRV stays 1.96.

## Capabilities

### New Capabilities

- `session-headless-backend`: the headless terminal backend — surfaces as turn-driven worker
  conversations, message delivery, the detached turn runner, output streams, activity and
  liveness, kill/close, read-only attach, detached services, and the owner-only run state.

### Modified Capabilities

- `code-architecture`: the terminal port reports whether its surfaces are interactive and each
  surface's activity, carries an optional non-interactive turn plan, and has a typed unsupported
  error. Headless process spawning is confined to the adapter and its runner.

## Impact

- **Builds on (must rebase over):**
  - v0.18.0 `terminal-backend-seam`: the port, `SurfaceId`, `SurfaceSpec`, `Key`, the fake, and
    its confinement and vocabulary audits. This change adds to that trait.
  - v0.18.0 `broker-host-separation`: `spawn_service` / `stop_service` / `service_alive`, and the
    *Broker host subcommand* reparent-exit rule, which this change relaxes for detached services.
    If that requirement has been archived into `broker-runtime` by the time this is applied, turn
    the ADDED rule into a MODIFIED delta of it.
  - v0.18.0 `drive-loop-functional-core`: its consumers of `activity` are
    `headless-drive-adaptation`.
  - v0.19.0 `cli-provider-record`: the one launch-command builder that produces the turn plan
    (wired in `headless-backend-selection`).
  - v0.22.0 `session-record-capture`: `stream_to` targets the session record's capture file; the
    owner-only and symlink-refusal rules apply to the runner's writes.
- **Successors:** `headless-drive-adaptation`, `headless-backend-selection` (v1.3.0).
- **Code:** new `src/terminal/headless.rs`, `src/terminal/headless/{state,runner}.rs`; a hidden
  `HeadlessRunner` command in `src/cli.rs` with its handler in `src/commands/`;
  `src/terminal/{mod,fake}.rs` (port additions); the `__broker` host's orphan-exit check
  (`src/commands/broker_host.rs` after v0.18.0); `PawError` gains an unsupported-operation
  variant.
- **Persisted state:** the run state lives under the user data directory
  (`<session_state_dir>/<session>.headless/`). No session-file or discovery-file field changes in
  this change.
- **Enum-variant ripple:** none (`BrokerMessage` and `SpecBackendKind` are untouched).
  `SurfaceActivity` is new and closed.
- **Security / safety:** the change spawns detached processes, writes state files and sends
  signals to process groups (threat analysis in design.md). Nothing reaches it until
  `headless-backend-selection` adds a launch-time policy gate.
- **Tests:** unit tests over the state machine (draft, inbox, turn log → activity), runner
  integration tests with a stub CLI script (no tmux), fake-backend additions, and a source audit.
- **Docs:** `docs/src/architecture.md` (module list gains the headless adapter);
  `docs/src/architecture-direction.md` (Terminal row: second adapter exists). User-facing docs
  ship with `headless-backend-selection`.
- **Dependencies:** none.
