## MODIFIED Requirements

### Requirement: Dashboard entry point

The system SHALL provide a public function with the signature:

```rust
pub fn run_dashboard(
    client: &BrokerClient,
    shutdown: &AtomicBool,
) -> Result<(), PawError>
```

where `BrokerClient` is the shared HTTP client (`src/broker/client.rs`) bound to the session's broker URL. This function SHALL:

1. Enter crossterm alternate screen and raw mode
2. Install a panic hook that restores the terminal before printing the panic
3. Run an event-driven draw loop whose data comes only from `client` (`GET /status`, `GET /log?since=`)
4. Exit when the user presses `q` OR when `shutdown` is set to `true` by an external signal handler
5. Restore the terminal (raw mode off, leave alternate screen) on exit or error
6. Return `Ok(())` on clean exit

The function SHALL NOT take a `BrokerState` or a `BrokerHandle`; exiting the dashboard SHALL have no effect on the broker.

#### Scenario: Dashboard starts and stops cleanly

- **GIVEN** a `BrokerClient` for a live broker and a `shutdown: &AtomicBool` initialised to `false`
- **WHEN** `run_dashboard` is called and the user presses `q`
- **THEN** the function returns `Ok(())`
- **AND** the terminal is restored to its pre-dashboard state
- **AND** the broker is still serving

#### Scenario: External shutdown flag exits the dashboard

- **GIVEN** a running dashboard
- **WHEN** another thread sets `shutdown.store(true, Ordering::Release)`
- **THEN** the dashboard exits cleanly within one input-poll interval
- **AND** the function returns `Ok(())`

### Requirement: Periodic state polling

The system SHALL refresh its model by polling the broker over HTTP — `GET /status` for the roster and the wave status, and `GET /log?since=<last_seq>` for the log panel — on each wake-up, requesting only log entries after the last sequence number it has already seen, and SHALL render a new frame whenever a draw is needed. The dashboard SHALL wait for input by polling its controlling terminal with a bounded idle timeout rather than busy-polling; the idle timeout SHALL be at least 800 milliseconds and at most 1000 milliseconds so that an idle dashboard consumes negligible CPU while still reflecting a state change within one second. Key input SHALL wake the wait immediately. The wait SHALL be bounded even when the terminal is gone: a hung-up controlling terminal SHALL be detected and SHALL cause the loop to exit rather than trap the wait indefinitely. HTTP requests SHALL have a bounded timeout so a slow broker cannot freeze the draw loop.

#### Scenario: Dashboard refreshes within one second of a state change

- **GIVEN** a running dashboard
- **WHEN** an agent's status changes on the broker
- **THEN** the dashboard displays the updated status within 1 second

#### Scenario: Input is responsive

- **GIVEN** a running dashboard awaiting input
- **WHEN** the user presses `q`
- **THEN** the dashboard reacts immediately — the terminal poll returns on the keypress rather than after the idle timeout

#### Scenario: Idle dashboard does not busy-loop

- **GIVEN** a running dashboard with no pending input and no state changes
- **THEN** it waits on the bounded terminal poll for the idle timeout between wakeups instead of redrawing continuously

#### Scenario: A slow broker does not freeze the loop

- **GIVEN** a broker that stops answering mid-session
- **WHEN** the dashboard polls it
- **THEN** each request SHALL time out within the bounded timeout and the loop SHALL continue to process input

### Requirement: Exit when orphaned

The dashboard SHALL terminate when its session is gone, and SHALL NOT busy-loop after that point regardless of how the session ended. Specifically:

- The dashboard SHALL exit when its parent process is no longer present — on Unix, when it has been reparented to init (`getppid() == 1`) — so it never outlives a session torn down without SIGHUP. On platforms without this signal the SIGHUP-based shutdown behavior is retained.
- The dashboard SHALL additionally exit when its controlling terminal is gone — detected by polling the controlling terminal for hang-up (`POLLHUP`/`POLLERR`/`POLLNVAL`) or by a failed write to the terminal. The terminal wait SHALL be bounded so a hung-up terminal cannot trap the loop before the lifecycle check runs.
- An unreachable broker SHALL NOT be an exit condition: the dashboard renders its degraded state and keeps polling.
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

- **GIVEN** a dashboard whose tmux pane was killed but whose parent is a lingering shell (not init)
- **WHEN** the draw loop next interacts with the (now-gone) controlling terminal
- **THEN** the tty-gone condition is detected and the dashboard exits rather than busy-looping

#### Scenario: Unreachable broker does not exit the dashboard

- **GIVEN** a running dashboard whose broker has been paused
- **WHEN** the draw loop's next poll fails to connect
- **THEN** the dashboard SHALL keep running and render the broker-unreachable state

#### Scenario: Lifecycle check is not bypassed on error paths

- **GIVEN** a running dashboard that takes an error or degraded branch of its loop
- **WHEN** that branch executes
- **THEN** the same shutdown / orphan / tty-gone check applies, so the dashboard cannot busy-loop on any path

### Requirement: Watcher restart resilience

The panel SHALL NOT clear its ring buffer when the broker restarts or becomes unreachable. Historical messages SHALL remain visible across a broker outage or pause; when the broker answers again the panel SHALL resume ingesting from its cursor. Because an empty `/log` response holds `last_seq` at the requested cursor, the panel SHALL detect a restarted host from `/status` — an `uptime_seconds` lower than the one it last saw — and SHALL then re-read the new log from sequence 0, ingesting it without duplicating entries it already holds.

#### Scenario: Buffer survives a broker outage

- **GIVEN** a panel with N messages in its buffer
- **WHEN** the broker becomes unreachable and later answers again without restarting
- **THEN** the panel SHALL still show the N historical messages and SHALL continue to display new messages as they arrive

#### Scenario: Fresh broker log is ingested without duplicates

- **GIVEN** a panel holding messages up to sequence 40
- **WHEN** a restarted broker host reports a log beginning again at sequence 1
- **THEN** the panel SHALL keep its 40 historical rows and append the new log's entries once each

## ADDED Requirements

### Requirement: Dashboard degrades when the broker is unreachable

The dashboard SHALL render a visible "broker unreachable" indicator in its status line whenever the most recent poll failed to reach the broker, SHALL keep displaying the last successful snapshot, SHALL keep polling on its idle cadence, and SHALL clear the indicator on the first successful poll. The indicator SHALL be independent of the stalled-wave indicator. It SHALL NOT exit and SHALL NOT busy-loop while the broker is unreachable.

#### Scenario: Paused broker shows the indicator

- **GIVEN** a running dashboard and `git paw pause` stopping the broker
- **WHEN** the dashboard's next poll fails
- **THEN** the status line SHALL show the broker-unreachable indicator
- **AND** the agent table SHALL still show the last snapshot

#### Scenario: Recovered broker clears the indicator

- **GIVEN** a dashboard showing the broker-unreachable indicator
- **WHEN** `git paw start` restarts the broker and the next poll succeeds
- **THEN** the indicator SHALL disappear and live data SHALL render
