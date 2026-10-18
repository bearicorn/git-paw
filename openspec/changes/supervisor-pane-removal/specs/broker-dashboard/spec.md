## MODIFIED Requirements

### Requirement: Dashboard chapter reflects the supervisor-as-pane state

`docs/src/user-guide/dashboard.md` SHALL describe the dashboard in its current state: it lives at pane 0 in every session mode (bare and supervised), shows the agents status table and the broker log panel, and does NOT include an interactive prompt inbox panel. The chapter SHALL NOT contain forward-looking statements claiming that an earlier version will add features that have since shipped or been removed, and SHALL NOT place the dashboard at pane 1.

#### Scenario: Dashboard chapter does not promise a prompt inbox

- **WHEN** `docs/src/user-guide/dashboard.md` is inspected
- **THEN** it does NOT contain forward-looking text claiming a future version will add an interactive prompt inbox panel

#### Scenario: Dashboard chapter places the dashboard at pane 0 in every mode

- **WHEN** `docs/src/user-guide/dashboard.md` is inspected
- **THEN** any reference to the dashboard's pane location states that it is at pane 0
- **AND** no sentence states that the dashboard is at pane 1

### Requirement: No prompt-inbox panel

The dashboard SHALL NOT render a prompt-inbox panel for `agent.question` messages. Specifically, the dashboard layout SHALL NOT include:

- A "Questions (N pending)" titled block listing pending `agent.question` messages
- An input field for replying to questions
- A focused-question cursor or any keybindings (Tab, Enter, Backspace, printable characters) for navigating or composing replies in the dashboard

`agent.question` messages SHALL continue to flow through the broker per the `message-delivery` capability (routed to the `"supervisor"` inbox), and the orchestrator — the operator's own agent session outside the tmux session, or a person — SHALL read and respond to them through the bundled helpers or the MCP tools. The dashboard's role is observation only — agent status and, optionally, a recent-messages log.

The `q` keybind SHALL remain the sole keyboard input handled by the dashboard.

#### Scenario: Dashboard layout has no Questions panel

- **GIVEN** a running dashboard with at least one `agent.question` in the broker's supervisor inbox
- **WHEN** the dashboard renders a frame
- **THEN** the rendered output does NOT contain the substring `"Questions ("`
- **AND** the rendered output does NOT contain a "Reply to" input prompt

#### Scenario: Tab key is not handled

- **GIVEN** a running dashboard
- **WHEN** the user presses Tab
- **THEN** the dashboard continues running without changing its display state
- **AND** no focus indicator is shown

#### Scenario: Printable character keys do not enter an input buffer

- **GIVEN** a running dashboard
- **WHEN** the user presses any printable character (e.g. `a`, `1`, space) other than `q`
- **THEN** the dashboard continues running without buffering the character
- **AND** no input field is updated or rendered

#### Scenario: Vertical layout collapses to the non-inbox shape

- **GIVEN** a running dashboard with `show_message_log = false`
- **WHEN** the dashboard renders a frame
- **THEN** the vertical layout contains, in order: a title chunk, the agent table chunk, and the status line chunk
- **AND** no prompts-section or input-field chunk is allocated
