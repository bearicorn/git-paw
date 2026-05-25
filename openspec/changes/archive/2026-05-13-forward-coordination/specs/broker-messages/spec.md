## ADDED Requirements

### Requirement: Intent message variant

The `BrokerMessage` enum SHALL include an `Intent` variant with serde tag `"agent.intent"`. The variant SHALL carry `agent_id: String` (the publishing agent — same convention as `Status`, `Artifact`, `Blocked`) and `payload: IntentPayload`.

`IntentPayload` SHALL contain:
- `files: Vec<String>` — file paths the agent intends to modify, relative to the repository root. Globs are permitted but discouraged; the validator does not parse globs.
- `summary: String` — a one-line human-readable description of the planned change.
- `valid_for_seconds: u64` — relative TTL after which a downstream consumer (e.g. the supervisor) MAY treat the intent as stale.

#### Scenario: Intent message round-trips through serde

- **WHEN** a `BrokerMessage::Intent` with `agent_id = "feat-auth"` and a populated `IntentPayload` is serialized to JSON and then deserialized back
- **THEN** the resulting value equals the original
- **AND** the intermediate JSON contains `"type": "agent.intent"` and `"agent_id": "feat-auth"` at the top level
- **AND** the intermediate JSON contains the payload nested under a `"payload"` key

#### Scenario: Intent payload with multiple files

- **WHEN** an `IntentPayload { files: vec!["src/auth.rs", "src/auth/client.rs"], summary: "wire AuthClient", valid_for_seconds: 900 }` is serialized and deserialized
- **THEN** the round-trip preserves the value
- **AND** the JSON contains the `files` array with both entries in order

#### Scenario: Intent payload with a single file

- **WHEN** an `IntentPayload { files: vec!["README.md"], summary: "doc fix", valid_for_seconds: 300 }` is serialized and deserialized
- **THEN** the round-trip preserves the value

### Requirement: Validation for Intent variant

The system SHALL validate `Intent` messages via `from_json` (the existing validating constructor). The system SHALL reject input where:

- `payload.files` is an empty array
- Any entry in `payload.files` is empty or contains only whitespace
- `payload.summary` is empty or contains only whitespace
- `payload.valid_for_seconds` is `0`

`agent_id` validation follows the same rules as every other variant (slug character set, non-empty, no whitespace-only).

#### Scenario: Empty files array is rejected

- **WHEN** a JSON message of type `agent.intent` with `payload.files = []` is parsed via `from_json`
- **THEN** validation fails with an error identifying the empty `files` field

#### Scenario: Whitespace-only file path is rejected

- **WHEN** a JSON message of type `agent.intent` with `payload.files = ["   "]` is parsed via `from_json`
- **THEN** validation fails with an error identifying the empty file path

#### Scenario: Empty summary is rejected

- **WHEN** a JSON message of type `agent.intent` with `payload.summary = ""` is parsed via `from_json`
- **THEN** validation fails with an error identifying the empty `summary` field

#### Scenario: Zero valid_for_seconds is rejected

- **WHEN** a JSON message of type `agent.intent` with `payload.valid_for_seconds = 0` is parsed via `from_json`
- **THEN** validation fails with an error identifying `valid_for_seconds`

#### Scenario: Valid Intent message produces a BrokerMessage

- **WHEN** a well-formed JSON message of type `agent.intent` is parsed via `from_json`
- **THEN** a `BrokerMessage::Intent` value is produced
- **AND** all fields of the resulting value match the input

### Requirement: Display for Intent variant

The `Display` impl SHALL format the `Intent` variant as:

```
[{agent_id}] intent: {N} files for {valid_for_seconds}s — {summary}
```

The output SHALL be a single line of plain text containing no newline characters and no ANSI escape codes.

#### Scenario: Intent Display output

- **WHEN** an `Intent` message with `agent_id = "feat-auth"`, `files = ["src/a.rs", "src/b.rs", "src/c.rs"]`, `summary = "wire AuthClient"`, `valid_for_seconds = 900` is formatted via `Display`
- **THEN** the resulting string is `[feat-auth] intent: 3 files for 900s — wire AuthClient`
- **AND** the string contains no newline characters
- **AND** the string contains no ANSI escape sequences

#### Scenario: Intent Display with one file

- **WHEN** an `Intent` message with `files = ["README.md"]`, `summary = "doc fix"`, `valid_for_seconds = 300` is formatted via `Display`
- **THEN** the resulting string is `[feat-x] intent: 1 files for 300s — doc fix`

### Requirement: status_label for Intent variant

`Intent` SHALL return `"intent"` from `status_label()`.

#### Scenario: status_label for Intent

- **WHEN** `status_label()` is called on an `Intent` message
- **THEN** the result is `"intent"`

### Requirement: agent_id for Intent variant

`agent_id()` SHALL return the `agent_id` field from the `Intent` variant.

#### Scenario: agent_id for Intent

- **WHEN** `agent_id()` is called on an `Intent` message with `agent_id = "feat-auth"`
- **THEN** the result is `"feat-auth"`
