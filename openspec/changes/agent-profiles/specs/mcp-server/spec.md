## ADDED Requirements

### Requirement: A server running inside a delegate run is never a root session

An MCP server whose environment carries `GIT_PAW_DELEGATE` SHALL treat itself as a non-root session regardless of the repository it resolves: it SHALL NOT advertise any root-only tool — the lifecycle tools and the authority publish tools — and a by-name call to one SHALL return an MCP-level error stating the tool is unavailable inside a delegate run.

#### Scenario: A delegate's MCP server has no root tools

- **GIVEN** `GIT_PAW_DELEGATE=explorer` in the environment of an MCP server resolved to the main working tree
- **WHEN** the client lists tools
- **THEN** the list SHALL NOT include `publish_verified`, `publish_feedback`, `start_session`, `stop_session` or `purge_session`

#### Scenario: A by-name call is refused

- **GIVEN** the same server
- **WHEN** the client calls `publish_verified` by name
- **THEN** the server SHALL return an MCP-level error stating it is unavailable inside a delegate run
- **AND** nothing SHALL be published
