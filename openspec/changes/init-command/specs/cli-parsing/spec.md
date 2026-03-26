## MODIFIED Requirements

### Requirement: Help output contains all subcommands and quick start

The `--help` output SHALL list all subcommands and include a Quick Start section.

#### Scenario: Help lists all subcommands
- **GIVEN** `--help` is passed
- **WHEN** the binary runs
- **THEN** stdout SHALL contain start, stop, purge, status, list-clis, add-cli, remove-cli, and init

Test: `cli_tests::help_shows_all_subcommands`

## ADDED Requirements

### Requirement: Init subcommand

The `init` subcommand SHALL parse with no required arguments.

#### Scenario: Init parses
- **WHEN** `init` is passed
- **THEN** the command SHALL be `Command::Init`

#### Scenario: Init help text
- **WHEN** `init --help` is passed
- **THEN** stdout SHALL contain a description of project initialization and examples
