## MODIFIED Requirements

### Requirement: Escalation of risky and unknown prompts is non-blocking

When a live prompt is classified `danger` or `unknown` (not safe), the drive loop SHALL escalate it as a review item and SHALL NOT block the wave waiting on it. The loop SHALL surface the escalation (via the broker and in the exit summary) and SHALL continue sweeping and progressing the remaining agents. The wave SHALL NOT freeze indefinitely on a single risky prompt.

The escalation channel SHALL be **uniform and supervisor-agnostic**: the loop escalates the same way regardless of whether a supervisor is running. The escalation is a drainable review item — a running supervisor consumes it (see `supervisor-skill-discipline`), and when no supervisor is running it persists on the broker for the human/driver to read. The loop SHALL NOT detect supervisor liveness or route escalations differently based on it.

Correspondingly, the drive loop SHALL be the **sole approver of classifier-safe prompts**: it approves the `safe` set and escalates the rest. No other component blanket-approves safe prompts while the loop is running (see `supervisor-skill-discipline`). This makes the loop's approvals and the supervisor's escalation-driven approvals **disjoint sets**, so two approvers never target the same prompt — the property that removes the approval-dispatch race without a claim marker or liveness detection.

#### Scenario: Risky prompt is escalated without blocking the wave

- **GIVEN** an `--unattended` session with two agents, one of which shows a live prompt classified `danger`
- **WHEN** the drive loop sweeps the panes
- **THEN** the loop SHALL NOT auto-approve the risky prompt
- **AND** SHALL record the prompt as an escalation for review
- **AND** SHALL continue progressing the other agent rather than blocking on the risky prompt

#### Scenario: Unknown classification is escalated, not approved

- **GIVEN** a live prompt the classifier returns as `unknown`
- **WHEN** the drive loop evaluates it
- **THEN** the loop SHALL NOT send approval keystrokes
- **AND** SHALL surface the prompt for review (broker + summary)

#### Scenario: Escalation is surfaced uniformly regardless of supervisor presence

- **WHEN** the drive loop escalates a `danger`/`unknown` prompt
- **THEN** it SHALL publish the escalation to the broker as a review item
- **AND** SHALL do so identically whether or not a supervisor is running
- **AND** SHALL NOT branch its escalation behaviour on supervisor liveness

#### Scenario: The loop approves only the safe set

- **GIVEN** the drive loop sweeps a pane showing a classifier-`safe` prompt and another showing an `unknown` prompt
- **WHEN** it acts
- **THEN** it SHALL send approval keystrokes only to the `safe` prompt
- **AND** SHALL escalate (never approve) the `unknown` prompt, leaving it for a consumer of the escalation stream
