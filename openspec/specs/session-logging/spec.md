# session-logging Specification

## Purpose
Capture raw terminal output from each tmux pane to per-branch log files using tmux pipe-pane, supporting session log directory creation, filename sanitization, and enumeration of available logs, and replay those captured logs — listing available sessions, displaying logs with ANSI codes stripped or preserved, fuzzy branch matching, and automatic selection of the most recent session.

## Requirements

### Requirement: Create session log directory

The system SHALL create a session-specific log directory at `.git-paw/logs/<session-id>/` when a session is launched with logging enabled.

#### Scenario: Log directory created on launch
- **WHEN** a session is launched with `[logging] enabled = true` and session name `paw-myproject`
- **THEN** `.git-paw/logs/paw-myproject/` SHALL be created

#### Scenario: Log directory already exists
- **WHEN** the session log directory already exists
- **THEN** it SHALL be reused without error

#### Scenario: Logging disabled
- **WHEN** a session is launched with `[logging] enabled = false`
- **THEN** no log directory SHALL be created

### Requirement: Derive log file path per pane

The system SHALL derive a log file path for each pane based on the branch name, sanitized for filesystem safety.

#### Scenario: Simple branch name
- **WHEN** a pane is assigned branch `add-auth`
- **THEN** the log path SHALL be `.git-paw/logs/<session-id>/add-auth.log`

#### Scenario: Branch name with slashes
- **WHEN** a pane is assigned branch `feat/add-auth`
- **THEN** the log path SHALL be `.git-paw/logs/<session-id>/feat--add-auth.log`

#### Scenario: Branch name with multiple slashes
- **WHEN** a pane is assigned branch `feat/auth/jwt`
- **THEN** the log path SHALL be `.git-paw/logs/<session-id>/feat--auth--jwt.log`

### Requirement: Attach pipe-pane to capture output

The system SHALL attach `tmux pipe-pane` to each pane to capture terminal output to the pane's log file.

#### Scenario: pipe-pane attached when logging enabled
- **WHEN** logging is enabled and a pane is created
- **THEN** `tmux pipe-pane -o -t <pane> "cat >> <log-path>"` SHALL be executed

#### Scenario: pipe-pane not attached when logging disabled
- **WHEN** logging is disabled
- **THEN** no `pipe-pane` command SHALL be executed

### Requirement: Log files contain raw terminal output

Log files SHALL contain the raw terminal output including ANSI escape codes. No stripping or formatting is applied at capture time.

#### Scenario: Log contains ANSI codes
- **WHEN** an AI CLI outputs colored text to the pane
- **THEN** the log file SHALL contain the raw ANSI escape sequences

### Requirement: List available log sessions (library API)

The system SHALL enumerate session log directories under `.git-paw/logs/`.

#### Scenario: Multiple sessions logged
- **WHEN** `list_log_sessions()` is called and `.git-paw/logs/` contains `paw-myproject` and `paw-other`
- **THEN** it SHALL return both session names

#### Scenario: No log sessions
- **WHEN** `list_log_sessions()` is called and `.git-paw/logs/` is empty
- **THEN** it SHALL return an empty list

#### Scenario: Logs directory does not exist
- **WHEN** `list_log_sessions()` is called and `.git-paw/logs/` does not exist
- **THEN** it SHALL return an empty list (not an error)

### Requirement: List logs for a session

The system SHALL enumerate log files within a session directory.

#### Scenario: Session with multiple logs
- **WHEN** `list_logs_for_session()` is called for a session with 3 log files
- **THEN** it SHALL return 3 `LogEntry` items with branch name and file path

#### Scenario: Session directory empty
- **WHEN** `list_logs_for_session()` is called for a session with no log files
- **THEN** it SHALL return an empty list

#### Scenario: Session directory does not exist
- **WHEN** `list_logs_for_session()` is called for a nonexistent session
- **THEN** it SHALL return `PawError::SessionError` mentioning the session name

### Requirement: LogEntry derives branch from filename

The `LogEntry.branch` SHALL reverse the filename sanitization to recover the original branch name.

#### Scenario: Simple log filename
- **WHEN** a log file is named `add-auth.log`
- **THEN** `LogEntry.branch` SHALL be `"add-auth"`

#### Scenario: Sanitized log filename
- **WHEN** a log file is named `feat--add-auth.log`
- **THEN** `LogEntry.branch` SHALL be `"feat/add-auth"`


### Requirement: List available log sessions (replay --list output)

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

The ANSI stripper SHALL remove all CSI sequences (starting with `\x1b[`) and OSC sequences (`\x1b]...\x07` and `\x1b]...\x1b\\`) while preserving all non-escape content.

#### Scenario: Plain text passes through unchanged
- **WHEN** content has no ANSI sequences
- **THEN** the stripped output SHALL be identical to the input

#### Scenario: Multiple sequences in one line
- **WHEN** a line contains `\x1b[1m\x1b[31mBold Red\x1b[0m Normal`
- **THEN** the stripped output SHALL be `Bold Red Normal`

#### Scenario: OSC sequences stripped
- **WHEN** the log file contains OSC sequences (`\x1b]0;window title\x07` or `\x1b]8;;https://example.com\x1b\\`)
- **THEN** the stripped output SHALL not contain those sequences

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
