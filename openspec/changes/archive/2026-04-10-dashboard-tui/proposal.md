## Why

With the HTTP broker running and agents posting status/artifact/blocked messages, the user needs visibility into what's happening across all agents without switching between tmux panes. Pane 0 is reserved for git-paw's own process (broker + dashboard, same binary). This change fills pane 0 with a live-updating status table so the user can see at a glance which agents are working, which are done, and which are stuck — without any user interaction beyond watching.

## What Changes

- Add a new module `src/dashboard.rs` implementing a ratatui-based terminal UI
- Expose a `run_dashboard(state: BrokerState, broker_handle: BrokerHandle) -> Result<(), PawError>` entry point that:
  - Enters ratatui alternate screen and raw mode
  - Installs a panic hook that restores the terminal on crash
  - Polls `BrokerState` on a periodic tick (1 second) for current agent records
  - Renders a status table with columns: agent ID, CLI name, status, time since last update, one-line summary
  - Handles `q` key to quit cleanly (which drops `BrokerHandle`, shutting down the broker)
  - Restores the terminal on exit
- Add new dependencies to `Cargo.toml`: `ratatui` and `crossterm` (both MIT, both approved)
- The dashboard is read-only in v0.3.0 — no input handling beyond `q` to quit, no prompt inbox, no scrolling. Those features are deferred to v0.4.
- All rendering logic is separated from state computation: pure functions take agent data in and return formatted rows/cells, making them testable without a terminal. The draw loop itself is thin and exempt from the 80% coverage gate.

## Capabilities

### New Capabilities

- `dashboard`: Terminal-based status display for pane 0 showing live agent state from the broker. Covers terminal lifecycle (setup, teardown, panic recovery), the render loop, state-to-display formatting, the quit keybind, and the public `run_dashboard` entry point that `broker-integration` will invoke.

### Modified Capabilities

<!-- None -->

## Impact

- **New file (owned by this change):** `src/dashboard.rs`
- **Modified file:** `src/main.rs` — add `mod dashboard;` declaration
- **Modified file:** `Cargo.toml` — add `ratatui` and `crossterm` to `[dependencies]`
- **Modified file:** `deny.toml` — allow new transitive licenses from ratatui/crossterm tree if flagged by `cargo deny check`
- **New runtime dependencies:** `ratatui` (MIT), `crossterm` (MIT). Both cross-platform on all supported targets.
- **Depends on:** `http-broker` (for `BrokerState` and `BrokerHandle` types, `AgentStatusEntry`), which transitively depends on `message-types`.
- **Dependents:** `broker-integration` (Wave 2) — calls `run_dashboard` as the pane 0 process entry point.
- **No CLI surface changes in this change.** No new commands, flags, or config fields. The dashboard is wired into pane 0 by `broker-integration`.
- **Coverage:** 80% line coverage on state-formatting and data-transformation functions. The ratatui draw loop and crossterm input handling are exempt from the coverage gate per the agreed v0.3.0 convention — they are tested manually via smoke tests in Phase 9.
