## ADDED Requirements

### Requirement: Learnings opt-in flag

The system SHALL extend `SupervisorConfig` with a `learnings: bool` field. The field SHALL default to `false` when the field or section is absent from `.git-paw/config.toml`.

When `[supervisor] learnings = true` is set AND supervisor mode is otherwise active for the session (per the existing supervisor-cli resolution chain, including respect for `--no-supervisor`), the broker SHALL start the learnings aggregator subsystem (per `learnings-mode`). When `learnings = false` (or absent), the aggregator SHALL NOT start.

#### Scenario: learnings defaults to false when absent

- **GIVEN** a config file with `[supervisor] enabled = true` and no `learnings` field
- **WHEN** the config is loaded
- **THEN** `supervisor.learnings` SHALL be `false`

#### Scenario: learnings = true is loaded

- **GIVEN** a config file with `[supervisor] enabled = true` and `learnings = true`
- **WHEN** the config is loaded
- **THEN** `supervisor.learnings` SHALL be `true`

#### Scenario: learnings round-trips through save and load

- **GIVEN** a `SupervisorConfig` with `learnings = true`
- **WHEN** saved to TOML and loaded back
- **THEN** the loaded value SHALL be `true`

#### Scenario: Pre-v0.5 configs load with learnings = false

- **GIVEN** a `.git-paw/config.toml` produced before v0.5.0 (no `learnings` field)
- **WHEN** the config is loaded with the v0.5.0 binary
- **THEN** loading SHALL succeed without error
- **AND** `supervisor.learnings` SHALL be `false`

### Requirement: LearningsConfig sub-table

The system SHALL extend `SupervisorConfig` with a nested `learnings_config: LearningsConfig` field (or equivalent name matching local conventions). `LearningsConfig` SHALL contain:

- `flush_interval_seconds: u64` — defaults to `60` when the field or section is absent. Used by the learnings aggregator's periodic flush timer.

The `[supervisor.learnings_config]` table (TOML key name follows local serde conventions; design suggests `[supervisor.learnings_config]` to avoid colliding with the boolean `learnings` field) SHALL be optional. A config file with `[supervisor] learnings = true` and no nested table SHALL load with `flush_interval_seconds = 60`.

#### Scenario: LearningsConfig defaults when section absent

- **GIVEN** a config file with `[supervisor] enabled = true`, `learnings = true`, and no `[supervisor.learnings_config]` section
- **WHEN** the config is loaded
- **THEN** `supervisor.learnings_config.flush_interval_seconds` SHALL be `60`

#### Scenario: Custom flush_interval_seconds is honoured

- **GIVEN** a config file with `[supervisor.learnings_config] flush_interval_seconds = 30`
- **WHEN** the config is loaded
- **THEN** the loaded `flush_interval_seconds` SHALL be `30`

#### Scenario: LearningsConfig round-trips through save and load

- **GIVEN** a `LearningsConfig { flush_interval_seconds: 90 }`
- **WHEN** saved to TOML and loaded back
- **THEN** the loaded value matches the original
