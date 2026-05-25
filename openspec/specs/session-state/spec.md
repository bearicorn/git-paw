## Purpose

Persist session state to disk for recovery after crashes, reboots, or manual stops. Stores one JSON file per session under the XDG data directory, with atomic writes and tmux liveness checks.
## Requirements
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

### Requirement: Load session by name

The system SHALL load a session from disk by name, returning `None` if the file does not exist.

#### Scenario: Loading a nonexistent session returns None
- **GIVEN** no session file exists with the given name
- **WHEN** `load_session()` is called
- **THEN** it SHALL return `Ok(None)`

Test: `session::tests::loading_nonexistent_session_returns_none`

### Requirement: Find session by repository path

The system SHALL scan all session files and return the session matching a given repository path.

#### Scenario: Finds correct session among multiple
- **GIVEN** two sessions for different repositories
- **WHEN** `find_session_for_repo()` is called with one repo path
- **THEN** it SHALL return the matching session

Test: `session::tests::finds_correct_session_among_multiple_by_repo_path`

#### Scenario: No matching session
- **GIVEN** saved sessions for other repositories
- **WHEN** `find_session_for_repo()` is called with a different path
- **THEN** it SHALL return `None`

Test: `session::tests::find_returns_none_when_no_repo_matches`

#### Scenario: No sessions directory
- **GIVEN** no sessions directory exists
- **WHEN** `find_session_for_repo()` is called
- **THEN** it SHALL return `None`

Test: `session::tests::find_returns_none_when_no_sessions_exist`

### Requirement: Delete session by name

The system SHALL delete a session file, succeeding even if the file does not exist (idempotent).

#### Scenario: Deleted session is no longer loadable
- **GIVEN** a saved session
- **WHEN** `delete_session()` is called
- **THEN** `load_session()` SHALL return `None`

Test: `session::tests::deleted_session_is_no_longer_loadable`

#### Scenario: Deleting nonexistent session succeeds
- **GIVEN** no session file with the given name
- **WHEN** `delete_session()` is called
- **THEN** it SHALL return `Ok(())`

Test: `session::tests::deleting_nonexistent_session_succeeds`

### Requirement: Effective status combines file state with tmux liveness

`Session::effective_status(is_tmux_alive)` SHALL combine the persisted `status` field with the result of `is_tmux_alive` to produce the runtime-effective status:

| Recorded status | tmux alive? | Effective status |
|-----------------|-------------|------------------|
| `Active`        | yes         | `Active`         |
| `Active`        | no          | `Stopped`        |
| `Paused`        | yes         | `Paused`         |
| `Paused`        | no          | `Stopped`        |
| `Stopped`       | any         | `Stopped`        |

The rule for `Paused`: tmux must still be alive for the `Paused` state to be valid — pause's whole purpose is to keep tmux + CLI panes running while the client is detached. If tmux died despite a recorded `Paused` state (e.g. tmux server crash), `effective_status` SHALL downgrade to `Stopped`, and `cmd_start` SHALL run the cold-recovery path (fresh CLI spawn) rather than the restart-from-pause path.

#### Scenario: Active + alive remains Active

- **GIVEN** a session with `status = Active`
- **WHEN** `effective_status(|_| true)` is called
- **THEN** it SHALL return `Active`

#### Scenario: Active + dead downgrades to Stopped

- **GIVEN** a session with `status = Active`
- **WHEN** `effective_status(|_| false)` is called
- **THEN** it SHALL return `Stopped`

#### Scenario: Paused + alive remains Paused

- **GIVEN** a session with `status = Paused`
- **WHEN** `effective_status(|_| true)` is called
- **THEN** it SHALL return `Paused`

#### Scenario: Paused + dead downgrades to Stopped

- **GIVEN** a session with `status = Paused`
- **WHEN** `effective_status(|_| false)` is called
- **THEN** it SHALL return `Stopped`

#### Scenario: Stopped remains Stopped regardless of tmux liveness

- **GIVEN** a session with `status = Stopped`
- **WHEN** `effective_status` is called with either liveness result
- **THEN** it SHALL return `Stopped`

### Requirement: SessionStatus display format

The `SessionStatus` enum SHALL display as lowercase strings.

#### Scenario: SessionStatus display strings
- **GIVEN** `SessionStatus::Active` and `SessionStatus::Stopped`
- **WHEN** formatted with `Display`
- **THEN** they SHALL render as `"active"` and `"stopped"`

Test: `session::tests::session_status_displays_as_lowercase_string`

### Requirement: Recovery data survives tmux crashes

After a tmux crash, the persisted session SHALL contain all data needed to reconstruct the session.

#### Scenario: Crashed session has all recovery data including broker fields
- **GIVEN** a saved session with worktrees and broker enabled
- **WHEN** tmux crashes and the session is loaded from disk
- **THEN** it SHALL have the session name, repo path, all worktree details, AND broker_port, broker_bind, broker_log_path

#### Scenario: Session recovery recreates dashboard pane when broker was enabled
- **GIVEN** a saved session with `broker_port = Some(9119)` and `broker_bind = Some("127.0.0.1")`
- **WHEN** `recover_session()` is called
- **THEN** the rebuilt tmux session SHALL have:
  - Dashboard pane in pane 0 running `git-paw __dashboard`
  - `GIT_PAW_BROKER_URL` environment variable set to `http://127.0.0.1:9119`
  - All original worktree panes in subsequent indices

#### Scenario: Session recovery uses original broker config, not current config
- **GIVEN** a saved session with `broker_port = Some(9119)`
- **AND** current repo config has `broker.enabled = false`
- **WHEN** `recover_session()` is called
- **THEN** the dashboard pane SHALL still be created with the original broker URL

#### Scenario: Session recovery without original broker creates no dashboard
- **GIVEN** a saved session with `broker_port = None`
- **WHEN** `recover_session()` is called
- **THEN** no dashboard pane SHALL be created

### Requirement: Session persistence SHALL work through the public API

#### Scenario: Save and load round-trip
- **GIVEN** a session with 2 worktrees
- **WHEN** `save_session_in()` and `load_session_from()` are called
- **THEN** all fields SHALL match

Test: `session_integration::save_and_load_round_trip`

#### Scenario: Find session by repo path
- **GIVEN** a saved session
- **WHEN** `find_session_for_repo_in()` is called with the matching repo path
- **THEN** the correct session SHALL be returned

Test: `session_integration::find_session_by_repo_path`

#### Scenario: Find returns None for unknown repo
- **GIVEN** no matching session
- **WHEN** `find_session_for_repo_in()` is called
- **THEN** it SHALL return `None`

Test: `session_integration::find_session_returns_none_for_unknown_repo`

#### Scenario: Find correct session among multiple
- **GIVEN** two sessions for different repos
- **WHEN** `find_session_for_repo_in()` is called for one
- **THEN** the correct session SHALL be returned

Test: `session_integration::find_correct_session_among_multiple`

#### Scenario: Delete removes session
- **GIVEN** a saved session
- **WHEN** `delete_session_in()` is called
- **THEN** `load_session_from()` SHALL return `None`

Test: `session_integration::delete_removes_session`

#### Scenario: Delete nonexistent is idempotent
- **GIVEN** no session file
- **WHEN** `delete_session_in()` is called
- **THEN** it SHALL succeed

Test: `session_integration::delete_nonexistent_is_idempotent`

#### Scenario: Load nonexistent returns None
- **GIVEN** no session file
- **WHEN** `load_session_from()` is called
- **THEN** it SHALL return `None`

Test: `session_integration::load_nonexistent_returns_none`

#### Scenario: Saving again replaces previous state
- **GIVEN** a saved session
- **WHEN** the status is changed and saved again
- **THEN** the loaded session SHALL have the new status

Test: `session_integration::saving_again_replaces_previous_state`

#### Scenario: Effective status active when tmux alive
- **GIVEN** a session with `Active` status and tmux alive
- **WHEN** `effective_status()` is called
- **THEN** it SHALL return `Active`

Test: `session_integration::effective_status_active_when_tmux_alive`

#### Scenario: Effective status stopped when tmux dead
- **GIVEN** a session with `Active` status and tmux dead
- **WHEN** `effective_status()` is called
- **THEN** it SHALL return `Stopped`

Test: `session_integration::effective_status_stopped_when_tmux_dead`

#### Scenario: Effective status stopped stays stopped
- **GIVEN** a session with `Stopped` status
- **WHEN** `effective_status()` is called
- **THEN** it SHALL return `Stopped` regardless of tmux

Test: `session_integration::effective_status_stopped_stays_stopped`

#### Scenario: Saved session has all recovery fields
- **GIVEN** a saved and reloaded session
- **WHEN** recovery fields are checked
- **THEN** session_name, repo_path, project_name, and all worktree entries SHALL be non-empty

Test: `session_integration::saved_session_has_all_recovery_fields`

### Requirement: Paused session status variant

The `SessionStatus` enum SHALL include a third variant `Paused` (alongside `Active` and `Stopped`). The serde representation SHALL serialize as the lowercase string `"paused"` and SHALL deserialize from the same string. The `Display` implementation SHALL render `Paused` as `"paused"`.

The `Paused` state means: the tmux session is intended to remain alive, all coding-agent CLI panes are intended to remain running, the user's tmux client is detached, and the broker is stopped. Session state files saved by v0.4.0 binaries (which only know `Active` and `Stopped`) SHALL continue to load successfully under v0.5+ binaries — the new variant only appears in files saved by v0.5+.

#### Scenario: Paused status serializes lowercase

- **GIVEN** a `Session` with `status = SessionStatus::Paused`
- **WHEN** `save_session()` is called and the JSON file is inspected
- **THEN** the `"status"` field SHALL be `"paused"`

#### Scenario: Paused status round-trips

- **GIVEN** a `Session` with `status = SessionStatus::Paused` saved to disk
- **WHEN** the session is loaded back via `load_session()`
- **THEN** `status` SHALL be `SessionStatus::Paused`

#### Scenario: v0.4-saved sessions load under v0.5

- **GIVEN** a session JSON file saved by v0.4.0 (only `"active"` or `"stopped"` in the status field)
- **WHEN** the file is loaded by a v0.5+ binary
- **THEN** the load SHALL succeed
- **AND** the `status` field SHALL match the original (`Active` or `Stopped`)

#### Scenario: Paused Display renders lowercase

- **WHEN** `format!("{}", SessionStatus::Paused)` is evaluated
- **THEN** the result SHALL be `"paused"`

### Requirement: Dashboard pane index persisted in session state

The `Session` struct SHALL include an optional field `dashboard_pane: Option<u32>` that records the pane index of the dashboard pane within the tmux session. The field SHALL use `#[serde(default, skip_serializing_if = "Option::is_none")]` so v0.4-saved sessions load with `None`. The field SHALL be populated by the start flow when broker is enabled (typically `0` for bare-start mode and `1` for supervisor mode).

The restart-from-pause flow (specced in the broker-lifecycle delta) SHALL read this field to determine where to re-spawn the dashboard pane. When the field is `None` (v0.4-saved session), the restart flow SHALL default to `0`.

#### Scenario: Dashboard pane index round-trips

- **GIVEN** a `Session` with `dashboard_pane = Some(1)` saved to disk
- **WHEN** the session is loaded back
- **THEN** `dashboard_pane` SHALL be `Some(1)`

#### Scenario: Session without dashboard_pane defaults to None on load

- **GIVEN** a session JSON file with no `dashboard_pane` field
- **WHEN** the session is loaded
- **THEN** `dashboard_pane` SHALL be `None`

#### Scenario: Dashboard pane field is omitted when None

- **GIVEN** a `Session` with `dashboard_pane = None`
- **WHEN** `save_session()` is called and the JSON file is inspected
- **THEN** the JSON SHALL NOT contain a `dashboard_pane` field

