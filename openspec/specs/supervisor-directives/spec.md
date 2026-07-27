# supervisor-directives Specification

## Purpose
Provides supervisor-pane directives for coordinating agents. `/agents` returns the current agent inventory (each row carrying branch_id, status, last_seen, cli, best-effort mode, and path-resolved pane_index), composed from broker `/status` and `tmux list-panes`, cached in memory with a refresh cadence and exposed as a reusable `coordination::inventory` library helper with target validation. `/tell <agent_id> <prompt>` validates the target against that inventory and routes a prompt to the agent via a selectable delivery mode (`agent.feedback` or `tmux send-keys`), recording each route in learnings, requiring user confirmation for proactive routes, and never invoking another CLI to generate the prompt.

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

### Requirement: /tell routing command in the supervisor pane

The supervisor SHALL recognise a `/tell <agent_id> <prompt>`
directive typed in its own tmux pane and route the prompt to the
named agent. The directive SHALL be parseable with the agent
identifier as the first whitespace-delimited token after `/tell`
and the prompt as the remainder of the line (or multi-line
content).

#### Scenario: Successful tell to a live agent

- **GIVEN** an active session with agent `feat/auth`
- **WHEN** the user types `/tell feat/auth rebase onto main`
  in the supervisor pane
- **THEN** the supervisor SHALL deliver `rebase onto main` to
  the `feat/auth` agent via the configured delivery mode and
  acknowledge the routing in its own pane

#### Scenario: Tell with multi-line prompt

- **WHEN** the user types `/tell feat/auth` followed on
  subsequent lines by multi-line content
- **THEN** the supervisor SHALL parse the entire content block
  as the prompt and route it whole

### Requirement: Target validation against the inventory

`/tell` SHALL validate the target agent identifier against the
[[supervisor-agent-inventory]] cache. Unknown identifiers SHALL
NOT be delivered; the supervisor SHALL respond in its own pane
with the candidate-list error from the shared validation helper.

#### Scenario: Unknown target produces a candidate list

- **GIVEN** an inventory with agents `feat/a` and `feat/b`
- **WHEN** the user types `/tell feat/ghost ...`
- **THEN** the supervisor SHALL NOT deliver anything and SHALL
  respond with a message listing `feat/a` and `feat/b` as the
  available targets

### Requirement: Delivery mode selection

`/tell` SHALL select a delivery mode using this precedence:
1. When `[supervisor.tell] mode = "send-keys"` is configured AND
   the target's detected mode is `accept-edits`, use
   `tmux send-keys` to inject the prompt directly into the
   target's pane.
2. When `[supervisor.tell] mode = "feedback"` (default), publish
   an `agent.feedback` broker message targeted at the agent.
3. When the configured mode is `send-keys` but the target's
   detected mode is `interactive` or `unknown`, fall back to
   `agent.feedback` and emit a stderr note explaining the
   fallback.

#### Scenario: Default delivery uses agent.feedback

- **GIVEN** no `[supervisor.tell] mode` setting (default)
- **WHEN** `/tell feat/auth rebase onto main` runs
- **THEN** the supervisor SHALL publish an `agent.feedback`
  broker message targeted at `feat/auth` carrying the prompt

#### Scenario: send-keys mode targets accept-edits agents

- **GIVEN** `[supervisor.tell] mode = "send-keys"` and an agent
  whose detected mode is `accept-edits`
- **WHEN** `/tell` targets that agent
- **THEN** the supervisor SHALL use `tmux send-keys` to inject
  the prompt into the agent's pane

#### Scenario: send-keys mode falls back when target mode is unknown

- **GIVEN** `[supervisor.tell] mode = "send-keys"` and an agent
  whose detected mode is `unknown`
- **WHEN** `/tell` targets that agent
- **THEN** the supervisor SHALL fall back to `agent.feedback`
  delivery and SHALL emit a stderr-side note explaining the
  fallback

### Requirement: Routing-decision recording

Every `/tell` invocation SHALL append an entry to a "Supervisor
routing" section of `.git-paw/session-learnings.md` when
`[supervisor] learnings = true`. Each entry SHALL include the
ISO timestamp, target agent, delivery mode, and the prompt
(truncated with `…` past 200 chars). When learnings mode is
disabled the system SHALL NOT write to the learnings file.

#### Scenario: Tell recorded in learnings

- **GIVEN** an active session with `learnings = true`
- **WHEN** `/tell feat/auth rebase onto main` runs
- **THEN** `.git-paw/session-learnings.md` SHALL contain a new
  entry in the "Supervisor routing" section with the
  timestamp, `feat/auth`, the delivery mode, and the prompt

#### Scenario: Learnings disabled means no recording

- **GIVEN** `learnings = false` or no `[supervisor]` config
- **WHEN** `/tell` runs successfully
- **THEN** no file SHALL be written under `.git-paw/`

### Requirement: Proactive routing requires user confirmation

The supervisor SHALL NOT invoke `/tell` autonomously. When the
supervisor identifies a candidate route (an agent blocked on a
question the user has implicitly addressed earlier), the supervisor
SHALL publish an `agent.question` in its own pane describing the
proposed routing and SHALL wait for explicit user confirmation
(e.g. `y`) before invoking `/tell`. No proactive route SHALL
execute without an affirmative reply in v0.6.0.

#### Scenario: Proactive route is offered, not auto-executed

- **GIVEN** a supervisor sweep detects agent `feat/auth` is
  blocked on a layout question the user has previously
  addressed
- **WHEN** the sweep completes
- **THEN** the supervisor SHALL post a question in its own
  pane offering the route, and SHALL NOT invoke `/tell` until
  the user replies affirmatively

#### Scenario: User declines proactive route

- **GIVEN** a proactive-route prompt awaiting confirmation
- **WHEN** the user replies with `n` (or anything other than the
  affirmative)
- **THEN** the supervisor SHALL NOT invoke `/tell` and SHALL
  drop the proposed route

### Requirement: No agent CLI invoked as inference backend

The `/tell` skill SHALL NOT invoke any agent CLI to generate the
prompt content. The prompt comes from the user (typed in the
supervisor pane) or from supervisor LLM reasoning over the
session context; it SHALL NOT be obtained by piping a question
into another agent CLI.

#### Scenario: No inference-backend invocation in the tell path

- **WHEN** `/tell` runs
- **THEN** the operation SHALL consist solely of (a) reading the
  user-typed prompt, (b) inventory lookup, and (c) the chosen
  delivery (broker publish OR `tmux send-keys`), with no agent
  CLI process spawned to produce the prompt content
