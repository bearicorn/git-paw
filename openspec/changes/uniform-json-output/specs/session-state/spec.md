## ADDED Requirements

### Requirement: Status JSON is a versioned document with terminal-neutral liveness and broker state

`git paw status --json` SHALL follow the `cli-machine-output` contract. When no session exists for the repository it SHALL print `{"schema_version": 1, "session": null}`. Otherwise the document SHALL contain `schema_version`, `session`, `status` (`active`, `paused`, `stopped`, or `stale`), `terminal_alive`, `tmux_running`, `broker`, and `worktrees`. `terminal_alive` SHALL report whether the session's terminal session exists (a session whose agent CLIs have all exited but whose terminal session is still up reports `true`); `tmux_running` SHALL carry the same value and SHALL be retained for compatibility. `broker` SHALL be `null` when the session recorded no broker, and otherwise an object with `url` and `state`, where `state` is one of `running`, `paused`, `not_responding`, or `foreign`, decided by the same broker probe the human output uses. Each `worktrees` entry SHALL keep `branch`, `cli`, `worktree_path`, and `cli_session_id`. The human-readable output SHALL be unchanged.

#### Scenario: No session yields a versioned null document

- **GIVEN** a repository with no session
- **WHEN** `git paw status --json` is run
- **THEN** stdout SHALL be a single object equal to `{"schema_version": 1, "session": null}`

#### Scenario: Liveness is reported under both keys

- **GIVEN** an active session whose terminal session is alive
- **WHEN** `git paw status --json` is run
- **THEN** `terminal_alive` and `tmux_running` SHALL both be `true`

#### Scenario: Exited agents in a live terminal session report alive

- **GIVEN** an active session whose agent CLIs have all exited while its terminal session is still up
- **WHEN** `git paw status --json` is run
- **THEN** `terminal_alive` and `tmux_running` SHALL both be `true`

#### Scenario: A stale receipt reports not alive

- **GIVEN** a session whose receipt says `active` but whose terminal session no longer exists
- **WHEN** `git paw status --json` is run
- **THEN** `status` SHALL be `"stale"` and `terminal_alive` SHALL be `false`

#### Scenario: Broker state is reported

- **GIVEN** an active session that recorded a broker URL and whose broker answers
- **WHEN** `git paw status --json` is run
- **THEN** `broker` SHALL be an object whose `url` is the recorded URL and whose `state` is `"running"`

#### Scenario: A session without a broker reports null

- **GIVEN** a session started with the broker disabled
- **WHEN** `git paw status --json` is run
- **THEN** `broker` SHALL be `null`

#### Scenario: Existing keys are unchanged

- **GIVEN** a session that produced a status JSON before this change
- **WHEN** `git paw status --json` is run after it
- **THEN** `session`, `status`, `tmux_running`, and every `worktrees` entry's keys SHALL carry the same values as before
