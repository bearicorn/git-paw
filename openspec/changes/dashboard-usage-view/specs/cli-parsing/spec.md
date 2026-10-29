## ADDED Requirements

### Requirement: --dashboard flag

The `start` subcommand SHALL accept a `--dashboard` flag (boolean, default `false`). When passed, the parsed `StartArgs` SHALL have `dashboard: bool` set to `true`, and the launch SHALL have a dashboard pane whatever `[dashboard] enabled` says, as defined in `broker-runtime`'s "Start flow conditionally creates dashboard pane". The flag SHALL combine with `--coordinated`, `--bare`, `--unattended` and every spec or branch selection flag. The flag SHALL appear in `git paw start --help` output with a description that names the usage-and-events view and states that the dashboard needs the broker.

#### Scenario: --dashboard sets the flag

- **GIVEN** the user invokes `git paw start --coordinated --dashboard`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs.dashboard` SHALL be `true`
- **AND** `StartArgs.coordinated` SHALL be `true`

#### Scenario: --dashboard absent leaves flag false

- **GIVEN** the user invokes `git paw start` without `--dashboard`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs.dashboard` SHALL be `false`

#### Scenario: --dashboard appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output contains `--dashboard`
- **AND** the output describes it as opening the usage-and-events dashboard and states that it needs the broker
