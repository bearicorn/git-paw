# learnings-mode Specification

## Purpose
TBD - created by archiving change learnings-mode. Update Purpose after archive.
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

### Requirement: No agent.learning broker variant in v0.5.0

The system SHALL NOT introduce an `agent.learning` `BrokerMessage` variant in v0.5.0. The aggregator's only output sink is the markdown file. The structured/programmatic surface for learnings is deferred to v0.6.0 alongside the MCP server, which will define the wire format with its consumer in mind.

The aggregator's internal data model (per-signal records with structured fields) SHALL be designed so that v0.6.0 can serialise it to a broker variant without re-deriving from messages.

#### Scenario: No agent.learning variant exists in BrokerMessage in v0.5.0

- **WHEN** the v0.5.0 `BrokerMessage` enum is inspected
- **THEN** there SHALL NOT be a `Learning` variant or any variant with serde tag `agent.learning`

#### Scenario: Aggregator does not publish to the broker

- **GIVEN** a running aggregator with at least one observed event
- **WHEN** the aggregator flushes
- **THEN** no message of any new variant SHALL be published to the broker
- **AND** the only side effect of the flush SHALL be appended content in `.git-paw/session-learnings.md`

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

