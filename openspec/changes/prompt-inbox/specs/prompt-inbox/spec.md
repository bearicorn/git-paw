## ADDED Requirements

### Requirement: Prompts section in the dashboard

The dashboard SHALL display a `## Questions` section below the agent status table showing all pending `agent.question` messages from the supervisor inbox. Each entry SHALL display the agent ID and the question text in a single line:

```
[feat-config] Should default_cli show "(default)" label?
```

The prompts section SHALL show at most the 5 most recent unanswered questions. When there are no pending questions, the section SHALL display `(no pending questions)`.

#### Scenario: Prompts section displays pending questions

- **GIVEN** two `agent.question` messages in the supervisor inbox from `feat-config` and `feat-detect`
- **WHEN** the dashboard renders a frame
- **THEN** the prompts section SHALL display both questions with their agent IDs

#### Scenario: Prompts section empty state

- **GIVEN** no `agent.question` messages in the supervisor inbox
- **WHEN** the dashboard renders a frame
- **THEN** the prompts section SHALL display "(no pending questions)"

#### Scenario: Prompts section shows at most 5 questions

- **GIVEN** 7 pending questions in the supervisor inbox
- **WHEN** the dashboard renders a frame
- **THEN** the prompts section SHALL display exactly 5 entries

### Requirement: Text input field for replying

The dashboard SHALL display an input field below the prompts section that shows the currently focused agent ID and a text cursor. The user may type a reply in this field and press `Enter` to send it.

The input field SHALL:
- Show the focused agent ID as a prefix: `Reply to feat-config>`
- Display a cursor at the end of the typed text
- Accept printable ASCII characters and backspace
- Clear after successful reply submission

#### Scenario: Input field shows focused agent

- **GIVEN** the user has pressed Tab to focus the `feat-config` question
- **WHEN** the dashboard renders the input field
- **THEN** the input field prefix SHALL show `Reply to feat-config>`

#### Scenario: Input field clears after reply sent

- **GIVEN** the user has typed a reply and pressed Enter
- **WHEN** the reply is sent successfully
- **THEN** the input field SHALL be cleared
- **AND** the answered question SHALL be removed from the prompts list

### Requirement: Tab cycles through pending questions

Pressing the `Tab` key SHALL advance the focused question to the next item in the prompts list (wrapping from last to first). The focused question SHALL be visually distinguished (e.g., `>` prefix instead of leading space).

When no questions are pending, `Tab` SHALL have no effect.

#### Scenario: Tab advances focus to next question

- **GIVEN** two pending questions with question 1 focused
- **WHEN** the user presses Tab
- **THEN** question 2 SHALL become focused

#### Scenario: Tab wraps from last to first

- **GIVEN** two pending questions with question 2 focused
- **WHEN** the user presses Tab
- **THEN** question 1 SHALL become focused

### Requirement: Enter sends reply to focused agent's pane

Pressing `Enter` with non-empty input SHALL:
1. Send the input text to the focused agent's tmux pane via `tmux send-keys -t <session>:<pane_index> "<text>" Enter`
2. Remove the answered question from the prompts list
3. Clear the input field

If the input is empty, `Enter` SHALL have no effect.

#### Scenario: Enter sends non-empty reply to agent pane

- **GIVEN** the user has typed "Yes, use (default) label" with `feat-config` focused
- **WHEN** the user presses Enter
- **THEN** `tmux send-keys` SHALL be invoked for the `feat-config` pane with the typed text

#### Scenario: Enter with empty input does nothing

- **GIVEN** the input field is empty
- **WHEN** the user presses Enter
- **THEN** no `tmux send-keys` call is made
- **AND** the focus and prompts list are unchanged

### Requirement: QuestionEntry type

The system SHALL define a `pub struct QuestionEntry` with at least these fields:

- `agent_id: String` — the agent asking the question
- `pane_index: usize` — the tmux pane index for reply routing
- `question: String` — the question text
- `seq: u64` — sequence number for ordering

`QuestionEntry` SHALL derive `Debug` and `Clone`.

#### Scenario: QuestionEntry is constructible from an agent.question message

- **GIVEN** an `agent.question` message with `agent_id = "feat-config"` and `question = "Should I skip tests?"`
- **WHEN** a `QuestionEntry` is constructed from this message
- **THEN** `agent_id` SHALL be `"feat-config"`
- **AND** `question` SHALL be `"Should I skip tests?"`
