## ADDED Requirements

### Requirement: Broker message envelope

The system SHALL define a single `BrokerMessage` type that represents every message exchanged between agents and the git-paw broker. The type SHALL be a Rust enum with three variants — `Status`, `Artifact`, and `Blocked` — each carrying an `agent_id: String` and a strongly-typed payload struct.

The wire format SHALL be JSON with an internally tagged discriminator field named `type`, taking the values `agent.status`, `agent.artifact`, or `agent.blocked`. Every message SHALL include `agent_id` and `payload` fields at the top level alongside `type`.

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

### Requirement: Status payload shape

The `StatusPayload` struct SHALL contain:

- `status: String` — a free-form short label such as `"working"`, `"idle"`, or `"committed"`
- `modified_files: Vec<String>` — zero or more file paths the agent has modified since its last status report
- `message: Option<String>` — an optional free-form human-readable note

#### Scenario: Status payload with all fields populated

- **WHEN** a `StatusPayload { status: "working", modified_files: ["src/a.rs", "src/b.rs"], message: Some("refactoring") }` is serialized
- **THEN** the resulting JSON contains all three fields with the expected values
- **AND** deserializing the same JSON produces an equal struct

#### Scenario: Status payload with empty modified_files and no message

- **WHEN** a `StatusPayload { status: "idle", modified_files: vec![], message: None }` is serialized and deserialized
- **THEN** the round-trip preserves the value
- **AND** the `message` field is absent from the JSON or serialized as `null`

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
