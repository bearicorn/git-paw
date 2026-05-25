## ADDED Requirements

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

## MODIFIED Requirements

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
