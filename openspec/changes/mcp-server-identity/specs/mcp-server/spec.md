## ADDED Requirements

### Requirement: Server identity

The MCP server SHALL advertise its own identity in the `initialize` handshake's `serverInfo`: `name` SHALL be `"git-paw"` (or the configured `[mcp].name` when set) and `version` SHALL be the git-paw crate version (`env!("CARGO_PKG_VERSION")`). The server SHALL NOT advertise the underlying MCP SDK's default identity.

#### Scenario: Default identity is git-paw

- **GIVEN** a repository with no `[mcp].name` configured
- **WHEN** an MCP client completes the `initialize` handshake
- **THEN** the response `serverInfo.name` SHALL be `"git-paw"`
- **AND** `serverInfo.version` SHALL be the git-paw crate version

#### Scenario: Configured name overrides the advertised identity

- **GIVEN** a repository with `[mcp] name = "my-project"` configured
- **WHEN** an MCP client completes the `initialize` handshake
- **THEN** the response `serverInfo.name` SHALL be `"my-project"`
- **AND** `serverInfo.version` SHALL still be the git-paw crate version
