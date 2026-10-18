## MODIFIED Requirements

### Requirement: No-orchestrator fallback preserves prior behavior

The drive loop SHALL record every judgment call it cannot make mechanically — a live permission prompt, an ambiguous `agent.question`, a `committed`/`done` artifact's merge decision, and a correction-exhausted branch — as a broker review item, and SHALL NOT inject anything into any surface. With no orchestrator pane inside the session this is the only escalation path in this release, and it is identical to the pre-relocation behaviour of a pure `--unattended` run with no orchestrator pane: the human remains the exception handler, re-engaged via the existing escalation record and heartbeat.

#### Scenario: Escalation is recorded as a broker review item only

- **GIVEN** a `--unattended` run and a worker showing a live permission prompt
- **WHEN** the drive loop escalates it
- **THEN** the loop SHALL record the escalation as a broker review item
- **AND** SHALL NOT inject anything into any surface
- **AND** SHALL continue progressing the other workers without blocking

#### Scenario: Merge decision is recorded, not injected

- **GIVEN** a `--unattended` run and a worker publishing an `agent.artifact` with status `committed`
- **WHEN** the drive loop processes it
- **THEN** the loop SHALL record the merge decision on the broker
- **AND** SHALL NOT type into any surface

## REMOVED Requirements

### Requirement: Judgment calls are handed to the orchestrator pane

**Reason**: There is no orchestrator pane; the orchestrator runs outside the session.

**Migration**: Judgment calls remain broker review items (see the modified fallback requirement). Delivery of questions and merge decisions to a headless, one-shot orchestrator invocation is specified by the `orchestrator-escalation-handoff` change; permission prompts are never handed to an orchestrator invocation (`supervisor-unattended-operation`, *Every live permission prompt is escalated without blocking the wave*).

### Requirement: Orchestration-sweep nudge on a longer cadence

**Reason**: There is no pane to nudge, and a periodic one-shot invocation would spend a fresh context on nothing; merge sequencing is already event-driven (each `committed`/`done` artifact is an escalation).

**Migration**: None. Orchestration reconsideration happens when the external orchestrator is invoked for an event or driven by the operator.
