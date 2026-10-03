## MODIFIED Requirements

### Requirement: Judgment calls are handed to the orchestrator pane

The drive loop SHALL hand a live judgment call to the orchestrator by injecting it as a prompt into the orchestrator's pane (in addition to recording it on the broker) whenever an orchestrator (supervisor CLI) pane is present under unattended operation. A judgment call is a live permission prompt on a worker pane, an ambiguous `agent.question`, or a merge decision. The loop SHALL NOT block the wave waiting on the orchestrator's response.

A permission-prompt hand-off SHALL name the agent and the prompted command and SHALL ask the orchestrator to relay the prompt to the human. It SHALL NOT ask the orchestrator to decide or approve the prompt itself.

#### Scenario: A live permission prompt is injected into the orchestrator pane

- **GIVEN** an unattended session with an orchestrator pane present and a worker showing a live permission prompt
- **WHEN** the drive loop processes the escalation
- **THEN** the loop SHALL inject the hand-off into the orchestrator's pane
- **AND** SHALL still record the escalation on the broker
- **AND** SHALL continue progressing the other workers without blocking

#### Scenario: The permission-prompt hand-off asks for a relay, not a decision

- **GIVEN** a permission-prompt escalation for agent `feat-a` and command `ls -la`
- **WHEN** its hand-off text is rendered
- **THEN** the text SHALL name `feat-a` and `ls -la` and ask the orchestrator to relay the prompt to the human
- **AND** SHALL NOT tell the orchestrator to decide the prompt or approve the pane itself

#### Scenario: An ambiguous question is injected into the orchestrator pane

- **GIVEN** an orchestrator pane is present and a worker publishes an ambiguous `agent.question`
- **WHEN** the drive loop processes it
- **THEN** the loop SHALL inject the question into the orchestrator's pane for the smart model to answer

#### Scenario: A merge decision is injected into the orchestrator pane

- **GIVEN** an orchestrator pane is present and a worker publishes an `agent.artifact` whose status is `committed` or `done`
- **WHEN** the drive loop processes it
- **THEN** the loop SHALL inject a merge-decision hand-off into the orchestrator's pane so it verifies the branch and sequences its merge
- **AND** SHALL NOT inject a merge-decision hand-off for any other artifact status

### Requirement: Orchestration-sweep nudge on a longer cadence

The drive loop SHALL nudge the orchestrator pane to run an orchestration sweep on a
cadence longer than its per-tick pane sweep, so the orchestrator periodically
reconsiders spawn order, merge sequencing, and blocked workers without a human
trigger. The nudge SHALL follow the send-keys nudge discipline — the text followed
by a separate `Enter` keystroke.

#### Scenario: Orchestration nudge fires on the longer cadence

- **GIVEN** an unattended session with an orchestrator pane present
- **WHEN** the longer orchestration-nudge cadence elapses without a completion condition
- **THEN** the loop SHALL inject an orchestration-sweep nudge into the orchestrator's pane
- **AND** SHALL send the nudge text followed by a separate `Enter`
