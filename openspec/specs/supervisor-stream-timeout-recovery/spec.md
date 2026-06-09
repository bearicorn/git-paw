# supervisor-stream-timeout-recovery Specification

## Purpose
TBD - created by archiving change supervisor-stream-timeout-recovery. Update Purpose after archive.
## Requirements
### Requirement: Stream-timeout recovery section in supervisor skill

The bundled supervisor skill SHALL include a "Stream-timeout
recovery" section teaching the supervisor LLM how to recover
from API stream timeouts mid-sweep. The section SHALL contain
four ordered pieces: error-shape recognition, pre-action
checkpoint, replay-missing-publishes recovery, and a
confirmation rule.

#### Scenario: Section exists with the four pieces in recovery order

- **WHEN** the bundled `supervisor.md` is inspected
- **THEN** the file SHALL contain a "Stream-timeout recovery"
  heading whose subsections cover error-shape recognition,
  pre-action checkpoint, replay-missing-publishes, and the
  confirmation rule, in that order

### Requirement: Error-shape recognition

The skill SHALL describe the visible symptoms of an API
stream timeout (mid-stream cutoff, transport error in the CLI
output, or equivalent) so the supervisor LLM names the failure
rather than continuing in silence. The phrasing SHALL be
generic enough to apply across CLI variants (claude,
claude-oss, future entries).

#### Scenario: Symptoms are named generically across CLIs

- **WHEN** the error-shape subsection is read
- **THEN** the prose SHALL describe at least two visible
  symptom patterns (e.g. "mid-stream cutoff" and "transport
  error / stream error in the CLI output") and SHALL NOT name
  a specific CLI's exact error string

### Requirement: Pre-action checkpoint via agent.status

The skill SHALL teach the supervisor to publish an
`agent.status` "checkpoint" record before any sweep iteration
that will publish more than one downstream record (e.g.
multiple `agent.feedback` or `agent.verified`). The checkpoint
SHALL describe the intended sub-actions so the recovery path
has a re-entry point.

#### Scenario: Checkpoint shape is documented

- **WHEN** the pre-action checkpoint subsection is read
- **THEN** the prose SHALL show a concrete `agent.status`
  shape with `status: "checkpoint"` and a `summary` enumerating
  intended targets

#### Scenario: Checkpoint required only for multi-publish iterations

- **WHEN** the checkpoint subsection is read
- **THEN** the prose SHALL state that the checkpoint applies
  to iterations with more than one intended downstream publish,
  not every sweep

### Requirement: Replay-missing-publishes recovery

The skill SHALL teach the supervisor, on recovery from a
stream timeout, to re-read its prior checkpoint, poll each
intended target's `/messages/<branch_id>` stream to identify
which publishes completed, and re-publish only the missing
ones. The replay SHALL be idempotent so duplicate publishes
remain safe.

#### Scenario: Per-target poll-then-replay pattern documented

- **WHEN** the replay subsection is read
- **THEN** the prose SHALL show the per-target loop: poll the
  target's message stream for the supervisor's prior publish
  since the checkpoint timestamp, and re-publish when the
  prior publish is absent

### Requirement: Confirmation rule

The skill SHALL state explicitly that the supervisor SHALL
NOT advance to the next sub-action just because a `publish`
HTTP call returned. The system SHALL require either polling
the target's message stream to confirm or re-publishing
idempotently. The rule SHALL be marked prominently (bold,
callout, or equivalent) so it is unmissable.

#### Scenario: Confirmation rule appears prominently

- **WHEN** the confirmation rule is rendered in the skill
- **THEN** the rule SHALL appear with prominent formatting
  (bold, callout block, or similar), and SHALL pair the rule
  with a one-sentence rationale referencing stream-timeout
  risk

### Requirement: Recovery learning record

On every recovery from a stream timeout, the supervisor SHALL
publish an `agent.learning` record with `category =
"recovery_cycles"`. The record's body SHALL identify the
checkpoint id, the intended targets, the replayed targets,
and any skipped targets so recurrent timeouts surface in
qualitative-learnings output.

#### Scenario: Skill prose names the recovery learning trigger

- **WHEN** the replay subsection or its adjacent prose is
  read
- **THEN** the prose SHALL state explicitly that each
  successful recovery emits a `recovery_cycles`
  `agent.learning` record with a structured body covering
  checkpoint id and target lists

### Requirement: Stack-agnostic phrasing

The new section SHALL pass the no-language-leak audit from
[[lang-agnostic-assets]]. The section SHALL NOT use
Rust-specific or any other stack-specific language in its
prose or examples.

#### Scenario: No-leak audit passes against the new section

- **WHEN** the no-leak audit runs after the section lands
- **THEN** the audit SHALL pass on the rendered supervisor
  skill across all supported spec backends

