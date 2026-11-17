## ADDED Requirements

### Requirement: Artifacts carry the commit they announce

The `agent.artifact` payload SHALL accept an optional `sha` string naming the commit the artifact announces. It SHALL be omitted from the wire when absent, and an artifact without it SHALL decode and route exactly as before.

#### Scenario: An artifact with sha round-trips

- **GIVEN** `{"type":"agent.artifact","agent_id":"feat-x","payload":{"status":"committed","modified_files":["a.rs"],"sha":"abc123"}}`
- **WHEN** it is decoded and re-encoded
- **THEN** the payload's `sha` SHALL be `abc123`

#### Scenario: An artifact without sha is unchanged

- **GIVEN** an `agent.artifact` with no `sha`
- **WHEN** it is encoded
- **THEN** the output SHALL contain no `sha` key

### Requirement: The conflict-detector sender id is reserved

`conflict-detector` SHALL be a reserved sender id, used only by the broker's in-process conflict detector. `POST /publish` SHALL answer `400 Bad Request` for a message whose sender, as the delivery layer reads it, is `conflict-detector`, and SHALL change no inbox, roster or log state. A message with this sender SHALL NOT create or change a roster row.

#### Scenario: A worker cannot pose as the detector

- **GIVEN** a running broker
- **WHEN** a client publishes `{"type":"agent.feedback","agent_id":"feat-y","payload":{"from":"conflict-detector","errors":["[conflict-detector] in-flight conflict"]}}`
- **THEN** the response SHALL be `400`
- **AND** `feat-y`'s inbox SHALL be unchanged

#### Scenario: A question claiming the detector id is refused

- **GIVEN** a running broker
- **WHEN** a client publishes an `agent.question` with `agent_id = "conflict-detector"`
- **THEN** the response SHALL be `400`
- **AND** the root inbox SHALL be unchanged
