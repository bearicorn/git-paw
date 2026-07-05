## MODIFIED Requirements

### Requirement: Periodic state polling

The system SHALL poll `BrokerState` via `agent_status_snapshot` and SHALL render a new frame whenever a draw is needed. The dashboard SHALL wait for input and events by BLOCKING on the event poll with a bounded idle timeout rather than busy-polling; the idle timeout SHALL be at least 800 milliseconds so that an idle dashboard consumes negligible CPU. Key input and redraw-triggering events SHALL wake the poll immediately, so responsiveness does not depend on the idle timeout. If the event poll returns an error (the terminal is gone), the loop SHALL exit.

The system SHALL NOT hold the `BrokerState` read lock across a draw call or a poll/wait. The lock SHALL be acquired, data cloned, and the lock released before any rendering or waiting occurs.

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
- **WHEN** the user presses `q`
- **THEN** the dashboard reacts immediately — the blocking poll returns on the keypress rather than after the idle timeout

#### Scenario: Idle dashboard does not busy-loop

- **GIVEN** a running dashboard with no pending input and no state changes
- **THEN** it blocks on the event poll for the idle timeout (at least 800 ms) between wakeups instead of redrawing continuously, keeping idle CPU negligible

## ADDED Requirements

### Requirement: Exit when orphaned

The dashboard SHALL terminate when its parent process is no longer present — on Unix, when it has been reparented to init (`getppid() == 1`) — so a dashboard process can never outlive the session that spawned it, regardless of how that session ended, including teardowns that deliver no SIGHUP (an abrupt `tmux kill-server`, a crash, or machine sleep). On platforms without this signal the prior SIGHUP-based shutdown behavior is retained.

#### Scenario: Orphaned dashboard exits

- **GIVEN** a running dashboard whose parent process has terminated, so the dashboard is reparented to init
- **WHEN** the draw loop next checks its lifecycle
- **THEN** the dashboard exits instead of continuing to render

#### Scenario: Dashboard keeps running while its parent is alive

- **GIVEN** a running dashboard whose parent process is still alive
- **WHEN** the draw loop checks its lifecycle
- **THEN** the dashboard continues running
