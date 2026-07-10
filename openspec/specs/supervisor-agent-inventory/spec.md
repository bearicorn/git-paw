# supervisor-agent-inventory Specification

## Purpose
Provides an `/agents` supervisor-pane directive that returns the current agent inventory (each row carrying branch_id, status, last_seen, cli, best-effort mode, and path-resolved pane_index), composed from broker `/status` and `tmux list-panes`, cached in memory with a refresh cadence, and exposed as a reusable `coordination::inventory` library helper with target validation.

## Requirements
### Requirement: /agents inventory command in the supervisor pane

The supervisor SHALL recognise an `/agents` directive typed in its
own tmux pane and respond with the current agent inventory. The
inventory SHALL list every agent registered with the broker plus
the supervisor's own row, each carrying `branch_id`, `status`,
`last_seen`, `cli`, detected `mode`, and `pane_index`.

#### Scenario: User asks for the agent inventory

- **GIVEN** an active supervisor session with N agents
- **WHEN** the user types `/agents` in the supervisor pane
- **THEN** the supervisor SHALL respond with a structured listing
  of the N agents plus itself, each row containing
  `branch_id`, `status`, `last_seen`, `cli`, `mode`, and
  `pane_index`

#### Scenario: Inventory after a mid-session add/remove

- **GIVEN** a session whose agent set has changed via
  [[git-paw-add]]'s add/remove subcommands
- **WHEN** the user types `/agents` after the sweep that
  refreshes the inventory
- **THEN** the listing SHALL reflect the post-change agent set

### Requirement: Inventory sourcing

The inventory SHALL be composed from broker `/status` (for
`branch_id`, `status`, `last_seen`, `cli`) and `tmux list-panes`
with `pane_current_path` (for `pane_index`). The system SHALL NOT
assume tmux pane index ordering matches branch order; it SHALL
resolve via the path mapping.

#### Scenario: Inventory pane_index is path-resolved

- **GIVEN** a session whose branch→pane mapping is non-sequential
  (e.g. after a middle-grid `remove`)
- **WHEN** the inventory is built
- **THEN** each entry's `pane_index` SHALL be derived by matching
  the agent's worktree path against `pane_current_path`, not by
  alphabetical or registration order

### Requirement: Inventory cache and refresh cadence

The supervisor SHALL maintain an in-memory cache of the latest
inventory, refreshed by the existing supervisor sweep (~270s by
default) and on `/tell`/`/agents` invocations when the cache is
older than the configured `[supervisor.tell]
inventory_max_age_seconds` (default 60). The cache SHALL NOT be
persisted to disk; supervisor restarts SHALL produce a fresh
inventory.

#### Scenario: Fresh inventory reused on rapid /agents

- **GIVEN** a supervisor whose inventory was just refreshed
- **WHEN** the user types `/agents` again within the
  max-age threshold
- **THEN** the supervisor SHALL serve the cached inventory
  without re-polling broker

#### Scenario: Stale inventory triggers refresh

- **GIVEN** a supervisor whose inventory is older than the
  configured max-age
- **WHEN** the user types `/agents`
- **THEN** the supervisor SHALL re-poll broker `/status` and
  rebuild the inventory before responding

### Requirement: Mode detection is best-effort with safe fallback

Each inventory entry SHALL include a `mode` field with one of
`accept-edits`, `interactive`, or `unknown`. Detection SHALL use
the agent's tmux pane title and/or recent capture-pane content
heuristics. When the heuristic is inconclusive, the entry SHALL
report `unknown`.

#### Scenario: Mode reported when detectable

- **GIVEN** an agent whose pane clearly indicates accept-edits
  mode (e.g. via pane title or characteristic CLI banner)
- **WHEN** the inventory is built
- **THEN** the entry's `mode` SHALL be `accept-edits`

#### Scenario: Unknown mode when undetectable

- **GIVEN** an agent whose CLI doesn't expose a clear mode signal
- **WHEN** the inventory is built
- **THEN** the entry's `mode` SHALL be `unknown`, and consumers
  (e.g. `/tell`) SHALL treat `unknown` as requiring the safe
  `agent.feedback` delivery mode

### Requirement: Inventory and validation helper is reusable

The inventory query and target validation logic SHALL be
exposed as a reusable library function in
`coordination::inventory` (or equivalent module) rather than
inlined in any single consumer. The helper's API SHALL be
stable enough that future consumers (notably the v1.0.0 MCP
write tools' `publish_agent_feedback`) can adopt it without
re-implementing inventory + validation semantics.

#### Scenario: Helper is callable as a library function

- **WHEN** the codebase is inspected after this change lands
- **THEN** the inventory + validation logic SHALL exist as a
  documented public function in `coordination::inventory`
  with `/tell` as one caller, NOT as a private helper buried
  inside the supervisor-skill code path

#### Scenario: Unknown target produces the documented error shape

- **GIVEN** an inventory with agents `feat/a` and `feat/b`
- **WHEN** the helper is invoked with target `feat/ghost`
- **THEN** the helper SHALL return a rejection containing the
  candidate list `feat/a, feat/b` in a documented error shape
  consumable by any future caller

