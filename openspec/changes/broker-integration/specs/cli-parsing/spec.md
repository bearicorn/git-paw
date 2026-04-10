## MODIFIED Requirements

### Requirement: Default to start when no subcommand is given

The system SHALL treat no arguments as equivalent to `start` with no flags.

The system SHALL also accept a hidden `__dashboard` subcommand that does not appear in `--help` output. This subcommand is used internally by pane 0 to run the broker and dashboard.

#### Scenario: No arguments yields None command
- **GIVEN** no arguments are passed
- **WHEN** the CLI is parsed
- **THEN** `command` SHALL be `None` (handled as `Start` in main)

#### Scenario: __dashboard subcommand parses
- **GIVEN** `__dashboard` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Dashboard`

#### Scenario: __dashboard does not appear in help
- **GIVEN** `--help` is passed
- **WHEN** the help text is rendered
- **THEN** the output SHALL NOT contain `__dashboard`
