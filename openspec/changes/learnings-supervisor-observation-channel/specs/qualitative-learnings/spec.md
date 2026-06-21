## ADDED Requirements

### Requirement: Bundled `sweep.sh learn` subcommand

The bundled `sweep.sh` helper SHALL provide a `learn <category> <title>
<body-json>` subcommand that publishes an `agent.learning` broker message
with `agent_id = "supervisor"`. The subcommand SHALL reuse the helper's
existing broker-URL discovery (`.git-paw/config.toml [broker]`, default
`127.0.0.1:9119`) and its internal `publish()` path. The supervisor skill
SHALL NOT hand-roll a raw `curl …/publish` call to emit `agent.learning`.

The subcommand SHALL pass the `<category>` and `<title>` arguments and the
`<body-json>` argument through to the `agent.learning` payload's `category`,
`title`, and `body` fields respectively, leaving the body shape to the
caller (the skill documents the per-category body).

#### Scenario: learn publishes an agent.learning through the helper

- **GIVEN** a running broker reachable via the helper's URL discovery
- **WHEN** `sweep.sh learn tooling_friction "Commit step re-prompts every sweep" '{"friction":"git commit re-prompts","occurrences":3,"suggestion":"pre-approve worktree-confined git commit"}'` is run
- **THEN** the broker SHALL receive an `agent.learning` message with
  `agent_id = "supervisor"`, `category = "tooling_friction"`, the given
  `title`, and the given `body` object

#### Scenario: learn resolves the broker URL from config

- **GIVEN** `.git-paw/config.toml` sets `[broker] port = 9200`
- **WHEN** `sweep.sh learn <category> <title> <body-json>` is run
- **THEN** the publish SHALL target the configured port, not a hardcoded one

#### Scenario: learn needs no broad curl grant

- **WHEN** the supervisor's permission allowlist is seeded
- **THEN** invoking `sweep.sh learn …` SHALL be covered by the existing
  by-path grant for `.git-paw/scripts/sweep.sh`
- **AND** no broad `curl *` grant SHALL be required to publish a learning

### Requirement: Tooling-friction qualitative category

The system SHALL recognise a fifth `agent.learning` category value
`tooling_friction`, carried on the existing `agent.learning` broker variant
with no wire-format change ([[agent-learning-variant]]'s open-enum contract
makes the addition transparent). The category SHALL capture friction the
supervisor absorbs about git-paw *itself* — a tool behaviour that made the
supervisor repeat work or work around the tool — as distinct from the four
project-scoped categories (`recurring_failure_shape`, `doc_gap`, `adr_drift`,
`scope_mistake`).

The `tooling_friction` body SHALL document the fields `friction` (what
git-paw made the supervisor do), `occurrences` (how many times it was
absorbed this session), and `suggestion` (the proposed tool change). The
primary dedup identifier for `tooling_friction` SHALL be `friction`.

#### Scenario: Broker routes a tooling_friction record

- **GIVEN** the broker is running
- **WHEN** the supervisor publishes an `agent.learning` with
  `category = "tooling_friction"` and a populated body
- **THEN** the broker SHALL accept and route the message identically to a
  v0.5.0 deterministic-category record

#### Scenario: tooling_friction body shape is documented

- **WHEN** a contributor or LLM reads the supervisor skill or the
  qualitative-learnings spec
- **THEN** the `tooling_friction` category SHALL list its expected body
  fields (`friction`, `occurrences`, `suggestion`)

### Requirement: Tooling-friction publish heuristic

The bundled supervisor skill SHALL include a heuristic that gates when
`tooling_friction` is published, with an explicit "do not publish unless…"
gate consistent with the existing four categories. The heuristic SHALL
require that the same friction was absorbed **at least twice in the session**
(e.g. the same prompt cleared on two or more sweeps, or the same
helper/tooling gap worked around two or more times) before publishing; a
one-off friction SHALL NOT be published.

#### Scenario: tooling_friction requires repeated absorption

- **WHEN** the supervisor skill is read
- **THEN** the `tooling_friction` heuristic SHALL specify that publishing
  requires the same friction to have been absorbed at least twice in the
  session
- **AND** SHALL forbid publishing a one-off friction

### Requirement: Operational qualitative capture in the sweep loop and at session end

The bundled supervisor skill SHALL wire qualitative-learning capture into the
operational monitoring loop at two moments, both routed through
`sweep.sh learn` and both deduped via each category's primary identifier
(per the existing within-session dedup discipline):

- **Opportunistic** — the continuous monitoring-loop / sweep section SHALL
  include a step that, when the sweep observes or absorbs friction matching a
  category gate, records a one-line learning in the moment. This step SHALL be
  a terminal, non-blocking step of the loop iteration (it SHALL NOT precede or
  displace approval clearing or stuck detection).
- **Session-end synthesis** — the wind-down / final-summary section SHALL
  include a reflective pass over the run that publishes the durable
  qualitative learnings not already captured in-session.

#### Scenario: Continuous sweep section includes a capture step

- **WHEN** the supervisor skill's continuous monitoring-loop / sweep section
  is read
- **THEN** it SHALL include a step directing the LLM to publish a qualitative
  learning via `sweep.sh learn` when a category gate is met during the sweep
- **AND** that step SHALL be ordered after approval clearing and stuck
  detection (non-blocking, terminal)

#### Scenario: Wind-down section includes a synthesis pass

- **WHEN** the supervisor skill's session-end / final-summary section is read
- **THEN** it SHALL include a reflective synthesis pass that publishes durable
  qualitative learnings via `sweep.sh learn`
- **AND** the pass SHALL instruct the LLM to dedup against `agent.learning`
  records already published in the session, by each category's primary
  identifier

### Requirement: Tooling-friction renderer section

The system SHALL render `tooling_friction` records into a dedicated
"Tooling friction" section of `.git-paw/session-learnings.md`, adjacent to the
four existing qualitative sections. A `tooling_friction` record SHALL NOT fall
through to the "Other learnings" fallback. The system SHALL preserve the
v0.5.0 deterministic sections and the four existing qualitative sections
unchanged, and SHALL apply the existing tolerant-rendering behaviour (title +
JSON dump) to a malformed `tooling_friction` body.

#### Scenario: A tooling_friction record appears under its section

- **WHEN** the file renderer processes an `agent.learning` record with
  `category = "tooling_friction"`
- **THEN** the rendered file SHALL contain that record under a
  "Tooling friction" section header
- **AND** the record SHALL NOT appear under "Other learnings"

#### Scenario: Malformed tooling_friction body is rendered as title + JSON

- **GIVEN** a `tooling_friction` record whose body lacks the documented
  `friction` field
- **WHEN** the file renderer processes it
- **THEN** the rendered output SHALL include the `title` line followed by the
  body serialised as JSON, under the "Tooling friction" section

#### Scenario: Existing sections are unchanged

- **WHEN** the file renderer processes the four existing qualitative
  categories and the v0.5.0 deterministic categories
- **THEN** their rendered output SHALL match the pre-change format
  byte-for-byte
- **AND** a genuinely unrecognised category SHALL still fall through to
  "Other learnings"
