## MODIFIED Requirements

### Requirement: Logging configuration section

The system SHALL support an optional `[logging]` section with the fields `enabled` (bool, default
`false`), `capture_terminal` (bool, default `true`) and `retain_sessions` (positive integer,
optional). `capture_terminal` and `retain_sessions` take effect only when `enabled` is `true`. A
`retain_sessions` value of `0` SHALL be rejected with a configuration error naming the field. An
unrecognized key in the section (including the `log_dir` key documented by earlier releases but
never implemented) SHALL be ignored so existing configs keep loading.

#### Scenario: Logging section with all fields
- **GIVEN** a TOML file with `[logging]` containing `enabled = true`, `capture_terminal = false` and `retain_sessions = 5`
- **WHEN** the file is loaded
- **THEN** `logging.enabled` SHALL be `true`, `logging.capture_terminal` SHALL be `false` and `logging.retain_sessions` SHALL be `5`

#### Scenario: Logging section with only enabled
- **GIVEN** a TOML file with `[logging]` containing only `enabled = true`
- **WHEN** the file is loaded
- **THEN** terminal capture SHALL resolve to on and retention SHALL resolve to unset

#### Scenario: Logging section defaults
- **GIVEN** a TOML file without a `[logging]` section
- **WHEN** the file is loaded
- **THEN** `logging` SHALL be `None`

#### Scenario: Zero retention rejected
- **GIVEN** a TOML file with `[logging]` containing `enabled = true` and `retain_sessions = 0`
- **WHEN** the file is loaded
- **THEN** loading SHALL fail with a configuration error that names `retain_sessions`

#### Scenario: Legacy log_dir key is ignored
- **GIVEN** a TOML file with `[logging]` containing `enabled = true` and `log_dir = ".git-paw/logs"`
- **WHEN** the file is loaded
- **THEN** loading SHALL succeed with `logging.enabled` `true`
