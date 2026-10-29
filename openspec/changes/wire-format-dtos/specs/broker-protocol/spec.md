## MODIFIED Requirements

### Requirement: Broker message envelope

The system SHALL define a single in-memory `BrokerMessage` type that represents every message exchanged between agents, the supervisor, and the git-paw broker, and a separate broker wire type that alone defines its JSON form; the two SHALL be converted by total mappers in both directions, and the in-memory type SHALL NOT derive `Serialize` or `Deserialize`.

The wire format SHALL be JSON with an internally tagged discriminator field named `type`, taking exactly these eleven values: `agent.status`, `agent.artifact`, `agent.blocked`, `agent.verified`, `agent.feedback`, `agent.answer`, `agent.question`, `agent.intent`, `agent.advanced-main`, `agent.learning`, and `supervisor.verify-now`. The eight `agent.*` variants other than `agent.advanced-main` and `agent.learning` SHALL carry `agent_id` and `payload` at the top level alongside `type`. `agent.advanced-main` SHALL carry its payload fields flattened at the top level. `agent.learning` SHALL carry only `payload`, which holds its own `agent_id`. `supervisor.verify-now` SHALL carry only `branch_id`. The envelope SHALL carry no version field; the broker's protocol version is reported by its HTTP responses. The serialized bytes of every variant SHALL be identical to those produced before the wire type was separated.

#### Scenario: Status message round-trips through serde

- **WHEN** a `BrokerMessage::Status` with `agent_id = "feat-x"` and a populated `StatusPayload` is encoded to JSON and decoded back
- **THEN** the resulting value equals the original
- **AND** the intermediate JSON contains `"type": "agent.status"` and `"agent_id": "feat-x"` at the top level
- **AND** the intermediate JSON contains the payload nested under a `"payload"` key

#### Scenario: Artifact message round-trips through serde

- **WHEN** a `BrokerMessage::Artifact` with `agent_id = "feat-errors"` and a populated `ArtifactPayload` is encoded and decoded back
- **THEN** the resulting value equals the original
- **AND** the intermediate JSON contains `"type": "agent.artifact"`

#### Scenario: Blocked message round-trips through serde

- **WHEN** a `BrokerMessage::Blocked` with `agent_id = "feat-config"` and a populated `BlockedPayload` is encoded and decoded back
- **THEN** the resulting value equals the original
- **AND** the intermediate JSON contains `"type": "agent.blocked"`

#### Scenario: Unknown message type is rejected

- **WHEN** a JSON object with `"type": "agent.unknown"` is decoded as a broker message
- **THEN** decoding fails with a deserialization error
- **AND** no `BrokerMessage` value is produced

#### Scenario: Envelope enumerates all eleven wire-format type values

- **WHEN** one message of each variant is encoded
- **THEN** the set of emitted `type` values SHALL equal exactly the eleven values listed in this requirement

#### Scenario: Every variant's bytes match the pre-change fixture

- **GIVEN** a golden JSON fixture per variant captured before the wire type was separated
- **WHEN** the equivalent `BrokerMessage` is encoded
- **THEN** the emitted bytes SHALL equal the fixture
- **AND** decoding the fixture SHALL produce an equal `BrokerMessage`

## ADDED Requirements

### Requirement: Broker HTTP shapes are shared wire types with a protocol schema version

The request and response bodies of the broker's HTTP endpoints — the `GET /status` response and its agent rows, the `GET /log` response and its entries, the `GET /messages/:agent_id` response, and the JSON error body — SHALL each be defined once, as public types in the broker wire module, and SHALL be used both by the broker server to encode them and by every in-tree HTTP client to decode them; no client SHALL re-declare these shapes. The `GET /status`, `GET /log`, and `GET /messages/:agent_id` response objects SHALL carry a top-level integer `schema_version` equal to 1, distinct from the existing `version` key that reports the git-paw package version. Decoding SHALL stay as tolerant as today: optional agent-row fields default when absent, and a log entry whose message cannot be decoded SHALL be skipped while the other entries are returned. The broker probe SHALL identify a git-paw broker by decoding the status response, not by substring matching.

#### Scenario: Responses declare the protocol schema version

- **WHEN** a client calls `GET /status`, `GET /log`, and `GET /messages/feat-x` on a running broker
- **THEN** each response object SHALL contain `"schema_version": 1`
- **AND** `/status` SHALL still contain `"git_paw": true` and the package `version`

#### Scenario: Every client decodes through the shared types

- **GIVEN** a broker whose `/status` reports an agent with a `phase`
- **WHEN** the drive loop's status fetch, the coordination inventory fetch, and the MCP session query read `/status`
- **THEN** each SHALL decode the same agent rows through the shared status type

#### Scenario: An undecodable log entry is skipped

- **GIVEN** a `/log` response containing one entry whose `message` is not a valid broker message
- **WHEN** a client fetches log entries
- **THEN** the other entries SHALL be returned and the invalid one omitted

#### Scenario: The probe recognises a broker by its decoded status

- **GIVEN** a listener whose `/status` body decodes with `git_paw` true, and another that returns unrelated JSON containing the text `"git_paw":true` inside a string
- **WHEN** each is probed
- **THEN** the first SHALL be classified as a git-paw broker and the second SHALL NOT

### Requirement: Helper-built messages decode through the broker wire type

Every message the bundled helper scripts publish SHALL decode through the broker wire type and pass broker validation. A standing test SHALL run each publishing verb of `broker.sh` (`status`, `artifact`, `blocked`, `question`, and `intent` with a TTL) and `sweep.sh` (`feedback-gate`, `verified`, `status-publish`, `learn`) against a recording endpoint and SHALL decode and validate every captured body.

#### Scenario: Each helper verb produces a decodable message

- **GIVEN** a recording endpoint standing in for the broker
- **WHEN** each listed helper verb publishes
- **THEN** each captured body SHALL decode through the broker wire type into the expected variant
- **AND** SHALL pass message validation
