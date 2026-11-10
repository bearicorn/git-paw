## ADDED Requirements

### Requirement: start_session messages name no CLI command

The warnings and the error `start_session` returns SHALL NOT tell the client to run a CLI command: an MCP client has no terminal. A launch hint that names `git paw add` or `--no-rebase` SHALL be removed from the message, keeping the reason. The CLI SHALL keep printing the hint.

#### Scenario: A selection-ignored warning reaches the client without the CLI hint

- **GIVEN** a launch warning that the selection was not applied, ending in the `git paw add` hint
- **WHEN** `start_session` returns it
- **THEN** the warning SHALL say the selection was not applied
- **AND** it SHALL NOT contain `git paw`

Test: `src/mcp/tools/session.rs::start_session_wording_drops_the_cli_hints`

#### Scenario: An empty-session refusal reaches the client without the CLI hint

- **GIVEN** a launch refusal for a running session recording no agents, ending in the `git paw add` hint
- **WHEN** `start_session` returns it as its error
- **THEN** the error SHALL say the session is running with no agents
- **AND** it SHALL NOT contain `git paw`

Test: `src/mcp/tools/session.rs::start_session_wording_drops_the_cli_hints`
