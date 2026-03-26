## ADDED Requirements

### Requirement: List available log sessions

The system SHALL display available log sessions and their branches when `--list` is passed.

#### Scenario: Sessions available
- **WHEN** `git paw replay --list` is run and log sessions exist
- **THEN** stdout SHALL display each session name, branch count, and branch names with their log filenames

#### Scenario: No sessions available
- **WHEN** `git paw replay --list` is run and no log sessions exist
- **THEN** stdout SHALL display a message indicating no logs are available

### Requirement: Display stripped log output

By default, the system SHALL display log content with ANSI escape codes removed.

#### Scenario: Replay branch log
- **WHEN** `git paw replay <branch>` is run and the log file exists
- **THEN** the log content SHALL be printed to stdout with all ANSI escape codes stripped

#### Scenario: Log contains SGR sequences
- **WHEN** the log file contains `\x1b[31mred text\x1b[0m`
- **THEN** the stripped output SHALL contain `red text` with no escape sequences

#### Scenario: Log contains cursor movement sequences
- **WHEN** the log file contains CSI cursor sequences (`\x1b[H`, `\x1b[2J`, etc.)
- **THEN** the stripped output SHALL not contain those sequences

### Requirement: Display colored log output

When `--color` is passed, the system SHALL display log content with ANSI codes preserved, piped through `less -R`.

#### Scenario: Replay with color
- **WHEN** `git paw replay <branch> --color` is run
- **THEN** the raw log content SHALL be piped through `less -R`

#### Scenario: less not available
- **WHEN** `--color` is passed and `less` is not found on PATH
- **THEN** the raw content SHALL be printed to stdout with a warning that `less` was not found

### Requirement: Default to most recent session

When `--session` is not specified, the system SHALL replay from the most recently modified session.

#### Scenario: Most recent session selected
- **WHEN** `git paw replay <branch>` is run without `--session` and multiple sessions exist
- **THEN** the log SHALL be read from the session directory with the most recent modification time

#### Scenario: Explicit session
- **WHEN** `git paw replay <branch> --session paw-myproject` is run
- **THEN** the log SHALL be read from the `paw-myproject` session directory

#### Scenario: Specified session does not exist
- **WHEN** `--session nonexistent` is passed and no such session directory exists
- **THEN** the system SHALL return an error mentioning the session name and suggesting `--list`

### Requirement: Fuzzy branch matching

The system SHALL match the `<branch>` argument against both the original branch name and the sanitized log filename.

#### Scenario: Match by original branch name
- **WHEN** `git paw replay feat/add-auth` is run and the log file is `feat--add-auth.log`
- **THEN** the log SHALL be found and displayed

#### Scenario: Match by sanitized name
- **WHEN** `git paw replay feat--add-auth` is run
- **THEN** the log SHALL be found and displayed

#### Scenario: No matching branch
- **WHEN** `git paw replay nonexistent` is run and no log matches
- **THEN** the system SHALL return an error listing available branches for the session

### Requirement: ANSI stripping correctness

The ANSI stripper SHALL remove all CSI sequences (starting with `\x1b[`) while preserving all non-escape content.

#### Scenario: Plain text passes through unchanged
- **WHEN** content has no ANSI sequences
- **THEN** the stripped output SHALL be identical to the input

#### Scenario: Multiple sequences in one line
- **WHEN** a line contains `\x1b[1m\x1b[31mBold Red\x1b[0m Normal`
- **THEN** the stripped output SHALL be `Bold Red Normal`

#### Scenario: Incomplete escape sequence at end of input
- **WHEN** content ends with `\x1b[` (incomplete CSI)
- **THEN** the incomplete sequence SHALL be stripped without error

### Requirement: Handle missing or empty logs

The system SHALL handle edge cases gracefully.

#### Scenario: Log file is empty
- **WHEN** `git paw replay <branch>` is run and the log file is empty
- **THEN** nothing SHALL be printed and the command SHALL succeed

#### Scenario: No logs directory
- **WHEN** `git paw replay <branch>` is run and `.git-paw/logs/` does not exist
- **THEN** the system SHALL return an error suggesting logging may not be enabled
