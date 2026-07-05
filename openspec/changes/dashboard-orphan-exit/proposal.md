## Why

Leaked `git paw __dashboard` processes were observed pegging CPU — dozens of orphans, ~473% CPU measured across 30. Two root causes in the dashboard loop: it busy-redrew the full TUI at ~20 Hz (`event::poll(ZERO)` + `thread::sleep(50 ms)`), burning CPU per live pane; and it only exited on SIGHUP, so any teardown that skips SIGHUP (abrupt `tmux kill-server`, crash, machine sleep, an e2e test dropping the session) left it reparented to init, busy-rendering to a dead terminal forever. This fix (already built on `fix/dashboard-cpu-leak` @ `4aaa435`) makes idle CPU negligible and guarantees the dashboard cannot outlive its session.

## What Changes

- The dashboard's draw loop **blocks on the event poll with a bounded idle timeout** instead of busy-polling at ~20 Hz, so it consumes negligible CPU while idle; key input and redraw-triggering events still wake it immediately. The idle timeout is raised to **≥800 ms** (the near-static dashboard does not need sub-second periodic redraws).
- The dashboard **exits when orphaned** — when its parent process is gone (on Unix, reparented to init, `getppid() == 1`) — so it can never survive its session however the session ended, not only on SIGHUP.
- The loop exits on a poll error (the terminal is gone).

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `dashboard`: MODIFY **Periodic state polling** (replace the ≤100 ms busy input-poll cycle with a blocking, event-driven poll bounded by a ≥800 ms idle timeout — negligible idle CPU, input still wakes immediately); ADD **Exit when orphaned** (terminate when reparented to init).

## Impact

- `src/dashboard.rs` (`run_dashboard_with_panes`): the poll/redraw loop and a `#[cfg(unix)]` orphan check (`getppid() == 1`). Already implemented on `fix/dashboard-cpu-leak` @ `4aaa435` (built at 250 ms — **raise to ≥800 ms on integration**).
- No config, CLI, or dependency change. Non-Unix builds retain the prior behavior (the orphan check is a stub).
