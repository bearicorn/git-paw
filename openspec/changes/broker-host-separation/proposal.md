## Why

`main.rs::cmd_dashboard` *is* the broker's host: the hidden `__dashboard` subcommand constructs
`BrokerState` (log path, verify-on-commit nudge, seeded orchestrator CLI, role-gating roster,
learnings aggregator), calls `start_broker_with` with the session's watch targets and conflict
config, installs the SIGHUP handler, and only then renders the TUI — and it refuses to run unless
`$TMUX` is set. `pause` stops the broker by killing the dashboard pane, and watchers, conflict
detection and learnings all die with it. The
v1.0.0 layer model has the dependency backwards ("dashboard requires broker"); in the code the
broker requires the dashboard.

This cycle needs it the right way round: verdict authority is enforced *in* the broker, the root
MCP write tools assume a broker that outlives any view, and an external orchestrator should not
lose its event source because a TUI pane closed. The pattern is already shipped: `src/mcp/` holds no
`BrokerState`, resolves `broker_url` from the session file and talks HTTP. The dashboard becomes the
same kind of thin client; the broker gets its own host.

One small broker-side gap rides along — the rest of **DS-3**: v0.17.0 records `spec_id` on every
worktree entry and seeds the learnings aggregator from it where the dashboard builds the broker.
The host now builds the aggregator, so it carries that seeding over, and an agent hot-added with
`git paw add --from-spec` must be mapped too.

Archive position 13 of 20 for v0.18 (after `terminal-backend-seam`).

## What Changes

- **New hidden `git paw __broker`** hosts everything `cmd_dashboard` hosts except the TUI:
  `BrokerState` (per-session log path, verify-on-commit nudge, seeded orchestrator CLI, role-gating
  context, learnings aggregator), the HTTP server with the repository-identity probe, watchers and
  the conflict detector. There is no approver to move: v0.17.0 `classifier-retirement` deleted the
  attended auto-approve thread. It exits on SIGHUP / reparent-to-init / tty hang-up (dropping
  `BrokerHandle` so the final flush runs), ignores SIGINT, never renders, and needs no `$TMUX`.
- **`__dashboard` becomes a thin HTTP client.** It resolves the broker URL from the saved session's
  `broker_port` / `broker_bind` whatever the recorded status (the MCP server's resolution returns
  nothing unless the session is `Active`, and restart-from-pause creates the dashboard while it is still
  `Paused`), polls `GET /status` and `GET /log?since=<seq>` by cursor (with `orchestrator-read-surface`'s
  cursor-holding empty response), renders a "broker unreachable" state instead of exiting, and runs
  with or without `$TMUX`. Killing the dashboard no longer affects the broker.
- **Where the host runs:** a detached service surface of the session from the terminal port — on
  tmux a window named `broker` outside window 0's grid, so pane indices are untouched and
  `kill-session` still delivers SIGHUP. `start` launches it right after `GIT_PAW_BROKER_URL` is set,
  handing it the session name and per-session log path on argv (the receipt does not exist yet), waits
  (bounded) for `/status`, then creates the dashboard and agent panes and registers each agent through
  `/watch` (with its spec id) before its boot prompt. The receipt is still saved only after the panes
  exist, so a failed start leaves persisted session state untouched. Recovery recreates the host the
  same way; the host seeds from the saved receipt.
- **Pause / stop / restart** keyed on the host: `pause` closes the `broker` surface and leaves the
  dashboard showing the paused state; `stop` kills the session (no prompt, no `--force`, per the
  v0.17 `stop-without-confirmation`); restart-from-pause recreates the host and the dashboard pane
  only if missing. A session started by an earlier binary (no `broker` surface) falls back to the old
  dashboard-pane kill on pause.
- **DS-3 (remainder):** the host seeds the aggregator from the `spec_id` v0.17.0 records on each
  worktree entry of a saved receipt; `/watch` carries an optional spec id, which the launcher of a
  fresh launch and `git paw add --from-spec` both send, so every agent is mapped.
- **`git paw status` reports the broker from the host's liveness (PP-27).** It reports the broker
  running only when the repository-scoped probe answers and the session's `broker` surface is alive,
  so a process still answering after `git paw stop` is not shown as this session's running broker.
- **Open sub-question, decided with recommendation:** its own process in a detached surface (this
  change) vs a long-lived session-host daemon (deferred to the tmux spike, which needs the same
  daemon). The process boundary, `__broker`, is identical under both.
- **Not decided here (downstream):** whether the dashboard TUI survives at all. This split makes
  keeping or dropping the view a cheap, reversible product call; it does not make it. No
  `[dashboard] enabled` switch, no tabs.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `broker-runtime`: `__broker` host (added); `__dashboard` as thin client (the "starts broker and
  dashboard" requirement removed); host lifecycle across start / recovery / pause / stop / restart;
  signal handling and panic isolation for the host; `/watch` carries an optional spec id; `status`
  reports the broker from the host's liveness.
- `broker-dashboard`: entry point takes a broker client; HTTP polling by cursor; orphan-exit rules;
  log resilience across outages; degraded state.
- `code-architecture`: the terminal port gains a detached service-surface operation.
- `session-state`: recovery recreates the broker host.
- `cli-parsing`: the hidden internal subcommands are `__broker`, `__dashboard` and `__prompt-profile`.
- `supervisor-learnings`: the host supplies the agent → spec mapping, including for agents
  registered through `/watch` (DS-3 remainder).

## Impact

- **Predecessors (archive order):** position 13, after `terminal-backend-seam` (12, the port this
  change adds `spawn_service` to) and `pane-less-session-parity` (9). `orchestrator-read-surface` (3)
  now archives first: it adds the cursor and wait to `broker::publish::fetch_log_entries_over_http`
  and holds `last_seq` at `since` on an empty `/log`; this change moves that reader into
  `BrokerClient`, and its thin dashboard is the cursor-polling client (the clause moved here from
  `orchestrator-read-surface`). v0.17 changes it builds on and
  modifies after: `broker-repo-identity` (MODIFIES *Dashboard subcommand starts broker and
  dashboard* — per-session log path — which this change then REMOVES, carrying the per-session log
  path and the repository-identity probe into *Broker host subcommand*; also MODIFIES *Stale broker
  detection* and *GET /status*, which this change leaves alone), `stop-without-confirmation`
  (MODIFIES *Stop flow shuts down broker via pane 0 exit*; this change modifies the same requirement
  again on top of that text — no prompt, no `--force`), `supervisor-boot-wiring` (ADDS *Worktree
  entry records the spec it was launched from* and MODIFIES *Conflict-event signal* to seed the
  aggregator from it; this change relies on both and modifies neither), and
  `classifier-retirement` (REMOVES *Auto-approve thread location in dashboard subprocess*,
  *Auto-approve common command classes* and *The drive loop is the sole auto-approver for
  unattended sessions*, and deletes the attended approver, `poll.rs` and `GIT_PAW_UNATTENDED`;
  this change therefore carries no approver into the host and touches none of those headers).
- **Successors:** `drive-loop-functional-core`, `cmd-library-surface` (`cmd_dashboard` already
  extracted), `broker-verdict-authority` (the launcher hands the host the credential on stdin, on top
  of this change's argv hand-off), `mcp-orchestrator-write-tools`.
- **Code:** `src/main.rs::cmd_dashboard` splits into `src/commands/broker_host.rs` (`__broker`) and a
  slim `__dashboard`; `src/dashboard.rs` `run_dashboard*` take a `BrokerClient`; new
  `src/broker/client.rs` consolidates the hand-rolled HTTP helpers: `broker::publish::{publish_to_broker_http,
  register_watch_target_http, fetch_log_entries_over_http (with `orchestrator-read-surface`'s cursor and
  wait), fetch_log_over_http}`, `supervisor::drive::fetch_status_over_http` (moved there from the deleted
  `poll.rs` by v0.17.0), `coordination::inventory::fetch_status_agents_over_http` (the MCP
  `get_session_status` `/status` reader) and the stale-broker probe (`broker::probe_broker*`,
  `src/broker/mod.rs:619`); `src/commands/{start,supervisor,recover,pause,stop}.rs` gain host launch /
  stop steps and the per-agent `/watch` registration; `src/commands/status.rs` (broker line from the
  host's liveness, PP-27); `src/terminal/{mod,tmux,fake}.rs` gain `spawn_service` / `stop_service` /
  `service_alive`; `src/cli.rs` hidden `Broker` command; `src/broker/server.rs` (`/watch` optional
  spec id); `src/commands/add.rs` (`--from-spec` sends the spec id).
- **Persisted state:** host surface addressed by name; no new field (`WorktreeEntry.spec_id` is
  v0.17.0's).
- **Tests:** `broker_integration.rs`, `broker_runtime_hardening.rs`, `pause_e2e.rs`,
  `e2e_supervisor_stop.rs`, `recover_integration.rs`, `dashboard_render.rs`,
  `broker_log_integration.rs`, `session_orchestration_robustness_e2e.rs` (orphan tests apply to
  `__broker`), `e2e_supervisor_launch.rs`, learnings classification test with real launch data;
  `main.rs::dashboard_aggregator_is_seeded_from_the_session_receipt` moves with the seeding into
  `commands/broker_host.rs`; `cli.rs::root_help_hides_internal_subcommands` gains `__broker`; status
  broker-line tests.
- **Docs:** `docs/src/architecture.md` (Broker Architecture, Start Flow, the `broker` window),
  `user-guide/dashboard.md`, `user-guide/pause.md`, `user-guide/session-lifecycle.md`,
  `user-guide/learnings.md`, `cli-reference.md` (hidden commands note).
- **Enum-variant ripple:** none.
- **Security / safety:** the broker host (and, after `broker-verdict-authority`, the credential) is
  headless and never shares a TTY with a view; it sends no keystrokes to any pane (no git-paw
  process answers a prompt since v0.17.0). The visible dashboard holds no state and no privilege. Bind stays `127.0.0.1`. The SIGHUP `unsafe`
  handler moves verbatim into the host — a specified change of location, reviewed under Gate 5
  (the `code-architecture` frozen-surface rule governs refactors).
- **Dependencies:** none (the client is the existing hand-rolled `TcpStream` GET/POST).

## Rebase note (2026-10-19)

v0.17.0 pulled DS-3 forward as `supervisor-boot-wiring`: `WorktreeEntry.spec_id` is set on every spec-driven launch path and the dashboard seeds the learnings aggregator's `agent_id → spec_id` map from it. This change's duplicate *Worktree entry records its spec id* was dropped; it only carries that seeding over to the `__broker` host and adds the `/watch` spec id for hot-added agents.

## Rebase note (2026-10-20)

v0.17.0 `classifier-retirement` deletes the attended auto-approve thread, `poll.rs` and the `GIT_PAW_UNATTENDED` signal, and REMOVES the three requirements this change used to modify (*Auto-approve thread location in dashboard subprocess*, *Auto-approve common command classes*, *The drive loop is the sole auto-approver for unattended sessions*). Those deltas were dropped and the host carries no approver.

## Rebase note (2026-10-25): audit against released v0.17.0

- Launch ordering (design call 3): a fresh launch saves its receipt only after the panes exist
  (`start.rs:377/:790`, `supervisor.rs:848`), which released *A failed start preserves persisted session
  state* requires, so a host started first found no receipt (log path fell back to `git-paw.broker.log`,
  no watch targets or `spec_id` seeding). The launcher now hands the host the session name and log path
  on argv and registers each agent through `/watch` (with its spec id) once its pane exists; the
  failed-start rule is untouched.
- The thin dashboard resolves the broker URL from `broker_port` / `broker_bind` whatever the status:
  `broker_url_from_session` returns `None` unless `Active` (`mcp/mod.rs:107-110`), and restart-from-pause
  creates the dashboard while the session is `Paused`. It detects a restarted host from `/status`
  uptime, since an empty `/log` now holds the cursor.
- Absorbs PP-27: `git paw status` reports the broker running only when the probe answers and the host's
  `broker` surface is alive (MODIFIED *Status shows broker information*).
- Task 1.1's helper list corrected: `coordination::inventory::fetch_status_agents_over_http`,
  `register_watch_target_http`, `fetch_log_over_http` and the stale-broker probe join the consolidation,
  as does `orchestrator-read-surface`'s cursor/wait reader (that change now archives first, position 3).
- Three hidden subcommands exist (`__prompt-profile` too); task 4.1 names the moving test
  (`dashboard_aggregator_is_seeded_from_the_session_receipt`).
