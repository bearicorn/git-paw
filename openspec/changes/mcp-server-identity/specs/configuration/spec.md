## ADDED Requirements

### Requirement: MCP configuration section

The system SHALL parse an optional `[mcp]` section in `.git-paw/config.toml` into an `McpConfig` exposed as `PawConfig.mcp`. The struct SHALL contain an optional `name: Option<String>` field defaulting to `None` when absent. A config file with no `[mcp]` section SHALL load with `McpConfig::default()` (`name: None`), so pre-existing configs load unchanged.

#### Scenario: Config with [mcp] name parses the field

- **GIVEN** a config file with `[mcp]` setting `name = "my-project"`
- **WHEN** the config is loaded
- **THEN** `PawConfig.mcp.name` SHALL be `Some("my-project")`

#### Scenario: Config without [mcp] section loads with defaults

- **GIVEN** a config file with no `[mcp]` section
- **WHEN** the config is loaded
- **THEN** `PawConfig.mcp` SHALL equal `McpConfig::default()` with `name` set to `None`
- **AND** loading SHALL NOT error

#### Scenario: MCP config survives round-trip serialization

- **GIVEN** a `PawConfig` whose `mcp.name` is set
- **WHEN** it is serialized to TOML and re-parsed
- **THEN** the re-parsed `mcp` field SHALL equal the original
