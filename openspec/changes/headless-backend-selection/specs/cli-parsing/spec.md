## ADDED Requirements

### Requirement: --backend flag selects the terminal backend of a new session

The `start` subcommand SHALL accept `--backend <BACKEND>` whose accepted values are exactly `tmux` and `headless`; any other value SHALL be rejected by argument parsing with exit code 2 before any side effect. The effective backend of a new session SHALL resolve as: `--backend` when given, else the configuration's `default_backend`, else `tmux`. The flag SHALL apply only to creating a session: for a repository that already has a session, a `--backend` naming a different backend than the session's recorded one SHALL fail with exit code 1. The flag SHALL appear in `git paw start --help` with a description naming both values, stating that `tmux` is the default, and pointing to the headless chapter, and the `start` long-about examples SHALL include one headless example.

#### Scenario: The flag parses both values

- **WHEN** `git paw start --backend headless` and `git paw start --backend tmux` are parsed
- **THEN** the parsed start arguments SHALL carry the headless and tmux backends respectively

#### Scenario: An unknown value is a parse error

- **WHEN** `git paw start --backend zellij` runs
- **THEN** it SHALL exit with code 2
- **AND** stderr SHALL name the accepted values `tmux` and `headless`
- **AND** no worktree, session file or terminal session SHALL be created

#### Scenario: Absent flag and key mean tmux

- **GIVEN** a configuration with no `default_backend`
- **WHEN** `git paw start` creates a session without `--backend`
- **THEN** the session SHALL be created on the tmux backend exactly as before this requirement

#### Scenario: Help documents the flag

- **WHEN** `git paw start --help` is run
- **THEN** the output SHALL contain `--backend`, `tmux` and `headless`

## MODIFIED Requirements

### Requirement: Attach subcommand

The `attach` subcommand SHALL reattach the current terminal to the running session for the current repository, resolving the session by the repository path (as `status` does) and acting through the session's recorded terminal backend. For a session on the tmux backend it SHALL invoke the tmux attach path. For a session on the headless backend it SHALL follow every worker's output read-only, each line prefixed with the worker's label, until interrupted, and SHALL then exit without affecting any worker. When no session exists for the repository, or the session is not alive, it SHALL exit with an actionable error naming that no session is running and how to start one. This provides a first-class `git paw attach` in place of the raw `tmux attach -t paw-<project>`.

#### Scenario: Attach reattaches to the repository's running session

- **GIVEN** a running tmux-backed session for the current repository
- **WHEN** the user runs `git paw attach`
- **THEN** the system SHALL attach the terminal to that session

#### Scenario: Attach follows a headless session's output

- **GIVEN** a running headless session for the current repository whose worker `feat-a` prints `building` during a turn
- **WHEN** the user runs `git paw attach`
- **THEN** the output SHALL include a line containing `feat-a` and `building`
- **AND** interrupting `attach` SHALL leave the worker's turn running

#### Scenario: Attach with no running session errors actionably

- **GIVEN** no running session exists for the current repository
- **WHEN** the user runs `git paw attach`
- **THEN** the system SHALL exit with an error stating no session is running and how to start one
