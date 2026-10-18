## MODIFIED Requirements

### Requirement: Default to start when no subcommand is given

The system SHALL treat no arguments as equivalent to `start` with no flags.

The system SHALL also accept the hidden internal subcommands `__broker`, which hosts the session's broker in its detached service surface; `__dashboard`, which renders the dashboard as a thin client of that broker in pane 0; and `__prompt-profile`, which prints one resolved CLI prompt-shape profile field for the bundled helper. None of them SHALL appear in `--help` output.

#### Scenario: No arguments yields None command
- **GIVEN** no arguments are passed
- **WHEN** the CLI is parsed
- **THEN** `command` SHALL be `None` (handled as `Start` in main)

#### Scenario: __dashboard subcommand parses
- **GIVEN** `__dashboard` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Dashboard`

#### Scenario: __broker subcommand parses
- **GIVEN** `__broker` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Broker`

#### Scenario: Hidden subcommands do not appear in help
- **GIVEN** `--help` is passed
- **WHEN** the help text is rendered
- **THEN** the output SHALL NOT contain `__dashboard`, `__broker` or `__prompt-profile`
