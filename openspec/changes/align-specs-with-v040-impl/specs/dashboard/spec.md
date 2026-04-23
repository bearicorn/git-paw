## MODIFIED Requirements

### Requirement: Dashboard entry point

The system SHALL provide a public function with the signature:

```rust
pub fn run_dashboard(
    state: &Arc<BrokerState>,
    broker_handle: BrokerHandle,
    shutdown: &AtomicBool,
) -> Result<(), PawError>
```

This function SHALL:

1. Enter crossterm alternate screen and raw mode
2. Install a panic hook that restores the terminal before printing the panic
3. Run an event-driven draw loop reading from `&Arc<BrokerState>` so the broker state is shared with HTTP handlers, watcher tasks, and the dashboard without copying
4. Exit when the user presses `q` OR when `shutdown` is set to `true` by an external signal handler
5. Restore the terminal (raw mode off, leave alternate screen) on exit or error
6. Return `Ok(())` on clean exit

The function SHALL take ownership of `BrokerHandle` so that the broker shuts down automatically when the dashboard exits and the handle is dropped. The `shutdown` flag SHALL allow `cmd_supervisor` and `cmd_start` to request a clean dashboard exit when their own signal handlers fire.

#### Scenario: Dashboard starts and stops cleanly

- **GIVEN** a valid `&Arc<BrokerState>`, `BrokerHandle`, and a `shutdown: &AtomicBool` initialised to `false`
- **WHEN** `run_dashboard` is called and the user presses `q`
- **THEN** the function returns `Ok(())`
- **AND** the terminal is restored to its pre-dashboard state

#### Scenario: External shutdown flag exits the dashboard

- **GIVEN** a running dashboard
- **WHEN** another thread sets `shutdown.store(true, Ordering::Release)`
- **THEN** the dashboard exits cleanly within one input-poll interval
- **AND** the function returns `Ok(())`

#### Scenario: BrokerHandle is dropped on dashboard exit

- **GIVEN** a valid state, handle, and shutdown flag pointing to a running broker
- **WHEN** `run_dashboard` returns
- **THEN** the `BrokerHandle` is dropped
- **AND** the broker stops accepting HTTP requests

### Requirement: Periodic state polling

The system SHALL poll `BrokerState` via `agent_status_snapshot` on every input-poll cycle and SHALL render a new frame whenever a draw is needed. The input-poll cycle SHALL be no longer than 100 milliseconds so that key presses (Tab, Enter, printable characters, Backspace, `q`) feel responsive in the prompt inbox.

The system SHALL NOT hold the `BrokerState` read lock across a draw call or a sleep. The lock SHALL be acquired, data cloned, and the lock released before any rendering or waiting occurs.

#### Scenario: Dashboard refreshes within one second of a state change

- **GIVEN** a running dashboard
- **WHEN** an agent's status changes in `BrokerState`
- **THEN** the dashboard displays the updated status within 1 second

#### Scenario: Lock is not held during rendering

- **GIVEN** a running dashboard
- **WHEN** the dashboard renders a frame
- **THEN** no `BrokerState` read lock is held during the ratatui draw call

#### Scenario: Input is responsive

- **GIVEN** a running dashboard awaiting input
- **WHEN** the user presses any handled key
- **THEN** the dashboard reacts within 100 milliseconds
