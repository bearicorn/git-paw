## Why

v0.10.0's `dashboard-orphan-exit` stops the common dashboard leak — a dashboard reparented to init (`getppid() == 1`) exits instead of busy-looping. But the v0.10.0 dogfood surfaced two residual gaps the guard does not cover, and together they still leak CPU-pegging dashboards:

1. **Broker-bind failure evades both guards.** The broker HTTP server runs inside the `__dashboard` process. When its bind fails (e.g. a stale dashboard is squatting the port), the dashboard enters a degraded ~100% CPU loop that never reaches the `event::poll` path where the poll-block AND the `getppid()` orphan-check live. Observed: a dashboard with `ppid == 1` AND 98.7% CPU that did not exit.
2. **Reparent-to-shell is not caught.** `getppid() == 1` only detects reparenting to *init*. A dashboard whose tmux pane died but whose parent is a lingering shell (not init) keeps busy-looping. 12 such orphans were found in one dogfood.

## What Changes

- On **broker-bind failure**, the dashboard SHALL exit (or degrade to a clearly-labelled no-broker render that still honors shutdown) rather than enter a busy-loop — and the shutdown/orphan check SHALL run on **every** loop path, including the error/bind-failure path.
- Broaden orphan detection beyond `getppid() == 1`: also exit when the **controlling terminal / stdout is gone** (a closed or dead tty), so a dashboard reparented to a lingering shell still terminates when its pane is gone.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `dashboard`: MODIFY **Exit when orphaned** to (a) exit on broker-bind failure instead of busy-looping and run the shutdown/orphan check on all loop paths, and (b) treat a gone controlling-tty/stdout as an exit condition in addition to reparent-to-init.

## Impact

- `src/dashboard.rs` (`run_dashboard_with_panes` + the broker-bind path): exit on bind failure; lifecycle check on all loop branches; add a tty/stdout-gone check alongside `orphaned()`.
- No config/CLI change. Non-Unix keeps prior behavior. Complements the shipped `dashboard-orphan-exit`; closes the residual leak the v0.10.0 dogfood found.
