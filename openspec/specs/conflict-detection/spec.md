# conflict-detection Specification

## Purpose
TBD - created by archiving change conflict-detection. Update Purpose after archive.
## Requirements
### Requirement: Conflict detector lifecycle

The system SHALL provide a broker-internal conflict-detector subsystem that runs alongside the filesystem watcher when supervisor mode is active. The detector SHALL start when the broker starts in supervisor mode and SHALL stop when the broker stops.

The detector SHALL NOT run when `[supervisor] enabled = false` or when the `[supervisor]` section is absent from config — under those configurations, the detector subsystem SHALL not be started, no `agent.feedback` SHALL be auto-emitted by the detector, and no `agent.question` SHALL be auto-emitted by the detector. `agent.intent` messages SHALL still be broadcast normally (per `forward-coordination`).

#### Scenario: Detector starts when supervisor mode is enabled

- **GIVEN** a broker started with `[supervisor] enabled = true`
- **WHEN** the broker is fully booted
- **THEN** the conflict detector subsystem SHALL be running

#### Scenario: Detector does not start when supervisor mode is disabled

- **GIVEN** a broker started with `[supervisor] enabled = false`
- **WHEN** the broker is fully booted
- **THEN** the conflict detector subsystem SHALL NOT be running
- **AND** publishing overlapping `agent.intent` messages SHALL NOT cause any auto-emitted `agent.feedback`

#### Scenario: Detector stops cleanly when broker stops

- **GIVEN** a running broker with the conflict detector active
- **WHEN** the `BrokerHandle` is dropped
- **THEN** the detector task SHALL stop within one poll interval
- **AND** no further auto-emitted messages SHALL be published

### Requirement: Active-intent tracker

The conflict detector SHALL maintain an in-memory active-intent tracker keyed by `agent_id`. On every `agent.intent` publish, the tracker SHALL insert or replace the record for the publishing agent with `(files, summary, received_at, valid_for)` derived from the message payload.

The tracker SHALL drop entries whose age (`now - received_at`) exceeds `valid_for`. Expiry SHALL be checked on every detector tick; expired entries SHALL NOT participate in any conflict check.

When an agent publishes a new `agent.intent`, any prior record for the same agent SHALL be overwritten — the new intent is authoritative. No "self-conflict" warning SHALL be emitted between an agent's old and new intents.

#### Scenario: Active intent is stored on publish

- **GIVEN** a running detector with an empty tracker
- **WHEN** an `agent.intent` from `feat-x` with `files = ["src/a.rs"]` and `valid_for_seconds = 600` is published
- **THEN** the tracker SHALL contain a record for `feat-x` with the listed file

#### Scenario: New intent replaces previous intent for same agent

- **GIVEN** the tracker contains an intent from `feat-x` for `["src/a.rs"]`
- **WHEN** `feat-x` publishes a new intent for `["src/a.rs", "src/b.rs"]`
- **THEN** the tracker contains exactly one record for `feat-x` with both files
- **AND** no `agent.feedback` is emitted to `feat-x` referring to its own prior intent

#### Scenario: Expired intent is dropped from tracker

- **GIVEN** the tracker contains an intent from `feat-x` published more than `valid_for_seconds` ago
- **WHEN** the detector tick runs
- **THEN** the tracker SHALL no longer contain the record for `feat-x`

#### Scenario: Expired intent does not trigger overlap warnings

- **GIVEN** an expired intent from `feat-x` for `["src/a.rs"]`
- **WHEN** `feat-y` publishes an intent for `["src/a.rs"]`
- **THEN** no `agent.feedback` SHALL be emitted to `feat-x`
- **AND** no `agent.feedback` SHALL be emitted to `feat-y` referring to `feat-x`'s intent

### Requirement: Forward-conflict detection

When an `agent.intent` from agent X is published and `[supervisor.conflict] warn_on_intent_overlap = true`, the detector SHALL compute the file overlap between X's intent and every *other* non-expired intent in the tracker. For each agent Y whose intent overlaps with X's intent on at least one file:

- The detector SHALL emit one `agent.feedback` to X with `from = "supervisor"` and at least one error message containing the substring `[conflict-detector] forward conflict`, the agent_id of Y, and the overlapping file paths.
- The detector SHALL emit one symmetric `agent.feedback` to Y with `from = "supervisor"` and at least one error message containing the substring `[conflict-detector] forward conflict`, the agent_id of X, and the overlapping file paths.
- Each ordered pair `(min(X, Y), max(X, Y))` SHALL be warned at most once until either intent is replaced or expires. Subsequent intent publishes by either party while both intents remain unchanged SHALL NOT re-emit warnings to the same pair.

When `warn_on_intent_overlap = false`, no forward-conflict `agent.feedback` SHALL be emitted, but the tracker SHALL still record the intent (so in-flight and ownership detection remain functional).

#### Scenario: Two agents publish overlapping intents

- **GIVEN** a running detector with `warn_on_intent_overlap = true` and an empty tracker
- **WHEN** `feat-x` publishes intent for `["src/a.rs", "src/b.rs"]`
- **AND** `feat-y` publishes intent for `["src/b.rs", "src/c.rs"]`
- **THEN** an `agent.feedback` SHALL be emitted to `feat-x` whose error text contains `[conflict-detector] forward conflict`, the substring `feat-y`, and the substring `src/b.rs`
- **AND** an `agent.feedback` SHALL be emitted to `feat-y` whose error text contains `[conflict-detector] forward conflict`, the substring `feat-x`, and the substring `src/b.rs`

#### Scenario: Non-overlapping intents do not trigger warnings

- **GIVEN** a running detector with `warn_on_intent_overlap = true`
- **WHEN** `feat-x` publishes intent for `["src/a.rs"]` and `feat-y` publishes intent for `["src/b.rs"]`
- **THEN** no `agent.feedback` SHALL be emitted by the detector

#### Scenario: Same agent pair is warned only once

- **GIVEN** `feat-x` and `feat-y` have already received forward-conflict warnings for overlap on `src/a.rs`
- **WHEN** `feat-x` re-publishes the same intent (same files)
- **THEN** no new `agent.feedback` SHALL be emitted to either agent for this pair

#### Scenario: Forward-conflict warnings are suppressed when disabled

- **GIVEN** a running detector with `warn_on_intent_overlap = false`
- **WHEN** `feat-x` and `feat-y` publish intents for the same file
- **THEN** no `agent.feedback` SHALL be emitted by the detector
- **AND** the tracker SHALL still contain records for both agents

### Requirement: In-flight conflict detection

When an `agent.status` from agent X carrying `modified_files` is published (typically by the filesystem watcher), the detector SHALL track X's current modified-file set, replacing any previous set for X.

For every other agent Y whose current modified-file set is non-empty, the detector SHALL compute the overlap between X's and Y's modified files. For each `file` in the overlap, ordered as `(min(X, Y), max(X, Y))`:

- If the triple `(min, max, file)` is being seen for the first time, the detector SHALL record `first_seen = now` and SHALL emit an `agent.feedback` to both X and Y with `from = "supervisor"` and an error message containing the substring `[conflict-detector] in-flight conflict` and the `file` path. This warning is the *initial* warning for the pair on that file.
- If the triple has been seen for at least `[supervisor.conflict] window_seconds` and has not yet been escalated, the detector SHALL emit an `agent.question` to inbox `"supervisor"` with `from = "supervisor"` and question text containing the substring `[conflict-detector]`, the `file` path, both agent_ids, and an indication that the window elapsed without resolution. The triple SHALL be marked escalated; subsequent ticks SHALL NOT re-emit the escalation.
- If `file` no longer appears in the intersection of X's and Y's modified files (one of them stopped touching it), the triple SHALL be removed from the in-flight tracker — the conflict has resolved without escalation.

#### Scenario: Two agents touching the same file are warned

- **GIVEN** a running detector and `feat-x` has `modified_files = ["src/a.rs"]`
- **WHEN** `feat-y` publishes `agent.status` with `modified_files = ["src/a.rs"]`
- **THEN** an `agent.feedback` SHALL be emitted to `feat-x` whose error text contains `[conflict-detector] in-flight conflict` and `src/a.rs`
- **AND** an `agent.feedback` SHALL be emitted to `feat-y` with the same content

#### Scenario: In-flight conflict escalates after the configured window

- **GIVEN** the in-flight tracker has carried `(feat-x, feat-y, src/a.rs)` for at least `window_seconds`
- **AND** both agents still report `src/a.rs` in their modified_files
- **WHEN** the detector tick runs
- **THEN** an `agent.question` SHALL be emitted to inbox `"supervisor"` whose question text contains `[conflict-detector]`, `src/a.rs`, `feat-x`, and `feat-y`
- **AND** the triple SHALL be marked escalated

#### Scenario: Escalation is emitted only once per triple

- **GIVEN** an already-escalated in-flight triple `(feat-x, feat-y, src/a.rs)`
- **WHEN** subsequent detector ticks run while both agents still touch the file
- **THEN** no additional `agent.question` SHALL be emitted for the same triple

#### Scenario: Conflict resolves when one agent stops touching the file

- **GIVEN** an in-flight triple `(feat-x, feat-y, src/a.rs)` that has not yet escalated
- **WHEN** `feat-x` publishes `agent.status` with `modified_files = []` (file no longer modified)
- **THEN** the in-flight tracker SHALL no longer contain the triple
- **AND** no escalation SHALL be emitted for this resolved conflict

### Requirement: Ownership-violation detection

The detector SHALL detect ownership violations — cases where an agent edits a file that lies inside another active agent's declared `agent.intent` and outside (or absent from) its own.

When an `agent.status` from agent X carrying `modified_files` is published, for each `file` in `modified_files`, the detector SHALL apply the following rules:

- If X has an active intent in the tracker AND `file` is in X's intent files, the file is in-scope for X — no violation.
- Else if X has no active intent OR `file` is not in X's intent files, AND some other agent Y has an active non-expired intent whose files include `file`, the detector SHALL recognise this as an ownership violation. Specifically:
  - The detector SHALL emit an `agent.feedback` to X with `from = "supervisor"` and an error message containing the substring `[conflict-detector] ownership violation`, the `file` path, and the agent_id of Y.
  - If `[supervisor.conflict] escalate_on_violation = true`, the detector SHALL also emit an `agent.question` to inbox `"supervisor"` with `from = "supervisor"` and question text containing the substring `[conflict-detector]`, the `file` path, and both agent_ids.
  - Each `(violator_agent_id, file)` pair SHALL receive at most one `agent.feedback` per detector lifetime — repeated reports of the same file by the same violator SHALL NOT re-emit warnings.

When neither X nor any other agent has claimed `file` via intent, the file is uncoordinated — no violation is reported. (Forward-conflict and in-flight detection still apply through their respective triggers.)

The `agent.feedback` to the violator SHALL fire regardless of `escalate_on_violation`. Only the supervisor-bound `agent.question` is gated by that flag.

#### Scenario: Violator is warned when editing a file inside another agent's intent

- **GIVEN** `feat-x` has an active intent for `["src/a.rs"]`
- **AND** `feat-y` has an active intent for `["src/b.rs"]`
- **WHEN** `feat-y` publishes `agent.status` with `modified_files = ["src/a.rs"]`
- **THEN** an `agent.feedback` SHALL be emitted to `feat-y` whose error text contains `[conflict-detector] ownership violation`, `src/a.rs`, and `feat-x`

#### Scenario: Ownership escalation is gated by config flag

- **GIVEN** `escalate_on_violation = true`, `feat-x` intent for `["src/a.rs"]`, and `feat-y` intent for `["src/b.rs"]`
- **WHEN** `feat-y` publishes `agent.status` with `modified_files = ["src/a.rs"]`
- **THEN** an `agent.question` SHALL be emitted to inbox `"supervisor"` whose question text contains `src/a.rs`, `feat-y`, and `feat-x`

#### Scenario: Ownership escalation is suppressed when flag is false

- **GIVEN** `escalate_on_violation = false`
- **WHEN** the same ownership-violation conditions occur
- **THEN** an `agent.feedback` SHALL still be emitted to the violator
- **AND** no `agent.question` SHALL be emitted to inbox `"supervisor"` for this violation

#### Scenario: No violation when no other agent has claimed the file

- **GIVEN** the tracker contains no intent referencing `src/orphan.rs`
- **WHEN** `feat-y` publishes `agent.status` with `modified_files = ["src/orphan.rs"]`
- **THEN** no `agent.feedback` for ownership violation SHALL be emitted

#### Scenario: Violation is not re-emitted on repeated status

- **GIVEN** `feat-y` already received an ownership-violation warning for `src/a.rs` (claimed by `feat-x`)
- **WHEN** `feat-y` publishes another `agent.status` still containing `src/a.rs`
- **THEN** no new ownership-violation `agent.feedback` SHALL be emitted to `feat-y` for `src/a.rs`

### Requirement: Auto-emitted message conventions

Auto-emitted messages from the detector SHALL conform to the following conventions:

- `agent.feedback` messages SHALL set `payload.from = "supervisor"` and SHALL place at least one error string in `payload.errors` whose first non-whitespace token is `[conflict-detector]`.
- `agent.question` messages emitted to the supervisor inbox SHALL set `agent_id = "supervisor"` (the recipient — and, by the auto-emitted-detector convention, the sender-identification slot for this variant, since `QuestionPayload` has no `from` field), and SHALL include `[conflict-detector]` as a token in the question text.

These conventions SHALL apply to forward, in-flight, and ownership message paths.

#### Scenario: Auto-emitted feedback uses supervisor as the from field

- **WHEN** the detector emits any `agent.feedback`
- **THEN** the message has `payload.from = "supervisor"`
- **AND** at least one error string starts with the token `[conflict-detector]`

#### Scenario: Auto-emitted question is addressed to the supervisor inbox

- **WHEN** the detector emits any `agent.question`
- **THEN** the message has `agent_id = "supervisor"`
- **AND** the question text contains the token `[conflict-detector]`

#### Scenario: Auto-emitted question payload has no from field

- **WHEN** the detector emits any `agent.question`
- **THEN** the serialized JSON payload contains a `question` field
- **AND** the serialized JSON payload does NOT contain a `from` field (the `QuestionPayload` type has no such field)
- **AND** the sender-identification information is carried by the envelope `agent_id = "supervisor"`, not by a payload field

