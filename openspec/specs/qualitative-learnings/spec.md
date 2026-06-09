# qualitative-learnings Specification

## Purpose
TBD - created by archiving change qualitative-learnings. Update Purpose after archive.
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

