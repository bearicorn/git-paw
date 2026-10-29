## ADDED Requirements

### Requirement: Serialized contracts are wire types separate from the domain model

Each format another process, a bundled script, or a future release reads SHALL be defined by a dedicated wire type with total mappers to and from the in-memory model, and the model type SHALL NOT derive `Serialize` or `Deserialize`. The contract edges are: the global session file, the per-repo discovery file, `config.toml`, the broker message envelope and the broker HTTP request and response bodies, and the manual-decision log. Each edge SHALL carry a schema version as its own requirement specifies, and SHALL evolve additively within a version. The broker's diagnostic text log (`broker.log`) SHALL NOT be a contract edge: its line format MAY change in any release, and machine consumers SHALL read broker history through `GET /log` instead. Structs that never leave the process SHALL NOT be given wire types. A standing guard SHALL fail if any model type behind a listed edge derives `Serialize` or `Deserialize`.

#### Scenario: Model types carry no serde derive

- **WHEN** the standing guard inspects `Session`, `WorktreeEntry`, `PawConfig`, `BrokerMessage`, and the manual-approval model
- **THEN** none SHALL derive `Serialize` or `Deserialize`

#### Scenario: Golden fixtures pin every edge

- **GIVEN** golden fixtures for each listed edge captured before its wire type was separated
- **WHEN** the equivalent model value is encoded through the edge's wire type
- **THEN** the output SHALL equal the fixture apart from the edge's added version field

#### Scenario: The broker text log is documented as diagnostic only

- **WHEN** the broker protocol reference is inspected
- **THEN** it SHALL state that `broker.log` is a diagnostic log whose format is not a stable contract
- **AND** it SHALL direct machine consumers to `GET /log`
