# qualitative-learnings Specification

## Purpose
Extends `agent.learning` with qualitative categories (recurring_failure_shape, doc_gap, adr_drift, scope_mistake, tooling_friction) that the supervisor publishes through the `sweep.sh learn` helper under gated heuristics, and renders them into dedicated sections of `.git-paw/session-learnings.md` so durable, judgment-based signals are captured without changing the broker wire format.

## Requirements
### Requirement: Four qualitative category values

The system SHALL recognise four new `agent.learning` category
values: `recurring_failure_shape`, `doc_gap`, `adr_drift`, and
`scope_mistake`. These values SHALL be carried on the existing
`agent.learning` broker variant without any wire-format change;
[[agent-learning-variant]]'s open-enum contract makes the
additions transparent to the broker.

#### Scenario: Broker routes a recurring_failure_shape record

- **GIVEN** the broker is running
- **WHEN** the supervisor publishes an `agent.learning` message
  with `category = "recurring_failure_shape"` and a populated
  body
- **THEN** the broker SHALL accept and route the message
  identically to a v0.5.0 deterministic-category record

#### Scenario: Each of the four categories has a documented body shape

- **WHEN** a contributor or LLM reads the supervisor skill or
  the qualitative-learnings spec
- **THEN** each category SHALL list the expected body fields
  (e.g. `shape`/`instances` for recurring_failure_shape;
  `convention`/`evidence_paths`/`suggestion` for doc_gap)

### Requirement: Supervisor-skill heuristics for qualitative publishing

The bundled supervisor skill SHALL include heuristics that gate
when each new category is published. The system SHALL produce
heuristics specific enough to keep false-positive rates low while
allowing LLM judgment on edge cases. Each heuristic SHALL include
an explicit "do not publish unless..." gate sentence.

#### Scenario: recurring_failure_shape requires multi-branch evidence

- **WHEN** the supervisor skill is read
- **THEN** the recurring_failure_shape heuristic SHALL specify
  that publishing requires at least three feedback cycles from
  at least two distinct branches with semantically similar error
  text

#### Scenario: doc_gap requires evidence the convention is missing

- **WHEN** the supervisor skill is read
- **THEN** the doc_gap heuristic SHALL specify that publishing
  requires the convention to be verifiable from code AND absent
  from the configured `[governance]` doc paths

#### Scenario: adr_drift requires a concrete code commit

- **WHEN** the supervisor skill is read
- **THEN** the adr_drift heuristic SHALL specify that publishing
  requires at least one commit on a non-trivial branch
  introducing the un-ADR'd pattern

#### Scenario: scope_mistake requires overlapping intents plus coordination

- **WHEN** the supervisor skill is read
- **THEN** the scope_mistake heuristic SHALL specify that
  publishing requires at least two branches with overlapping
  `agent.intent` AND at least two `agent.feedback` messages
  about coordination AND a commit on each branch

### Requirement: Within-session dedup discipline

The supervisor skill SHALL teach the LLM to consult prior
`agent.learning` records published in the current session before
emitting a qualitative record. The system SHALL NOT republish a
substantially-similar record with the same category and the same
primary identifier (`shape`, `convention`, `decision_area`, or
`branches` set, depending on category).

#### Scenario: Skill prose names the primary identifier per category

- **WHEN** the supervisor skill's qualitative-learnings section
  is read
- **THEN** the dedup section SHALL name a primary identifier
  field per category and SHALL instruct the LLM to suppress
  publish when an active session record carries the same value

#### Scenario: Hour-bucket id collisions are independently handled

- **GIVEN** an exact-duplicate publish within an hour
- **WHEN** the broker accepts the duplicate
- **THEN** the deterministic `id` from [[agent-learning-variant]]
  SHALL produce identical ids so broker consumers can dedupe at
  their boundary, even when the skill-level dedup misses

### Requirement: File renderer new sections

The system SHALL render qualitative-learning records into four
new sections of `.git-paw/session-learnings.md` adjacent to the
v0.5.0 deterministic sections. The system SHALL include a
fallback "Other learnings" section that absorbs records whose
category is not recognised. The system SHALL preserve the v0.5.0
deterministic sections unchanged.

#### Scenario: A recurring_failure_shape record appears under its section

- **WHEN** the file renderer processes an `agent.learning`
  record with `category = "recurring_failure_shape"`
- **THEN** the rendered file SHALL contain that record under a
  "Recurring failure shapes" section header

#### Scenario: Each new category has its own section

- **WHEN** the file renderer runs against fixture records
  covering all four new categories
- **THEN** the rendered file SHALL contain a section for each
  category: "Recurring failure shapes", "Documentation gaps",
  "ADR / architectural drift", "Scope-mistake signals"

#### Scenario: Unknown category falls through to Other learnings

- **WHEN** the file renderer processes a record with an
  unrecognised `category` value
- **THEN** the rendered file SHALL place the record under an
  "Other learnings" section and SHALL NOT silently drop it

#### Scenario: v0.5.0 sections unchanged

- **WHEN** the file renderer processes a v0.5.0 deterministic
  record (conflict_event, stuck_duration, recovery_cycles,
  permission_pattern)
- **THEN** the rendered output for that record SHALL match the
  v0.5.0 format byte-for-byte

### Requirement: Tolerant rendering of malformed bodies

The file renderer SHALL tolerate qualitative records whose body
shape doesn't match the documented body fields. The system SHALL
fall back to rendering the record's `title` plus a JSON dump of
its `body` rather than failing or dropping the record.

#### Scenario: Malformed body is rendered as title + JSON

- **GIVEN** a `recurring_failure_shape` record whose body lacks
  the documented `instances` field
- **WHEN** the file renderer processes it
- **THEN** the rendered output SHALL include the `title` line
  followed by the body content serialised as JSON, under the
  category's section

### Requirement: No confidence field in payload

The system SHALL NOT include a `confidence` field in the
`agent.learning` body for qualitative records. Confidence SHALL
be signalled by publishing or not publishing — i.e., the
supervisor's heuristic gate is the confidence gate.

#### Scenario: Skill prose forbids speculative publishing

- **WHEN** the supervisor skill is read
- **THEN** the qualitative-learnings section SHALL include
  language forbidding publishing speculative records "just in
  case", and SHALL NOT introduce a body field that lets the LLM
  encode uncertainty

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

