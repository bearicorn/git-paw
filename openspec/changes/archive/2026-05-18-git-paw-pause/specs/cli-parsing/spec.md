## ADDED Requirements

### Requirement: Pause subcommand

The `pause` subcommand SHALL parse with no additional arguments and SHALL be visible in `git paw --help` output. The subcommand SHALL include an `about` string ("Pause the session (detaches client, stops broker, leaves CLIs running)") and a `long_about` string that names the RAM trade-off and points the reader at `stop` and (forthcoming v1.0.0) `hibernate` for the destructive and RAM-free alternatives respectively.

The pause subcommand SHALL appear in the root `after_help` quick-start guide alongside `start`, `stop`, and `purge`.

#### Scenario: Pause parses

- **GIVEN** `pause` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Pause`

#### Scenario: Pause accepts no flags

- **GIVEN** `pause --anything` is passed (any flag)
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an unknown-argument error

#### Scenario: Pause appears in help

- **WHEN** `git paw --help` is run
- **THEN** the output SHALL list a `pause` subcommand
- **AND** the output SHALL include the `pause` line in the quick-start `after_help` block

#### Scenario: Pause help text names the RAM trade-off

- **WHEN** `git paw pause --help` is run
- **THEN** the output SHALL mention that CLI processes remain running
- **AND** the output SHALL mention the RAM-allocation trade-off (or words conveying "RAM stays held")
- **AND** the output SHALL suggest `git paw stop` for the RAM-releasing alternative

## MODIFIED Requirements

### Requirement: Stop subcommand

The `stop` subcommand SHALL accept an optional `--force` flag (boolean, defaults to `false`). When `--force` is omitted AND stdin is a TTY, `cmd_stop` SHALL render an interactive confirmation prompt describing the destructive nature of stop and pointing at `git paw pause` (soft alternative) and `git paw purge` (full reset). When `--force` is set OR stdin is not a TTY, the prompt SHALL be skipped and the stop SHALL proceed immediately.

The `long_about` help text for `stop` SHALL name all three teardown verbs (`pause`, `stop`, `purge`) with a one-line summary of each, so users can choose the right verb at `--help` time.

#### Scenario: Stop parses without flags

- **GIVEN** `stop` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Stop { force: false }`

#### Scenario: Stop parses with --force

- **GIVEN** `stop --force` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Stop { force: true }`

#### Scenario: Stop help names all three teardown verbs

- **WHEN** `git paw stop --help` is run
- **THEN** the output SHALL mention `pause` as the soft alternative
- **AND** the output SHALL mention `purge` as the full reset
- **AND** the output SHALL describe what `stop` itself does (kills CLI processes, preserves worktrees)

#### Scenario: Stop with --force from a TTY skips the prompt

- **GIVEN** an active session and `--force` is passed
- **WHEN** `git paw stop --force` is run with stdin attached to a TTY
- **THEN** no interactive prompt SHALL be rendered
- **AND** the session SHALL be killed immediately
