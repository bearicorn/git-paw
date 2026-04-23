## MODIFIED Requirements

### Requirement: Broker message envelope

The `BrokerMessage` enum SHALL be extended with a fourth variant `Question` carrying `agent_id: String` and a `QuestionPayload` struct.

The wire format discriminator SHALL use the value `agent.question`. The existing variants (`Status`, `Artifact`, `Blocked`) and their wire format are unchanged.

#### Scenario: Question message round-trips through serde

- **WHEN** a `BrokerMessage::Question` with `agent_id = "feat-config"` and `question = "Should I skip tests?"` is serialized to JSON and then deserialized back
- **THEN** the resulting value equals the original
- **AND** the intermediate JSON contains `"type": "agent.question"` and `"agent_id": "feat-config"` at the top level

#### Scenario: Unknown message type still rejected

- **WHEN** a JSON object with `"type": "agent.unknown"` is parsed as a `BrokerMessage`
- **THEN** parsing fails with a deserialization error

## ADDED Requirements

### Requirement: Question payload shape

The `QuestionPayload` struct SHALL contain:

- `question: String` — the question text the agent is asking

#### Scenario: Question payload with non-empty question

- **WHEN** a `QuestionPayload { question: "Should I add a config field?" }` is serialized and deserialized
- **THEN** the round-trip preserves the value

### Requirement: Message validation

The validation rules SHALL be extended for the `Question` variant:

- `question` MUST NOT be empty

#### Scenario: Empty question field in agent.question is rejected

- **WHEN** a JSON message of type `agent.question` with `payload.question = ""` is parsed via the validating constructor
- **THEN** validation fails with an error identifying the empty `question` field

### Requirement: Message display formatting

The `Display` implementation SHALL be extended for the `Question` variant. The format SHALL be:

```
[feat-config] question: Should I add a config field?
```

#### Scenario: Question message Display output

- **WHEN** a `BrokerMessage::Question` with `agent_id = "feat-config"` and `question = "Should I add a config field?"` is formatted via `Display`
- **THEN** the resulting string is `[feat-config] question: Should I add a config field?`
- **AND** the string contains no newline characters

### Requirement: BrokerMessage helper methods

The `agent_id()` and `status_label()` helper methods SHALL be extended to cover the `Question` variant:

- `agent_id()` — returns `agent_id` from the `Question` variant
- `status_label()` — returns `"question"` for the `Question` variant

#### Scenario: status_label returns question for Question variant

- **WHEN** `status_label()` is called on a `Question` message
- **THEN** the result is `"question"`
