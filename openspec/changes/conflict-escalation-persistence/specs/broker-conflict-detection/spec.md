## MODIFIED Requirements

### Requirement: In-flight conflict detection

When an `agent.status` from agent X carrying `modified_files` is published (typically by the filesystem watcher), the detector SHALL track X's current modified-file set, replacing any previous set for X.

For every other agent Y whose current modified-file set is non-empty, the detector SHALL compute the overlap between X's and Y's modified files. For each `file` in the overlap, ordered as `(min(X, Y), max(X, Y))`:

- If the triple `(min, max, file)` is being seen for the first time AND no escalation record exists for that pair and file, the detector SHALL record `first_seen = now` and SHALL emit an `agent.feedback` to both X and Y with `from = "supervisor"` and an error message containing the substring `[conflict-detector] in-flight conflict` and the `file` path. This warning is the *initial* warning for the pair on that file.
- If the triple has been seen for at least `[supervisor.conflict] window_seconds` and has not yet had its escalation decision made, the detector SHALL classify the overlap on `file` as **additive** or **true** using the two agents' active-intent region declarations for `file` (the `regions` carried on `agent.intent` per `conflict-detector-fn-granularity`), then act as follows:
  - The overlap SHALL be classified **true** when the detector cannot prove the agents' edits are disjoint — specifically, when at least one of X or Y has no active intent for `file`, OR at least one declares `file` at file level (no regions), OR both declare regions for `file` and those region sets intersect under the `conflict-detector-fn-granularity` intersection rules (same named region / same insertion anchor, overlapping line ranges, or a conservative cross-kind named-vs-range comparison).
  - The overlap SHALL be classified **additive** only when BOTH X and Y declare at least one region for `file` AND their region sets for `file` are disjoint (do not intersect) — i.e. well-separated hunks or differently-named regions.
  - For a **true** overlap, the detector SHALL emit an `agent.question` to inbox `"supervisor"` with `from = "supervisor"` and question text containing the substring `[conflict-detector]`, the `file` path, both agent_ids, and an indication that the window elapsed without resolution.
  - For an **additive** overlap, the detector SHALL NOT emit an `agent.question`. Instead it SHALL emit exactly one informational `agent.feedback` to both X and Y with `from = "supervisor"` and an error message containing the substring `[conflict-detector]`, an indication that the file is shared but the changes are additive (e.g. "shared file, additive — resolve at merge"), and the `file` path.
  - In both the true and additive cases the triple SHALL be marked as having had its escalation decision made; subsequent ticks SHALL NOT re-emit the escalation `agent.question` nor re-emit the additive `agent.feedback` while the triple's region declarations are unchanged. The triple SHALL remain recorded in the in-flight tracker (it SHALL NOT be removed by the decision itself) so the overlap is never silently dropped.
- If `file` no longer appears in the intersection of X's and Y's modified files (one of them stopped touching it), the triple SHALL be removed from the in-flight tracker — the conflict has resolved without escalation.

The escalation record for a pair and file SHALL be durable independently of the triple's residency in the in-flight tracker. Removing a triple from the tracker SHALL NOT discard an escalation decision already made for that pair and file, and SHALL NOT reset its `first_seen` clock. A transient emptying of an agent's `modified_files` — which a commit produces — therefore SHALL NOT cause the same conflict to be escalated a second time when the file reappears. The escalation record SHALL be discarded only when the conflict is genuinely over, or when the agents' region declarations for that file materially change, in which case a fresh escalation is permitted.

The detector SHALL accept an acknowledgement for a pair and file, recording that the conflict has been adjudicated. While an acknowledgement stands, the detector SHALL NOT emit a further `agent.question` for that pair and file. The acknowledgement SHALL be invalidated when the agents' region declarations for that file materially change, so a conflict that genuinely changes shape can escalate again. An acknowledgement SHALL NOT remove the overlap from the in-flight tracker, so the conflict remains visible.

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

#### Scenario: A commit does not re-escalate an already-escalated conflict

- **GIVEN** an escalated conflict `(feat-x, feat-y, src/a.rs)`
- **WHEN** `feat-x` publishes `agent.status` with `modified_files = []` (its commit), and then republishes `modified_files = ["src/a.rs"]`
- **AND** more than `window_seconds` elapses with both agents reporting the file
- **THEN** no further `agent.question` SHALL be emitted for that pair and file
- **AND** the escalation record SHALL still reflect the earlier decision

#### Scenario: A commit does not reset the first-seen clock

- **GIVEN** an unescalated in-flight overlap `(feat-x, feat-y, src/a.rs)` first seen `T` ago, where `T` is less than `window_seconds`
- **WHEN** `feat-x` briefly publishes `modified_files = []` and then republishes the file
- **THEN** the overlap's `first_seen` SHALL remain the original time
- **AND** the escalation SHALL fire once `window_seconds` has elapsed from that original time, not from the republish

#### Scenario: An acknowledged conflict is not escalated again

- **GIVEN** an escalated conflict `(feat-x, feat-y, src/a.rs)` that has been acknowledged
- **WHEN** subsequent detector ticks run while both agents still touch the file, including across commits
- **THEN** no further `agent.question` SHALL be emitted for that pair and file
- **AND** the overlap SHALL remain present in the in-flight tracker

#### Scenario: A materially changed conflict escalates again despite acknowledgement

- **GIVEN** an acknowledged conflict `(feat-x, feat-y, src/a.rs)`
- **WHEN** the agents' region declarations for `src/a.rs` materially change
- **THEN** the acknowledgement SHALL be invalidated
- **AND** a further `agent.question` MAY be emitted for the changed conflict

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
