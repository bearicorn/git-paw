## ADDED Requirements

### Requirement: An acknowledgement also silences repeat in-flight warnings

While an acknowledgement stands for a pair and file, the detector SHALL NOT emit an in-flight `agent.feedback` warning or an additive-downgrade feedback for that pair and file, including when the overlap leaves and re-enters the in-flight view. An acknowledgement recorded before the first overlap SHALL still let the initial warning through.

#### Scenario: A re-entering acknowledged overlap is not re-warned

- **GIVEN** an acknowledged conflict `(feat-x, feat-y, src/a.rs)`
- **WHEN** `feat-x` commits, emptying its `modified_files`, and later edits `src/a.rs` again while `feat-y` still holds it
- **THEN** neither agent SHALL receive a further `[conflict-detector] in-flight conflict` feedback for `src/a.rs`

### Requirement: An acknowledgement outlives the overlap's absence

An acknowledged decision record SHALL NOT be disposed of by the absence retention. It SHALL end only when the agents' region declarations for the file materially change, when either agent is withdrawn, or when the broker stops. Unacknowledged records keep the existing retention.

#### Scenario: The question does not return after a quiet spell

- **GIVEN** an acknowledged conflict `(feat-x, feat-y, src/a.rs)` and a 60-second decision retention
- **WHEN** the overlap is absent for 10 minutes and then recurs for longer than the escalation window
- **THEN** no `agent.question` SHALL be emitted for that pair and file

#### Scenario: A material change still re-escalates

- **GIVEN** the acknowledged conflict above
- **WHEN** `feat-y` widens its intent for `src/a.rs` from a named region to the whole file and the overlap persists past the window
- **THEN** the detector SHALL emit an `agent.question` for that pair and file

### Requirement: A withdrawn agent leaves the conflict tracker

When an agent is withdrawn from the broker, the detector SHALL drop its current modified-file set, its active intent, and every in-flight triple, decision record and warned pair that involves it. No later message SHALL produce a warning or question that names the withdrawn agent until it publishes again.

#### Scenario: A removed agent's last edits raise no question

- **GIVEN** `feat-x` and `feat-y` both holding `src/a.rs` in their modified files
- **WHEN** `feat-x` is withdrawn and `feat-y` keeps editing `src/a.rs` past the escalation window
- **THEN** no feedback or question naming `feat-x` SHALL be emitted

### Requirement: The detector tells its own messages from the root's by sender

The detector SHALL decide whether a message is its own output by the detector id `conflict-detector` alone: it SHALL skip an `agent.feedback` whose `payload.from` is `conflict-detector` and an `agent.question` whose `agent_id` is `conflict-detector`. It SHALL honour a `[conflict-ack]` acknowledgement only when the feedback's `payload.from` is the root identity, and it SHALL NOT honour one whose `payload.from` is `conflict-detector`.

#### Scenario: The detector does not re-process its own warning

- **GIVEN** a running detector
- **WHEN** it emits an in-flight `agent.feedback` with `from = "conflict-detector"` to `feat-x`
- **THEN** that message SHALL NOT be fed back into the tracker and SHALL NOT cause a further emission

#### Scenario: The root's acknowledgement is honoured

- **GIVEN** an escalated conflict `(feat-x, feat-y, src/a.rs)`
- **WHEN** the root session publishes an `agent.feedback` carrying the `[conflict-ack]` form for that pair and file, with `from` set to the root identity
- **THEN** the detector SHALL record the acknowledgement

#### Scenario: An acknowledgement claiming the detector id is ignored

- **GIVEN** an unescalated in-flight overlap `(feat-x, feat-y, src/a.rs)`
- **WHEN** an `agent.feedback` carrying the `[conflict-ack]` form arrives with `from = "conflict-detector"`
- **THEN** the detector SHALL NOT record an acknowledgement
- **AND** the overlap SHALL still escalate once the window elapses

### Requirement: Detector message conventions

Auto-emitted messages from the detector SHALL carry the sender id `conflict-detector`, never the root identity, and SHALL follow these conventions:

- `agent.feedback` SHALL set `payload.from = "conflict-detector"`. At least one string in `payload.errors` SHALL start with the token `[conflict-detector]`.
- `agent.question` SHALL set `agent_id = "conflict-detector"`, its only sender slot. It SHALL go to the root inbox, and its text SHALL contain the token `[conflict-detector]`.

#### Scenario: Auto-emitted feedback uses the detector id as the from field

- **WHEN** the detector emits any `agent.feedback`
- **THEN** the message has `payload.from = "conflict-detector"`
- **AND** at least one error string starts with the token `[conflict-detector]`

#### Scenario: Auto-emitted question is sent by the detector to the root inbox

- **WHEN** the detector emits any `agent.question`
- **THEN** the message has `agent_id = "conflict-detector"`
- **AND** it is delivered to the root inbox
- **AND** the question text contains the token `[conflict-detector]`

#### Scenario: Auto-emitted question payload has no from field

- **WHEN** the detector emits any `agent.question`
- **THEN** the serialized JSON payload contains a `question` field
- **AND** the serialized JSON payload does NOT contain a `from` field (the `QuestionPayload` type has no such field)
- **AND** the sender-identification information is carried by the envelope `agent_id = "conflict-detector"`, not by a payload field

### Requirement: The detector id replaces the root identity as detector sender

Where another requirement of this capability says a detector message carries `from = "supervisor"` or `agent_id = "supervisor"` as its sender, that sender SHALL be read as `conflict-detector`, whichever spelling the root identity has. The detector id SHALL NOT appear as a row in the agent roster.

#### Scenario: An in-flight warning names the detector as sender

- **GIVEN** `feat-x` and `feat-y` both report `src/a.rs` in their modified files
- **WHEN** the detector emits the initial in-flight warning
- **THEN** both `agent.feedback` messages SHALL have `payload.from = "conflict-detector"`

#### Scenario: Detector output mints no roster row

- **GIVEN** a running detector that has emitted feedback and a question
- **WHEN** a client reads `GET /status`
- **THEN** no roster row SHALL have `agent_id = "conflict-detector"`

## REMOVED Requirements

### Requirement: Auto-emitted message conventions

**Reason**: Two of its scenario titles, "Auto-emitted feedback uses supervisor as the from field" and "Auto-emitted question is addressed to the supervisor inbox", name the root identity as the detector's sender. The detector now has its own sender id. OpenSpec rejects a scenario rename inside a MODIFIED block, so the requirement returns under a new name.

**Migration**: Replaced by "Detector message conventions" in this change. The `[conflict-detector]` tag rule and "Auto-emitted question payload has no from field" carry over. The two retitled scenarios now name `conflict-detector` as the sender. "Detector output mints no roster row" is new, in "The detector id replaces the root identity as detector sender".
