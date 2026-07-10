# broker-roster-hygiene Specification

## Purpose
Keeps the broker `/status` roster honest: a row appears only once a pane actually publishes `agent.status` — never from the identity fields of feedback/question/verified messages, and never for a seeded-but-unpublished pane — so no phantom rows appear and none survive a restart. It also specifies that each row's CLI column is filled authoritatively from the launcher-known value (session JSON or `[supervisor].cli`), so a self-reported guess never clobbers it and the column is never blank.

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

