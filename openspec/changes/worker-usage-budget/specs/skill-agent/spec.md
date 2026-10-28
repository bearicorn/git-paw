## ADDED Requirements

### Requirement: Supervisor skill — budget guardrail

The bundled supervisor skill (exported as the orchestrator skill) SHALL carry a budget step that
instructs the driving agent to read attributed usage from `git paw status --json` before opening a
new wave or dispatching new work; when a configured budget cap is reached, to stop opening or
dispatching work and report the totals to the operator; to treat `null` usage as unknown rather than
zero; and never to stop, interrupt or kill a worker on account of the budget. The step SHALL name no
specific agent CLI, vendor or model.

#### Scenario: The rendered skill carries the budget step

- **WHEN** the supervisor skill is rendered for any consumer project
- **THEN** it SHALL contain the budget step: read usage from `git paw status --json`, stop dispatching on a breach, report to the operator, unknown is not zero, never kill a worker

#### Scenario: The budget step is vendor-agnostic

- **WHEN** the budget step is inspected by the agnostic-asset audit
- **THEN** it SHALL contain no agent CLI product name, vendor name or model name
