## ADDED Requirements

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

### Requirement: List available log sessions

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
