## ADDED Requirements

### Requirement: The verify nudge names its trigger and tip

Every verify nudge the broker emits SHALL carry `trigger`, `"commit"` for a `committed` artifact and `"complete"` for a `done` artifact, and SHALL carry the triggering artifact's `sha` when it has one. A nudge without these fields SHALL still decode, so older readers keep working.

#### Scenario: A committed artifact nudges with trigger commit

- **GIVEN** the verify nudge is enabled
- **WHEN** `feat-x` publishes `agent.artifact { status: "committed", sha: "abc123" }`
- **THEN** the orchestrator inbox SHALL receive a verify nudge with `branch_id = "feat-x"`, `trigger = "commit"` and `sha = "abc123"`

### Requirement: Change complete nudges verification and a rewrite does not

When the verify nudge is enabled, the broker SHALL emit one verify nudge with `trigger = "complete"` for each `agent.artifact` with `status: "done"`. It SHALL emit no verify nudge for `status: "rewritten"`. Disabling the nudge SHALL suppress both triggers.

#### Scenario: A done artifact nudges with trigger complete

- **GIVEN** the verify nudge is enabled
- **WHEN** `feat-x` publishes `agent.artifact { status: "done", sha: "def456" }`
- **THEN** the orchestrator inbox SHALL receive one verify nudge with `trigger = "complete"` and `sha = "def456"`

#### Scenario: A rewrite draws no nudge

- **GIVEN** the verify nudge is enabled
- **WHEN** an `agent.artifact { status: "rewritten" }` for `feat-x` is accepted
- **THEN** no verify nudge SHALL be emitted

### Requirement: The orchestrator skill freezes a branch before its gates and gates on change complete

The orchestrator skill SHALL instruct the orchestrator to run the full verification gates when a `complete` nudge arrives, to freeze the branch with `sweep.sh freeze` before the first gate starts, and to lift the freeze only through `sweep.sh verified` or `sweep.sh feedback-gate`. It SHALL describe `commit` nudges as optional early checks.

#### Scenario: Skill orders freeze before gates

- **WHEN** the bundled orchestrator skill is inspected
- **THEN** it SHALL state that `sweep.sh freeze <agent>` runs before the first gate on a `complete` nudge
- **AND** it SHALL state that the freeze is lifted by `verified` or `feedback-gate`
