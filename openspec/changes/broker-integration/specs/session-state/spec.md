## MODIFIED Requirements

### Requirement: Save session state atomically

The system SHALL serialize session data to JSON and write it atomically using a temp file and rename to prevent corruption.

The session data SHALL include optional broker fields: `broker_port` (`Option<u16>`), `broker_bind` (`Option<String>`), and `broker_log_path` (`Option<PathBuf>`). These fields SHALL be omitted from the JSON when `None` and SHALL default to `None` when absent during deserialization.

#### Scenario: Saved session round-trips with all fields intact
- **GIVEN** an active session with 3 worktrees
- **WHEN** `save_session()` is called and the session is loaded back
- **THEN** all fields (session_name, repo_path, project_name, created_at, status, worktrees) SHALL match the original

#### Scenario: Saved session with broker fields round-trips
- **GIVEN** an active session with `broker_port = Some(9119)`, `broker_bind = Some("127.0.0.1")`, `broker_log_path = Some("/path/to/broker.log")`
- **WHEN** `save_session()` is called and the session is loaded back
- **THEN** all broker fields SHALL match the original

#### Scenario: Session without broker fields loads successfully
- **GIVEN** a session JSON file saved by v0.2.0 (no broker fields)
- **WHEN** the session is loaded
- **THEN** `broker_port`, `broker_bind`, and `broker_log_path` SHALL all be `None`
- **AND** all existing fields SHALL load correctly

#### Scenario: Saving again replaces previous state
- **GIVEN** a previously saved session
- **WHEN** `save_session()` is called with updated fields
- **THEN** the new state SHALL overwrite the old state

### Requirement: Recovery data survives tmux crashes

After a tmux crash, the persisted session SHALL contain all data needed to reconstruct the session.

#### Scenario: Crashed session has all recovery data including broker fields
- **GIVEN** a saved session with worktrees and broker enabled
- **WHEN** tmux crashes and the session is loaded from disk
- **THEN** it SHALL have the session name, repo path, all worktree details, AND broker_port, broker_bind, broker_log_path
