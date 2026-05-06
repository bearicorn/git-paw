# terminal-status-protection Specification

## Purpose
TBD - created by archiving change terminal-status-sticky. Update Purpose after archive.
## Requirements
### Requirement: Terminal status helper

The system SHALL expose an `is_terminal_status` helper that returns `true` for the four terminal status strings (`"done"`, `"verified"`, `"blocked"`, `"committed"`) and `false` for any other status string.

#### Scenario: Helper recognizes terminal statuses

- **WHEN** `is_terminal_status` is invoked with `"done"`, `"verified"`, `"blocked"`, or `"committed"`
- **THEN** the result is `true`

#### Scenario: Helper rejects non-terminal statuses

- **WHEN** `is_terminal_status` is invoked with any string other than the four terminal statuses (e.g. `"working"`, `"idle"`, `"error"`, `""`)
- **THEN** the result is `false`

### Requirement: Terminal status is sticky in update_agent_record

When `update_agent_record` updates an agent record's status, the system SHALL preserve a terminal current status by only overwriting it when the incoming status is also a terminal status. Specifically, the record's status SHALL be updated when (the current status is NOT terminal) OR (the new status IS terminal); otherwise the existing status SHALL remain unchanged. This protection SHALL apply uniformly regardless of which message variant (`agent.status`, `agent.artifact`, `agent.blocked`, etc.) triggered the call, and the protection SHALL silently retain the terminal status without raising an error. The watcher's automatic status updates (e.g. inferring `"working"` from filesystem activity) SHALL therefore NOT downgrade an agent that has already reached a terminal status.

#### Scenario: Terminal status is not overwritten by non-terminal status

- **GIVEN** an agent record with `status = "done"`
- **WHEN** `update_agent_record` is invoked with a new status of `"working"`
- **THEN** the stored status remains `"done"`
- **AND** no error is returned

#### Scenario: Terminal status can be overwritten by another terminal status

- **GIVEN** an agent record with `status = "done"`
- **WHEN** `update_agent_record` is invoked with a new status of `"verified"`
- **THEN** the stored status becomes `"verified"`

#### Scenario: Non-terminal status can be overwritten by terminal status

- **GIVEN** an agent record with `status = "working"`
- **WHEN** `update_agent_record` is invoked with a new status of `"done"`
- **THEN** the stored status becomes `"done"`

#### Scenario: All terminal statuses are protected from non-terminal overwrites

- **GIVEN** agent records with each of the four terminal statuses (`"done"`, `"verified"`, `"blocked"`, `"committed"`)
- **WHEN** each record receives an update attempt with `status = "working"`
- **THEN** every record retains its original terminal status

#### Scenario: Watcher cannot downgrade a terminal status

- **GIVEN** an agent record with `status = "committed"` reached via an `agent.artifact` message
- **WHEN** the filesystem watcher subsequently calls `update_agent_record` with `status = "working"` because of file activity in the worktree
- **THEN** the stored status remains `"committed"`

#### Scenario: Artifact message with non-terminal status does not downgrade

- **GIVEN** an agent record with `status = "verified"`
- **WHEN** an `agent.artifact` message arrives carrying `status = "working"` and is routed through `update_agent_record`
- **THEN** the stored status remains `"verified"`

