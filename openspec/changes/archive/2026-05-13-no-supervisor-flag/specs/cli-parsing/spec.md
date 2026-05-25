## ADDED Requirements

### Requirement: --no-supervisor flag

The `start` subcommand SHALL accept a `--no-supervisor` flag (boolean, default `false`). When passed, the parsed `StartArgs` SHALL have `no_supervisor: bool` set to `true`. The flag SHALL appear in `git paw start --help` output with a description that names the use case (overriding `[supervisor] enabled = true` for a single session).

#### Scenario: --no-supervisor sets the flag

- **GIVEN** the user invokes `git paw start --no-supervisor`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs.no_supervisor` SHALL be `true`
- **AND** `StartArgs.supervisor` SHALL be `false`

#### Scenario: --no-supervisor absent leaves flag false

- **GIVEN** the user invokes `git paw start` with neither `--supervisor` nor `--no-supervisor`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs.no_supervisor` SHALL be `false`
- **AND** `StartArgs.supervisor` SHALL be `false`

#### Scenario: --no-supervisor appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output contains `--no-supervisor`
- **AND** the output describes the flag as disabling supervisor for the session and overriding any `[supervisor] enabled = true` config setting

### Requirement: --supervisor and --no-supervisor are mutually exclusive

The system SHALL reject any invocation that combines `--supervisor` and `--no-supervisor` on the same `start` command. clap's parse step SHALL produce an error before the command runs. The error message SHALL clearly state that the two flags express opposing intents and SHALL list both.

#### Scenario: Both flags together are rejected

- **GIVEN** `start --supervisor --no-supervisor`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both `--supervisor` and `--no-supervisor`

#### Scenario: --no-supervisor combines with other flags

- **GIVEN** `start --no-supervisor --cli claude --branches feat/a,feat/b`
- **WHEN** the CLI is parsed
- **THEN** `no_supervisor` SHALL be `true`
- **AND** `cli` SHALL be `Some("claude")`
- **AND** `branches` SHALL contain `feat/a` and `feat/b`
- **AND** parsing SHALL succeed
