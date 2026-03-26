## MODIFIED Requirements

### Requirement: Help output contains all subcommands and quick start

The `--help` output SHALL list all subcommands and include a Quick Start section.

#### Scenario: Help lists all subcommands
- **GIVEN** `--help` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain start, stop, purge, status, list-clis, add-cli, remove-cli, init, and replay

Test: `cli_tests::help_shows_all_subcommands`

## ADDED Requirements

### Requirement: Replay subcommand

The `replay` subcommand SHALL accept an optional `<branch>` positional argument, a `--list` flag, a `--color` flag, and an optional `--session` flag.

#### Scenario: Replay with branch
- **WHEN** `replay feat/add-auth` is passed
- **THEN** `branch` SHALL be `Some("feat/add-auth")`, `list` SHALL be `false`, `color` SHALL be `false`

#### Scenario: Replay with --list
- **WHEN** `replay --list` is passed
- **THEN** `list` SHALL be `true` and `branch` SHALL be `None`

#### Scenario: Replay with --color
- **WHEN** `replay feat/add-auth --color` is passed
- **THEN** `color` SHALL be `true`

#### Scenario: Replay with --session
- **WHEN** `replay feat/add-auth --session paw-myproject` is passed
- **THEN** `session` SHALL be `Some("paw-myproject")`

#### Scenario: Replay with no arguments and no --list
- **WHEN** `replay` is passed with no arguments and no `--list`
- **THEN** parsing SHALL fail with an error indicating either a branch or `--list` is required

#### Scenario: Replay help text
- **WHEN** `replay --help` is passed
- **THEN** stdout SHALL contain descriptions of `--list`, `--color`, and `--session` flags with examples
