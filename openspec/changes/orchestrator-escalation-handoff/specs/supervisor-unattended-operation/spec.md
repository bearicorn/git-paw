## ADDED Requirements

### Requirement: Exit summary reports headless invocation outcomes

When the drive loop handed any judgment call to a headless orchestrator invocation during the run, its exit summary SHALL additionally state the number of invocations attempted with a per-outcome count (completed, non-zero exit, timed out), the number still queued when the loop stopped dequeuing, and the path of the escalation log. When no invocation was attempted, the summary SHALL state that headless hand-off was unavailable or disabled, with the reason resolved at loop start. Escalations handed off SHALL still be listed among the escalations awaiting review.

#### Scenario: Summary reports invocation outcomes

- **GIVEN** a drive loop that ran two headless invocations, one completed and one timed out
- **WHEN** the summary is printed
- **THEN** it SHALL state 2 invocations, 1 completed, 1 timed out
- **AND** SHALL name the escalation log path

#### Scenario: Summary explains an unavailable hand-off

- **GIVEN** a drive loop started with no resolvable orchestrator CLI
- **WHEN** the summary is printed
- **THEN** it SHALL state that headless hand-off was unavailable and why

#### Scenario: Handed-off escalations remain listed

- **GIVEN** a correction-exhausted branch that was escalated and handed to a one-shot that exited 0
- **WHEN** the summary is printed
- **THEN** the escalation SHALL still appear in the list awaiting review
