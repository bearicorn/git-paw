## ADDED Requirements

### Requirement: Verified message variant

The `BrokerMessage` enum SHALL include a `Verified` variant with serde tag `"agent.verified"`. The variant SHALL carry `agent_id: String` and `payload: VerifiedPayload`.

`VerifiedPayload` SHALL contain:
- `verified_by: String` — the agent_id of the verifier (typically `"supervisor"`)
- `message: Option<String>` — optional human-readable summary

#### Scenario: Verified message round-trips through serde

- **WHEN** a `BrokerMessage::Verified` with `agent_id = "feat-errors"` and `verified_by = "supervisor"` is serialized and deserialized
- **THEN** the resulting value equals the original
- **AND** the JSON contains `"type": "agent.verified"`

#### Scenario: Verified message with optional message

- **WHEN** a `BrokerMessage::Verified` with `message = Some("all 12 tests pass")` is serialized
- **THEN** the JSON contains the message field

#### Scenario: Verified message without message

- **WHEN** a `BrokerMessage::Verified` with `message = None` is serialized and deserialized
- **THEN** the round-trip preserves the value

### Requirement: Feedback message variant

The `BrokerMessage` enum SHALL include a `Feedback` variant with serde tag `"agent.feedback"`. The variant SHALL carry `agent_id: String` and `payload: FeedbackPayload`.

`FeedbackPayload` SHALL contain:
- `from: String` — the agent_id of the sender (typically `"supervisor"`)
- `errors: Vec<String>` — list of error messages the agent should address

#### Scenario: Feedback message round-trips through serde

- **WHEN** a `BrokerMessage::Feedback` with `agent_id = "feat-errors"`, `from = "supervisor"`, and `errors = ["test failed", "missing doc comment"]` is serialized and deserialized
- **THEN** the resulting value equals the original
- **AND** the JSON contains `"type": "agent.feedback"`

#### Scenario: Feedback with empty errors list is valid

- **WHEN** a `BrokerMessage::Feedback` with `errors = []` is serialized
- **THEN** the JSON contains `"errors": []`

### Requirement: Validation for new variants

The system SHALL validate new variants via `from_json`:

- `Verified`: `verified_by` MUST NOT be empty
- `Feedback`: `from` MUST NOT be empty, `errors` MUST NOT be empty

#### Scenario: Verified with empty verified_by is rejected

- **WHEN** a JSON message of type `agent.verified` with `verified_by = ""` is parsed via `from_json`
- **THEN** validation fails with an error

#### Scenario: Feedback with empty from is rejected

- **WHEN** a JSON message of type `agent.feedback` with `from = ""` is parsed via `from_json`
- **THEN** validation fails with an error

#### Scenario: Feedback with empty errors is rejected

- **WHEN** a JSON message of type `agent.feedback` with `errors = []` is parsed via `from_json`
- **THEN** validation fails with an error

### Requirement: Display for new variants

The `Display` impl SHALL format new variants as:

- Verified without message: `[{agent_id}] verified by {verified_by}`
- Verified with message: `[{agent_id}] verified by {verified_by} — {message}`
- Feedback: `[{agent_id}] feedback from {from}: {N} errors`

#### Scenario: Verified Display without message

- **WHEN** a `Verified` message with `agent_id = "feat-errors"`, `verified_by = "supervisor"`, `message = None` is formatted
- **THEN** the result is `[feat-errors] verified by supervisor`

#### Scenario: Verified Display with message

- **WHEN** a `Verified` message with `message = Some("all tests pass")` is formatted
- **THEN** the result is `[feat-errors] verified by supervisor — all tests pass`

#### Scenario: Feedback Display

- **WHEN** a `Feedback` message with `agent_id = "feat-errors"`, `from = "supervisor"`, `errors` with 3 entries is formatted
- **THEN** the result is `[feat-errors] feedback from supervisor: 3 errors`

### Requirement: status_label for new variants

- `Verified` SHALL return `"verified"`
- `Feedback` SHALL return `"feedback"`

#### Scenario: status_label for Verified

- **WHEN** `status_label()` is called on a `Verified` message
- **THEN** the result is `"verified"`

#### Scenario: status_label for Feedback

- **WHEN** `status_label()` is called on a `Feedback` message
- **THEN** the result is `"feedback"`

### Requirement: agent_id for new variants

`agent_id()` SHALL return the `agent_id` field from both new variants.

#### Scenario: agent_id for Verified

- **WHEN** `agent_id()` is called on a `Verified` message with `agent_id = "feat-x"`
- **THEN** the result is `"feat-x"`

#### Scenario: agent_id for Feedback

- **WHEN** `agent_id()` is called on a `Feedback` message with `agent_id = "feat-x"`
- **THEN** the result is `"feat-x"`
