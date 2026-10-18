## MODIFIED Requirements

### Requirement: MCP configuration section

The system SHALL parse an optional `[mcp]` section in `.git-paw/config.toml` into an `McpConfig` exposed as `PawConfig.mcp`. The struct SHALL contain an optional `name: Option<String>` field defaulting to `None` when absent, and an `allow_repo_writes: bool` field defaulting to `false` when absent and omitted from serialized output when `false`. A config file with no `[mcp]` section SHALL load with `McpConfig::default()` (`name: None`, `allow_repo_writes: false`), so pre-existing configs load unchanged. When the user-level and repository configs are merged, `allow_repo_writes` SHALL take the repository config's value only; a value set in the user-level config SHALL have no effect.

#### Scenario: Config with [mcp] name parses the field

- **GIVEN** a config file with `[mcp]` setting `name = "my-project"`
- **WHEN** the config is loaded
- **THEN** `PawConfig.mcp.name` SHALL be `Some("my-project")`

#### Scenario: Config with [mcp] allow_repo_writes parses the field

- **GIVEN** a repository config file with `[mcp]` setting `allow_repo_writes = true`
- **WHEN** the config is loaded
- **THEN** `PawConfig.mcp.allow_repo_writes` SHALL be `true`

#### Scenario: Config without [mcp] section loads with defaults

- **GIVEN** a config file with no `[mcp]` section
- **WHEN** the config is loaded
- **THEN** `PawConfig.mcp` SHALL equal `McpConfig::default()` with `name` set to `None` and `allow_repo_writes` set to `false`
- **AND** loading SHALL NOT error

#### Scenario: The user-level config cannot enable repository writes

- **GIVEN** a user-level config setting `[mcp] allow_repo_writes = true`
- **AND** a repository config that does not set it
- **WHEN** the merged config is loaded
- **THEN** `PawConfig.mcp.allow_repo_writes` SHALL be `false`

#### Scenario: MCP config survives round-trip serialization

- **GIVEN** a `PawConfig` whose `mcp.name` is set and whose `mcp.allow_repo_writes` is `true`
- **WHEN** it is serialized to TOML and re-parsed
- **THEN** the re-parsed `mcp` field SHALL equal the original

#### Scenario: A false switch is not serialized

- **GIVEN** a `PawConfig` whose `mcp.allow_repo_writes` is `false`
- **WHEN** it is serialized to TOML
- **THEN** the output SHALL NOT contain `allow_repo_writes`
