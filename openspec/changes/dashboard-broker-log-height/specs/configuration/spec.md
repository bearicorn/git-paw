# configuration Specification Delta — dashboard-broker-log-height

## ADDED Requirements

### Requirement: Broker log panel height configuration

The system SHALL support an optional `height_lines` field on the
`[dashboard.broker_log]` table, a positive integer giving the number of
terminal rows the Broker log panel occupies when visible. When the field
or the `[dashboard.broker_log]` table is absent, the effective height
SHALL default to a value strictly greater than the v0.6.0 fixed `12`
(the documented default), so configs written before this field exists —
including v0.5.0/v0.6.0/v0.7.0 configs — load unchanged. The field SHALL
participate in the repo-overrides-global merge as a scalar (repo value
wins) and SHALL survive round-trip serialization.

#### Scenario: height_lines explicitly configured

- **GIVEN** a TOML file with `[dashboard.broker_log] height_lines = 24`
- **WHEN** the config is loaded
- **THEN** `dashboard.broker_log.height_lines` SHALL be `24`

#### Scenario: height_lines absent uses the default

- **GIVEN** a TOML file with a `[dashboard.broker_log]` table that omits
  `height_lines`
- **WHEN** the config is loaded
- **THEN** `dashboard.broker_log.height_lines` SHALL equal the documented
  default, which SHALL be strictly greater than `12`

#### Scenario: Pre-existing config without the field loads unchanged

- **GIVEN** a `.git-paw/config.toml` written before this field exists
  (e.g. a v0.5.0 `[dashboard]` section with no `broker_log` table)
- **WHEN** the config is loaded
- **THEN** loading SHALL NOT error
- **AND** `dashboard.broker_log.height_lines` SHALL equal the documented
  default

#### Scenario: height_lines round-trips through save and load

- **GIVEN** a config with `[dashboard.broker_log] height_lines = 30`
- **WHEN** the config is serialized to TOML and re-parsed
- **THEN** the re-parsed `dashboard.broker_log.height_lines` SHALL be `30`
