# broker-messages Specification

## Purpose
Defines the `BrokerMessage` wire protocol: a single JSON-tagged enum with seven variants (Status, Artifact, Blocked, Verified, Feedback, Question, Intent), their payload shapes, validating construction, `Display` formatting, and `status_label`/`agent_id` accessors. It also specifies the `slugify_branch` branch-to-`agent_id` conversion, the `build_status_message` helper, and the broker `/publish` agent_id and placeholder validation every message is checked against.
## Requirements
### Requirement: Broker message envelope

The system SHALL define a single `BrokerMessage` type that represents every message exchanged between agents and the git-paw broker. The type SHALL be a Rust enum with seven variants — `Status`, `Artifact`, `Blocked`, `Verified`, `Feedback`, `Question`, and `Intent` — each carrying an `agent_id: String` and a strongly-typed payload struct.

The wire format SHALL be JSON with an internally tagged discriminator field named `type`, taking the values `agent.status`, `agent.artifact`, `agent.blocked`, `agent.verified`, `agent.feedback`, `agent.question`, or `agent.intent`. Every message SHALL include `agent_id` and `payload` fields at the top level alongside `type`.

#### Scenario: Status message round-trips through serde

- **WHEN** a `BrokerMessage::Status` with `agent_id = "feat-x"` and a populated `StatusPayload` is serialized to JSON and then deserialized back
- **THEN** the resulting value equals the original
- **AND** the intermediate JSON contains `"type": "agent.status"` and `"agent_id": "feat-x"` at the top level
- **AND** the intermediate JSON contains the payload nested under a `"payload"` key

#### Scenario: Artifact message round-trips through serde

- **WHEN** a `BrokerMessage::Artifact` with `agent_id = "feat-errors"` and a populated `ArtifactPayload` is serialized to JSON and then deserialized back
- **THEN** the resulting value equals the original
- **AND** the intermediate JSON contains `"type": "agent.artifact"`

#### Scenario: Blocked message round-trips through serde

- **WHEN** a `BrokerMessage::Blocked` with `agent_id = "feat-config"` and a populated `BlockedPayload` is serialized to JSON and then deserialized back
- **THEN** the resulting value equals the original
- **AND** the intermediate JSON contains `"type": "agent.blocked"`

#### Scenario: Unknown message type is rejected

- **WHEN** a JSON object with `"type": "agent.unknown"` is parsed as a `BrokerMessage`
- **THEN** parsing fails with a deserialization error
- **AND** no `BrokerMessage` value is produced

#### Scenario: Envelope enumerates all seven wire-format type values

- **WHEN** the requirement's wire-format enumeration is read
- **THEN** it lists every accepted `type` discriminator value: `agent.status`, `agent.artifact`, `agent.blocked`, `agent.verified`, `agent.feedback`, `agent.question`, and `agent.intent`
- **AND** the list matches the seven `#[serde(rename = "...")]` attributes on the `BrokerMessage` enum variants in `src/broker/messages.rs`

### Requirement: Status payload shape

The `StatusPayload` struct SHALL contain:

- `status: String` — a free-form short label such as `"working"`, `"idle"`, or `"committed"`
- `modified_files: Vec<String>` — zero or more file paths the agent has modified since its last status report
- `message: Option<String>` — an optional free-form human-readable note
- `cli: Option<String>` — an optional CLI name (e.g. `"claude"`) identifying the CLI running in the publishing agent's pane. The field SHALL be annotated with `#[serde(default, skip_serializing_if = "Option::is_none")]` so that older JSON payloads that omit it deserialise as `None`, and newer payloads with `cli: None` omit the field from the serialised bytes. Publishers SHALL set this field when they know which CLI they are running under (the supervisor pane resolves it from `[supervisor].cli` configuration); coding-agent panes MAY omit it and rely on the broker's watch-target map.
- `phase: Option<String>` — an optional free-form phase label identifying the publishing agent's current lifecycle phase (e.g. `"baseline"`, `"watching"`, `"approving"`, `"answering"`, `"merging"`, `"summary"` for the supervisor). The field SHALL be annotated with `#[serde(default, skip_serializing_if = "Option::is_none")]`. When `phase` is `Some(_)`, downstream consumers (notably the dashboard) SHALL prefer the phase label over the message-type-derived `status_label()` when rendering the agent's row.

#### Scenario: Status payload with all fields populated

- **WHEN** a `StatusPayload { status: "working", modified_files: ["src/a.rs", "src/b.rs"], message: Some("refactoring"), cli: Some("claude"), phase: Some("watching") }` is serialized
- **THEN** the resulting JSON contains all five fields with the expected values
- **AND** deserializing the same JSON produces an equal struct

#### Scenario: Status payload with empty modified_files and no message

- **WHEN** a `StatusPayload { status: "idle", modified_files: vec![], message: None, cli: None, phase: None }` is serialized and deserialized
- **THEN** the round-trip preserves the value
- **AND** the `message` field is absent from the JSON or serialized as `null`
- **AND** the `cli` field is absent from the JSON (skip-serializing-if-none)
- **AND** the `phase` field is absent from the JSON (skip-serializing-if-none)

#### Scenario: Status payload backward compatibility on the wire (missing cli and phase)

- **GIVEN** legacy JSON `{"status": "working", "modified_files": [], "message": "Supervisor booting"}` produced by a v0.4 or earlier binary
- **WHEN** the JSON is deserialized as `StatusPayload`
- **THEN** the resulting struct has `cli = None` and `phase = None`
- **AND** the round-trip back to JSON omits both fields

#### Scenario: Status payload with only cli populated

- **WHEN** a `StatusPayload { status: "working", modified_files: vec![], message: None, cli: Some("claude"), phase: None }` is serialized
- **THEN** the resulting JSON contains `"cli": "claude"` but not a `phase` key
- **AND** deserializing the JSON produces an equal struct

#### Scenario: Status payload with only phase populated

- **WHEN** a `StatusPayload { status: "feedback", modified_files: vec![], message: None, cli: None, phase: Some("merging") }` is serialized
- **THEN** the resulting JSON contains `"phase": "merging"` but not a `cli` key
- **AND** deserializing the JSON produces an equal struct

### Requirement: Artifact payload shape

The `ArtifactPayload` struct SHALL contain:

- `status: String` — a label such as `"done"` or `"verified"`
- `exports: Vec<String>` — zero or more public symbol names the agent's work exposes (types, functions, constants)
- `modified_files: Vec<String>` — zero or more file paths the agent created or modified

#### Scenario: Artifact payload with exports

- **WHEN** an `ArtifactPayload { status: "done", exports: vec!["PawError", "NotAGitRepo"], modified_files: vec!["src/error.rs"] }` is serialized and deserialized
- **THEN** the round-trip preserves the value

#### Scenario: Artifact payload with no exports

- **WHEN** an `ArtifactPayload { status: "done", exports: vec![], modified_files: vec!["docs/foo.md"] }` is serialized and deserialized
- **THEN** the round-trip preserves the value
- **AND** the `exports` field is present as an empty JSON array

### Requirement: Blocked payload shape

The `BlockedPayload` struct SHALL contain:

- `needs: String` — a free-form description of what the agent is blocked on
- `from: String` — the `agent_id` of the peer expected to unblock the requester

#### Scenario: Blocked payload round-trip

- **WHEN** a `BlockedPayload { needs: "PawError type", from: "feat-errors" }` is serialized and deserialized
- **THEN** the round-trip preserves the value

### Requirement: Message validation

Construction of a `BrokerMessage` from untrusted input (e.g. an HTTP request body) SHALL go through a validating constructor. The system SHALL reject input where:

- `agent_id` is empty or contains only whitespace
- `agent_id` contains characters outside the slug character set `[a-z0-9-_]`
- For `Status`: `status` is empty
- For `Artifact`: `status` is empty
- For `Blocked`: `needs` is empty OR `from` is empty

Once a `BrokerMessage` value exists, it SHALL be valid by construction. Holders of a `BrokerMessage` MUST NOT need to revalidate it.

#### Scenario: Empty agent_id is rejected

- **WHEN** a JSON message with `"agent_id": ""` is parsed via the validating constructor
- **THEN** validation fails with an error identifying `agent_id` as the cause
- **AND** no `BrokerMessage` value is produced

#### Scenario: Whitespace-only agent_id is rejected

- **WHEN** a JSON message with `"agent_id": "   "` is parsed via the validating constructor
- **THEN** validation fails with an error identifying `agent_id` as the cause

#### Scenario: agent_id with invalid characters is rejected

- **WHEN** a JSON message with `"agent_id": "feat/x"` is parsed via the validating constructor
- **THEN** validation fails with an error identifying `agent_id` as the cause
- **AND** the error message indicates the slug character set is required

#### Scenario: Empty status field in agent.status is rejected

- **WHEN** a JSON message of type `agent.status` with `payload.status = ""` is parsed via the validating constructor
- **THEN** validation fails with an error identifying the empty `status` field

#### Scenario: Empty needs field in agent.blocked is rejected

- **WHEN** a JSON message of type `agent.blocked` with `payload.needs = ""` is parsed via the validating constructor
- **THEN** validation fails with an error identifying the empty `needs` field

#### Scenario: Empty from field in agent.blocked is rejected

- **WHEN** a JSON message of type `agent.blocked` with `payload.from = ""` is parsed via the validating constructor
- **THEN** validation fails with an error identifying the empty `from` field

#### Scenario: Valid message produces a BrokerMessage

- **WHEN** a well-formed JSON message of any of the three types is parsed via the validating constructor
- **THEN** a `BrokerMessage` value is produced
- **AND** all fields of the resulting value match the input

### Requirement: Message display formatting

The `BrokerMessage` type SHALL implement `std::fmt::Display`. The output SHALL be a single line of plain text containing no ANSI escape codes, suitable for embedding in dashboard rows and session log files.

The format SHALL include the agent identifier in brackets, the message type as a short label, and a one-line summary of the payload.

#### Scenario: Status message Display output

- **WHEN** a `BrokerMessage::Status` with `agent_id = "feat-http-broker"`, status `"working"`, and two modified files is formatted via `Display`
- **THEN** the resulting string is `[feat-http-broker] status: working (2 files modified)`
- **AND** the string contains no newline characters
- **AND** the string contains no ANSI escape sequences

#### Scenario: Artifact message Display output

- **WHEN** a `BrokerMessage::Artifact` with `agent_id = "feat-errors"`, status `"done"`, and exports `["PawError", "NotAGitRepo"]` is formatted via `Display`
- **THEN** the resulting string is `[feat-errors] artifact: done — exports: PawError, NotAGitRepo`
- **AND** the string contains no newline characters

#### Scenario: Blocked message Display output

- **WHEN** a `BrokerMessage::Blocked` with `agent_id = "feat-config"`, needs `"PawError"`, and from `"feat-errors"` is formatted via `Display`
- **THEN** the resulting string is `[feat-config] blocked: needs PawError from feat-errors`
- **AND** the string contains no newline characters

### Requirement: Branch slug function

The system SHALL provide a free function with the signature `pub fn slugify_branch(branch: &str) -> String` that converts a git branch name into a stable broker `agent_id`.

The function SHALL be total and infallible. The function SHALL apply the following rules in order:

1. Convert ASCII uppercase letters to lowercase
2. Replace every character not in `[a-z0-9_]` with `-`
3. Collapse runs of consecutive `-` characters to a single `-`
4. Trim leading and trailing `-` characters
5. If the resulting string is empty, return the literal string `"agent"`

The output SHALL contain only characters from the set `[a-z0-9-_]`. The function SHALL be deterministic — calling it twice with the same input always produces the same output.

#### Scenario: Simple feature branch is slugified

- **WHEN** `slugify_branch("feat/http-broker")` is called
- **THEN** the result is `"feat-http-broker"`

#### Scenario: Uppercase letters are lowercased

- **WHEN** `slugify_branch("Feat/HTTP_Broker")` is called
- **THEN** the result is `"feat-http_broker"`

#### Scenario: Nested branch path is slugified

- **WHEN** `slugify_branch("users/jane/feat/x")` is called
- **THEN** the result is `"users-jane-feat-x"`

#### Scenario: Underscores are preserved

- **WHEN** `slugify_branch("feat/my_feature")` is called
- **THEN** the result is `"feat-my_feature"`

#### Scenario: Runs of separators are collapsed

- **WHEN** `slugify_branch("feat//x")` is called
- **THEN** the result is `"feat-x"`

#### Scenario: Leading and trailing separators are trimmed

- **WHEN** `slugify_branch("/feat/x/")` is called
- **THEN** the result is `"feat-x"`

#### Scenario: Non-ASCII characters are replaced

- **WHEN** `slugify_branch("feat/日本語")` is called
- **THEN** the result is `"feat"`
- **AND** the result contains only ASCII characters from the slug set

#### Scenario: Empty input falls back to default

- **WHEN** `slugify_branch("")` is called
- **THEN** the result is `"agent"`

#### Scenario: All-separator input falls back to default

- **WHEN** `slugify_branch("///")` is called
- **THEN** the result is `"agent"`

#### Scenario: Slug function is deterministic

- **WHEN** `slugify_branch("feat/http-broker")` is called twice
- **THEN** both calls return the same string

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

### Requirement: build_status_message accepts an optional cli parameter

The free function `build_status_message` in `src/broker/publish.rs` SHALL accept an optional CLI name parameter and populate the `cli` field of the constructed `StatusPayload` accordingly. The signature SHALL be:

```rust
pub fn build_status_message(
    agent_id: &str,
    status: &str,
    message: Option<String>,
    cli: Option<&str>,
) -> BrokerMessage
```

When `cli` is `Some(name)`, the resulting `BrokerMessage::Status`'s payload SHALL have `cli = Some(name.to_string())`. When `cli` is `None`, the payload's `cli` field SHALL be `None`.

The function SHALL NOT populate the `phase` field — publishers that want to publish phase information SHALL construct the `BrokerMessage::Status` directly with a fully-populated `StatusPayload`. `build_status_message` is intended for status pings (boot announcements, supervisor heartbeats) where only `status`, `message`, and optionally `cli` need to be set; richer publications go through direct construction.

#### Scenario: build_status_message with explicit cli produces a payload with cli populated

- **WHEN** `build_status_message("supervisor", "working", Some("Supervisor booting".to_string()), Some("claude"))` is called
- **THEN** the returned `BrokerMessage::Status` has `payload.cli = Some("claude")`
- **AND** `payload.status = "working"`, `payload.message = Some("Supervisor booting")`
- **AND** `payload.phase = None`

#### Scenario: build_status_message with None cli omits the cli field

- **WHEN** `build_status_message("feat-x", "working", None, None)` is called
- **THEN** the returned `BrokerMessage::Status` has `payload.cli = None`
- **AND** `payload.phase = None`
- **AND** serializing the message produces JSON without a `cli` key in the payload

### Requirement: Broker `/publish` enforces agent_id validation in code

The `src/broker/server.rs::publish` HTTP handler SHALL execute the validation already specified in `openspec/specs/broker-messages/spec.md` under "Broker rejects invalid agent_id strings" and "Broker rejects payload fields matching placeholder syntax" (propagated from the archived `supervisor-as-pane-followups` change). Today those spec requirements describe behaviour the binary does NOT implement; this change closes the gap.

Specifically, the handler SHALL:

1. Reject the request with HTTP 400 when the deserialized `BrokerMessage`'s top-level `agent_id` does NOT match the regular expression `^(supervisor|feat/[a-z0-9][a-z0-9-]+|feat-[a-z0-9][a-z0-9-]+)$`.
2. Reject the request with HTTP 400 when any of `payload.question`, `payload.message`, `payload.needs`, or any string element of `payload.errors[]` matches `^<.*>$` exactly.

The error body shape and error message text are as defined in the existing main-spec scenarios — no new wording.

A single compiled `OnceLock<Regex>` per pattern is acceptable; the broker's hot path SHALL NOT rebuild the regex per request.

#### Scenario: Single-letter agent_id is rejected by the running broker

- **GIVEN** a running broker on port `<P>` with the validation implemented
- **WHEN** a client POSTs `{"type":"agent.status","agent_id":"a","payload":{"status":"working","modified_files":[],"message":null}}` to `http://127.0.0.1:<P>/publish`
- **THEN** the HTTP response status SHALL be 400
- **AND** the response body SHALL be a JSON object containing the substring `"invalid agent_id"`
- **AND** a subsequent `GET /status` SHALL NOT contain an entry with `agent_id = "a"`

#### Scenario: Placeholder-shaped agent_id is rejected by the running broker

- **GIVEN** a running broker
- **WHEN** a client POSTs `{"type":"agent.question","agent_id":"<agent-id>","payload":{"question":"placeholder text"}}`
- **THEN** the HTTP response status SHALL be 400
- **AND** the response body SHALL contain the substring `"invalid agent_id"`

#### Scenario: Placeholder-shaped payload.question is rejected by the running broker

- **GIVEN** a running broker
- **WHEN** a client POSTs `{"type":"agent.question","agent_id":"feat-x","payload":{"question":"<your specific question>"}}`
- **THEN** the HTTP response status SHALL be 400
- **AND** the response body SHALL contain the substring `"unfilled placeholder"` and the substring `"question"`

#### Scenario: Valid supervisor and feat-* publishers succeed

- **GIVEN** a running broker
- **WHEN** a client POSTs a well-formed `agent.status` message with `agent_id = "supervisor"`
- **THEN** the HTTP response status SHALL be 200 or 204
- **AND** the message SHALL be appended to the supervisor's inbox

The same SHALL hold for `agent_id = "feat-test-branch"` and `agent_id = "feat/test-branch"`.

#### Scenario: Real human content passes through

- **GIVEN** a running broker
- **WHEN** a client POSTs `{"type":"agent.question","agent_id":"feat-x","payload":{"question":"Should we use bcrypt or argon2?"}}`
- **THEN** the HTTP response status SHALL be 200 or 204

#### Scenario: Existing test fixtures using non-conforming agent_ids are updated

- **WHEN** the test suite is run after this change lands
- **THEN** every `/publish` test caller in `tests/broker_integration.rs`, `tests/conflict_detection_integration.rs`, `tests/learnings_mode_integration.rs`, `tests/e2e_*.rs`, and any other broker-touching test file SHALL use an `agent_id` matching the regex (e.g. `feat-x`, `feat-test`, `supervisor`) and SHALL NOT use ad-hoc identifiers like `"test"`, `"agent1"`, or single letters
- **AND** the broker-side validation SHALL be active for all those tests (no opt-out)

### Requirement: agent.answer message type

The broker SHALL accept an `agent.answer` message variant carrying a non-error supervisor→agent reply. The envelope's `agent_id` SHALL name the TARGET agent (the one being answered), and the payload SHALL contain:

- `from: String` — the sender (typically `"supervisor"`); required non-empty
- `answer: String` — the reply text; required non-empty
- `re: Option<String>` — an optional short reference to the question being answered; omitted from serialization when absent

Validation SHALL reject an empty `from` or an empty `answer` with a named error, mirroring `agent.feedback`'s field validation. The variant SHALL serialize with `type = "agent.answer"`.

#### Scenario: Valid answer round-trips through serde

- **GIVEN** the JSON `{"type":"agent.answer","agent_id":"feat-x","payload":{"from":"supervisor","answer":"Use the existing helper; do not add a dependency","re":"add crate X?"}}`
- **WHEN** it is parsed and re-serialized
- **THEN** parsing SHALL succeed and the round-trip SHALL preserve all fields

#### Scenario: Empty answer is rejected

- **GIVEN** an `agent.answer` payload with `answer = ""`
- **WHEN** the message is validated
- **THEN** validation SHALL fail with an error naming the empty answer field

#### Scenario: Empty from is rejected

- **GIVEN** an `agent.answer` payload with `from = ""`
- **WHEN** the message is validated
- **THEN** validation SHALL fail with an error naming the empty from field

#### Scenario: re is optional

- **GIVEN** an `agent.answer` payload with no `re` field
- **WHEN** the message is validated and serialized
- **THEN** validation SHALL pass and the serialized JSON SHALL omit `re`

#### Scenario: Answer is not an error channel

- **WHEN** the coordination skill's message documentation is rendered
- **THEN** it SHALL describe `agent.answer` as an authoritative supervisor reply to act on — distinct from `agent.feedback`, which carries corrective errors

