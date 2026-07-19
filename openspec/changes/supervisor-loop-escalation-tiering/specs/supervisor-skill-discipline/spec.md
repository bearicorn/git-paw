## ADDED Requirements

### Requirement: Escalation-first, no blanket-approve when a drive loop is running

When the supervisor's boot context indicates a drive loop is running (an unattended session), the supervisor SHALL, each supervision cycle:

1. **Drain the drive loop's escalations first** — read the loop's escalation/review items from its broker inbox, reason about each, and either targeted-approve the specific escalated pane or publish feedback. This precedes the rest of the sweep so agents blocked on a prompt the loop could not classify safe are unblocked fastest.
2. **Then perform its normal sweep** — verification, merge orchestration, conflict handling, detect-stuck, and status publishing — as it otherwise would.

While a drive loop is running, the supervisor SHALL NOT blanket-approve classifier-safe prompts by sweeping panes: the loop owns safe-prompt approval, and the supervisor acts only on prompts the loop escalated. This keeps the two approvers' actions disjoint (see `unattended-operation`) and removes the approval-dispatch race.

When no drive loop is running (an attended supervisor session), the supervisor performs the full sweep INCLUDING approving classifier-safe prompts, as its sole-approver role requires — this preserves existing attended behaviour.

#### Scenario: With a loop running, escalations are handled before the sweep

- **GIVEN** a supervisor whose boot context indicates a drive loop is running
- **WHEN** it runs a supervision cycle
- **THEN** it SHALL process the loop's escalations (targeted approve / feedback) before its verify/merge/status sweep
- **AND** SHALL NOT blanket-approve classifier-safe prompts by sweeping panes

#### Scenario: With no loop, the supervisor approves safe prompts itself

- **GIVEN** a supervisor whose boot context does NOT indicate a drive loop
- **WHEN** it sweeps the panes
- **THEN** it SHALL approve classifier-safe prompts itself as the sole approver
