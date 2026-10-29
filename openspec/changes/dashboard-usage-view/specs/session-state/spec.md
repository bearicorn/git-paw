## ADDED Requirements

### Requirement: Session records headless orchestrator invocations

When the drive loop starts a headless orchestrator invocation, it SHALL append a record of it to the session file through the session lock. The record SHALL carry the invocation's CLI id and the CLI session id assigned to it, or no session id when the CLI has no session-id template. The list SHALL be an additive field, annotated `#[serde(default, skip_serializing_if = "Vec::is_empty")]`, so session files written earlier load unchanged and a session without invocations serializes as before. Restart from pause and recovery SHALL keep the list; it SHALL be deleted together with the session. An invocation recorded without a session id SHALL count as unknown usage.

#### Scenario: An invocation is recorded at spawn

- **GIVEN** an unattended drive loop whose orchestrator CLI has a session-id template
- **WHEN** it starts a headless invocation
- **THEN** the session file SHALL list that invocation with its CLI id and assigned session id before the invocation exits

#### Scenario: Earlier session files load unchanged

- **GIVEN** a session file written before this field existed
- **WHEN** it is loaded
- **THEN** loading SHALL succeed with an empty invocation list
- **AND** saving it back SHALL produce no invocation field

#### Scenario: An invocation without a session id counts as unknown

- **GIVEN** an orchestrator CLI with no session-id template
- **WHEN** the drive loop starts a headless invocation
- **THEN** the session file SHALL record it with its CLI id and no session id
- **AND** usage readers SHALL report it as one invocation with unknown usage

### Requirement: Status JSON reports orchestrator invocation usage

`git paw status --json` SHALL include a top-level `orchestrator_usage` field, which is `null` when the session records no headless orchestrator invocation. Otherwise it SHALL be an object carrying:

- the number of recorded invocations;
- the summed known token totals, and cost when a report carries one;
- the number of invocations whose usage is unknown.

The figures SHALL come from the same attributed-usage query as each worktree's `usage`. `usage_total` SHALL keep covering workers only. The field SHALL be additive, and the human-readable output SHALL NOT change because of it.

#### Scenario: Recorded invocations are summed

- **GIVEN** a session recording two invocations with known usage of 30 in / 5 out and 10 in / 5 out
- **WHEN** `git paw status --json` is run
- **THEN** `orchestrator_usage` SHALL report 2 invocations, 40 input and 10 output tokens, and 0 unknown
- **AND** `usage_total` SHALL be unchanged by them

#### Scenario: No invocations yields null

- **GIVEN** a session recording no invocation
- **WHEN** `git paw status --json` is run
- **THEN** `orchestrator_usage` SHALL be `null`

### Requirement: Status shows each agent's broker-reported state

When the session's broker answers, `git paw status` SHALL show three things on each worktree line: the agent's broker-reported phase (or its status when it reports no phase), how long ago it last reported, and its pane index. `git paw status --json` SHALL add an `agent` field to each worktree object. The field SHALL be an object with `status`, `phase` (or `null`), `last_seen_seconds` and `pane_index` (or `null` when the pane cannot be resolved), or `null` when the broker has no record of that agent or does not answer. The broker request SHALL use the bounded HTTP timeout. When the broker does not answer, the human worktree lines SHALL be printed exactly as they were before this requirement.

#### Scenario: A live broker adds the agent state

- **GIVEN** an active session whose broker reports agent `feat-a` with status `working` and phase `running tests`, last seen 40 seconds ago, in pane 1
- **WHEN** `git paw status` is run
- **THEN** the `feat/a` worktree line SHALL contain `running tests`, `40s ago` and pane `1`

#### Scenario: JSON carries the agent object

- **GIVEN** the same session
- **WHEN** `git paw status --json` is run
- **THEN** the `feat/a` worktree object SHALL contain `agent` with `status` `working`, `phase` `running tests`, `last_seen_seconds` 40 and `pane_index` 1

#### Scenario: An unreachable broker leaves the lines unchanged

- **GIVEN** an active session whose broker does not answer
- **WHEN** `git paw status` and `git paw status --json` are run
- **THEN** the human worktree lines SHALL be identical to the output before this requirement
- **AND** every worktree object SHALL contain `"agent": null`

## MODIFIED Requirements

### Requirement: Dashboard pane index persisted in session state

The `Session` struct SHALL include an optional field `dashboard_pane: Option<u32>` that records the pane index of the dashboard pane within the tmux session. The field SHALL use `#[serde(default, skip_serializing_if = "Option::is_none")]` so sessions saved without it load with `None`. Every start flow that creates a dashboard pane SHALL populate the field with `Some(0)`, in bare and coordinated sessions alike. A start flow that creates no dashboard pane SHALL leave it absent, including when the broker runs.

A session *has a dashboard* when it records `dashboard_pane`, or when it records no pane layout (it was saved before the layout record existed) and it recorded a broker.

Consumers that need the first coding-agent pane index or the dashboard index SHALL derive them from these records, which read the RECORDED `dashboard_pane`. Those consumers are `status`, `stop`, `pause`, restart from pause, `add`, `remove`, liveness, recovery, and the keystroke senders: the drive loop's sweep (its nudge, stale-input and buffered-input submit keystrokes) and the explicit `sweep.sh approve` action, through the discovery roster it reads. When `dashboard_pane` is `Some(n)`, the dashboard is pane `n` and the first coding agent is pane `n + 1`. When it is absent and the session records its pane layout, the session has no dashboard and the first coding agent is pane `0`. A session saved by a pre-relocation binary with `dashboard_pane = Some(1)` (supervisor at 0, dashboard at 1, agents from 2) therefore keeps resolving its agent panes from index 2 while it is still alive after an upgrade. When the field is absent and the session records no pane layout, the earlier (pre-relocation) rules SHALL apply, so live sessions saved by older binaries keep resolving: a supervised session's coding agents start at pane 2, a bare session's at pane 1 when it recorded a broker and pane 0 when it did not, and the dashboard index defaults to `0` for pause and restart from pause. `SessionMode` SHALL be consulted only when the session records neither `dashboard_pane` nor a pane layout, and no consumer SHALL derive the index from the broker fields alone.

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

#### Scenario: A launch with a dashboard records it at pane 0

- **GIVEN** `[broker] enabled = true`, `[dashboard] enabled = true` and a coordinated launch
- **WHEN** the session state is saved
- **THEN** `dashboard_pane` SHALL be `Some(0)`
- **AND** the first recorded coding agent's `pane_index` in the per-repo discovery file SHALL be `1`

#### Scenario: A broker session without a dashboard starts agents at pane 0

- **GIVEN** `[broker] enabled = true` and no dashboard enabled
- **WHEN** the session state is saved
- **THEN** `dashboard_pane` SHALL be absent and the session SHALL record its pane layout
- **AND** the first recorded coding agent's `pane_index` SHALL be `0`

#### Scenario: A session saved before the layout record keeps its implied dashboard

- **GIVEN** a saved bare session with `broker_port = Some(9119)`, no `dashboard_pane` and no layout record
- **WHEN** `git paw status` resolves its agent panes
- **THEN** it SHALL treat pane 0 as the dashboard and pane 1 as the first coding-agent pane

#### Scenario: A pre-relocation live session keeps its agent offset

- **GIVEN** a saved session with `mode = Supervisor`, `dashboard_pane = Some(1)` and broker enabled
- **WHEN** `git paw status` resolves the session's agent panes
- **THEN** it SHALL treat pane 2 as the first coding-agent pane
- **AND** neither the drive loop's sweep nor `sweep.sh approve` SHALL treat pane 0 or pane 1 of that session as a coding agent

#### Scenario: A session without dashboard_pane follows the earlier rules

- **GIVEN** a live saved session with `mode = Supervisor`, `dashboard_pane = None`, no layout record and no broker (a broker-less supervised session from a pre-relocation binary)
- **WHEN** `git paw status` resolves the session's agent panes
- **THEN** it SHALL treat pane 2 as the first coding-agent pane
- **AND** a saved bare session with `dashboard_pane = None` and no layout record SHALL resolve its first coding agent at pane 1 when it recorded a broker and at pane 0 when it did not

### Requirement: Recovery data survives tmux crashes

After a tmux crash, the persisted session SHALL contain all data needed to reconstruct the session, including whether it has a dashboard.

#### Scenario: Crashed session has all recovery data including broker fields
- **GIVEN** a saved session with worktrees and broker enabled
- **WHEN** tmux crashes and the session is loaded from disk
- **THEN** it SHALL have the session name, repo path, all worktree details, AND broker_port, broker_bind, broker_log_path, and `dashboard_pane` when it had a dashboard

#### Scenario: Session recovery recreates the broker host and the recorded dashboard pane
- **GIVEN** a saved session with `broker_port = Some(9119)`, `broker_bind = Some("127.0.0.1")` and `dashboard_pane = Some(0)`
- **WHEN** `recover_session()` is called
- **THEN** the rebuilt tmux session SHALL have:
  - A detached `broker` window running `git-paw __broker`
  - Dashboard pane in pane 0 running `git-paw __dashboard`
  - `GIT_PAW_BROKER_URL` environment variable set to `http://127.0.0.1:9119`
  - All original worktree panes in subsequent indices

#### Scenario: Session recovery of a broker session without a dashboard
- **GIVEN** a saved session with `broker_port = Some(9119)`, a recorded pane layout and no `dashboard_pane`
- **WHEN** `recover_session()` is called
- **THEN** the rebuilt tmux session SHALL have the `broker` window and `GIT_PAW_BROKER_URL`
- **AND** no pane SHALL run `git-paw __dashboard` and the worktree panes SHALL start at pane 0

#### Scenario: Session recovery uses the recorded session, not current config
- **GIVEN** a saved session with `broker_port = Some(9119)` and `dashboard_pane = Some(0)`
- **AND** current repo config has `broker.enabled = false` and `[dashboard] enabled = false`
- **WHEN** `recover_session()` is called
- **THEN** the broker host and dashboard pane SHALL still be created with the original broker URL

#### Scenario: Session recovery without original broker creates no dashboard
- **GIVEN** a saved session with `broker_port = None`
- **WHEN** `recover_session()` is called
- **THEN** no broker host and no dashboard pane SHALL be created

### Requirement: Recovery rebuilds exactly the session's panes

Recovering a stopped session SHALL rebuild in window 0 exactly the panes the session defines: the dashboard pane when the session has a dashboard, plus one pane per recorded worktree. That is `N + 1` panes with a dashboard and `N` without one. The recovery SHALL NOT create more panes in window 0 than the session defines and SHALL NOT create an orchestrator pane, including when the stopped session was saved with an earlier layout. The detached `broker` service window is recreated separately (`broker-runtime`) and is not counted.

#### Scenario: Recovery of an N-worktree session with a dashboard creates N+1 panes

- **GIVEN** a stopped coordinated session with a dashboard and 3 recorded worktrees
- **WHEN** `git paw start` recovers it
- **THEN** window 0 of the rebuilt tmux session SHALL have exactly 4 panes (dashboard + 3 agents)

#### Scenario: Recovery of an N-worktree session without a dashboard creates N panes

- **GIVEN** a stopped coordinated session without a dashboard and 3 recorded worktrees
- **WHEN** `git paw start` recovers it
- **THEN** window 0 of the rebuilt tmux session SHALL have exactly 3 panes, the agents at panes 0-2

#### Scenario: Recovery of a pre-relocation session rebuilds the new layout

- **GIVEN** a stopped supervised session saved with `dashboard_pane = Some(1)` and 2 recorded worktrees
- **WHEN** `git paw start` recovers it
- **THEN** window 0 of the rebuilt session SHALL have exactly 3 panes with the dashboard at pane 0
- **AND** the re-saved session SHALL record `dashboard_pane = Some(0)`

#### Scenario: Recovery that cannot fit fails cleanly

- **GIVEN** a recovery whose panes cannot be tiled in the available canvas
- **WHEN** the rebuild fails
- **THEN** it SHALL NOT leave a half-tiled session and SHALL NOT mutate the persisted session state
