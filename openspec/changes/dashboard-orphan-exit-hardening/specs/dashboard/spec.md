## MODIFIED Requirements

### Requirement: Exit when orphaned

The dashboard SHALL terminate when its session is gone, and SHALL NOT busy-loop after that point regardless of how the session ended or whether its in-process broker started. Specifically:

- The dashboard SHALL exit when its parent process is no longer present — on Unix, when it has been reparented to init (`getppid() == 1`) — so it never outlives a session torn down without SIGHUP (an abrupt `tmux kill-server`, a crash, or machine sleep). On platforms without this signal the prior SIGHUP-based shutdown behavior is retained.
- The dashboard SHALL additionally exit when its controlling terminal / stdout is gone (the event poll returns an error, or a write to the terminal fails), so a dashboard reparented to a lingering shell (a parent that is alive but is not init) still terminates once its pane is gone.
- If the in-process broker fails to bind its port, the dashboard SHALL emit a diagnostic and exit rather than enter a render/retry busy-loop.
- The shutdown / orphan / tty-gone check SHALL be evaluated on every loop path, including any error or degraded path, so no branch can bypass it and busy-loop.

#### Scenario: Orphaned-to-init dashboard exits

- **GIVEN** a running dashboard whose parent process has terminated, so it is reparented to init
- **WHEN** the draw loop next checks its lifecycle
- **THEN** the dashboard exits instead of continuing to render

#### Scenario: Dashboard keeps running while its parent is alive and its pane is present

- **GIVEN** a running dashboard whose parent process is alive and whose controlling terminal is present
- **WHEN** the draw loop checks its lifecycle
- **THEN** the dashboard continues running

#### Scenario: Reparented-to-shell dashboard exits when its pane is gone

- **GIVEN** a dashboard whose tmux pane was killed but whose parent is a lingering shell (not init), so `getppid()` is a live but unrelated process
- **WHEN** the draw loop next interacts with the (now-gone) controlling terminal
- **THEN** the tty-gone condition is detected and the dashboard exits rather than busy-looping

#### Scenario: Broker-bind failure exits instead of busy-looping

- **GIVEN** a dashboard whose in-process broker cannot bind its port (for example, a stale dashboard is still holding it)
- **WHEN** the dashboard starts
- **THEN** it emits a diagnostic and exits, rather than entering a high-CPU render/retry loop

#### Scenario: Lifecycle check is not bypassed on error paths

- **GIVEN** a running dashboard that takes an error or degraded branch of its loop
- **WHEN** that branch executes
- **THEN** the same shutdown / orphan / tty-gone check applies, so the dashboard cannot busy-loop on any path
