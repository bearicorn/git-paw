## ADDED Requirements

### Requirement: `[supervisor.auto_approve]` config section

The system SHALL accept a `[supervisor.auto_approve]` table in `.git-paw/config.toml` to control auto-approval behaviour.

#### Scenario: Default config when section absent

- **GIVEN** a `.git-paw/config.toml` with no `[supervisor.auto_approve]` section
- **WHEN** the config is loaded
- **THEN** `AutoApproveConfig::default()` SHALL apply with `enabled = true`, `safe_commands = []` (defaults provided in code), and `stall_threshold_seconds = 30`

#### Scenario: Backward-compatible parse

- **GIVEN** an existing v0.3.0 config with no supervisor section
- **WHEN** the config is loaded
- **THEN** parsing SHALL succeed without error
- **AND** the optional `supervisor.auto_approve` field SHALL serialise as `None` when not set

### Requirement: Configurable enable flag

The `enabled` field SHALL gate the entire auto-approval feature.

#### Scenario: Disabled at runtime

- **GIVEN** `[supervisor.auto_approve] enabled = false`
- **WHEN** the supervisor poll loop runs
- **THEN** detection SHALL NOT capture panes
- **AND** auto-approval SHALL NOT fire
- **AND** stall detection alone (without approval) MAY still run

#### Scenario: Enabled by default

- **GIVEN** the supervisor section is present but `enabled` is omitted
- **WHEN** the config is loaded
- **THEN** `enabled` SHALL default to `true`

### Requirement: Configurable safe-command list

The `safe_commands` field SHALL be a list of strings that are appended to the built-in defaults.

#### Scenario: Custom command added

- **GIVEN** `safe_commands = ["just smoke"]` in config
- **WHEN** classification runs against the command `just smoke -v`
- **THEN** `is_safe_command(...)` SHALL return `true`

#### Scenario: Empty list keeps defaults

- **GIVEN** `safe_commands = []`
- **WHEN** classification runs against `cargo test`
- **THEN** `is_safe_command(...)` SHALL still return `true`

### Requirement: Configurable stall threshold

The `stall_threshold_seconds` field SHALL govern how long an agent's `last_seen` must be older than the current time before stall detection treats it as stuck.

#### Scenario: Custom threshold

- **GIVEN** `stall_threshold_seconds = 60`
- **WHEN** an agent's `last_seen` is 45 seconds old
- **THEN** stall detection SHALL NOT classify it as stalled

#### Scenario: Threshold floor

- **GIVEN** `stall_threshold_seconds = 0`
- **WHEN** the config is loaded
- **THEN** the system SHALL clamp the effective threshold to a minimum of 5 seconds to avoid pathological poll loops
- **AND** SHALL emit a warning to stderr describing the clamp

### Requirement: Approval level coarse switch

The system SHALL accept a coarse `approval_level` field that maps to common policy presets.

#### Scenario: `safe` preset

- **GIVEN** `approval_level = "safe"`
- **WHEN** the config is loaded
- **THEN** the effective whitelist SHALL be the built-in defaults only (no extras)
- **AND** `enabled` SHALL be `true`

#### Scenario: `conservative` preset

- **GIVEN** `approval_level = "conservative"`
- **WHEN** the config is loaded
- **THEN** the effective whitelist SHALL exclude `git push` and `curl` entries
- **AND** `enabled` SHALL be `true`

#### Scenario: `off` preset

- **GIVEN** `approval_level = "off"`
- **WHEN** the config is loaded
- **THEN** `enabled` SHALL be forced to `false` regardless of other fields
