## MODIFIED Requirements

### Requirement: Escalation-first, no blanket-approve when a drive loop is running

When the supervisor's boot context indicates a drive loop is running (an unattended session), the supervisor SHALL, each supervision cycle:

1. **Drain the drive loop's escalations first.** Read the loop's escalation and review items from its broker inbox and act on each before the rest of the sweep. For a permission prompt, relay it to the human (in the supervisor's own pane, naming the agent and the command) and approve it with `sweep.sh approve <pane>` only when the human explicitly says so. For any other item, reason about it and publish feedback or an answer.
2. **Then perform its normal sweep** — verification, merge orchestration, conflict handling, detect-stuck, and status publishing — as it otherwise would.

Whether or not a drive loop is running, the supervisor SHALL NOT approve a permission prompt on its own judgment of the command's safety, and SHALL NOT sweep panes to approve prompts in bulk. In an attended session (no drive loop) it finds stuck-on-prompt agents through its own `sweep.sh detect-stuck` sweep and relays their prompts to the human in the same way.

#### Scenario: With a loop running, escalations are handled before the sweep

- **GIVEN** a supervisor whose boot context indicates a drive loop is running
- **WHEN** it runs a supervision cycle
- **THEN** it SHALL process the loop's escalations before its verify/merge/status sweep
- **AND** SHALL relay each escalated permission prompt to the human rather than approving it

#### Scenario: An approval follows an explicit human instruction

- **GIVEN** the supervisor has relayed agent `feat-a`'s permission prompt to the human
- **WHEN** the human tells the supervisor to approve it
- **THEN** the supervisor SHALL run `sweep.sh approve <pane>` for `feat-a`'s pane
- **AND** without such an instruction it SHALL NOT run `sweep.sh approve`

#### Scenario: With no loop, the supervisor still does not approve on its own

- **GIVEN** a supervisor whose boot context does NOT indicate a drive loop
- **WHEN** its sweep finds an agent stuck on a permission prompt
- **THEN** it SHALL relay the prompt to the human
- **AND** SHALL NOT approve it on its own judgment
