# supervisor-autonomous-orchestrator Specification

## Purpose
The pump-to-brain layer of unattended operation. When a supervisor (orchestrator) pane is present, the drive loop hands each judgment call it cannot make mechanically — a `danger`/`unknown` prompt, a worker's `agent.question`, a `committed`/`done` artifact's merge decision, and a non-converging (correction-exhausted) branch — to the orchestrator pane by injecting it as a task prompt, in addition to the uniform broker record, so the smart model is actively triggered rather than relied on to poll an inbox nobody reads. A longer-cadence nudge asks the orchestrator to re-run its sweep; worker-directed nudges stop only once a worker has finished (`done`/`verified`), never for the `blocked`/`committed` states the correction loop must still reach; injection is skipped while the pane shows a prompt or is mid-response and never blocks the wave; and with no supervisor pane nothing is injected, so a pure `--unattended` run is byte-for-byte the prior behavior. The orchestrator's judgment-call responsibilities are documented project-agnostically in the bundled supervisor skill.
## Requirements
### Requirement: Worker-directed nudges skip finished workers

Any nudge the drive loop directs at a worker pane SHALL be suppressed when that
worker has finished — its broker status is `done` or `verified`; a finished worker
SHALL never receive an idle nudge. The terminal signal SHALL be the worker's broker
status, NOT a pane-content diff, because a finished worker's pane is also unchanging
and a content diff cannot distinguish "idle because done" from "idle because stuck."

The suppressed set SHALL be the two FINISHED statuses only, and SHALL NOT include
`blocked` or `committed`. Those two are exactly the states a worker occupies while
awaiting correction — a worker that has committed and is standing by for
re-verification, or one blocked and not polling its inbox — so suppressing them
would silently disable the `supervisor-correction-loop` re-engagement whose stated
purpose is to reach them. "Finished" and "quiet" are different conditions, and only
the former ends a worker's eligibility for a nudge.

#### Scenario: A verified worker is never nudged

- **GIVEN** a worker that has published a finished status of `verified`
- **WHEN** the drive loop considers nudging its pane
- **THEN** the loop SHALL send no nudge to that worker's pane
- **AND** SHALL make this determination from the worker's broker status, not from whether its pane output changed

#### Scenario: A working worker remains eligible for a nudge

- **GIVEN** a worker in a non-finished status (e.g. `working`)
- **WHEN** the drive loop evaluates it for a nudge
- **THEN** the worker SHALL remain eligible to receive a nudge

#### Scenario: A committed or blocked worker awaiting correction is still nudged

- **GIVEN** a worker whose broker status is `committed` or `blocked` and which has a pending gate-failure correction
- **WHEN** the drive loop evaluates it for a nudge
- **THEN** the worker SHALL remain eligible to receive a nudge
- **AND** the correction re-engagement SHALL reach its pane

### Requirement: No-orchestrator fallback preserves prior behavior

Every judgment call the drive loop cannot make mechanically — a live permission prompt, an ambiguous `agent.question`, a `committed`/`done` artifact's merge decision, and a correction-exhausted branch — SHALL be recorded as a broker review item, and the loop SHALL NOT inject anything into any surface. The loop SHALL publish the review item for a live permission prompt and for a correction-exhausted branch; for an ambiguous question and a merge decision the review item SHALL be the worker's own `agent.question` or `agent.artifact` message, which the loop leaves on the broker and SHALL NOT republish. With no orchestrator pane inside the session this is the only escalation path in this release, and it is identical to the pre-relocation behaviour of a pure `--unattended` run with no orchestrator pane: the human remains the exception handler, re-engaged via the existing escalation record and heartbeat.

#### Scenario: Escalation is recorded as a broker review item only

- **GIVEN** a `--unattended` run and a worker showing a live permission prompt
- **WHEN** the drive loop escalates it
- **THEN** the loop SHALL record the escalation as a broker review item
- **AND** SHALL NOT inject anything into any surface
- **AND** SHALL continue progressing the other workers without blocking

#### Scenario: Merge decision is recorded, not injected

- **GIVEN** a `--unattended` run and a worker publishing an `agent.artifact` with status `committed`
- **WHEN** the drive loop processes it
- **THEN** the worker's artifact SHALL remain the broker record of the merge decision, with no copy published by the loop
- **AND** the loop SHALL NOT type into any surface

### Requirement: Orchestrator judgment-call responsibilities are documented in the skill

The bundled supervisor/orchestrator skill SHALL document the orchestrator's
judgment-call responsibilities so the smart model knows how to act when handed one:
(a) dependency-aware spawn order derived from `agent.intent` / conflict edges, (b)
answering ambiguous `agent.question`s from specs and cross-agent state, escalating
to the human only when genuinely undecidable, (c) merge sequencing, and (d)
declaring a worker unrecoverable per the `supervisor-correction-loop`
`max_cycles` / `on_exhausted` law. These responsibilities SHALL be
project-agnostic — no consumer-specific toolchain baked into the exported skill.

#### Scenario: The skill enumerates the four responsibilities

- **WHEN** the bundled supervisor/orchestrator skill content is inspected
- **THEN** it SHALL enumerate dependency-aware spawn order, answering ambiguous questions, merge sequencing, and declaring a worker unrecoverable

#### Scenario: The documented responsibilities are project-agnostic

- **WHEN** the exported orchestrator skill is inspected
- **THEN** its judgment-call guidance SHALL NOT hard-code any consumer's specific test/build/stack toolchain
- **AND** SHALL express the responsibilities in terms every consumer project can supply via its own injected instructions

