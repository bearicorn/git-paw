## MODIFIED Requirements

### Requirement: Periodic state polling

The system SHALL poll `BrokerState` via `agent_status_snapshot` on every input-poll cycle and SHALL render a new frame whenever a draw is needed. The input-poll cycle SHALL be no longer than 100 milliseconds so that the `q` keybind feels responsive.

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
- **WHEN** the user presses `q`
- **THEN** the dashboard reacts within 100 milliseconds

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

**Supervisor row placement.** When the agent snapshot contains an entry with `agent_id == "supervisor"`, the table SHALL render that entry as the first data row (row 0 below the header), regardless of the alphabetical ordering of the other entries. A visually distinguishable divider row SHALL be rendered immediately below the supervisor row to separate it from the coding-agent rows. The coding-agent rows SHALL follow the divider in their existing alphabetical-by-`agent_id` order.

When no `agent_id == "supervisor"` entry is present in the snapshot, no divider SHALL be rendered, and the coding-agent rows SHALL render in their existing alphabetical order starting from row 0.

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

#### Scenario: Supervisor row is pinned to the top of the data rows

- **GIVEN** `agent_status_snapshot` returns three agents: `feat-broker`, `supervisor`, `feat-dashboard` (in alphabetical order, that ordering is `feat-broker`, `feat-dashboard`, `supervisor`)
- **WHEN** the dashboard renders a frame
- **THEN** the first data row (below the header) is the `supervisor` row
- **AND** a visually distinguishable divider row is rendered immediately below the supervisor row
- **AND** the subsequent rows are `feat-broker` then `feat-dashboard` (alphabetical)

#### Scenario: No divider when supervisor row is absent

- **GIVEN** `agent_status_snapshot` returns two agents: `feat-broker` and `feat-dashboard`, neither of which is `supervisor`
- **WHEN** the dashboard renders a frame
- **THEN** the first data row is `feat-broker` and the second is `feat-dashboard`
- **AND** no divider row is rendered

### Requirement: Agent row formatting as pure functions

The system SHALL provide pure functions for formatting agent data into display-ready rows. These functions SHALL perform no I/O, hold no locks, and be deterministic given the same inputs.

- `pub fn format_agent_rows(agents: &[AgentStatusEntry], now: Instant) -> Vec<AgentRow>` — converts raw agent data into formatted row structs
- `pub fn format_status_line(total: usize, working: usize, done: usize, blocked: usize, committed: usize) -> String` — produces a summary line like `"5 agents: 2 working, 1 done, 1 blocked, 1 committed"`

`AgentRow` SHALL be a public struct with `String` fields: `agent_id`, `cli`, `status`, `age`, `summary`.

**Phase preference.** When `format_agent_rows` builds the row for an entry whose underlying snapshot carries a most-recent `BrokerMessage::Status` with `payload.phase = Some(p)`, the row's `status` field SHALL render `p` (with the same status-symbol prefixing applied as for any other label). When `payload.phase` is `None` (or the most-recent message is not a `Status` variant), the row's `status` field SHALL fall back to the existing message-type-derived label.

#### Scenario: format_agent_rows produces correct row count

- **GIVEN** a list of 3 `AgentStatusEntry` values
- **WHEN** `format_agent_rows(agents, now)` is called
- **THEN** the result contains exactly 3 `AgentRow` values

#### Scenario: format_agent_rows populates all fields

- **GIVEN** an `AgentStatusEntry` with `agent_id = "feat-errors"`, status `"done"`, last seen 180 seconds ago
- **WHEN** `format_agent_rows` is called
- **THEN** the resulting `AgentRow` has `agent_id = "feat-errors"`, a non-empty `status` field containing `"done"`, and `age = "3m ago"`

#### Scenario: format_status_line produces a summary

- **WHEN** `format_status_line(4, 2, 1, 1, 0)` is called
- **THEN** the result is `"4 agents: 2 working, 1 done, 1 blocked, 0 committed"`

#### Scenario: format_agent_rows prefers phase over status_label for supervisor

- **GIVEN** an `AgentStatusEntry` for `agent_id = "supervisor"` whose most-recent message is a `BrokerMessage::Status` with `payload.status = "feedback"`, `payload.phase = Some("merging")`
- **WHEN** `format_agent_rows` is called
- **THEN** the resulting supervisor `AgentRow`'s `status` field contains `"merging"`
- **AND** the `status` field does NOT contain `"feedback"`

#### Scenario: format_agent_rows falls back to status_label when phase is None

- **GIVEN** an `AgentStatusEntry` for `agent_id = "feat-broker"` whose most-recent message is a `BrokerMessage::Status` with `payload.status = "working"`, `payload.phase = None`
- **WHEN** `format_agent_rows` is called
- **THEN** the resulting `AgentRow`'s `status` field contains `"working"`

## ADDED Requirements

### Requirement: No prompt-inbox panel

The dashboard SHALL NOT render a prompt-inbox panel for `agent.question` messages. Specifically, the dashboard layout SHALL NOT include:

- A "Questions (N pending)" titled block listing pending `agent.question` messages
- An input field for replying to questions
- A focused-question cursor or any keybindings (Tab, Enter, Backspace, printable characters) for navigating or composing replies in the dashboard

`agent.question` messages SHALL continue to flow through the broker per the `message-delivery` capability (routed to the `"supervisor"` inbox), and the supervisor agent SHALL read and respond to them via curl + the supervisor pane. The dashboard's role is observation only — agent status and (optionally, in v0.6.0+) a recent-messages log.

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
