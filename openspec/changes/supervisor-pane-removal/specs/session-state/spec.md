## MODIFIED Requirements

### Requirement: Dashboard pane index persisted in session state

The `Session` struct SHALL include an optional field `dashboard_pane: Option<u32>` that records the pane index of the dashboard pane within the tmux session. The field SHALL use `#[serde(default, skip_serializing_if = "Option::is_none")]` so sessions saved without it load with `None`. The field SHALL be populated with `Some(0)` by every supervised start flow (the supervised layout always places the dashboard at pane 0) and by every bare start flow that enables the broker, because the dashboard is pane 0 in both layouts; only a bare start without the broker records `None`.

Consumers that need the first coding-agent pane index or the dashboard index (`status`, `stop`, `pause`, restart from pause, `remove`, liveness, recovery, the drive loop) SHALL derive them from the session's layout record, which reads the RECORDED `dashboard_pane`. When it is `Some(n)`, the dashboard is pane `n` and the first coding agent is pane `n + 1`. A session saved by a pre-relocation binary with `dashboard_pane = Some(1)` (supervisor at 0, dashboard at 1, agents from 2) therefore keeps resolving its agent panes from index 2 while it is still alive after an upgrade. When the field is `None`, the layout record SHALL apply the earlier (pre-relocation) rules, so live sessions saved by older binaries keep resolving: a supervised session's coding agents start at pane 2, a bare session's at pane 1 when it recorded a broker and pane 0 when it did not, and the dashboard index defaults to `0` for pause and restart from pause. `SessionMode` SHALL be consulted only when `dashboard_pane` is `None`.

#### Scenario: Dashboard pane index round-trips

- **GIVEN** a `Session` with `dashboard_pane = Some(0)` saved to disk
- **WHEN** the session is loaded back
- **THEN** `dashboard_pane` SHALL be `Some(0)`

#### Scenario: Session without dashboard_pane defaults to None on load

- **GIVEN** a session JSON file with no `dashboard_pane` field
- **WHEN** the session is loaded
- **THEN** `dashboard_pane` SHALL be `None`

#### Scenario: Dashboard pane field is omitted when None

- **GIVEN** a `Session` with `dashboard_pane = None`
- **WHEN** `save_session()` is called and the JSON file is inspected
- **THEN** the JSON SHALL NOT contain a `dashboard_pane` field

#### Scenario: Supervised start records the dashboard at pane 0

- **GIVEN** `[broker] enabled = true` and a supervised launch
- **WHEN** the session state is saved
- **THEN** `dashboard_pane` SHALL be `Some(0)`
- **AND** the first recorded coding agent's `pane_index` in the per-repo discovery file SHALL be `1`

#### Scenario: A pre-relocation live session keeps its agent offset

- **GIVEN** a saved session with `mode = Supervisor`, `dashboard_pane = Some(1)` and broker enabled
- **WHEN** `git paw status` resolves the session's agent panes
- **THEN** it SHALL treat pane 2 as the first coding-agent pane
- **AND** no keystroke sender (the drive loop, or `sweep.sh approve`) SHALL treat pane 0 or pane 1 of that session as a coding agent

#### Scenario: A session without dashboard_pane follows the earlier rules

- **GIVEN** a live saved session with `mode = Supervisor`, `dashboard_pane = None` and no broker (a broker-less supervised session from a pre-relocation binary)
- **WHEN** `git paw status` resolves the session's agent panes
- **THEN** it SHALL treat pane 2 as the first coding-agent pane
- **AND** a saved bare session with `dashboard_pane = None` SHALL resolve its first coding agent at pane 1 when it recorded a broker and at pane 0 when it did not

#### Scenario: A broker-less supervised start still records the dashboard

- **GIVEN** `[broker] enabled = false` and a supervised launch
- **WHEN** the session state is saved
- **THEN** `dashboard_pane` SHALL be `Some(0)`

### Requirement: Recovery rebuilds exactly the session's panes

Recovering a stopped session SHALL rebuild exactly the panes the session defines — in a supervised session, the dashboard pane plus one pane per recorded worktree (`N + 1`). The recovery SHALL NOT create more panes than the session defines and SHALL NOT create an orchestrator pane, including when the stopped session was saved with an earlier layout.

#### Scenario: Recovery of an N-worktree supervised session creates N+1 panes

- **GIVEN** a stopped supervised session with 3 recorded worktrees
- **WHEN** `git paw start` recovers it
- **THEN** the rebuilt tmux session SHALL have exactly 4 panes (dashboard + 3 agents)

#### Scenario: Recovery of a pre-relocation session rebuilds the new layout

- **GIVEN** a stopped supervised session saved with `dashboard_pane = Some(1)` and 2 recorded worktrees
- **WHEN** `git paw start` recovers it
- **THEN** the rebuilt session SHALL have exactly 3 panes with the dashboard at pane 0
- **AND** the re-saved session SHALL record `dashboard_pane = Some(0)`

#### Scenario: Recovery that cannot fit fails cleanly

- **GIVEN** a recovery whose panes cannot be tiled in the available canvas
- **WHEN** the rebuild fails
- **THEN** it SHALL NOT leave a half-tiled session and SHALL NOT mutate the persisted session state

### Requirement: Per-repo JSON shape matches sweep.sh expectations

The per-repo `paw-<project>.json` SHALL include the session name plus an agent list, each entry carrying `branch_id`, `worktree_path`, `cli`, and `pane_index`. The shape SHALL match what the bundled `assets/scripts/sweep.sh` helper reads — invoked from the orchestrator's own session at the repository root, outside the tmux session — so the helper works against the file without modification. The agent list SHALL list coding agents only.

#### Scenario: sweep.sh discovers agents via the per-repo JSON

- **GIVEN** a freshly-started session and the bundled `sweep.sh` helper invoked from a shell at the repository root that is not attached to the tmux session
- **WHEN** sweep.sh reads `.git-paw/sessions/paw-<project>.json`
- **THEN** the helper SHALL enumerate the full agent list with branch_id, worktree path, CLI, and pane index — no fields missing or renamed relative to its expectations

#### Scenario: Adding a field is backwards-compatible

- **WHEN** a future change adds a new field to the per-repo JSON
- **THEN** the current `sweep.sh` SHALL still find every documented field; unknown extra fields SHALL be ignored

## REMOVED Requirements

### Requirement: Session file records the orchestrator pane

**Reason**: There is no orchestrator pane inside the session; the orchestrator drives from outside tmux.

**Migration**: `RepoSessionFile.orchestrator` keeps its `#[serde(default)]` so discovery files written by earlier versions still load; new files never carry the key. `sweep.sh` never read it. The roster's `supervisor` row comes from the broker when the external orchestrator publishes its status, and the agent inventory reports it with no pane.
