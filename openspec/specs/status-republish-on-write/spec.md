# status-republish-on-write Specification

## Purpose
Has the filesystem watcher republish an agent's status from `committed` back to `working` (once per rate-limited burst) when it observes a worktree file write within a configurable post-commit TTL (default 60s, `0` disabling it), and accepts the `committed → working` transition in the dashboard so continued post-commit activity is reflected.

## Requirements
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

