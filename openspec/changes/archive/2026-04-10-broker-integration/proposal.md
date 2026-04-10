## Why

Wave 1 shipped the broker, dashboard, skill templates, and message types as standalone modules. Wave 2's `peer-messaging` filled in the delivery logic. But none of these are connected to the actual `git paw start`/`stop`/`purge`/`status` lifecycle yet — the broker doesn't start when a session starts, the dashboard doesn't run in pane 0, and agents don't have `GIT_PAW_BROKER_URL` in their environment. This change is the wiring that turns the v0.3.0 modules into a working end-to-end system.

## What Changes

- Add a hidden `__dashboard` subcommand to `src/cli.rs` that pane 0 executes. It reads `[broker]` config, constructs `BrokerState` (with log path from session state directory), calls `start_broker`, then calls `run_dashboard`. When the dashboard exits, `BrokerHandle` drops and the broker shuts down.
- Modify the `start` flow in `src/main.rs` to:
  - Check `[broker] enabled` in config
  - If enabled, set pane 0 to run `git paw __dashboard` instead of a coding CLI
  - Inject `GIT_PAW_BROKER_URL` into the tmux session environment via `tmux set-environment -t <session> GIT_PAW_BROKER_URL <url>` so all panes inherit it
  - Save broker config (port, bind, log path) into the session state file
- Modify the `stop` flow: no additional work needed — `git paw stop` kills tmux, which kills pane 0, which drops `BrokerHandle`, which shuts down the broker gracefully. Confirm this in tests.
- Modify the `purge` flow: after existing cleanup (kill tmux, remove worktrees, delete session state), also delete `broker.log` from the session state directory if it exists.
- Modify the `status` output: when a session is active and broker is enabled, display broker state (port, agent count from `/status` probe or session state).
- Update session state JSON to include optional broker fields: `broker_port`, `broker_bind`, `broker_log_path`.

## Capabilities

### New Capabilities

- `broker-lifecycle`: Wiring the broker and dashboard into the git-paw session lifecycle. Covers the hidden `__dashboard` subcommand, the `start` flow changes (pane 0 assignment, env var injection, session state updates), the `purge` cleanup of `broker.log`, and the `status` output additions.

### Modified Capabilities

- `cli-parsing`: Add the hidden `__dashboard` subcommand to the clap command tree. It SHALL NOT appear in `--help` output.
- `session-state`: Session state JSON gains optional broker fields (`broker_port`, `broker_bind`, `broker_log_path`). Existing sessions without these fields SHALL load successfully with defaults.
- `tmux-orchestration`: When broker is enabled, pane 0 SHALL run `git paw __dashboard` instead of a coding CLI. The `TmuxSessionBuilder` SHALL support setting environment variables on the session.

## Impact

- **Modified files:**
  - `src/main.rs` — start/stop/purge/status flow changes
  - `src/cli.rs` — add hidden `__dashboard` subcommand
  - `src/session.rs` — add optional broker fields to session state struct
  - `src/tmux.rs` — add `set_environment` support to the builder
  - `src/config.rs` — read `[broker]` config (already added by `http-broker`; this change uses it)
- **No new files.** This change exclusively modifies existing modules.
- **No new dependencies.** Uses existing crate functionality.
- **Depends on:** `http-broker` (for `start_broker`, `BrokerState`, `BrokerHandle`, `BrokerConfig`), `dashboard-tui` (for `run_dashboard`), `peer-messaging` (for working delivery — the end-to-end flow needs it), `skill-templates` (used by `skill-injection`, not directly by this change).
- **Merge order:** `broker-integration` merges after `peer-messaging` and `skill-injection` in Wave 2 (it's the final integration point).
- **CLI surface change:** adds one hidden subcommand (`__dashboard`) not visible in `--help`. Modifies `git paw status` output when broker is enabled.
