## MODIFIED Requirements

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
