## MODIFIED Requirements

### Requirement: Parse TOML config with all fields

The system SHALL parse a TOML configuration file containing `default_cli`, `mouse`, `clis`, `presets`, and an optional `[broker]` section with fields `enabled` (bool), `port` (u16), and `bind` (String).

#### Scenario: Config with all fields populated
- **GIVEN** a TOML file with `default_cli`, `mouse`, custom CLIs, presets, and `[broker]` section
- **WHEN** the file is loaded
- **THEN** all fields SHALL be correctly parsed including broker fields

#### Scenario: All fields are optional
- **GIVEN** a TOML file with only `default_cli`
- **WHEN** the file is loaded
- **THEN** missing fields SHALL default to `None` or empty collections
- **AND** `broker` SHALL default to `BrokerConfig { enabled: false, port: 9119, bind: "127.0.0.1" }`

#### Scenario: No config files exist
- **GIVEN** neither global nor repo config files exist
- **WHEN** `load_config()` is called
- **THEN** it SHALL return a default config with all fields empty/None
- **AND** `broker.enabled` SHALL be `false`

#### Scenario: Invalid TOML reports error with file path
- **GIVEN** a malformed TOML file
- **WHEN** it is loaded
- **THEN** the error message SHALL include the file path

## ADDED Requirements

### Requirement: Broker configuration section

The system SHALL support an optional `[broker]` section with the following fields:

- `enabled: bool` — defaults to `false` when the field or section is absent
- `port: u16` — defaults to `9119` when absent
- `bind: String` — defaults to `"127.0.0.1"` when absent

The `BrokerConfig` struct SHALL provide a `url(&self) -> String` method returning `http://<bind>:<port>`.

#### Scenario: Broker section with all fields
- **GIVEN** a TOML file with `[broker]` containing `enabled = true`, `port = 9200`, `bind = "127.0.0.1"`
- **WHEN** the file is loaded
- **THEN** `broker.enabled` SHALL be `true`, `broker.port` SHALL be `9200`, `broker.bind` SHALL be `"127.0.0.1"`

#### Scenario: Broker section defaults
- **GIVEN** a TOML file without a `[broker]` section
- **WHEN** the file is loaded
- **THEN** `broker` SHALL have `enabled = false`, `port = 9119`, `bind = "127.0.0.1"`

#### Scenario: Partial broker section
- **GIVEN** a TOML file with `[broker]` containing only `enabled = true`
- **WHEN** the file is loaded
- **THEN** `broker.enabled` SHALL be `true`, `broker.port` SHALL be `9119`, `broker.bind` SHALL be `"127.0.0.1"`

#### Scenario: BrokerConfig url method
- **GIVEN** `BrokerConfig { enabled: true, port: 9200, bind: "127.0.0.1" }`
- **WHEN** `url()` is called
- **THEN** the result SHALL be `"http://127.0.0.1:9200"`

#### Scenario: Broker config round-trips through save and load
- **GIVEN** a config with `[broker]` fully populated
- **WHEN** saved and loaded back
- **THEN** all broker fields SHALL match the original

## MODIFIED Requirements

### Requirement: Default config generation

The system SHALL provide a function to generate a default `config.toml` string with active defaults and commented-out fields including the `[broker]` section.

#### Scenario: Generated config contains commented broker examples
- **WHEN** the default config string is generated
- **THEN** it SHALL contain commented-out examples for `[broker]` with `enabled`, `port`, and `bind` fields

#### Scenario: Generated config contains commented examples
- **WHEN** the default config string is generated
- **THEN** it SHALL contain commented-out examples for `default_spec_cli`, `branch_prefix`, `[specs]`, `[logging]`, and `[broker]`
