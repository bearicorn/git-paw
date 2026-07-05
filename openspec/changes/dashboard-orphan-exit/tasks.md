## 1. Poll / redraw loop

- [ ] 1.1 Replace the busy `event::poll(ZERO)` + `thread::sleep` loop with a blocking `event::poll(TICK)`; exit the loop when the poll returns an error (the tty is gone)
- [ ] 1.2 Set the idle tick interval to ≥800 ms (the built branch used 250 ms — raise it on integration)

## 2. Orphan exit

- [ ] 2.1 Add a `#[cfg(unix)]` `orphaned()` helper returning `getppid() == 1`, and break the loop on `shutdown || orphaned()`; provide a non-Unix stub returning `false` so those targets keep the prior SIGHUP behavior

## 3. Tests

- [ ] 3.1 Behavioral test: `orphaned()` is `false` when the parent process is alive
- [ ] 3.2 Confirm the idle loop waits on the poll timeout rather than redrawing continuously (TUI draw loop is coverage-exempt — a smoke-level assertion is sufficient)

## 4. Integration

- [ ] 4.1 Rebase `fix/dashboard-cpu-leak` @ `4aaa435` onto `feat/v0.10.0-specs`, bumping `TICK_INTERVAL` to ≥800 ms, and mark these tasks complete
