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
