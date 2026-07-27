# broker-protocol Specification

## Purpose
The complete broker wire protocol together with its in-memory routing and state layer.

The wire format defines the `BrokerMessage` type: a single JSON-tagged enum with seven variants (Status, Artifact, Blocked, Verified, Feedback, Question, Intent) plus the `agent.answer` reply, their payload shapes, validating construction, `Display` formatting, and `status_label`/`agent_id` accessors. It also specifies the `slugify_branch` branch-to-`agent_id` conversion, the `build_status_message` helper, and the broker `/publish` agent_id and placeholder validation every message is checked against.

The delivery layer routes each variant by its rule: publishing a status-publishing variant (Status, Artifact, Blocked, Intent) updates the sender's agent record and lazily registers its inbox, while identity and routing variants (Verified, Feedback, Answer, Question) are routed without minting a roster row. Status is logged-only; artifact/verified/intent broadcast to peers; blocked/feedback/answer are targeted; question is routed to the supervisor inbox. The layer assigns globally-monotonic sequence numbers, serves non-destructive cursor-based polling and lock-scoped status snapshots, and flushes an append-only message log to disk best-effort.

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

Construction of a `BrokerMessage` from untrusted input (e.g. an HTTP request body) SHALL go through a validating constructor. The constructor SHALL reject input where:

- `agent_id` is empty or contains only whitespace
- For `Status`: `status` is empty
- For `Artifact`: `status` is empty
- For `Blocked`: `needs` is empty OR `from` is empty

The constructor deliberately does NOT enforce an `agent_id` character set or shape: that is the HTTP boundary's responsibility (see "Broker `/publish` enforces agent_id validation in code"). The constructor's job is only to guarantee that no `BrokerMessage` value holds an empty or whitespace-only required field, so non-HTTP callers still trip a clear error on garbage input before the typed value flows further. A slug such as `feat/x` is therefore VALID at the constructor.

Once a `BrokerMessage` value exists, it SHALL be valid by construction. Holders of a `BrokerMessage` MUST NOT need to revalidate it.

#### Scenario: Empty agent_id is rejected

- **WHEN** a JSON message with `"agent_id": ""` is parsed via the validating constructor
- **THEN** validation fails with an error identifying `agent_id` as the cause
- **AND** no `BrokerMessage` value is produced

#### Scenario: Whitespace-only agent_id is rejected

- **WHEN** a JSON message with `"agent_id": "   "` is parsed via the validating constructor
- **THEN** validation fails with an error identifying `agent_id` as the cause

#### Scenario: Slash-containing agent_id passes the constructor

- **WHEN** a JSON message with `"agent_id": "feat/x"` is parsed via the validating constructor
- **THEN** validation succeeds and a `BrokerMessage` value is produced
- **AND** any shape restriction is left to the HTTP `/publish` boundary, not the constructor

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

The `src/broker/server.rs::publish` HTTP handler SHALL reject malformed publishers at the HTTP boundary, before the message reaches any inbox. It enforces the `agent_id` shape and the placeholder-syntax guard described in "Broker rejects invalid agent_id strings" and "Broker rejects payload fields matching placeholder syntax".

Specifically, the handler SHALL:

1. Reject the request with HTTP 400 when the deserialized `BrokerMessage`'s top-level `agent_id` does NOT match the regular expression `^(supervisor|[a-z0-9][a-z0-9-]*[/-][a-z0-9][a-z0-9-]*)$`. This accepts `supervisor` and any `{prefix}{/ or -}{name}` slug (lowercase alphanumeric plus hyphens, e.g. `feat/add-auth`, `feat-add-auth`, `fix/db-timeout`, `spec/add-thing`); it rejects a bare word with no separator, a single letter, and empty input. The prefix is NOT restricted to `feat` — `branch_prefix` is user-configurable (default `spec/`) and the shipped config advertises non-`feat` branches.
2. Reject the request with HTTP 400 when any of `payload.question`, `payload.message`, `payload.needs`, or any string element of `payload.errors[]` matches `^<.*>$` exactly.

The 400 response body SHALL be a JSON object whose `error` is the substring `invalid agent_id` (for shape violations) or `unfilled placeholder` (for placeholder violations); the human-readable `detail` MAY name the accepted shapes. No inbox state is mutated for a rejected request.

A single compiled `OnceLock<Regex>` per pattern is acceptable; the broker's hot path SHALL NOT rebuild the regex per request.

#### Scenario: Single-letter agent_id is rejected by the running broker

- **GIVEN** a running broker on port `<P>` with the validation implemented
- **WHEN** a client POSTs `{"type":"agent.status","agent_id":"a","payload":{"status":"working","modified_files":[],"message":null}}` to `http://127.0.0.1:<P>/publish`
- **THEN** the HTTP response status SHALL be 400
- **AND** the response body SHALL be a JSON object containing the substring `"invalid agent_id"`
- **AND** a subsequent `GET /status` SHALL NOT contain an entry with `agent_id = "a"`

#### Scenario: Prefixless bare-word agent_id is rejected by the running broker

- **GIVEN** a running broker
- **WHEN** a client POSTs a well-formed `agent.status` message with `agent_id = "foo"` (a bare word with no `/` or `-` separator)
- **THEN** the HTTP response status SHALL be 400
- **AND** the response body SHALL contain the substring `"invalid agent_id"`

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
- **AND** the same SHALL hold for `agent_id = "feat-test-branch"` and `agent_id = "feat/test-branch"`

#### Scenario: Slash-prefixed non-feat agent_id is accepted

- **GIVEN** a running broker
- **WHEN** a client POSTs a well-formed `agent.status` message with `agent_id = "fix/olx-auth-error-mapping"`
- **THEN** the HTTP response status SHALL be 200 or 204

#### Scenario: Dash-prefixed non-feat agent_id is accepted

- **GIVEN** a running broker
- **WHEN** a client POSTs a well-formed `agent.status` message with `agent_id = "fix-db-timeout"`
- **THEN** the HTTP response status SHALL be 200 or 204

#### Scenario: Configured branch_prefix agent_id is accepted

- **GIVEN** a running broker and a project whose configured `branch_prefix` is `spec/`
- **WHEN** a client POSTs a well-formed `agent.status` message with `agent_id = "spec/add-thing"`
- **THEN** the HTTP response status SHALL be 200 or 204

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

### Requirement: Publish updates sender's agent record

When `publish_message` is called with a **status-publishing** message variant — `Status`, `Artifact`, `Blocked`, or `Intent` — the system SHALL update the sender's `AgentRecord` in `BrokerStateInner`:

- Set `last_seen` to the current instant
- Set `status` to the message's status label (e.g. `"working"`, `"done"`, `"blocked"`)
- Set `last_message` to a clone of the published message
- If no `AgentRecord` exists for the sender's `agent_id`, one SHALL be created automatically (lazy registration)
- If no inbox queue exists for the sender's `agent_id`, one SHALL be created automatically

Identity and routing variants (`Verified`, `Feedback`, `Answer`, `Question`) SHALL NOT create or mutate any `AgentRecord` — see "Roster updates exclude identity and routing variants". This scoping is the W15-16 phantom-row fix: a verifier, feedback sender, answer sender, or questioner is not publishing its own status, so it must not mint a roster row or bump one's `last_seen`.

#### Scenario: First publish from an agent creates its record

- **GIVEN** a `BrokerState` with no known agents
- **WHEN** `publish_message` is called with an `agent.status` message from `agent_id = "feat-errors"`
- **THEN** `BrokerStateInner.agents` contains a record for `"feat-errors"`
- **AND** the record's `status` is `"working"`
- **AND** the record's `last_seen` is approximately `Instant::now()`

#### Scenario: Subsequent publish updates an existing record

- **GIVEN** a `BrokerState` with an existing record for `"feat-errors"` with status `"working"`
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"` with status `"done"`
- **THEN** the record's `status` is updated to `"done"`
- **AND** `last_seen` is updated

#### Scenario: Publish creates an inbox for the sender

- **GIVEN** a `BrokerState` with no known agents
- **WHEN** `publish_message` is called from `"feat-errors"`
- **THEN** `BrokerStateInner.queues` contains an inbox entry for `"feat-errors"`

### Requirement: Roster updates exclude identity and routing variants

The system SHALL NOT create or mutate any `AgentRecord` when `agent.verified`, `agent.feedback`, `agent.answer`, or `agent.question` is published. These variants are routed and logged (per their delivery requirements) but are not status publications, so they SHALL NOT mint a phantom roster row for a sender that has not otherwise published its status, nor bump an existing row's `last_seen`, `status`, or `last_message`. Only the status-publishing variants (`Status`, `Artifact`, `Blocked`, `Intent`) update the roster (see "Publish updates sender's agent record" and "Agent record updated for Intent variant").

#### Scenario: Verified does not mutate the verifier's roster record

- **WHEN** `publish_message` is called with an `agent.verified` from `"supervisor"`
- **THEN** no `AgentRecord` is created for `"supervisor"` by this publish
- **AND** any pre-existing `"supervisor"` record's `last_seen`, `status`, and `last_message` are left unchanged

Test: `broker::delivery::tests::verified_does_not_mutate_verifier_record`

#### Scenario: Feedback does not mutate the sender's roster record

- **WHEN** `publish_message` is called with an `agent.feedback` from a sender
- **THEN** the sender's `AgentRecord` is neither created nor mutated by this publish

Test: `broker::delivery::tests::feedback_does_not_mutate_sender_record`

#### Scenario: Question does not create a sender roster row

- **GIVEN** no `AgentRecord` exists for `"feat-x"`
- **WHEN** `publish_message` is called with an `agent.question` from `"feat-x"`
- **THEN** no roster row is created for `"feat-x"` (the question is still routed to the supervisor inbox)

Test: `broker::delivery::tests::question_does_not_create_sender_roster_row`

#### Scenario: Question leaves an existing sender row unchanged

- **GIVEN** an existing `AgentRecord` for `"feat-x"`
- **WHEN** `publish_message` is called with an `agent.question` from `"feat-x"`
- **THEN** that row's `last_seen`, `status`, and `last_message` are unchanged

Test: `broker::delivery::tests::question_leaves_existing_sender_row_unchanged`

### Requirement: Status messages are not routed

When a `BrokerMessage::Status` is published, the system SHALL update the sender's agent record but SHALL NOT enqueue the message in any agent's inbox. Status messages are informational — the dashboard reads them via `agent_status_snapshot`.

#### Scenario: Status message does not appear in any inbox

- **GIVEN** agents `"feat-errors"` and `"feat-detect"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.status` message from `"feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` returns no new messages
- **AND** `poll_messages` for `"feat-errors"` returns no new messages

### Requirement: Artifact messages are broadcast to all other agents

When a `BrokerMessage::Artifact` is published, the system SHALL enqueue the message in every known agent's inbox EXCEPT the sender's own inbox. Agents whose inboxes do not yet exist (not yet registered via a publish) SHALL NOT receive the broadcast.

#### Scenario: Artifact broadcast reaches all peers

- **GIVEN** three agents `"feat-errors"`, `"feat-detect"`, and `"feat-config"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` returns the artifact message
- **AND** `poll_messages` for `"feat-config"` returns the artifact message

#### Scenario: Artifact broadcast skips the sender

- **GIVEN** agents `"feat-errors"` and `"feat-detect"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"`
- **THEN** `poll_messages` for `"feat-errors"` returns no new messages

#### Scenario: Artifact broadcast skips agents not yet registered

- **GIVEN** agent `"feat-errors"` has an existing inbox but `"feat-detect"` has never published
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"`
- **THEN** no inbox exists for `"feat-detect"`
- **AND** no error occurs

### Requirement: Blocked messages are delivered to the target agent

When a `BrokerMessage::Blocked` is published, the system SHALL enqueue the message in the inbox of the agent identified by `payload.from` (the agent that can unblock the sender). If the target agent's inbox does not exist, the message SHALL be silently dropped (the target has not yet registered).

#### Scenario: Blocked message reaches the target agent

- **GIVEN** agents `"feat-config"` and `"feat-errors"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.blocked` message from `"feat-config"` with `payload.from = "feat-errors"`
- **THEN** `poll_messages` for `"feat-errors"` returns the blocked message

#### Scenario: Blocked message does not reach other agents

- **GIVEN** agents `"feat-config"`, `"feat-errors"`, and `"feat-detect"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.blocked` message from `"feat-config"` with `payload.from = "feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` returns no new messages

#### Scenario: Blocked message to unregistered target is silently dropped

- **GIVEN** agent `"feat-config"` has an existing inbox but `"feat-errors"` has never published
- **WHEN** `publish_message` is called with an `agent.blocked` message from `"feat-config"` with `payload.from = "feat-errors"`
- **THEN** no error occurs
- **AND** no inbox is created for `"feat-errors"`

### Requirement: Cursor-based message polling

`poll_messages(state, agent_id, since)` SHALL return a tuple `(Vec<BrokerMessage>, u64)` containing:

- All messages in the agent's inbox with sequence numbers strictly greater than `since`
- A cursor equal to the **greater of `since` and the highest sequence number among the returned messages**. The cursor SHALL be monotonic: it SHALL NOT regress below `since`, and in particular an empty result (no messages newer than `since`) SHALL return `since` itself, not `0`. This lets a client advance with `cursor = last_seq` on every poll — including empty polls — without ever re-reading already-seen messages.

Polling SHALL be non-destructive — messages are retained in the inbox and can be re-read with a smaller `since` value. Each message SHALL have a globally unique, auto-incrementing `u64` sequence number assigned at publish time. Cursor advancement SHALL be independent of message type: no message variant (including `Question`) may wedge the cursor or prevent later messages from being delivered on subsequent polls.

#### Scenario: Poll returns all messages when since is 0

- **GIVEN** agent `"feat-x"` has 3 messages in its inbox with sequences 1, 2, 3
- **WHEN** `poll_messages(&state, "feat-x", 0)` is called
- **THEN** the result contains 3 messages
- **AND** `last_seq` is `3`

#### Scenario: Poll returns only newer messages

- **GIVEN** agent `"feat-x"` has messages with sequences 1, 2, 3, 4, 5
- **WHEN** `poll_messages(&state, "feat-x", 3)` is called
- **THEN** the result contains 2 messages (sequences 4 and 5)
- **AND** `last_seq` is `5`

#### Scenario: Poll with since equal to latest returns empty but holds the cursor

- **GIVEN** agent `"feat-x"` has messages up to sequence 5
- **WHEN** `poll_messages(&state, "feat-x", 5)` is called
- **THEN** the result contains 0 messages
- **AND** `last_seq` is `5` (the cursor holds at `since`; it does NOT regress to `0`)

#### Scenario: Repeated polls return the same messages

- **GIVEN** agent `"feat-x"` has messages with sequences 1, 2, 3
- **WHEN** `poll_messages(&state, "feat-x", 0)` is called twice
- **THEN** both calls return the same 3 messages with the same `last_seq`

#### Scenario: Poll for unknown agent returns empty

- **GIVEN** no agent `"feat-unknown"` has ever published
- **WHEN** `poll_messages(&state, "feat-unknown", 0)` is called
- **THEN** the result contains 0 messages
- **AND** `last_seq` is `0`
- **AND** no error occurs

#### Scenario: Poll uses a read lock only

- **WHEN** `poll_messages` is called
- **THEN** it acquires a read lock on `BrokerState` (not a write lock)

#### Scenario: A Question does not wedge later messages in a mixed inbox

- **GIVEN** the `"supervisor"` inbox receives, in order, an `agent.question` (sequence `q`) then an `agent.artifact` (sequence `a`, with `a > q`)
- **WHEN** a client polls with `since = 0`, advances to the returned `last_seq`, and polls again with that cursor
- **THEN** the first poll returns the question (and any messages up to its cursor) and reports `last_seq >= q`
- **AND** the second poll returns the artifact and reports `last_seq >= a`
- **AND** at no point does a poll re-return the question after the cursor has advanced past `q`

### Requirement: Agent status snapshot

`agent_status_snapshot(state)` SHALL return an `AgentStatusEntry` for every known agent. The function SHALL:

- Take a read lock on `BrokerState`
- Clone each agent's record into an `AgentStatusEntry`
- Release the lock before returning

The returned snapshot SHALL be an owned value that can be used for rendering or serialization without holding any lock.

#### Scenario: Snapshot contains all registered agents

- **GIVEN** three agents have published at least one message each
- **WHEN** `agent_status_snapshot(&state)` is called
- **THEN** the result contains exactly 3 `AgentStatusEntry` values

#### Scenario: Snapshot reflects latest status

- **GIVEN** agent `"feat-errors"` has published two messages: first `agent.status` with status `"working"`, then `agent.artifact` with status `"done"`
- **WHEN** `agent_status_snapshot(&state)` is called
- **THEN** the entry for `"feat-errors"` has `status = "done"`

#### Scenario: Snapshot is empty when no agents have published

- **GIVEN** a fresh `BrokerState` with no published messages
- **WHEN** `agent_status_snapshot(&state)` is called
- **THEN** the result is an empty `Vec`

#### Scenario: Snapshot uses a read lock

- **WHEN** `agent_status_snapshot` is called
- **THEN** it acquires a read lock on `BrokerState` (not a write lock)

### Requirement: Sequence number assignment

Each message stored in any agent's inbox SHALL be assigned a globally unique, auto-incrementing `u64` sequence number. The sequence SHALL start at `1` for the first message in a session and SHALL monotonically increase. The sequence counter SHALL be shared across all agents — sequence numbers are globally ordered, not per-agent.

#### Scenario: First message gets sequence 1

- **GIVEN** a fresh `BrokerState`
- **WHEN** an `agent.artifact` message is published and broadcast to one peer
- **THEN** the peer's inbox contains the message with sequence `1`

#### Scenario: Sequence numbers are globally monotonic

- **GIVEN** agents `"a"` and `"b"` both with existing inboxes
- **WHEN** agent `"a"` publishes an artifact (broadcast to `"b"`) and then agent `"b"` publishes an artifact (broadcast to `"a"`)
- **THEN** the message in `"b"`'s inbox has a lower sequence than the message in `"a"`'s inbox

### Requirement: Message log accumulation

Every message passed to `publish_message` SHALL be stored in an in-memory log within `BrokerStateInner` as a tuple of `(seq, timestamp, message)`. This log SHALL be append-only and SHALL never be truncated during a session. The log serves as the data source for the periodic background flush to disk.

#### Scenario: Published messages appear in the message log

- **GIVEN** a fresh `BrokerState`
- **WHEN** 3 messages are published
- **THEN** the in-memory message log contains exactly 3 entries
- **AND** each entry has a unique sequence number, a timestamp, and the original message

#### Scenario: Message log includes all message types

- **WHEN** one `agent.status`, one `agent.artifact`, and one `agent.blocked` message are published
- **THEN** the in-memory message log contains all three, regardless of routing (status messages are logged even though they are not routed to inboxes)

### Requirement: Periodic log flush to disk

The system SHALL spawn a `std::thread` (not a tokio task) that periodically flushes new message log entries to a plain text file. The flush thread SHALL:

- Run every ~5 seconds
- Take a read lock on `BrokerState`, read entries with `seq > last_flushed_seq`, release the lock
- Append formatted lines to the log file outside of any lock
- Use the `Display` impl of `BrokerMessage` for formatting each line as `[seq] timestamp [agent_id] message_display`
- Be best-effort — disk write failures SHALL NOT affect message delivery or crash the broker
- Perform one final flush when signaled to stop (on `BrokerHandle` drop)

If no log path is configured in `BrokerState` (e.g. during tests), the flush thread SHALL NOT be spawned.

#### Scenario: Flush thread writes new messages to disk

- **GIVEN** a `BrokerState` with a configured log path and 3 published messages
- **WHEN** the flush thread runs its periodic cycle
- **THEN** the log file contains 3 lines, one per message
- **AND** each line contains the sequence number and the `Display` output of the message

#### Scenario: Flush thread only writes new entries

- **GIVEN** a flush thread has already written messages 1-3 to the log file
- **WHEN** 2 more messages are published and the flush thread runs again
- **THEN** the log file now contains 5 lines total (original 3 + 2 new)

#### Scenario: Final flush on shutdown

- **GIVEN** messages have been published since the last periodic flush
- **WHEN** `BrokerHandle` is dropped
- **THEN** the flush thread performs one final flush before exiting
- **AND** all messages are present in the log file

#### Scenario: No flush thread without log path

- **GIVEN** a `BrokerState` with no configured log path
- **WHEN** `start_broker` is called
- **THEN** no flush thread is spawned
- **AND** message delivery works normally

#### Scenario: Disk write failure does not affect delivery

- **GIVEN** a `BrokerState` with a log path pointing to a read-only directory
- **WHEN** a message is published and the flush thread attempts to write
- **THEN** the write fails silently
- **AND** the message is still present in the in-memory log and routable via `poll_messages`

### Requirement: BrokerMessage helper methods

The system SHALL add two public methods to the `BrokerMessage` type in `src/broker/messages.rs`:

- `pub fn agent_id(&self) -> &str` — returns the `agent_id` field from whichever variant the message is
- `pub fn status_label(&self) -> &str` — returns a short label: `"working"` for `Status` (from `payload.status`), `"done"` for `Artifact` (from `payload.status`), `"blocked"` for `Blocked`

These methods SHALL be purely additive — no existing code in `messages.rs` is changed.

#### Scenario: agent_id returns the correct value for each variant

- **WHEN** `agent_id()` is called on a `Status` message with `agent_id = "feat-x"`
- **THEN** the result is `"feat-x"`

#### Scenario: status_label returns payload status for Status variant

- **WHEN** `status_label()` is called on a `Status` message with `payload.status = "working"`
- **THEN** the result is `"working"`

#### Scenario: status_label returns payload status for Artifact variant

- **WHEN** `status_label()` is called on an `Artifact` message with `payload.status = "done"`
- **THEN** the result is `"done"`

#### Scenario: status_label returns blocked for Blocked variant

- **WHEN** `status_label()` is called on a `Blocked` message
- **THEN** the result is `"blocked"`

### Requirement: Verified messages are broadcast to all agents

When a `BrokerMessage::Verified` is published, the system SHALL enqueue the message in every known agent's inbox EXCEPT the sender's own inbox. This follows the same broadcast pattern as `agent.artifact`.

#### Scenario: Verified broadcast reaches all peers

- **GIVEN** agents `"feat-errors"`, `"feat-detect"`, and `"supervisor"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.verified` message from `"supervisor"` for `agent_id = "feat-errors"`
- **THEN** `poll_messages` for `"feat-errors"` returns the verified message
- **AND** `poll_messages` for `"feat-detect"` returns the verified message

#### Scenario: Verified broadcast skips the sender

- **GIVEN** agents `"feat-errors"` and `"supervisor"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.verified` from `"supervisor"`
- **THEN** `poll_messages` for `"supervisor"` returns no new messages from this publish

### Requirement: Feedback messages are delivered to the target agent only

When a `BrokerMessage::Feedback` is published, the system SHALL enqueue the message in the inbox of the agent identified by `agent_id` (the agent receiving feedback). This follows the same targeted delivery pattern as `agent.blocked`.

#### Scenario: Feedback reaches the target agent

- **GIVEN** agents `"feat-errors"` and `"supervisor"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.feedback` message with `agent_id = "feat-errors"`
- **THEN** `poll_messages` for `"feat-errors"` returns the feedback message

#### Scenario: Feedback does not reach other agents

- **GIVEN** agents `"feat-errors"`, `"feat-detect"`, and `"supervisor"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.feedback` for `agent_id = "feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` returns no new messages

### Requirement: Intent messages are broadcast to all other agents

When a `BrokerMessage::Intent` is published, the system SHALL enqueue the message in every known agent's inbox EXCEPT the sender's own inbox. Agents whose inboxes do not yet exist (not yet registered via a publish) SHALL NOT receive the broadcast. This follows the same broadcast pattern as `agent.artifact` and `agent.verified`.

#### Scenario: Intent broadcast reaches all peers

- **GIVEN** three agents `"feat-auth"`, `"feat-detect"`, and `"supervisor"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.intent` message from `"feat-auth"`
- **THEN** `poll_messages` for `"feat-detect"` returns the intent message
- **AND** `poll_messages` for `"supervisor"` returns the intent message

#### Scenario: Intent broadcast skips the sender

- **GIVEN** agents `"feat-auth"` and `"feat-detect"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.intent` message from `"feat-auth"`
- **THEN** `poll_messages` for `"feat-auth"` returns no new messages from this publish

#### Scenario: Intent broadcast skips agents not yet registered

- **GIVEN** agent `"feat-auth"` has an existing inbox but `"feat-detect"` has never published
- **WHEN** `publish_message` is called with an `agent.intent` message from `"feat-auth"`
- **THEN** no inbox is created for `"feat-detect"`
- **AND** no error occurs

### Requirement: Agent record updated for Intent variant

When `agent.intent` is published, the sender's agent record SHALL be updated (last_seen, status, last_message) following the same pattern as existing message types. The `status` field on the agent record SHALL be set to the value returned by `status_label()` for the `Intent` variant (i.e. `"intent"`).

#### Scenario: Intent updates sender record last_seen

- **WHEN** `publish_message` is called with an `agent.intent` from `"feat-auth"`
- **THEN** the agent record for `"feat-auth"` has its `last_seen` updated

#### Scenario: Intent updates sender record status to "intent"

- **WHEN** `publish_message` is called with an `agent.intent` from `"feat-auth"`
- **THEN** the agent record for `"feat-auth"` has its `status` set to `"intent"`

### Requirement: Question messages are routed to the supervisor inbox

When a `BrokerMessage::Question` is published, the system SHALL enqueue the message in the inbox of the agent whose `agent_id` is exactly `"supervisor"`. If a `"supervisor"` inbox does not yet exist at delivery time, the system SHALL create it before enqueuing — `Question` is the only variant whose delivery creates a new inbox if missing.

The system SHALL NOT enqueue the message in the sender's inbox or in any other agent's inbox.

This routing differs from `Blocked` (which silently drops if the target inbox is missing) because the supervisor is a singleton recipient and may not have published any prior message at the time the first `Question` arrives.

#### Scenario: Question routed to existing supervisor inbox

- **GIVEN** agents `"feat-x"` and `"supervisor"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.question` message from `"feat-x"`
- **THEN** `poll_messages` for `"supervisor"` returns the question message
- **AND** `poll_messages` for `"feat-x"` returns no new messages from this publish

#### Scenario: Question creates supervisor inbox when absent

- **GIVEN** agent `"feat-x"` has an existing inbox AND no inbox exists for `"supervisor"`
- **WHEN** `publish_message` is called with an `agent.question` message from `"feat-x"`
- **THEN** a new inbox is created for `"supervisor"` containing the question message
- **AND** subsequent `poll_messages` for `"supervisor"` returns the question

#### Scenario: Question does not reach unrelated agents

- **GIVEN** agents `"feat-x"`, `"feat-y"`, and `"supervisor"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.question` from `"feat-x"`
- **THEN** `poll_messages` for `"feat-y"` returns no new messages from this publish

### Requirement: Answer delivery routes to the target agent

The delivery layer SHALL route an `agent.answer` message to the inbox of the agent named by the envelope's `agent_id` (the target), mirroring `agent.feedback` routing. The message's sender for roster purposes SHALL be the payload's `from` field, so publishing an answer SHALL NOT create a phantom roster entry for the target.

#### Scenario: Answer lands in the target agent's inbox

- **GIVEN** a published `agent.answer` with `agent_id = "feat-x"` and `payload.from = "supervisor"`
- **WHEN** agent `feat-x` polls its inbox
- **THEN** the answer message SHALL be delivered to `feat-x`
- **AND** other agents' inboxes SHALL NOT receive it

#### Scenario: Answer publish does not distort the roster

- **GIVEN** the same published answer
- **WHEN** the broker roster is inspected
- **THEN** the publish SHALL be attributed to `supervisor` (the `from` sender), not to `feat-x`

### Requirement: Duplicate question suppression

When routing a `BrokerMessage::Question` to the `"supervisor"` inbox, the broker SHALL suppress the enqueue if an identical question — same `agent_id` and same `payload.question` text — is already resident in the supervisor inbox. This prevents a blocked agent that re-publishes the same question every poll cycle from flooding the supervisor inbox with duplicates.

Suppression SHALL be scoped to identical `(agent_id, question)` pairs; a question with different text, or the same text from a different agent, SHALL still be enqueued. Suppression SHALL NOT drop the message silently in a way that loses the first copy — the first occurrence is always enqueued; only exact re-publishes of a still-resident question are dropped.

#### Scenario: Identical re-published question is enqueued only once

- **GIVEN** the `"supervisor"` inbox is empty
- **WHEN** agent `"feat-x"` publishes an `agent.question` with `question = "Which error type?"` and then publishes the identical `agent.question` again before the first is drained
- **THEN** the supervisor inbox contains exactly one copy of that question

#### Scenario: Distinct questions from the same agent both enqueue

- **GIVEN** the `"supervisor"` inbox is empty
- **WHEN** agent `"feat-x"` publishes an `agent.question` with `question = "Which error type?"` and then an `agent.question` with `question = "Which module?"`
- **THEN** the supervisor inbox contains both questions

#### Scenario: Same question text from a different agent still enqueues

- **GIVEN** the supervisor inbox already holds a question `"Which error type?"` from `"feat-x"`
- **WHEN** agent `"feat-y"` publishes an `agent.question` with the identical text `"Which error type?"`
- **THEN** the supervisor inbox holds both copies (one per agent)

