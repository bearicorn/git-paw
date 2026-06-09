# per-commit-verification Specification

## Purpose
TBD - created by archiving change per-commit-verification-v0-6-x. Update Purpose after archive.
## Requirements
### Requirement: Skill mandates per-event verification

The bundled supervisor skill SHALL include a "Verify on each
event, never batch" subsection stating in MUST/MUST-NOT terms
that the supervisor verifies each agent's commit as its
`committed` event arrives and SHALL NOT defer verification to
batch it with other agents' commits. The subsection SHALL
name the wave-1 batching failure mode by example.

#### Scenario: Skill contains the per-event rule

- **WHEN** the bundled `supervisor.md` is inspected
- **THEN** it SHALL contain a "Verify on each event"
  subsection with MUST/MUST-NOT language and a worked
  example of the batching anti-pattern

#### Scenario: Dependency-driven deferral remains permitted

- **WHEN** the subsection is read
- **THEN** it SHALL state that the only acceptable deferral
  reason is a genuine dependency (one agent's work requires
  another's merge first), which the supervisor SHALL state
  explicitly when deferring

### Requirement: Optional verify-now broker nudge

The broker SHALL, when
`[supervisor].verify_on_commit_nudge` is `true` (default),
publish a `supervisor.verify-now` message to the supervisor
inbox upon receiving `agent.artifact { status: "committed" }`.
The nudge SHALL carry the committing `branch_id`. When the
config field is `false`, no nudge SHALL be published.

#### Scenario: Nudge published on committed event

- **GIVEN** `verify_on_commit_nudge = true` (or unset)
- **WHEN** the broker receives an
  `agent.artifact { status: "committed" }` from `feat/foo`
- **THEN** the broker SHALL publish a `supervisor.verify-now`
  message carrying `branch_id: "feat/foo"` to the supervisor
  inbox

#### Scenario: Nudge suppressed when disabled

- **GIVEN** `[supervisor].verify_on_commit_nudge = false`
- **WHEN** the broker receives a committed artifact
- **THEN** no `supervisor.verify-now` message SHALL be
  published

#### Scenario: Default config enables the nudge

- **GIVEN** no `verify_on_commit_nudge` field in config
- **WHEN** a committed artifact arrives
- **THEN** the nudge SHALL be published (default true)

### Requirement: Skill permits concurrent verification

The supervisor skill SHALL state that verifying one agent's
commit does not block starting another agent's verification,
since gate sweeps run per-branch in isolated worktrees.

#### Scenario: Concurrency permission documented

- **WHEN** the "Verify on each event" subsection is read
- **THEN** it SHALL state that per-branch verifications may
  run concurrently and that verifying agent A does not block
  verifying agent B

### Requirement: Stack-agnostic phrasing

The new subsection SHALL pass the no-language-leak audit from
[[lang-agnostic-assets]].

#### Scenario: No-leak audit passes

- **WHEN** the no-leak audit runs against the updated
  supervisor.md
- **THEN** the audit SHALL pass across all supported spec
  backends

