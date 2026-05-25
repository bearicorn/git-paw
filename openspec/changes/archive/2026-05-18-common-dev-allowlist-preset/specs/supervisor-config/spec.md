## ADDED Requirements

### Requirement: Common dev allowlist sub-table

The system SHALL extend `SupervisorConfig` with a nested
`common_dev_allowlist: CommonDevAllowlistConfig` field (or equivalent
name matching local serde conventions).

`CommonDevAllowlistConfig` SHALL contain:

- `enabled: bool` — defaults to `true` when the field or section is
  absent from `.git-paw/config.toml`. Controls whether the
  dev-allowlist seeder (per `dev-command-allowlist`) runs on supervisor
  start.
- `extra: Vec<String>` — defaults to empty when the field or section
  is absent. User-supplied additional prefix patterns appended to the
  built-in preset by the seeder.

The `[supervisor.common_dev_allowlist]` TOML table SHALL be fully
optional. A config file with `[supervisor] enabled = true` and no
nested `common_dev_allowlist` table SHALL load with `enabled = true`
and `extra = []`.

The field SHALL use `#[serde(default)]` so missing fields parse to the
documented defaults rather than triggering parse errors.

#### Scenario: Defaults when sub-table is absent

- **GIVEN** a config file with `[supervisor] enabled = true` and no
  `[supervisor.common_dev_allowlist]` section
- **WHEN** the config is loaded
- **THEN** `supervisor.common_dev_allowlist.enabled` SHALL be `true`
- **AND** `supervisor.common_dev_allowlist.extra` SHALL be empty

#### Scenario: Enabled false opt-out is honoured

- **GIVEN** a config file with
  `[supervisor.common_dev_allowlist] enabled = false`
- **WHEN** the config is loaded
- **THEN** `supervisor.common_dev_allowlist.enabled` SHALL be `false`

#### Scenario: Extra patterns parsed

- **GIVEN** a config file with
  `[supervisor.common_dev_allowlist] extra = ["pnpm test", "deno fmt"]`
- **WHEN** the config is loaded
- **THEN** `supervisor.common_dev_allowlist.extra` SHALL equal
  `["pnpm test", "deno fmt"]`

#### Scenario: Sub-table round-trips through save and load

- **GIVEN** a `SupervisorConfig` with
  `common_dev_allowlist = CommonDevAllowlistConfig { enabled: false, extra: vec!["x".to_string()] }`
- **WHEN** the config is serialised to TOML and parsed back
- **THEN** the round-trip loaded value SHALL equal the original

#### Scenario: Pre-v0.5 configs load with defaults

- **GIVEN** a `.git-paw/config.toml` produced before v0.5.0
  (no `[supervisor.common_dev_allowlist]` table)
- **WHEN** the config is loaded with the v0.5.0 binary
- **THEN** loading SHALL succeed without error
- **AND** `supervisor.common_dev_allowlist.enabled` SHALL be `true`
- **AND** `supervisor.common_dev_allowlist.extra` SHALL be empty
