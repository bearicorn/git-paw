## MODIFIED Requirements

### Requirement: Stop subcommand

The `stop` subcommand SHALL take no confirmation flag and SHALL NOT prompt for confirmation. Stopping is recoverable — the worktrees, branches and session state survive, and `git paw start` recovers the session — so `stop` SHALL proceed immediately whether or not stdin is a TTY. The `stop` subcommand SHALL NOT accept a `--force` flag; passing one SHALL be rejected by argument parsing as an unexpected argument.

The `long_about` help text for `stop` SHALL name all three teardown verbs (`pause`, `stop`, `purge`) with a one-line summary of each, so users can choose the right verb at `--help` time, and SHALL NOT describe a confirmation prompt or a `--force` flag.

#### Scenario: Stop parses without flags

- **GIVEN** `stop` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Stop`

#### Scenario: Stop rejects --force

- **GIVEN** `stop --force` is passed
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an unexpected-argument error naming `--force`

#### Scenario: Stop help names all three teardown verbs

- **WHEN** `git paw stop --help` is run
- **THEN** the output SHALL mention `pause` as the soft alternative
- **AND** the output SHALL mention `purge` as the full reset
- **AND** the output SHALL describe what `stop` itself does (kills CLI processes, preserves worktrees)
- **AND** the output SHALL NOT mention a confirmation prompt or a `--force` flag

#### Scenario: Stop from a TTY does not prompt

- **GIVEN** an active session and stdin attached to a TTY
- **WHEN** `git paw stop` is run
- **THEN** no interactive prompt SHALL be rendered
- **AND** the session SHALL be stopped immediately
