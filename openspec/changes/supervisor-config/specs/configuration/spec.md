## MODIFIED Requirements

### Requirement: Parse TOML config with all fields

The system SHALL parse a TOML configuration file containing `default_cli`, `mouse`, `clis`, `presets`, and optional sections `[specs]`, `[logging]`, `[broker]`, and `[supervisor]`.

#### Scenario: Config with all fields populated
- **GIVEN** a TOML file with `default_cli`, `mouse`, custom CLIs, presets, `[broker]`, and `[supervisor]` sections
- **WHEN** the file is loaded
- **THEN** all fields SHALL be correctly parsed including supervisor fields

#### Scenario: All fields are optional
- **GIVEN** a TOML file with only `default_cli`
- **WHEN** the file is loaded
- **THEN** missing fields SHALL default to `None` or empty collections
- **AND** `supervisor` SHALL be `None`

### Requirement: Default config generation

The system SHALL provide a function to generate a default `config.toml` string with active defaults and commented-out fields including the `[supervisor]` section.

#### Scenario: Generated config contains commented supervisor examples
- **WHEN** the default config string is generated
- **THEN** it SHALL contain commented-out examples for `[supervisor]` with `enabled`, `cli`, `test_command`, and `agent_approval` fields

#### Scenario: Generated config contains commented examples
- **WHEN** the default config string is generated
- **THEN** it SHALL contain commented-out examples for `default_spec_cli`, `branch_prefix`, `[specs]`, `[logging]`, `[broker]`, and `[supervisor]`
