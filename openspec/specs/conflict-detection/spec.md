# conflict-detection Specification

## Purpose
A broker-internal detector, active only in supervisor mode, that tracks agent intents and modified-file sets to flag three classes of coordination conflict — forward (overlapping intents), in-flight (concurrent edits to the same file), and ownership violations (editing files claimed by another agent) — auto-emitting `agent.feedback` to the involved agents and escalating unresolved collisions to the supervisor inbox. It further extends `agent.intent` file entries with optional sub-file `regions` (function, class, block, or line range) so the detector can distinguish disjoint edits within a shared file from true collisions, falling back to file-level detection whenever regions are omitted to preserve v0.5.0 safety.

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
- If the triple has been seen for at least `[supervisor.conflict] window_seconds` and has not yet had its escalation decision made, the detector SHALL classify the overlap on `file` as **additive** or **true** using the two agents' active-intent region declarations for `file` (the `regions` carried on `agent.intent` per `conflict-detector-fn-granularity`), then act as follows:
  - The overlap SHALL be classified **true** when the detector cannot prove the agents' edits are disjoint — specifically, when at least one of X or Y has no active intent for `file`, OR at least one declares `file` at file level (no regions), OR both declare regions for `file` and those region sets intersect under the `conflict-detector-fn-granularity` intersection rules (same named region / same insertion anchor, overlapping line ranges, or a conservative cross-kind named-vs-range comparison).
  - The overlap SHALL be classified **additive** only when BOTH X and Y declare at least one region for `file` AND their region sets for `file` are disjoint (do not intersect) — i.e. well-separated hunks or differently-named regions.
  - For a **true** overlap, the detector SHALL emit an `agent.question` to inbox `"supervisor"` with `from = "supervisor"` and question text containing the substring `[conflict-detector]`, the `file` path, both agent_ids, and an indication that the window elapsed without resolution.
  - For an **additive** overlap, the detector SHALL NOT emit an `agent.question`. Instead it SHALL emit exactly one informational `agent.feedback` to both X and Y with `from = "supervisor"` and an error message containing the substring `[conflict-detector]`, an indication that the file is shared but the changes are additive (e.g. "shared file, additive — resolve at merge"), and the `file` path.
  - In both the true and additive cases the triple SHALL be marked as having had its escalation decision made; subsequent ticks SHALL NOT re-emit the escalation `agent.question` nor re-emit the additive `agent.feedback` while the triple's region declarations are unchanged. The triple SHALL remain recorded in the in-flight tracker (it SHALL NOT be removed by the decision itself) so the overlap is never silently dropped.
- If `file` no longer appears in the intersection of X's and Y's modified files (one of them stopped touching it), the triple SHALL be removed from the in-flight tracker — the conflict has resolved without escalation.

#### Scenario: Two agents touching the same file are warned

- **GIVEN** a running detector and `feat-x` has `modified_files = ["src/a.rs"]`
- **WHEN** `feat-y` publishes `agent.status` with `modified_files = ["src/a.rs"]`
- **THEN** an `agent.feedback` SHALL be emitted to `feat-x` whose error text contains `[conflict-detector] in-flight conflict` and `src/a.rs`
- **AND** an `agent.feedback` SHALL be emitted to `feat-y` with the same content

#### Scenario: True collision (same anchor) escalates after the configured window

- **GIVEN** the in-flight tracker has carried `(feat-x, feat-y, coordination.md)` for at least `window_seconds`
- **AND** `feat-x` and `feat-y` both have active intents declaring a region on `coordination.md` whose ranges/anchors intersect (e.g. both inserting at the same anchor)
- **AND** both agents still report `coordination.md` in their modified_files
- **WHEN** the detector tick runs
- **THEN** an `agent.question` SHALL be emitted to inbox `"supervisor"` whose question text contains `[conflict-detector]`, `coordination.md`, `feat-x`, and `feat-y`
- **AND** the triple SHALL be marked as having had its escalation decision made

#### Scenario: Additive overlap is downgraded, not escalated to the human

- **GIVEN** the in-flight tracker has carried `(feat-x, feat-y, src/config.rs)` for at least `window_seconds`
- **AND** `feat-x` declared `range { start_line: 10, end_line: 30 }` and `feat-y` declared `range { start_line: 80, end_line: 120 }` on `src/config.rs` (disjoint, well-separated regions)
- **AND** both agents still report `src/config.rs` in their modified_files
- **WHEN** the detector tick runs
- **THEN** no `agent.question` SHALL be emitted to inbox `"supervisor"` for `src/config.rs`
- **AND** an informational `agent.feedback` SHALL be emitted whose error text contains `[conflict-detector]`, indicates the file is shared and additive (resolve at merge), and contains `src/config.rs`

#### Scenario: Additive downgrade records the overlap and does not re-emit

- **GIVEN** an in-flight triple `(feat-x, feat-y, src/config.rs)` that was downgraded as additive on a prior tick
- **AND** both agents still report `src/config.rs` and their region declarations are unchanged
- **WHEN** subsequent detector ticks run
- **THEN** the in-flight tracker SHALL still contain the triple (the overlap is recorded, not dropped)
- **AND** no additional `agent.feedback` SHALL be emitted for the additive downgrade
- **AND** no `agent.question` SHALL be emitted for the triple

#### Scenario: Conservative escalation when regions are not declared

- **GIVEN** the in-flight tracker has carried `(feat-x, feat-y, src/a.rs)` for at least `window_seconds`
- **AND** neither `feat-x` nor `feat-y` declared regions for `src/a.rs` (file-level intents or no active intent)
- **AND** both agents still report `src/a.rs` in their modified_files
- **WHEN** the detector tick runs
- **THEN** an `agent.question` SHALL be emitted to inbox `"supervisor"` whose question text contains `[conflict-detector]`, `src/a.rs`, `feat-x`, and `feat-y`

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

### Requirement: Optional regions field on agent.intent files

The `agent.intent` broker message variant SHALL accept each
`files` entry in one of two shapes: a plain string (the
v0.5.0 file-level form) OR an object `{ path: string,
regions?: Region[] }`. Both shapes SHALL be accepted within
the same `files` array (mixed entries permitted). Omitting
`regions` from an object entry SHALL be equivalent to using
the plain string form.

#### Scenario: String entry parses as file-level intent

- **WHEN** an intent message with `"files": ["src/main.rs"]`
  is published
- **THEN** the broker SHALL accept it and the file SHALL be
  treated as file-level (no regions declared)

#### Scenario: Object entry with regions parses correctly

- **WHEN** an intent message with
  `"files": [{ "path": "src/auth.rs",
    "regions": [{ "kind": "function", "name":
    "validate_token" }] }]` is published
- **THEN** the broker SHALL accept it and the file SHALL
  carry the declared regions

#### Scenario: Mixed string and object entries accepted

- **WHEN** an intent's `files` array contains both string and
  object entries
- **THEN** the broker SHALL accept the mixed shape, treating
  each entry per its own shape

### Requirement: Four region kinds

The system SHALL accept exactly four region kinds in v0.6.0:
`function { name }`, `class { name }`, `block { anchor }`,
and `range { start_line, end_line }`. Unknown `kind` values
SHALL cause the broker to reject the publish with a 400-
class error.

#### Scenario: Known kinds round-trip cleanly

- **WHEN** an intent is published with one region of each
  documented kind
- **THEN** the broker SHALL accept and route the message,
  preserving each region's structure

#### Scenario: Unknown kind is rejected loudly

- **WHEN** an intent publishes a region with `kind:
  "macro"` (not in the v0.6.0 set)
- **THEN** the broker SHALL reject the publish with a
  message identifying the offending region

### Requirement: Region-aware forward-conflict detection

The forward-conflict detector SHALL evaluate per-file
overlap as follows:

- When both intents declare regions for a shared file, the
  detector SHALL trigger only when at least one pair of
  regions intersects.
- When at least one intent omits regions for a shared file,
  the detector SHALL fall back to file-level conflict
  (v0.5.0 behaviour).
- Region intersection rules:
  - Same kind + matching `name` (for function/class/block)
    → intersect. Name matching SHALL compare NORMALIZED
    names: case-folded, trimmed, with separator characters
    (space, underscore, hyphen) collapsed to a single form,
    a trailing `()` stripped, and a leading declaration
    keyword (`fn`, `def`, `function`, `class`) stripped —
    so spelling variants of the same symbol intersect.
  - Named-vs-named comparisons across DIFFERENT kinds
    (function vs class vs block) with matching normalized
    names SHALL be treated as intersecting conservatively,
    with the same conservative-comparison hint as the
    named-vs-range rule.
  - Two `range` regions with overlapping
    `[start_line, end_line]` intervals → intersect.
  - Cross-kind comparisons (named vs range) SHALL be
    treated as intersecting conservatively (we cannot
    resolve names to lines without source parsing).

#### Scenario: Non-overlapping functions in the same file do not conflict

- **GIVEN** intents A and B both naming `src/auth.rs`, with
  A declaring `function validate_token` and B declaring
  `function refresh_session`
- **WHEN** the forward-conflict detector runs
- **THEN** the detector SHALL NOT trigger a conflict

#### Scenario: Overlapping functions in the same file conflict

- **GIVEN** intents A and B both declaring
  `function validate_token` on `src/auth.rs`
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a forward-conflict
  warning identifying both branches and the intersecting
  function

#### Scenario: Spelling variants of the same symbol intersect

- **GIVEN** intent A declaring `function validate_token` and
  intent B declaring `function Validate Token()` on the same
  file
- **WHEN** the detector runs
- **THEN** normalization SHALL equate the two names and the
  detector SHALL trigger a forward-conflict warning

#### Scenario: Named regions of different kinds with the same name intersect conservatively

- **GIVEN** intent A declaring `function DEV_ALLOWLIST_PRESET`
  and intent B declaring `block DEV_ALLOWLIST_PRESET` on the
  same file
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a conflict and SHALL
  include a hint that the comparison was conservative

#### Scenario: File-level fallback when regions omitted

- **GIVEN** intent A naming `src/auth.rs` with regions
  declared, and intent B naming `src/auth.rs` as a plain
  string (no regions)
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a file-level conflict
  (preserving v0.5.0 safety)

#### Scenario: Cross-kind comparison intersects conservatively

- **GIVEN** intent A declaring
  `function validate_token` on `src/auth.rs` and intent B
  declaring `range { start_line: 10, end_line: 50 }` on
  the same file
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a conflict and SHALL
  include a hint that the cross-kind comparison was
  conservative

#### Scenario: Overlapping ranges intersect

- **GIVEN** intent A declaring
  `range { 10, 30 }` and intent B declaring
  `range { 25, 45 }` on the same file
- **WHEN** the detector runs
- **THEN** the detector SHALL trigger a conflict naming the
  overlapping range

#### Scenario: Non-overlapping ranges do not intersect

- **GIVEN** intent A declaring
  `range { 10, 20 }` and intent B declaring
  `range { 30, 40 }` on the same file
- **WHEN** the detector runs
- **THEN** the detector SHALL NOT trigger a conflict on
  that file

### Requirement: Detector warning identifies intersecting regions

The detector SHALL name the intersecting regions explicitly
in any warning it produces (supervisor pane prose,
`agent.feedback` message, or learnings record) so consumers
can act on them. This applies whenever a region-level
conflict triggers.

#### Scenario: Warning enumerates the intersecting regions

- **GIVEN** a region-level conflict on two functions in
  `src/auth.rs`
- **WHEN** the warning is produced
- **THEN** the warning text SHALL list each intersecting
  region with its kind and name (or range)

### Requirement: Coordination skill teaches region declaration

The bundled `assets/agent-skills/coordination.md` SHALL
include guidance on when to declare regions, when to omit
them, and explicit language forbidding manufactured-narrow
regions to dodge conflict warnings.

The region-declaration prose SHALL additionally instruct
agents to: declare region names using the CANONICAL symbol
spelling exactly as it appears in source; declare ALL
regions the work touches, including shared constant blocks,
import sections, and asset files (not only the headline
function); and RE-PUBLISH `agent.intent` when the work's
scope grows beyond the declared regions mid-task.

#### Scenario: Skill prose covers when to declare and when to omit

- **WHEN** the forward-coordination section of
  coordination.md is read
- **THEN** the new region-declaration prose SHALL include
  both "declare when..." and "skip when..." guidance with
  at least two examples per direction

#### Scenario: Skill prose forbids dodging the detector

- **WHEN** the region-declaration prose is read
- **THEN** the prose SHALL contain explicit language
  warning against manufacturing narrow regions to avoid the
  forward-conflict warning, with a one-sentence rationale

#### Scenario: Skill prose requires canonical names, full coverage, and re-publication

- **WHEN** the region-declaration prose is read
- **THEN** it SHALL instruct canonical source spelling for
  region names, declaring every touched region including
  shared blocks, and re-publishing `agent.intent` when scope
  grows

### Requirement: Backwards compatibility with v0.5.0 publishers

The system SHALL treat v0.5.0 publishers (intents whose
`files` array contains only plain strings) byte-equivalently
to v0.5.0. The detector SHALL produce the same conflict
warnings v0.5.0 would for the same string-only inputs.

#### Scenario: v0.5.0 publisher round-trip matches v0.5.0

- **GIVEN** an intent published with `"files":
  ["src/foo.rs", "src/bar.rs"]` (v0.5.0 shape)
- **WHEN** the broker stores and emits the message AND the
  detector evaluates it
- **THEN** the routed message and the detector's behaviour
  SHALL match v0.5.0 byte-for-byte for the same inputs

### Requirement: User guide includes a Conflict Detection chapter

The mdBook user guide SHALL include a chapter at
`docs/src/user-guide/conflict-detection.md` documenting the
broker's automatic conflict detection. The chapter SHALL cover
the three failure shapes (forward, in-flight, ownership), the
`[conflict-detector]` tag prefix on auto-emitted feedback, the
supervisor inbox routing for `agent.question` escalations, and
how detection interacts with the filesystem watcher's
auto-published `modified_files`.

The chapter SHALL be linked from `docs/src/SUMMARY.md` under the
User Guide group.

#### Scenario: Conflict detection chapter exists and is linked

- **WHEN** `docs/src/SUMMARY.md` is inspected
- **THEN** it contains a link to `user-guide/conflict-detection.md` under the User Guide section

#### Scenario: Conflict detection chapter describes the three failure shapes

- **WHEN** `docs/src/user-guide/conflict-detection.md` is inspected
- **THEN** it explains forward conflicts (overlapping `agent.intent`)
- **AND** it explains in-flight conflicts (overlapping `agent.status.modified_files`)
- **AND** it explains ownership violations

#### Scenario: Conflict detection chapter mentions the tag prefix

- **WHEN** `docs/src/user-guide/conflict-detection.md` is inspected
- **THEN** it contains the substring `[conflict-detector]`

#### Scenario: Conflict detection chapter documents supervisor inbox routing

- **WHEN** `docs/src/user-guide/conflict-detection.md` is inspected
- **THEN** it states that `agent.question` escalations are routed to the supervisor inbox
