## Context

Retro-spec for an already-built fix (`fix/dashboard-cpu-leak` @ `4aaa435`) addressing leaked, CPU-pegging `__dashboard` orphans. Root cause in `src/dashboard.rs::run_dashboard_with_panes`: a busy `event::poll(ZERO)` + `thread::sleep(50 ms)` loop that unconditionally redrew ~20 Hz, and a SIGHUP-only exit that let unclean teardowns strand the process.

## Goals / Non-Goals

**Goals:** negligible idle CPU; the dashboard cannot outlive its session however it ended.
**Non-Goals:** no change to what the dashboard renders or its layout; no new config or dependency.

## Decisions

- **D1 — Block in `event::poll(TICK)` rather than `poll(ZERO)` + `sleep`.** Input and redraw-triggering events wake the poll immediately; idle time yields the CPU. `TICK` is raised from 50 ms to **≥800 ms** — the UI is near-static, so a longer idle interval cuts CPU further while `q`/events stay instant. The loop exits if the poll returns `Err` (the tty is gone). *Alternative:* keep busy-polling at a slower rate — rejected: still wastes CPU and doesn't fix responsiveness/orphaning.
- **D2 — Orphan detection via `getppid() == 1` (Unix), checked each iteration; break on `shutdown || orphaned()`.** Catches every teardown that skips SIGHUP (abrupt `kill-server`, crash, sleep, e2e drop). Non-Unix targets use a stub returning `false` and retain the prior SIGHUP behavior. *Alternative:* rely on SIGHUP only — rejected: that is exactly the leak.

## Risks / Trade-offs

- Longer idle timeout delays a purely state-driven redraw up to the interval → bounded to ≤1 s at 800 ms; input/events are unaffected → acceptable.
- `getppid() == 1` is Unix-specific → gated `#[cfg(unix)]`; other platforms keep prior behavior.
