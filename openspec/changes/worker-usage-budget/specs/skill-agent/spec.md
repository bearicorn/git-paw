## ADDED Requirements

### Requirement: Supervisor skill — budget guardrail

The embedded orchestrator skill SHALL carry a budget step telling the driving agent: before new
work, compare `usage_total.tokens` from `git paw status --json`, plus a running total of removed
workers' noted `usage`, with `budget_max_tokens`; on a breach, stop dispatching and report to the
operator; treat `null` usage as unknown, not zero; never stop or kill a worker for the budget.

#### Scenario: The rendered skill carries the budget step

- **WHEN** the supervisor skill is rendered for any consumer project
- **THEN** it SHALL contain the budget step: read usage from `git paw status --json`, keep a running total of removed workers' usage, stop dispatching on a breach, report to the operator, unknown is not zero, never kill a worker

### Requirement: The budget step is vendor-agnostic

The supervisor skill's budget step SHALL name no specific agent CLI, vendor or model.

#### Scenario: The budget step is vendor-agnostic

- **WHEN** the budget step is inspected by the agnostic-asset audit
- **THEN** it SHALL contain no agent CLI product name, vendor name or model name
