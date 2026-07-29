# supervisor-learnings Specification

## Purpose
An opt-in, broker-internal aggregator (active only under supervisor mode with `learnings = true`) that observes broker messages to derive both deterministic signals — stuck-duration, recovery-cycle, conflict-event, and permission-pattern — and qualitative, judgment-based signals the supervisor publishes through the `sweep.sh learn` helper (recurring_failure_shape, doc_gap, adr_drift, scope_mistake, tooling_friction). It appends them under per-session, per-category headings in a local `.git-paw/session-learnings.md`, flushing periodically and on shutdown, performing no telemetry, and printing a privacy disclosure at session start.

When the broker is enabled the aggregator emits each record on the `agent.learning` broker variant — carrying a deterministic hour-bucketed id for idempotent re-emission — in addition to appending it to the file; when the broker is off, behaviour is file-only and matches v0.5.0 exactly, and the MCP `get_learnings` tool reads from the broker when running or from the file otherwise. The aggregator's internal `LearningRecord` data model serialises directly to the `agent.learning` broker variant at the conversion boundary, without re-deriving records from broker messages and without a parallel internal representation — a forward-design property now realised by the `agent.learning` variant.

## Requirements
### Requirement: Learnings aggregator lifecycle

The system SHALL provide a broker-internal learnings aggregator subsystem that runs alongside the filesystem watcher and the conflict detector when supervisor mode is active AND `[supervisor] learnings = true` is set in config.

The aggregator SHALL NOT run when:
- `[supervisor] enabled = false` or the `[supervisor]` section is absent (no supervisor → no learnings).
- `[supervisor] learnings = false` (the default; users opt in explicitly).
- The `--no-supervisor` flag is passed for the session.

When the aggregator is not running, no `.git-paw/session-learnings.md` writes SHALL occur.

The aggregator SHALL stop cleanly when the broker stops, performing one final flush before exit (per the "Periodic flush + shutdown flush" requirement).

#### Scenario: Aggregator starts when supervisor and learnings are both enabled

- **GIVEN** a broker started with `[supervisor] enabled = true` and `[supervisor] learnings = true`
- **WHEN** the broker is fully booted
- **THEN** the learnings aggregator subsystem SHALL be running

#### Scenario: Aggregator does not start when learnings flag is false

- **GIVEN** a broker started with `[supervisor] enabled = true` and `[supervisor] learnings = false` (or absent)
- **WHEN** the broker is fully booted
- **THEN** the learnings aggregator SHALL NOT be running
- **AND** no `.git-paw/session-learnings.md` writes SHALL occur

#### Scenario: Aggregator does not start when supervisor is disabled

- **GIVEN** a broker started with `[supervisor] enabled = false` (or section absent), regardless of the learnings flag
- **WHEN** the broker is fully booted
- **THEN** the learnings aggregator SHALL NOT be running

#### Scenario: Aggregator flushes on broker shutdown

- **GIVEN** a running aggregator with at least one observed event since the last flush
- **WHEN** the `BrokerHandle` is dropped
- **THEN** one final flush SHALL be performed before the aggregator task exits
- **AND** any newly-observed events since the last periodic flush SHALL be present in the markdown file

### Requirement: Stuck-duration signal

The aggregator SHALL track stuck duration per agent. On observing an `agent.blocked` from agent X with `payload.from = Y`, the aggregator SHALL record the block start time. On observing the next `agent.artifact` from X subsequent to that block, the aggregator SHALL record the elapsed duration as the stuck duration and clear the pending-block entry.

If a session ends with a pending block still open, the aggregator SHALL record the entry as unresolved with the duration measured up to session end.

Each stuck-duration record contributes one bullet to the markdown file's "Where agents got stuck" section at the next flush.

#### Scenario: Stuck duration recorded when block resolves

- **GIVEN** agent X published `agent.blocked` with `from = Y` at time T
- **WHEN** agent X subsequently publishes `agent.artifact` at time T + 11m12s
- **THEN** the aggregator SHALL record a stuck-duration learning with `agent_id = X`, `blocked_on = Y`, `duration_seconds ≈ 672`, marked as resolved
- **AND** the markdown file's next flush SHALL include a corresponding bullet under "Where agents got stuck"

#### Scenario: Unresolved block at session end is reported

- **GIVEN** agent X published `agent.blocked` with `from = Y` and never published a subsequent `agent.artifact`
- **WHEN** the broker shuts down
- **THEN** the aggregator's final flush SHALL include a stuck-duration entry marked unresolved with the duration up to the shutdown time

### Requirement: Recovery-cycle signal

The aggregator SHALL count the number of `agent.feedback` messages addressed to each agent X (`Feedback.agent_id = X`) before the agent's eventual `agent.verified`. The count SHALL be recorded as a learning when X is verified, OR at session end if X never verifies.

Each recovery-cycle record contributes one bullet to the markdown file's "Recovery cycles" section at the next flush.

#### Scenario: Recovery cycles recorded when agent verifies

- **GIVEN** agent X received 3 `agent.feedback` messages followed by an `agent.verified`
- **WHEN** the aggregator processes the verified event
- **THEN** the aggregator SHALL record a recovery-cycles learning with `agent_id = X`, `count = 3`
- **AND** the next flush SHALL append a corresponding bullet to the markdown file

#### Scenario: Zero recovery cycles produces no learning

- **GIVEN** agent X received zero `agent.feedback` messages and was verified
- **WHEN** the aggregator processes the verified event
- **THEN** no recovery-cycles learning SHALL be recorded (zero is not noise-worthy)

### Requirement: Conflict-event signal

The aggregator SHALL track conflict events by subscribing to `agent.feedback` and `agent.question` messages whose error/question text begins with the `[conflict-detector]` tag (per the conflict-detection capability's emission convention).

For each tagged message, the aggregator SHALL classify the event into one of:
- `forward-conflict-intra-spec` — both implicated agent_ids belong to the same `SpecEntry` family
- `forward-conflict-cross-spec` — the agent_ids belong to different `SpecEntry` families
- `in-flight-conflict` — text matches the in-flight pattern
- `ownership-violation` — text matches the ownership pattern

Each classified event contributes one bullet to the markdown file's "Conflict events" section at the next flush. Intra-vs-cross-spec classification SHALL use the agent → `SpecEntry` mapping the broker session tracks at the time of the event.

#### Scenario: Forward-conflict-intra-spec is classified

- **GIVEN** the conflict detector emitted `agent.feedback` to agents X and Y, both belonging to spec `003-user-list`, with text containing `[conflict-detector] forward conflict`
- **WHEN** the aggregator processes those messages
- **THEN** one entry SHALL be recorded with category `forward-conflict-intra-spec` referencing the same agent pair
- **AND** the next flush SHALL append a corresponding bullet under "Conflict events"

#### Scenario: Forward-conflict-cross-spec is classified

- **GIVEN** the conflict detector emitted `agent.feedback` to agents X (spec `003-user-list`) and Y (spec `004-error-handling`)
- **WHEN** the aggregator processes those messages
- **THEN** one entry SHALL be recorded with category `forward-conflict-cross-spec` and the entry SHALL name both spec ids

#### Scenario: Ownership violation is classified

- **WHEN** the conflict detector emits `agent.feedback` with text containing `[conflict-detector] ownership violation`
- **THEN** the aggregator SHALL record an entry with category `ownership-violation` naming the violator and owner agent ids and the file path

### Requirement: Permission-pattern signal

When the supervisor's auto-approve subsystem records a hit (existing v0.4 behaviour: an `agent.status` message tagged `auto_approved` with a command-class label), the aggregator SHALL increment a counter keyed on the command class. At each flush AND at session end, the aggregator SHALL record one entry per command class with `count` ≥ a configurable threshold (default 5; lower-count classes produce no entry to avoid noise).

Each recorded permission-pattern entry contributes one bullet to the markdown file's "Permission patterns" section at the next flush.

#### Scenario: High-count command class produces an entry

- **GIVEN** 23 auto-approve hits across the session for command class `cargo check`
- **WHEN** the aggregator flushes
- **THEN** a permission-pattern entry SHALL be recorded with `command_class = "cargo check"`, `count = 23`
- **AND** a corresponding bullet SHALL be appended to the markdown file under "Permission patterns"

#### Scenario: Low-count command class produces no entry

- **GIVEN** 2 auto-approve hits for command class `git status`
- **WHEN** the aggregator flushes
- **THEN** no permission-pattern entry SHALL be recorded for that class
- **AND** the counter is preserved across flushes (a later session burst could push the count over the threshold)

### Requirement: Markdown file output

The aggregator SHALL maintain `.git-paw/session-learnings.md` in the repository root. The file SHALL be append-only:

- The first flush of a new session SHALL append an H2 heading containing the session start time as an ISO 8601 UTC timestamp (e.g. `## Session Learnings — 2026-04-22T14:35:09Z`).
- Subsequent flushes within the same session SHALL append new entries under the existing session heading.
- The file SHALL NOT be rewritten or shuffled. Prior session content SHALL be preserved.

Each session's content SHALL be organised under H3 sub-headings, one per signal category that produced at least one entry in the session. Empty categories SHALL be omitted entirely (no `### Conflict events\n_(none)_` placeholders).

H3 categories owned by the deterministic aggregator:
- `### Conflict events` — entries from forward / in-flight / ownership categories
- `### Where agents got stuck` — stuck-duration entries
- `### Recovery cycles` — recovery-cycle entries
- `### Permission patterns` — permission-pattern entries

Each H3 SHALL contain a bullet list. Each bullet is one learning event in human-readable form, with optional follow-up `Suggestion: ...` line indented under the bullet.

#### Scenario: New session writes ISO-timestamped H2 heading

- **GIVEN** a freshly-started session with the aggregator running
- **WHEN** the first flush occurs (after the first observed event)
- **THEN** the markdown file contains an H2 heading matching `^## Session Learnings — \d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$`

#### Scenario: Empty categories are omitted

- **GIVEN** a session with conflict events but no stuck-duration events
- **WHEN** flushes complete
- **THEN** the markdown contains a `### Conflict events` heading
- **AND** the markdown does NOT contain a `### Where agents got stuck` heading

#### Scenario: Subsequent sessions append, do not overwrite

- **GIVEN** an existing `.git-paw/session-learnings.md` from a prior session with content
- **WHEN** a new session's aggregator runs and flushes
- **THEN** the prior session's content is unchanged
- **AND** new content appears at the end of the file under a new H2 heading

### Requirement: Periodic flush + shutdown flush

The aggregator SHALL flush on a periodic timer at `[supervisor.learnings] flush_interval_seconds` (default 60s). Each flush SHALL append entries to the markdown file corresponding to events accumulated since the last flush.

The aggregator SHALL ALSO perform one flush at broker shutdown. Bursts of detector events between flushes SHALL NOT trigger eager flushes — they batch into the next periodic or shutdown flush.

#### Scenario: Periodic flush writes accumulated entries

- **GIVEN** the aggregator has observed 3 events since the last flush
- **WHEN** the next periodic flush timer fires
- **THEN** the markdown file SHALL gain 3 corresponding bullet entries

#### Scenario: Burst of events does not trigger eager flush

- **GIVEN** the aggregator just performed a flush
- **WHEN** 5 conflict events arrive within 2 seconds
- **THEN** no flush occurs immediately
- **AND** the next flush at the periodic interval writes all 5 events together

### Requirement: Configurable flush interval

The system SHALL expose `[supervisor.learnings] flush_interval_seconds` (positive `u64`, default `60`) for tuning the flush cadence. The value SHALL be honoured at aggregator startup; runtime changes are not supported in v0.5.0.

#### Scenario: Default flush interval is 60 seconds

- **GIVEN** a config with `[supervisor] learnings = true` and no `[supervisor.learnings_config]` section
- **WHEN** the aggregator starts
- **THEN** the flush interval SHALL be 60 seconds

#### Scenario: Custom flush interval is honoured

- **GIVEN** a config with `[supervisor.learnings_config] flush_interval_seconds = 30`
- **WHEN** the aggregator starts
- **THEN** the flush interval SHALL be 30 seconds

### Requirement: No-telemetry privacy guarantee

Learnings mode SHALL perform no telemetry. The learnings aggregator SHALL write only to the local `.git-paw/session-learnings.md` file and SHALL NOT transmit learnings content to any network destination outside the operator's own machine. git-paw SHALL NOT collect, upload, or phone home learnings data under any configuration.

#### Scenario: Learnings output stays local

- **GIVEN** a session running with `[supervisor] learnings = true`
- **WHEN** the aggregator records and flushes learnings
- **THEN** the only artifact produced SHALL be the local `.git-paw/session-learnings.md` file
- **AND** no learnings content SHALL be transmitted to any destination other than the operator's machine

### Requirement: Session-start learnings disclosure notice

When a session starts with learnings mode enabled (`[supervisor] learnings = true`), git-paw SHALL print a concise notice to the user that states: (a) the local path the learnings file is written to, (b) that nothing is sent anywhere / no telemetry, and (c) that the file may be reviewed and optionally shared with the maintainers via a GitHub issue to improve the tool, after reviewing it and stripping or anonymising any sensitive repo-specific details (a task the user's own LLM can assist with).

The notice SHALL NOT be printed when learnings mode is disabled (the default), so a session that has not opted in behaves identically to before this change.

#### Scenario: Notice prints when learnings is enabled

- **GIVEN** a configuration with `[supervisor] enabled = true` and `[supervisor] learnings = true`
- **WHEN** the session starts
- **THEN** git-paw SHALL print a notice that names the local `.git-paw/session-learnings.md` path, states that no telemetry is performed, and invites optional sharing via a GitHub issue with a review/anonymise caveat

#### Scenario: No notice when learnings is disabled

- **GIVEN** a configuration with `[supervisor] learnings = false` or the `[supervisor]` section absent
- **WHEN** the session starts
- **THEN** git-paw SHALL NOT print the learnings disclosure notice
- **AND** session start output SHALL be identical to the pre-change behavior

### Requirement: Documentation states privacy stance and sharing invitation

The learnings user-guide documentation SHALL state that learnings mode performs no telemetry, that its output is a local opt-in file, and SHALL invite users to optionally share the file with the maintainers via a GitHub issue to improve the tool — including the caveat that the file contains repo-specific details that should be reviewed and may be stripped or anonymised (e.g. with the user's own LLM) before sharing.

#### Scenario: Learnings doc carries the privacy and sharing section

- **WHEN** a reader opens the learnings user-guide chapter
- **THEN** it SHALL contain a section stating the no-telemetry / local / opt-in stance
- **AND** it SHALL contain the optional-sharing invitation with the review-and-anonymise caveat and a link to open a GitHub issue

### Requirement: Four qualitative category values

The system SHALL recognise four new `agent.learning` category
values: `recurring_failure_shape`, `doc_gap`, `adr_drift`, and
`scope_mistake`. These values SHALL be carried on the existing
`agent.learning` broker variant without any wire-format change;
[[supervisor-learnings]]'s open-enum contract makes the
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
- **THEN** the deterministic `id` from [[supervisor-learnings]]
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

### Requirement: Qualitative payload schema

The qualitative `agent.learning` payload's fields SHALL be the
category value plus the body text (the per-category body shape
documented for each category). The payload SHALL carry no
per-entry confidence field: confidence is expressed by publishing
or not publishing, so the supervisor's heuristic gate IS the
confidence mechanism.

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
with no wire-format change ([[supervisor-learnings]]'s open-enum contract
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

- **GIVEN** a descendant change ([[supervisor-learnings]]) adds
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

### Requirement: User guide includes a Learnings Mode chapter

The mdBook user guide SHALL include a chapter at
`docs/src/user-guide/learnings.md` documenting learnings mode.
The chapter SHALL cover the opt-in `[supervisor] learnings = true`
flag, the location and append-only shape of
`.git-paw/session-learnings.md`, and the five deterministic
categories tracked (stuck duration, recovery-cycle
count, forward conflicts, in-flight conflicts, ownership
violations). The chapter SHALL state that the broker
`agent.learning` wire variant is deferred to v0.6.0 and that
the tool ships file-only output.

The chapter SHALL be linked from `docs/src/SUMMARY.md` under the
User Guide group.

#### Scenario: Learnings chapter exists and is linked

- **WHEN** `docs/src/SUMMARY.md` is inspected
- **THEN** it contains a link to `user-guide/learnings.md` under the User Guide section

#### Scenario: Learnings chapter documents the opt-in flag

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it contains the substring `[supervisor]` and references the `learnings` flag
- **AND** it states the default value is `false` (or equivalent — "opt-in")

#### Scenario: Learnings chapter names the output file

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it contains the substring `.git-paw/session-learnings.md`

#### Scenario: Learnings chapter enumerates the deterministic categories

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it mentions stuck duration (or "where agents got stuck")
- **AND** it mentions recovery-cycle count (or "recovery cycles")
- **AND** it mentions forward conflicts
- **AND** it mentions in-flight conflicts
- **AND** it mentions ownership violations

#### Scenario: Learnings chapter defers `agent.learning` to v0.6.0

- **WHEN** `docs/src/user-guide/learnings.md` is inspected
- **THEN** it states that the `agent.learning` broker variant (or programmatic access) is deferred to v0.6.0
