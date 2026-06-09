# agent-learning-variant Specification

## Purpose
TBD - created by archiving change agent-learning-variant. Update Purpose after archive.
## Requirements
### Requirement: agent.learning broker message variant

The broker SHALL accept and route an `agent.learning` message
variant. Each message SHALL carry the fields `id` (deterministic
hash string), `agent_id`, `branch_id` (optional, null for
cross-cutting records), `category` (one of `conflict_event`,
`stuck_duration`, `recovery_cycles`, `permission_pattern`, plus
any future categories added by descendant changes), `title`
(short human-readable summary), `body` (category-specific
structured object), and `timestamp` (ISO 8601 UTC).

#### Scenario: Broker accepts and stores a conflict_event record

- **WHEN** the aggregator publishes an `agent.learning` message
  with `category = "conflict_event"` and the documented body
  fields
- **THEN** the broker SHALL accept the message and SHALL include
  it in its `messages/<branch_id>` stream

#### Scenario: Broker accepts a category from a descendant change

- **GIVEN** a descendant change ([[qualitative-learnings]]) adds
  a new category value
- **WHEN** the aggregator publishes a record with the new
  category
- **THEN** the broker SHALL accept and route it without rejecting
  on an unknown enum value (categories are open strings,
  validated client-side)

#### Scenario: Required field omission is rejected

- **WHEN** a publish attempt omits `category`, `title`, `body`,
  or `timestamp`
- **THEN** the broker SHALL return a 400-class error identifying
  the missing field

### Requirement: Deterministic id for idempotent re-emission

The aggregator SHALL produce a deterministic `id` for each
`agent.learning` record. The system SHALL compute the id as a
hex-encoded SHA-256 prefix (16 hex characters) of a canonical
serialisation comprising `category`, `branch_id`, the
category-specific body fields in a stable order, and the UTC
hour bucket (`YYYY-MM-DDTHH`). Re-publishing the same logical
record within the same hour SHALL produce the same id.

#### Scenario: Same record within the hour gets the same id

- **WHEN** the aggregator commits the same logical record twice
  within a single UTC hour
- **THEN** both broker messages SHALL carry identical `id`
  values

#### Scenario: Same record across hour boundaries gets different ids

- **GIVEN** a record committed at 13:59 UTC and the same record
  committed at 14:01 UTC
- **WHEN** both publish
- **THEN** the two messages SHALL carry different `id` values

### Requirement: Dual output when broker is enabled

The aggregator SHALL append every record to
`.git-paw/session-learnings.md` (preserving v0.5.0 behaviour) and
SHALL additionally publish the record as an `agent.learning`
broker message when `[broker] enabled = true`. When the broker is
disabled, the system SHALL produce file output only — matching
v0.5.0 exactly.

#### Scenario: File-only output when broker is disabled

- **GIVEN** `[supervisor] learnings = true` and
  `[broker] enabled = false`
- **WHEN** the aggregator commits a record
- **THEN** the system SHALL append to the learnings file and
  SHALL NOT attempt any broker publish

#### Scenario: Both outputs when broker is enabled

- **GIVEN** `[supervisor] learnings = true` and
  `[broker] enabled = true`
- **WHEN** the aggregator commits a record
- **THEN** the system SHALL append to the learnings file AND
  publish an `agent.learning` broker message

#### Scenario: File output unchanged from v0.5.0 format

- **WHEN** the aggregator commits a record
- **THEN** the appended file entry SHALL match the v0.5.0
  Markdown shape exactly so existing parsers and human readers
  continue to work

### Requirement: Internal model serialises directly

The system SHALL serialise the aggregator's existing
`LearningRecord` data model (introduced in v0.5.0) into the
`agent.learning` broker message without a parallel internal
representation. Field-name differences between the internal model
and the wire schema SHALL be resolved at the conversion boundary
(`From<&LearningRecord> for BrokerMessage`), not by duplicating
fields in the model.

#### Scenario: No new internal LearningRecord-like type appears

- **WHEN** the broker variant is added
- **THEN** the codebase SHALL contain exactly one in-memory
  representation of a learning record (the v0.5.0
  `LearningRecord`), with the broker payload produced by a
  conversion function

### Requirement: MCP get_learnings consumes the variant

The MCP get_learnings tool SHALL prefer broker records when the
broker is running and SHALL fall back to parsing the learnings
file when the broker is off. The tool's response SHALL include a
`source` field indicating which path produced the records. This
applies to the `get_learnings()` tool defined in [[mcp-server]]'s
`mcp-read-tools` capability.

#### Scenario: Broker-running mode returns broker records

- **GIVEN** an active session with the broker running and
  committed learning records
- **WHEN** an MCP client calls `get_learnings()`
- **THEN** the response SHALL list the broker records, and the
  `source` field SHALL be `"broker"`

#### Scenario: Broker-off mode falls back to file parsing

- **GIVEN** a repository with a learnings file but no active
  broker
- **WHEN** an MCP client calls `get_learnings()`
- **THEN** the response SHALL list records parsed from the file,
  and the `source` field SHALL be `"file"`

#### Scenario: Identical record shape across both sources

- **WHEN** the same record is read via broker mode and via file
  mode (after broker stops)
- **THEN** the structured fields the client sees (`category`,
  `title`, `body`, `timestamp`, `id`) SHALL be equivalent

### Requirement: Backwards compatibility with v0.5.0

The system SHALL produce no change in observable behaviour for
v0.5.0 users that have `[broker] enabled = false` (or no broker
section at all). The learnings file format SHALL remain
unchanged from v0.5.0.

#### Scenario: v0.5.0 config produces v0.5.0 behaviour

- **GIVEN** a `.git-paw/config.toml` identical to a v0.5.0 config
  (no broker section, `[supervisor] learnings = true`)
- **WHEN** the aggregator runs to completion across a session
- **THEN** the system SHALL produce a `session-learnings.md`
  file byte-equivalent to what v0.5.0 would produce for the
  same input events

