## Why

`git paw stop` is currently the only way to "take a break" from a running
session, and it is destructive: it calls `tmux::kill_session`, which kills
the tmux server's hold on the session, which kills every CLI process in
every pane, which discards each agent's in-memory conversation state and
Claude `--continue` markers. Re-running `git paw start` recovers the
worktrees and re-spawns CLI processes, but those processes have no prior
context — every agent starts from a cold prompt.

Two distinct dogfood frustrations converge on this:

- **Drift 60 — "no freeze-state-on-stop path."** The user described the
  ideal short-break flow as "freeze the state on stop and not really
  close down the CLIs" — leave the running session in the background,
  detach the user's tmux client, optionally stop the broker. tmux already
  supports this natively: `tmux detach-client` detaches the client
  without killing the server, the session, or any pane process.
- **Drift 60 — "current `stop` is too destructive UX-wise."** From a UX
  angle, `stop` leaves the session in a *worse* state than `purge`:
  worktrees and branches dangle on disk, no running session, no broker,
  but state-file says "stopped, recoverable." Users forget the
  half-state exists. `purge` is the clean nuke; `stop` is the
  half-finished one.
- **Drift 35 hangover.** Broker death scenarios already cause `git paw
  start` recovery to spawn fresh CLI processes without prior context.
  A soft-stop path that preserves CLIs survives a much broader class of
  "I need to pause for an hour" interruptions than the recovery flow.

This change adds a new soft-stop verb — `git paw pause` — that performs
exactly the freeze-state operation the user described: detach the
tmux client, stop the broker, leave the tmux session + every CLI pane
running. `git paw start` against a paused session restarts the broker
and re-attaches the existing tmux without spawning new CLIs. The user
opens a terminal an hour later, runs `git paw start`, and the agents
continue mid-conversation as if no time had passed.

The trade-off is RAM: ~10 Claude panes at ~300 MB each = 3-5 GB sitting
idle. Pause is for short breaks (lunch, meeting, end-of-day). It is
*not* a long-term hibernation primitive — that lives in v1.0.0 (drift
61, "per-CLI session resumption" via Claude `--continue` /
`--resume`). Pause documents this trade-off in `--help` and the design
doc so users understand when to use which.

The proposal also resolves the open question from drift 60 on what to
do about `git paw stop` now that `pause` exists. See design.md D5 for
the decision (option **(b)**: stop keeps current behaviour but grows a
confirmation prompt + soft-stop suggestion; `--force` skips the
prompt for back-compat with scripts).

## What Changes

**New `git paw pause` subcommand.** Detaches the user's tmux client,
gracefully stops the broker process (without killing tmux), and updates
session state to `paused`. Idempotent: running pause against an already
paused session is a no-op with a friendly message. The CLI parses with
no flags; help text explains the RAM trade-off and points at `stop` for
the destructive path and the future `hibernate` for the RAM-free path.

**New `Paused` variant in `SessionStatus`.** Adds a third state
alongside `Active` and `Stopped`. `paused` means "tmux session and CLI
panes are still running, but no client is attached and the broker is
stopped." `git paw status` differentiates the three states visually
(emoji + label) and surfaces what restart action will run on the next
`git paw start` (re-attach vs recover vs fresh launch).

**Broker-stop helper extraction.** The broker shutdown logic currently
lives inside `BrokerHandle::Drop` and is triggered by the dashboard
pane exiting. Pause needs the same shutdown without killing the
dashboard pane's host tmux session. The change extracts a
`stop_broker_in_pane(session_name: &str, dashboard_pane: u32)` helper
that targets the dashboard subprocess (via `tmux send-keys C-c` or
`tmux kill-pane` of the dashboard pane *only*, leaving agent panes
alive). See design.md D2 for the chosen mechanism.

**Smart `git paw start` for paused sessions.** When the session state
is `paused` and the tmux session is alive, `cmd_start` detects the
paused state and runs the restart-from-pause flow: spawn the dashboard
pane (recreating the broker), then `tmux attach`. Skips worktree
creation, skips CLI process spawning, skips boot-prompt injection.
Cheap and fast.

**`git paw stop` UX upgrade (per design.md D5, option (b)).** `stop`
keeps its current destructive behaviour for back-compat, but grows:

1. An interactive confirmation prompt when stdin is a TTY, similar to
   `purge`'s confirmation but with different wording: "Stop kills all
   CLI processes and loses agent conversation context. Use
   `git paw pause` for a soft stop that preserves state, or
   `git paw purge` for a full reset. Continue stopping? [y/N]"
2. A new `--force` flag that skips the prompt (for scripts).
3. Updated `long_about` help text naming the three verbs and when to
   use each (`pause` for short breaks, `stop` for hard reset of CLI
   processes while preserving worktrees, `purge` for full nuke).

**`stop` after `pause` cleanup.** If the user runs `pause` and then
later runs `stop` (without restarting), `stop` detects the paused
state and proceeds with the kill-session path normally. The CLI
processes that were still running after pause get killed by the
session kill. Idempotent and predictable.

### Capabilities

#### New Capabilities
*(none — all changes modify existing capabilities)*

#### Modified Capabilities

- `cli-parsing`: adds the new `Pause` subcommand with no flags; adds
  the `--force` flag to `Stop`; updates the root `after_help`
  quick-start to mention `pause` alongside `stop` / `purge`.
- `broker-lifecycle`: adds a soft-stop path for the broker that does
  not require killing the host tmux session. Adds the
  `dashboard-pane-only` shutdown sequence and updates the "Stop flow"
  requirements to cover the new pause-vs-stop split.
- `session-state`: extends `SessionStatus` with a third variant
  `Paused`; updates `effective_status` to leave a `Paused` session
  alone (do not down-grade to `Stopped` just because tmux is alive —
  paused sessions intentionally keep tmux alive).

## Impact

**Code (informational — implementation lives in a follow-up change):**

- `src/cli.rs` — add `Command::Pause` variant; add `force: bool` field
  to `Command::Stop`; refresh `after_help`.
- `src/main.rs` — add `cmd_pause()`; extend `cmd_stop()` with the
  confirmation prompt + force-flag bypass; extend `cmd_start()` to
  detect `SessionStatus::Paused` and route to the restart-from-pause
  path (recreate dashboard pane + attach, skip worktree/CLI spawn).
- `src/main.rs::cmd_status` — render the `Paused` state distinctly
  (different emoji from `Active` / `Stopped`).
- `src/session.rs` — add `SessionStatus::Paused`; update `Display`,
  `effective_status`, and the lowercase serde rename.
- `src/tmux.rs` — add `detach_client(session_name: &str)` wrapping
  `tmux detach-client -s <session>`; add a `kill_pane(session_name,
  pane_index)` helper if not already present.
- `src/broker/lifecycle.rs` (or wherever the broker is started) — no
  changes needed inside the broker itself; `BrokerHandle::Drop`
  already handles graceful shutdown. The pause flow triggers the drop
  by killing the dashboard pane only (not the whole session). See
  design.md D2.

**Tests (informational):**

- `cli::tests` — `pause_parses`, `stop_with_force`, `stop_without_force`.
- `session::tests` — `session_status_paused_serializes_lowercase`,
  `paused_session_round_trips`, `effective_status_paused_when_alive`.
- `main::tests` — `cmd_pause_detaches_and_stops_broker`,
  `cmd_pause_is_idempotent`, `cmd_start_against_paused_reattaches`,
  `cmd_stop_after_pause_kills_remaining_panes`.
- Integration test `tests/pause_e2e.rs` — `assert_cmd` driven: start a
  session, pause it, assert tmux session is alive AND broker port is
  free; then start, assert reattach succeeds.

**Backward compatibility:**

- `git paw stop` (no flags) keeps current behaviour for sessions
  invoked via TTY *except* the new confirmation prompt. Scripts that
  call `git paw stop` from non-TTY contexts (CI, automation) see the
  same behaviour as today: no prompt, immediate kill. Scripts that
  call from TTY contexts need `--force` to skip the prompt — this is
  a small surface change that release notes call out.
- `SessionStatus::Paused` is a new serde variant. Sessions saved by
  v0.4.0 (only `active` / `stopped`) load fine; the new variant only
  appears in sessions saved by v0.5.0+. No migration step needed.
- `git paw start` for any pre-pause state (active / stopped / fresh)
  behaves identically to today.

**Cross-references to deferred work:**

- Drift 61 (per-CLI `--continue` / `--resume` on cold start) is OUT of
  scope. Pause is the same-process freeze; drift 61 is the
  cold-process resume. They compose cleanly: pause for short breaks,
  cold-resume via drift-61 work for long ones. See design.md D8.
- A future `git paw hibernate` (snapshot tmux state → disk → killable
  → re-hydratable) is v1.0.0 material and not specced here.
