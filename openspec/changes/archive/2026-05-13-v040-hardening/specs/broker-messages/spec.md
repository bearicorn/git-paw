## ADDED Requirements

### Requirement: Question message variant

The `BrokerMessage` enum SHALL include a `Question` variant with serde tag `"agent.question"`. The variant SHALL carry `agent_id: String` (the asking agent — typically a coding agent or the supervisor itself) and `payload: QuestionPayload`.

`QuestionPayload` SHALL contain a single field:
- `question: String` — the free-text question the agent is asking. The recipient is implied by the routing rule (`Question` messages are routed to the `"supervisor"` inbox; see `message-delivery`).

The variant SHALL derive `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, and `Deserialize` matching the existing variant conventions.

#### Scenario: Question message round-trips through serde

- **WHEN** a `BrokerMessage::Question` with `agent_id = "feat-x"` and a populated `QuestionPayload` is serialized to JSON and then deserialized back
- **THEN** the resulting value equals the original
- **AND** the intermediate JSON contains `"type": "agent.question"` and `"agent_id": "feat-x"` at the top level
- **AND** the intermediate JSON contains the payload nested under a `"payload"` key with `"question": "<text>"`

#### Scenario: Question payload with whitespace-only question is rejected

- **WHEN** a JSON message of type `agent.question` with `payload.question = "   "` is parsed via the validating constructor
- **THEN** validation fails with an error identifying the empty/whitespace-only `question` field

### Requirement: Validation for Question variant

The system SHALL validate `Question` messages via the existing `from_json` validating constructor. The system SHALL reject input where:

- `agent_id` violates the existing slug rules (empty, whitespace-only, contains characters outside the slug character set).
- `payload.question` is empty or contains only whitespace after trimming.

`payload.question` length is unbounded in v0.5.0; long questions are accepted as-is (matching the shipped `MessageError::EmptyQuestionField` validation behaviour).

#### Scenario: Empty question is rejected

- **WHEN** a JSON message of type `agent.question` with `payload.question = ""` is parsed via `from_json`
- **THEN** validation fails with the `EmptyQuestionField` error variant (or equivalent error identifying `question` as the cause)

#### Scenario: Whitespace-only question is rejected

- **WHEN** a JSON message of type `agent.question` with `payload.question = "  \n  "` is parsed via `from_json`
- **THEN** validation fails with an error identifying `question` as the cause

#### Scenario: Empty agent_id on Question is rejected

- **WHEN** a JSON message of type `agent.question` with `agent_id = ""` is parsed via `from_json`
- **THEN** validation fails with an error identifying `agent_id` as the cause

#### Scenario: Valid Question JSON produces a BrokerMessage

- **WHEN** a well-formed JSON message of type `agent.question` is parsed via `from_json`
- **THEN** a `BrokerMessage::Question` value is produced
- **AND** all fields of the resulting value match the input

### Requirement: Display for Question variant

The `Display` impl SHALL format the `Question` variant as:

```
[{agent_id}] question: {payload.question}
```

The output SHALL be a single line of plain text containing no newline characters and no ANSI escape codes.

#### Scenario: Question Display output

- **WHEN** a `Question` message with `agent_id = "supervisor"` and `payload.question = "Should I merge feat-a before feat-b?"` is formatted via `Display`
- **THEN** the resulting string is `[supervisor] question: Should I merge feat-a before feat-b?`
- **AND** the string contains no newline characters
- **AND** the string contains no ANSI escape sequences

### Requirement: status_label for Question variant

The `BrokerMessage::status_label()` method SHALL return `"question"` for the `Question` variant.

#### Scenario: status_label for Question

- **WHEN** `status_label()` is called on a `Question` message
- **THEN** the result is `"question"`

### Requirement: agent_id for Question variant

The `BrokerMessage::agent_id()` method SHALL return the `agent_id` field of the `Question` variant.

#### Scenario: agent_id for Question

- **WHEN** `agent_id()` is called on a `Question` message with `agent_id = "feat-x"`
- **THEN** the result is `"feat-x"`
