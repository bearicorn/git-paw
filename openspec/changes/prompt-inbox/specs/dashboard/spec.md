## MODIFIED Requirements

### Requirement: Dashboard entry point

The `run_dashboard` function SHALL be extended to support multi-section layout and additional key handling. The function signature remains `pub fn run_dashboard(state: BrokerState, broker_handle: BrokerHandle) -> Result<(), PawError>`.

The function SHALL now accept an optional pane map (agent ID → tmux pane index) to enable reply routing. The dashboard SHALL poll both `agent_status_snapshot` and the supervisor inbox for pending questions on each tick.

#### Scenario: Dashboard starts with multi-section layout

- **GIVEN** a valid `BrokerState`, `BrokerHandle`, and pane map
- **WHEN** `run_dashboard` is called
- **THEN** the rendered output includes all three sections: status table, prompts section, and input field

### Requirement: Quit keybind

The `q` key SHALL still exit the draw loop. The following additional key bindings SHALL be processed:

- `Tab` — cycle focused question in the prompts list
- `Enter` — submit the current input as a reply to the focused agent
- Printable ASCII characters — append to the input buffer
- `Backspace` — remove the last character from the input buffer

No key other than `q` causes the dashboard to exit.

#### Scenario: Printable character appends to input

- **GIVEN** a running dashboard with an empty input field
- **WHEN** the user presses 'Y'
- **THEN** the input buffer contains "Y"
- **AND** the dashboard continues running

#### Scenario: Backspace removes last character

- **GIVEN** the input buffer contains "Yes"
- **WHEN** the user presses Backspace
- **THEN** the input buffer contains "Ye"

#### Scenario: q still exits

- **GIVEN** a running dashboard
- **WHEN** the user presses q
- **THEN** `run_dashboard` returns `Ok(())`

### Requirement: Periodic state polling

The system SHALL poll both `agent_status_snapshot` and the supervisor inbox for pending `agent.question` messages on each tick. The supervisor inbox poll SHALL use `poll_messages(&state, "supervisor", last_question_seq)` to retrieve new questions since the last tick.

#### Scenario: Dashboard polls supervisor inbox each tick

- **GIVEN** a running dashboard
- **WHEN** an agent publishes an `agent.question` message
- **THEN** the question SHALL appear in the prompts section within 2 seconds
