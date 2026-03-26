## MODIFIED Requirements

### Requirement: Parse TOML config with all fields

The system SHALL parse a TOML configuration file containing `default_cli`, `mouse`, `clis`, `presets`, `default_spec_cli`, `branch_prefix`, `specs`, and `logging` fields.

#### Scenario: Config with all fields populated
- **GIVEN** a TOML file with `default_cli`, `mouse`, custom CLIs, presets, `default_spec_cli`, `branch_prefix`, `specs`, and `logging`
- **WHEN** the file is loaded
- **THEN** all fields SHALL be correctly parsed

Test: `config::tests::parses_config_with_all_fields`

#### Scenario: All fields are optional
- **GIVEN** a TOML file with only `default_cli`
- **WHEN** the file is loaded
- **THEN** missing fields SHALL default to `None` or empty collections

Test: `config::tests::all_fields_are_optional`

#### Scenario: No config files exist
- **GIVEN** neither global nor repo config files exist
- **WHEN** `load_config()` is called
- **THEN** it SHALL return a default config with all fields empty/None

Test: `config::tests::returns_defaults_when_no_files_exist`

#### Scenario: Invalid TOML reports error with file path
- **GIVEN** a malformed TOML file
- **WHEN** it is loaded
- **THEN** the error message SHALL include the file path

Test: `config::tests::reports_error_for_invalid_toml`

### Requirement: Merge repo config over global config

The system SHALL merge per-repo configuration on top of global configuration, with repo values taking precedence for scalar fields and map entries.

#### Scenario: Repo overrides global scalar fields
- **GIVEN** global config has `default_cli = "claude"` and `mouse = true`, and repo has `default_cli = "gemini"`
- **WHEN** configs are merged
- **THEN** `default_cli` SHALL be `"gemini"` and `mouse` SHALL be `true` (preserved from global)

Test: `config::tests::repo_config_overrides_global_scalars`

#### Scenario: Repo overrides new v0.2.0 scalar fields
- **GIVEN** global config has `default_spec_cli = "claude"` and repo has `default_spec_cli = "gemini"`
- **WHEN** configs are merged
- **THEN** `default_spec_cli` SHALL be `"gemini"`

#### Scenario: CLI maps are merged
- **GIVEN** global config has CLI `agent-a` and repo config has CLI `agent-b`
- **WHEN** configs are merged
- **THEN** both CLIs SHALL be present

Test: `config::tests::repo_config_merges_cli_maps`

#### Scenario: Repo CLI overrides global CLI with same name
- **GIVEN** both global and repo define a CLI named `my-agent`
- **WHEN** configs are merged
- **THEN** the repo definition SHALL win

Test: `config::tests::repo_cli_overrides_global_cli_with_same_name`

#### Scenario: Only global config exists
- **GIVEN** a global config file but no repo config
- **WHEN** `load_config()` is called
- **THEN** global values SHALL be used

Test: `config::tests::load_config_from_reads_global_file_when_no_repo`

#### Scenario: Only repo config exists
- **GIVEN** a repo config file but no global config
- **WHEN** `load_config()` is called
- **THEN** repo values SHALL be used

Test: `config::tests::load_config_from_reads_repo_file_when_no_global`

## ADDED Requirements

### Requirement: Specs configuration section

The system SHALL support an optional `[specs]` section with `specs_dir` and `enabled` fields.

#### Scenario: Specs section with all fields
- **GIVEN** a TOML file with `[specs]` containing `specs_dir = "openspec/specs"` and `enabled = true`
- **WHEN** the file is loaded
- **THEN** `specs.specs_dir` SHALL be `"openspec/specs"` and `specs.enabled` SHALL be `true`

#### Scenario: Specs section defaults
- **GIVEN** a TOML file without a `[specs]` section
- **WHEN** the file is loaded
- **THEN** `specs` SHALL be `None`

### Requirement: Logging configuration section

The system SHALL support an optional `[logging]` section with `enabled` and `log_dir` fields.

#### Scenario: Logging section with all fields
- **GIVEN** a TOML file with `[logging]` containing `enabled = true` and `log_dir = ".git-paw/logs"`
- **WHEN** the file is loaded
- **THEN** `logging.enabled` SHALL be `true` and `logging.log_dir` SHALL be `".git-paw/logs"`

#### Scenario: Logging section defaults
- **GIVEN** a TOML file without a `[logging]` section
- **WHEN** the file is loaded
- **THEN** `logging` SHALL be `None`

### Requirement: Default config generation

The system SHALL provide a function to generate a default `config.toml` string with active defaults and commented-out v0.2.0 fields.

#### Scenario: Generated config is valid TOML
- **WHEN** the default config string is generated
- **THEN** it SHALL be parseable as valid TOML

#### Scenario: Generated config contains commented examples
- **WHEN** the default config string is generated
- **THEN** it SHALL contain commented-out examples for `default_spec_cli`, `branch_prefix`, `[specs]`, and `[logging]`

### Requirement: Config round-trip with new fields

A `PawConfig` with v0.2.0 fields populated SHALL be identical after save and reload.

#### Scenario: Config with specs and logging round-trips
- **GIVEN** a config with `default_spec_cli`, `branch_prefix`, `specs`, and `logging` populated
- **WHEN** saved and loaded back
- **THEN** it SHALL be equal to the original
