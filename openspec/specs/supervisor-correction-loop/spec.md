# supervisor-correction-loop Specification

## Purpose
The configurable self-healing correction loop that closes a gate-failure → re-engage → re-verify cycle with no human hop. Driven by an optional `[supervisor.correction]` table (`auto_loopback`, `max_cycles`, `on_exhausted`, `escalate_after_cycles`): when `auto_loopback` is enabled the unattended drive loop re-engages a gate-failed worker's pane with the gate feedback instead of parking it in an inbox the blocked worker never polls, bounds re-engagement by a per-branch cycle count, applies an `escalate` or `abandon` policy at `max_cycles` with an early `escalate_after_cycles` heads-up, and — when learnings are enabled — records a `correction_exhausted` learning capturing the branch, worker CLI, and cycle count for tiered-model tuning. Absent or with `auto_loopback` disabled, behavior is byte-for-byte the prior version.
## Requirements
### Requirement: Optional `[supervisor.correction]` configuration

The system SHALL accept an optional `[supervisor.correction]` config table whose
fields are all optional with documented defaults, so that an existing config that
omits the table loads without error and behaves exactly as the previous version.
The table SHALL define: `auto_loopback` (boolean, default `false`), `max_cycles`
(positive integer, default `5`), `on_exhausted` (one of `escalate` or `abandon`,
default `escalate`), and `escalate_after_cycles` (positive integer, default `3`).
An `on_exhausted` value outside the accepted set SHALL be a config error with an
actionable message, and a `max_cycles` or `escalate_after_cycles` of `0` SHALL
likewise be a config error naming the offending field.

#### Scenario: Table absent loads with defaults

- **WHEN** a config has `[supervisor] enabled = true` but no `[supervisor.correction]` table
- **THEN** the config SHALL load successfully
- **AND** the resolved correction settings SHALL be `auto_loopback = false`, `max_cycles = 5`, `on_exhausted = escalate`, `escalate_after_cycles = 3`

#### Scenario: Fields parse to the resolved policy

- **GIVEN** a `[supervisor.correction]` table setting `auto_loopback = true`, `max_cycles = 3`, `on_exhausted = "abandon"`
- **WHEN** the config is loaded
- **THEN** the resolved policy SHALL reflect those values
- **AND** the omitted `escalate_after_cycles` SHALL fall back to its default

#### Scenario: Invalid `on_exhausted` is a config error

- **GIVEN** a `[supervisor.correction]` table with `on_exhausted = "reboot"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an actionable error naming the accepted values

#### Scenario: A zero cycle count is a config error

- **GIVEN** a `[supervisor.correction]` table with `max_cycles = 0` or `escalate_after_cycles = 0`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an actionable error naming the offending field
- **AND** a value of `1` SHALL load successfully as the accepted floor

### Requirement: Automatic worker re-engagement on gate failure

When `auto_loopback` is enabled, the drive loop SHALL re-engage the worker whose
branch just failed a supervisor gate by injecting that gate's feedback into the
worker's pane, rather than only publishing `agent.feedback` to an inbox the
blocked worker does not poll. When `auto_loopback` is disabled (or the table is
absent), the system SHALL publish the feedback and SHALL NOT re-engage the pane —
behavior identical to the previous version.

#### Scenario: Gate failure re-engages the worker pane

- **GIVEN** `auto_loopback = true` and a worker whose branch just failed a gate
- **WHEN** the correction loop processes the failing gate feedback
- **THEN** the worker's pane SHALL receive the gate-tagged feedback and a re-engagement prompt
- **AND** the feedback SHALL still be recorded on the broker

#### Scenario: Disabled loopback preserves today's behavior

- **GIVEN** `auto_loopback = false` (or no `[supervisor.correction]` table)
- **WHEN** a gate fails
- **THEN** the feedback SHALL be published as before
- **AND** no re-engagement keystrokes SHALL be sent to the worker's pane

### Requirement: Bounded correction cycles

The system SHALL track a correction-cycle count per branch and SHALL stop
re-engaging a branch once its count reaches `max_cycles`. A branch that passes its
gate SHALL exit the correction cycle without reaching `max_cycles`.

#### Scenario: Cycles are bounded by `max_cycles`

- **GIVEN** `auto_loopback = true`, `max_cycles = 3`, and a branch that fails its gate on every attempt
- **WHEN** the correction loop has re-engaged the branch `max_cycles` times
- **THEN** the loop SHALL NOT re-engage that branch again
- **AND** the exhaustion policy SHALL be applied

#### Scenario: Passing the gate exits the cycle early

- **GIVEN** a branch under correction with a cycle count below `max_cycles`
- **WHEN** the branch passes its gate
- **THEN** the loop SHALL stop re-engaging that branch
- **AND** SHALL NOT apply the exhaustion policy

### Requirement: Exhaustion policy `on_exhausted`

When a branch reaches `max_cycles`, the system SHALL apply the configured
`on_exhausted` action. `escalate` SHALL flag the unrecoverable branch to the
orchestrator/human. `abandon` SHALL stop correcting the branch and mark it failed
without further re-engagement. In both cases the loop SHALL NOT continue to
re-engage the exhausted branch.

#### Scenario: `escalate` flags the branch

- **GIVEN** `on_exhausted = "escalate"` and a branch that has reached `max_cycles`
- **WHEN** the exhaustion policy is applied
- **THEN** the branch SHALL be flagged to the orchestrator/human as unrecoverable
- **AND** no further re-engagement SHALL occur for that branch

#### Scenario: `abandon` stops correcting the branch

- **GIVEN** `on_exhausted = "abandon"` and a branch that has reached `max_cycles`
- **WHEN** the exhaustion policy is applied
- **THEN** the branch SHALL be marked failed and left un-corrected
- **AND** no further re-engagement SHALL occur for that branch

### Requirement: Early escalation before exhaustion

The system SHALL flag a slow-converging worker to the orchestrator/human once, as
an early heads-up, when its correction-cycle count reaches `escalate_after_cycles`
before the branch has reached `max_cycles`, and SHALL continue re-engaging the
branch until `max_cycles`. When `escalate_after_cycles` is greater than or equal
to `max_cycles`, no distinct early flag SHALL be emitted.

#### Scenario: Early flag at `escalate_after_cycles`

- **GIVEN** `escalate_after_cycles = 2` and `max_cycles = 5`
- **WHEN** a branch's correction-cycle count reaches `2`
- **THEN** the loop SHALL emit an early heads-up flag for that branch
- **AND** SHALL continue re-engaging the branch on subsequent failures until `max_cycles`

### Requirement: Exhaustion learning emission

On exhaustion, when `[learnings]` is enabled, the system SHALL emit an
`agent.learning` with category `correction_exhausted` capturing the branch, the
worker CLI, and the cycle count, so an operator can tell when a worker's model
tier is under-powered for a class of task. When `[learnings]` is disabled, the
system SHALL NOT emit the learning — no telemetry without consent.

#### Scenario: Learning emitted when learnings enabled

- **GIVEN** `[learnings]` enabled and a branch that reaches `max_cycles`
- **WHEN** the exhaustion policy is applied
- **THEN** an `agent.learning` with category `correction_exhausted` SHALL be emitted
- **AND** it SHALL carry the branch, the worker CLI, and the cycle count

#### Scenario: No learning when learnings disabled

- **GIVEN** `[learnings]` disabled and a branch that reaches `max_cycles`
- **WHEN** the exhaustion policy is applied
- **THEN** no `correction_exhausted` learning SHALL be emitted

### Requirement: Gate vocabulary is configurable

The set of gate names the correction loop recognises as gate verdicts SHALL be configurable under `[supervisor.correction]`. The field SHALL default to git-paw's current gate list — `testing`, `regression`, `spec audit`, `doc audit`, `security audit`, and `scope` — so an installation that does not configure it behaves identically to before.

A consumer whose review process uses different gate names SHALL be able to name them, so their gate verdicts are recognised and start a correction cycle. A tagged message whose tag is not in the configured set SHALL continue to be treated as a non-gate producer and SHALL NOT start a correction cycle.

#### Scenario: Default vocabulary reproduces current behaviour

- **GIVEN** a configuration that does not set the gate vocabulary
- **WHEN** the correction loop evaluates a gate verdict
- **THEN** the recognised gate names SHALL be `testing`, `regression`, `spec audit`, `doc audit`, `security audit`, and `scope`

#### Scenario: Configured vocabulary is recognised

- **GIVEN** a configuration naming a custom gate
- **WHEN** a verdict tagged with that custom gate name arrives
- **THEN** it SHALL be recognised as a gate verdict
- **AND** it SHALL be eligible to start a correction cycle

#### Scenario: Unrecognised tag does not start a correction cycle

- **GIVEN** a configured gate vocabulary
- **WHEN** a tagged message arrives whose tag is not in that vocabulary
- **THEN** it SHALL NOT be treated as a gate verdict
- **AND** it SHALL NOT start a correction cycle

#### Scenario: Existing configuration loads unchanged

- **WHEN** a configuration written before this field existed is loaded
- **THEN** it SHALL load without error
- **AND** the gate vocabulary SHALL be the default list

