## ADDED Requirements

### Requirement: Dashboard entry point

The system SHALL provide a public function `pub fn run_dashboard(state: BrokerState, broker_handle: BrokerHandle) -> Result<(), PawError>` that:

1. Enters crossterm alternate screen and raw mode
2. Installs a panic hook that restores the terminal before printing the panic
3. Runs a tick-based draw loop reading from `BrokerState`
4. Exits when the user presses `q`
5. Restores the terminal (raw mode off, leave alternate screen) on exit or error
6. Returns `Ok(())` on clean exit

The function SHALL take ownership of `BrokerHandle` so that the broker shuts down automatically when the dashboard exits and the handle is dropped.

#### Scenario: Dashboard starts and stops cleanly

- **GIVEN** a valid `BrokerState` and `BrokerHandle`
- **WHEN** `run_dashboard` is called and the user presses `q`
- **THEN** the function returns `Ok(())`
- **AND** the terminal is restored to its pre-dashboard state

#### Scenario: BrokerHandle is dropped on dashboard exit

- **GIVEN** a valid `BrokerState` and `BrokerHandle` pointing to a running broker
- **WHEN** `run_dashboard` returns
- **THEN** the `BrokerHandle` is dropped
- **AND** the broker stops accepting HTTP requests

### Requirement: Terminal lifecycle management

The system SHALL manage terminal state transitions for the ratatui TUI. On entry, the system SHALL enable raw mode and enter the alternate screen. On exit — whether from a clean `q` press, an error, or a panic — the system SHALL disable raw mode and leave the alternate screen so the user's terminal is usable.

#### Scenario: Terminal restored after clean exit

- **WHEN** `run_dashboard` returns `Ok(())`
- **THEN** raw mode is disabled
- **AND** the alternate screen is exited

#### Scenario: Terminal restored after error

- **WHEN** `run_dashboard` encounters an error and returns `Err(...)`
- **THEN** raw mode is disabled
- **AND** the alternate screen is exited

#### Scenario: Terminal restored after panic

- **WHEN** a panic occurs inside the draw loop
- **THEN** the installed panic hook disables raw mode and exits the alternate screen before the panic message is printed
- **AND** the panic message is readable in the normal terminal

### Requirement: Periodic state polling

The system SHALL poll `BrokerState` via `agent_status_snapshot` on a periodic tick of approximately 1 second. Each tick SHALL read the current agent records, format them into display rows, and render a new frame.

The system SHALL NOT hold the `BrokerState` read lock across a draw call or a sleep. The lock SHALL be acquired, data cloned, and the lock released before any rendering or waiting occurs.

#### Scenario: Dashboard refreshes every tick

- **GIVEN** a running dashboard
- **WHEN** an agent's status changes in `BrokerState`
- **THEN** the dashboard displays the updated status within 2 seconds

#### Scenario: Lock is not held during rendering

- **GIVEN** a running dashboard
- **WHEN** the dashboard renders a frame
- **THEN** no `BrokerState` read lock is held during the ratatui draw call

### Requirement: Quit keybind

The system SHALL exit the draw loop when the user presses the `q` key. No other keyboard input SHALL be processed in v0.3.0. The system SHALL poll for keyboard events with a non-blocking timeout so that key presses are detected promptly without blocking the tick cycle.

#### Scenario: Pressing q exits the dashboard

- **GIVEN** a running dashboard
- **WHEN** the user presses `q`
- **THEN** the draw loop exits
- **AND** `run_dashboard` returns `Ok(())`

#### Scenario: Other keys are ignored

- **GIVEN** a running dashboard
- **WHEN** the user presses any key other than `q`
- **THEN** the dashboard continues running

### Requirement: Agent status table rendering

The system SHALL render a table displaying all known agents with the following columns:

| Column | Content | Width |
|---|---|---|
| Agent | The `agent_id` (slugified branch name) | Flexible |
| CLI | The CLI name (e.g. `"claude"`) | Fixed ~10 |
| Status | A Unicode symbol + status label | Fixed ~15 |
| Last Update | Relative time since last message | Fixed ~10 |
| Summary | One-line summary from the last message's `Display` output | Flexible |

The table SHALL have a header row with column labels. When no agents are known (e.g. at session start before any agent has posted), the table SHALL display a single row or message indicating "No agents connected yet".

#### Scenario: Table displays agent rows

- **GIVEN** `agent_status_snapshot` returns two agents: `feat-errors` (status "done", 3 minutes ago) and `feat-detect` (status "working", 30 seconds ago)
- **WHEN** the dashboard renders a frame
- **THEN** the table contains two data rows with the correct agent IDs, statuses, and relative times

#### Scenario: Table displays empty state

- **GIVEN** `agent_status_snapshot` returns an empty list
- **WHEN** the dashboard renders a frame
- **THEN** the table area displays "No agents connected yet"

#### Scenario: Table has a header row

- **WHEN** the dashboard renders a frame with at least one agent
- **THEN** the first row of the table contains column labels: Agent, CLI, Status, Last Update, Summary

### Requirement: Agent row formatting as pure functions

The system SHALL provide pure functions for formatting agent data into display-ready rows. These functions SHALL perform no I/O, hold no locks, and be deterministic given the same inputs.

- `pub fn format_agent_rows(agents: &[AgentStatusEntry], now: Instant) -> Vec<AgentRow>` — converts raw agent data into formatted row structs
- `pub fn format_status_line(total: usize, working: usize, done: usize, blocked: usize) -> String` — produces a summary line like `"4 agents: 2 working, 1 done, 1 blocked"`

`AgentRow` SHALL be a public struct with `String` fields: `agent_id`, `cli`, `status`, `age`, `summary`.

#### Scenario: format_agent_rows produces correct row count

- **GIVEN** a list of 3 `AgentStatusEntry` values
- **WHEN** `format_agent_rows(agents, now)` is called
- **THEN** the result contains exactly 3 `AgentRow` values

#### Scenario: format_agent_rows populates all fields

- **GIVEN** an `AgentStatusEntry` with `agent_id = "feat-errors"`, status `"done"`, last seen 180 seconds ago
- **WHEN** `format_agent_rows` is called
- **THEN** the resulting `AgentRow` has `agent_id = "feat-errors"`, a non-empty `status` field containing `"done"`, and `age = "3m ago"`

#### Scenario: format_status_line produces a summary

- **WHEN** `format_status_line(4, 2, 1, 1)` is called
- **THEN** the result is `"4 agents: 2 working, 1 done, 1 blocked"`

#### Scenario: format_status_line with all done

- **WHEN** `format_status_line(3, 0, 3, 0)` is called
- **THEN** the result is `"3 agents: 0 working, 3 done, 0 blocked"`

### Requirement: Human-readable age formatting

The system SHALL provide a pure function `pub fn format_age(elapsed: Duration) -> String` that converts a duration into a human-readable relative time string:

- Less than 60 seconds → `"Xs ago"` (e.g. `"30s ago"`)
- 1 to 59 minutes → `"Xm ago"` (e.g. `"3m ago"`)
- 60 minutes or more → `"Xh Ym ago"` (e.g. `"1h 15m ago"`)

#### Scenario: Seconds range

- **WHEN** `format_age(Duration::from_secs(30))` is called
- **THEN** the result is `"30s ago"`

#### Scenario: Zero seconds

- **WHEN** `format_age(Duration::from_secs(0))` is called
- **THEN** the result is `"0s ago"`

#### Scenario: Minutes range

- **WHEN** `format_age(Duration::from_secs(180))` is called
- **THEN** the result is `"3m ago"`

#### Scenario: Hours and minutes

- **WHEN** `format_age(Duration::from_secs(4500))` is called
- **THEN** the result is `"1h 15m ago"`

#### Scenario: Exact hour boundary

- **WHEN** `format_age(Duration::from_secs(3600))` is called
- **THEN** the result is `"1h 0m ago"`

### Requirement: Status symbol mapping

The system SHALL provide a pure function `pub fn status_symbol(status: &str) -> &'static str` that maps agent status labels to Unicode symbols:

| Input | Output |
|---|---|
| `"working"` | `"🔵"` |
| `"done"` | `"🟢"` |
| `"verified"` | `"🟢"` |
| `"blocked"` | `"🟡"` |
| `"idle"` | `"⚪"` |
| any other value | `"⚪"` |

#### Scenario: Working status symbol

- **WHEN** `status_symbol("working")` is called
- **THEN** the result is `"🔵"`

#### Scenario: Done status symbol

- **WHEN** `status_symbol("done")` is called
- **THEN** the result is `"🟢"`

#### Scenario: Verified status symbol

- **WHEN** `status_symbol("verified")` is called
- **THEN** the result is `"🟢"`

#### Scenario: Blocked status symbol

- **WHEN** `status_symbol("blocked")` is called
- **THEN** the result is `"🟡"`

#### Scenario: Idle status symbol

- **WHEN** `status_symbol("idle")` is called
- **THEN** the result is `"⚪"`

#### Scenario: Unknown status falls back to default

- **WHEN** `status_symbol("something-unexpected")` is called
- **THEN** the result is `"⚪"`

### Requirement: Dashboard title

The system SHALL display a title line above the agent table containing the text `"git-paw dashboard"`. The title SHALL be visible at all times while the dashboard is running.

#### Scenario: Title is displayed

- **WHEN** the dashboard renders a frame
- **THEN** the rendered output includes the text `"git-paw dashboard"`
