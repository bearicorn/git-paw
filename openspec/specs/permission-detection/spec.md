# permission-detection Specification

## Purpose
TBD - created by archiving change auto-approve-patterns. Update Purpose after archive.
## Requirements
### Requirement: Permission prompt detection via tmux capture-pane

The system SHALL detect agent CLI permission prompts by capturing pane output and matching it against known prompt patterns.

#### Scenario: Approval prompt detected in working agent

- **GIVEN** a tmux pane running an agent CLI that has produced a permission prompt
- **WHEN** the supervisor polls the pane via `tmux capture-pane -p -t <session>:<pane>`
- **THEN** the system SHALL return `Some(PermissionType::<class>)` when the captured content contains an approval-prompt marker
- **AND** SHALL return `None` when no marker is present

#### Scenario: Detection is non-invasive

- **GIVEN** any agent CLI (claude, aider, codex, etc.)
- **WHEN** detection runs
- **THEN** detection SHALL only read pane output and SHALL NOT modify the agent's process or input

### Requirement: Prompt class identification

The detector SHALL classify each detected prompt into one of a fixed set of permission types so callers can decide whether to auto-approve.

#### Scenario: Curl prompts classified as Curl

- **GIVEN** captured pane content containing both an approval marker and `curl`
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Curl`

#### Scenario: Cargo prompts classified as Cargo

- **GIVEN** captured pane content containing an approval marker and one of `cargo fmt`, `cargo clippy`, `cargo test`, or `cargo build`
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Cargo`

#### Scenario: Unknown prompts classified as Unknown

- **GIVEN** captured pane content containing an approval marker but no recognised command class
- **WHEN** classification runs
- **THEN** the result SHALL be `PermissionType::Unknown`
- **AND** auto-approval SHALL NOT be triggered for `Unknown`

### Requirement: Capture is rate-limited

The detector SHALL NOT capture pane output more often than necessary to avoid load on tmux.

#### Scenario: Capture only on stall

- **GIVEN** an agent whose `last_seen` timestamp has not exceeded the stall threshold
- **WHEN** the supervisor's poll loop runs
- **THEN** detection SHALL NOT call `tmux capture-pane` for that agent

#### Scenario: Capture during stall

- **GIVEN** an agent whose status is `working` but whose `last_seen` is older than the configured stall threshold
- **WHEN** the supervisor's poll loop runs
- **THEN** detection SHALL call `tmux capture-pane` for that pane exactly once per poll tick

