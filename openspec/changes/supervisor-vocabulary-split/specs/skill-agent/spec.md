## ADDED Requirements

### Requirement: The supervisor skill name is a deprecated alias

Until v1.0.0, resolving a skill by the name `supervisor` — through `skills::resolve`, the MCP `get_skill` tool, or a user override stored as `supervisor.md` — SHALL keep returning the orchestrator skill exactly as before, and SHALL print one deprecation warning through the shared helper naming `supervisor`, its removal in v1.0.0 and `orchestrator`. Resolving by `orchestrator` SHALL NOT warn. Over MCP the warning SHALL go to the server's stderr and the JSON-RPC stream SHALL be unchanged.

#### Scenario: The old name warns and still works

- **GIVEN** an MCP client
- **WHEN** it calls `get_skill` with the name `supervisor`
- **THEN** the result SHALL be the rendered orchestrator skill
- **AND** the server's stderr SHALL carry one deprecation warning naming `supervisor` and `orchestrator`

#### Scenario: The canonical name does not warn

- **WHEN** a skill is resolved by the name `orchestrator`
- **THEN** no deprecation warning SHALL be emitted

#### Scenario: A legacy override file warns

- **GIVEN** a user override at `<config_dir>/git-paw/agent-skills/supervisor.md` and none at `orchestrator.md`
- **WHEN** the orchestrator skill is resolved
- **THEN** the override's content SHALL be returned
- **AND** one deprecation warning SHALL name the `supervisor.md` override and `orchestrator.md`
