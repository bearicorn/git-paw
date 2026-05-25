## MODIFIED Requirements

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

## ADDED Requirements

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
