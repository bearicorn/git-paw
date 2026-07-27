# broker-watcher-and-state Specification

## Purpose

This capability keeps the broker's `/status` roster and per-agent state honest as worktrees change. It covers the broker-internal filesystem watcher that polls each mapped worktree's `git status --porcelain` and auto-publishes `agent.status`; roster hygiene (a row appears only once a pane actually publishes `agent.status`, and its CLI column is filled authoritatively from the launcher-known value rather than a self-reported guess); republishing an agent's status from `committed` back to `working` on post-commit file writes within a configurable TTL; and terminal-status stickiness in `update_agent_record` so a completed agent cannot be downgraded by later inferred activity.

## Requirements

### Requirement: Roster populated only from agent.status publishers

The broker SHALL populate the `/status` agent roster only from
agents that publish `agent.status`. A roster row SHALL appear
only once a pane has actually published — a pane whose CLI is
known/seeded but which has not yet published SHALL NOT show a
row (no phantom row for an unstarted or aborted pane, supervisor
included). The system SHALL NOT create or update a roster entry
from the `from` or `target` identity fields of
`agent.feedback`, `agent.question`, or `agent.verified`
messages. Those messages SHALL still be routed and stored.

#### Scenario: Feedback from a non-agent identity creates no roster row

- **GIVEN** a running broker with N agent.status publishers
  registered
- **WHEN** an `agent.feedback` is published with
  `payload.from = "human"` (and `agent_id` of an existing
  publisher or the supervisor)
- **THEN** the roster SHALL still contain exactly the N status
  publishers (plus supervisor) — no `"human"` row is created

#### Scenario: Question/verified identities create no roster rows

- **WHEN** `agent.question` or `agent.verified` messages carry
  `from`/`target`/`verified_by` identities
- **THEN** none of those identities SHALL appear as roster rows
  unless they independently publish `agent.status`

#### Scenario: Feedback is still delivered

- **WHEN** an `agent.feedback` is published targeting an agent
- **THEN** the message SHALL still be routed/stored and
  retrievable by that agent's poll (delivery unaffected by the
  roster gating)

#### Scenario: Seeded-but-unpublished pane shows no row

- **GIVEN** the broker has a known/seeded CLI for a pane (from a
  `WatchTarget` or the supervisor seed) that has not yet
  published any `agent.status`
- **THEN** the `/status` roster SHALL contain no row for that
  pane — the row appears only once the pane publishes, so an
  unstarted or aborted launch leaves no phantom row

### Requirement: Agent CLI populated in the roster

The CLI map SHALL be pre-filled authoritatively at launch from
the value git-paw used to start each pane — coding agents from
their `WatchTarget` (the per-repo session JSON,
`.git-paw/sessions/paw-<project>.json`) at broker start, and the
supervisor (which is not a filesystem watch target) from the
launcher-resolved `[supervisor].cli` falling back to
`default_cli` via the broker-state seed. When a pane publishes
and its roster row appears, that row's CLI column SHALL render
the pre-filled value. The bundled skills SHALL NOT require
agents to self-report their CLI — they would only be guessing.

#### Scenario: Published agent's row carries the seeded CLI

- **GIVEN** a `cli = "claude-oss"` session (the CLI map seeded
  from watch targets at broker start)
- **WHEN** a coding agent publishes an `agent.status` (with no
  `cli` field of its own)
- **THEN** its roster row SHALL show `cli = "claude-oss"`,
  resolved from the authoritative seed, not blank and not a
  self-reported value

#### Scenario: Supervisor CLI seeded authoritatively from config

- **GIVEN** a `[supervisor].cli = "claude-oss"` (or `default_cli`)
  session, the CLI map seeded for `supervisor` at broker start
- **WHEN** the supervisor publishes its bootstrap `agent.status`
  (which carries no `cli` field)
- **THEN** the roster's `supervisor` row SHALL show
  `cli = "claude-oss"` from the seed — without the supervisor
  self-reporting its CLI

#### Scenario: Authoritative seed wins over a wrong self-report

- **GIVEN** the broker has seeded the supervisor's CLI as
  `claude-oss` from config
- **WHEN** the supervisor self-reports a different CLI in its
  `agent.status` (e.g. `cli = "claude"`, a wrong guess from the
  bootstrap placeholder)
- **THEN** the roster SHALL keep the seeded `claude-oss` — a
  self-reported CLI fills the map only when no authoritative
  value was seeded, so a guess never clobbers the launcher-known
  value

#### Scenario: Broker resolves CLI from session JSON when status omits it

- **GIVEN** an agent whose `agent.status` payload has no `cli`
  field, but the per-repo session JSON lists its `cli`
- **WHEN** the roster entry is rendered
- **THEN** the broker SHALL resolve the CLI from the session
  JSON so the row is not blank

#### Scenario: Dashboard CLI column populated for all agents

- **GIVEN** a session with multiple coding agents
- **WHEN** the dashboard renders the agent table
- **THEN** every agent row's CLI column SHALL be populated
  (not just the supervisor row)

#### Scenario: Unknown CLI shows a placeholder, not blank

- **GIVEN** an agent whose CLI cannot be resolved from status
  or session JSON
- **THEN** the CLI column SHALL show a documented
  "unknown" placeholder rather than an empty string

### Requirement: Phantom rows do not survive a broker restart

The broker roster is in-memory; the system SHALL NOT persist
phantom rows. A broker restart SHALL produce a roster built
solely from fresh `agent.status` publishers.

#### Scenario: Restart clears any pre-existing phantom

- **GIVEN** a broker that (under old behaviour) had a phantom
  row
- **WHEN** the broker restarts and agents re-register via
  `agent.status`
- **THEN** the new roster SHALL contain only the real
  status-publishing agents

### Requirement: Watch worktree git state for changes

The broker process SHALL poll each worktree at a fixed interval using `git status --porcelain` and auto-publish `agent.status` when the set of reported paths differs from the previous tick. The `modified_files` field SHALL contain the paths currently reported by git status.

The poll interval SHALL be 2 seconds. The watcher SHALL NOT publish when the snapshot is unchanged from the previous tick.

#### Scenario: File edit triggers status publish

- **GIVEN** a running broker watching worktree `/path/to/git-paw-feat-x`
- **WHEN** a file at `src/lib.rs` is modified in that worktree
- **THEN** within 3 seconds the broker publishes `agent.status` for `feat-x` with `modified_files` containing `src/lib.rs`

#### Scenario: Multiple rapid edits are collapsed into one publish

- **GIVEN** a running broker watching a worktree
- **WHEN** 5 files are modified within a single poll interval
- **THEN** a single `agent.status` is published with all 5 files in `modified_files`

#### Scenario: Build artifacts are excluded via gitignore

- **GIVEN** a running broker watching a worktree whose `.gitignore` lists `target/` and `node_modules/`
- **WHEN** files change in `target/` or `node_modules/`
- **THEN** no `agent.status` is published for those changes

#### Scenario: Unchanged state does not re-publish

- **GIVEN** a running broker that has just published an `agent.status` for a worktree
- **WHEN** the next poll tick runs and `git status --porcelain` output is byte-identical to the previous tick
- **THEN** no new `agent.status` is published

#### Scenario: Watcher stops when broker stops

- **GIVEN** a running broker with active watchers
- **WHEN** the `BrokerHandle` is dropped
- **THEN** all watcher tasks stop within one poll interval

### Requirement: Worktree-to-agent mapping

The broker SHALL accept a list of `WatchTarget { agent_id, worktree_path }` at startup. Each watcher task SHALL publish status for the `agent_id` of its assigned `WatchTarget`.

#### Scenario: Events map to correct agent

- **GIVEN** two watch targets: `feat-a` at `/wt-a/` and `feat-b` at `/wt-b/`
- **WHEN** a file changes in `/wt-a/src/lib.rs`
- **THEN** the status is published for agent `feat-a`, not `feat-b`

### Requirement: Watcher republishes working on post-commit file writes

The filesystem watcher SHALL transition an agent's state from
`committed` back to `working` when it observes a file
modification inside the agent's worktree within a configurable
TTL window after the `agent.artifact status: "committed"`
event. The default TTL SHALL be 60 seconds.

#### Scenario: File write within TTL republishes working

- **GIVEN** an agent that just published `agent.artifact
  status: "committed"` 10 seconds ago
- **WHEN** the watcher observes a file modification inside
  the agent's worktree
- **THEN** the watcher SHALL publish `agent.status:
  working` so dashboard + MCP consumers reflect the agent's
  continued activity

#### Scenario: File write after TTL does NOT republish

- **GIVEN** the same agent 5 minutes after its `committed`
  event
- **WHEN** the watcher observes a file modification
- **THEN** the watcher SHALL NOT auto-republish `working`
  (the agent is considered settled; only an explicit
  `agent.status` publish from the agent itself transitions
  out of `committed`)

#### Scenario: Multiple writes within TTL republish only once

- **GIVEN** an agent within its post-commit TTL window
- **WHEN** the watcher observes a burst of file
  modifications (e.g. ten files in two seconds)
- **THEN** the watcher SHALL publish `agent.status:
  working` exactly once for that burst (rate-limited),
  preserving v0.5.0's watcher rate-limit semantics

### Requirement: TTL configurable via broker.watcher config

The system SHALL accept
`[broker.watcher].republish_working_ttl_seconds` as a numeric
config field defaulting to `60`. Values less than 5 SHALL be
clamped to 5 (matching the v0.5.0 auto-approve threshold
floor pattern) with a stderr warning. Values 0 SHALL be
treated as "disable the auto-republish behaviour."

#### Scenario: Default TTL is 60 seconds

- **GIVEN** no `[broker.watcher]` section in config
- **WHEN** the watcher initialises
- **THEN** the configured TTL SHALL resolve to 60 seconds

#### Scenario: TTL of 0 disables auto-republish

- **GIVEN** `[broker.watcher].republish_working_ttl_seconds
  = 0`
- **WHEN** a post-commit write fires
- **THEN** the watcher SHALL NOT publish a synthetic
  `working` status; v0.5.0 behaviour is preserved

### Requirement: Dashboard accepts committed → working transition

The dashboard state machine SHALL accept `working` as a
valid transition out of `committed` for the supervisor row
and all agent rows. The dashboard SHALL re-render the
agent's row accordingly when the transition fires.

#### Scenario: Dashboard re-renders on the transition

- **GIVEN** an agent row currently displaying `committed`
- **WHEN** a new `agent.status: working` message arrives
  for that agent
- **THEN** the next dashboard frame SHALL show the row as
  `working` (the previous v0.5.0 behaviour of locking on
  `committed` SHALL NOT apply)

### Requirement: Behavioural opt-out preserves v0.5.0 model

The system SHALL provide a behavioural opt-out that restores
v0.5.0's "committed is terminal until explicit republish"
semantics exactly. Setting
`[broker.watcher].republish_working_ttl_seconds = 0` SHALL
disable the auto-republish behaviour entirely.

#### Scenario: Opt-out produces v0.5.0 behaviour

- **GIVEN** the TTL configured to 0
- **WHEN** an agent commits and then continues editing
- **THEN** the watcher SHALL NOT republish `working`; the
  dashboard SHALL display `committed` until the agent
  itself publishes a new `agent.status` (matching v0.5.0
  byte-for-byte)

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
