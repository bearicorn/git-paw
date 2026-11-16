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
