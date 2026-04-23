## MODIFIED Requirements

### Requirement: Start subcommand with optional flags

The `start` subcommand SHALL be extended to accept a `--supervisor` flag (boolean, defaults to `false`). The flag MAY be combined with any other `start` flags.

When `--supervisor` is passed, the parsed `StartArgs` struct SHALL have `supervisor: bool` set to `true`.

#### Scenario: Start with --supervisor flag

- **GIVEN** `start --supervisor`
- **WHEN** the CLI is parsed
- **THEN** `supervisor` SHALL be `true`

#### Scenario: Start with --supervisor combined with other flags

- **GIVEN** `start --supervisor --cli claude --branches feat/a,feat/b`
- **WHEN** the CLI is parsed
- **THEN** `supervisor` SHALL be `true`
- **AND** `cli` SHALL be `Some("claude")`
- **AND** `branches` SHALL be `["feat/a", "feat/b"]`

#### Scenario: Start without --supervisor defaults to false

- **GIVEN** `start --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `supervisor` SHALL be `false`

### Requirement: Help output contains all subcommands and quick start

The `start --help` output SHALL list the `--supervisor` flag with a description.

#### Scenario: Start help shows --supervisor flag

- **GIVEN** `start --help` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain `--supervisor`
