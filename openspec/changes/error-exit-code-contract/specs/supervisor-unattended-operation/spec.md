## ADDED Requirements

### Requirement: Unattended exit status reflects the wave outcome

The `git paw start --unattended` process SHALL exit with a status derived from the drive loop's outcome, after printing the exit summary: `0` for `completed`; `3` for `escalated-for-review`, `stuck`, `heartbeat` and `session-torn-down`. A coding agent whose CLI became unavailable is a `cli-unavailable` escalation, so its wave ends `escalated-for-review` and exits `3`; there is no `cli-unavailable` outcome since v0.18.0 moved the orchestrator out of the session. An error that stops the loop itself exits `1`, like any other `PawError`. A wrapper script SHALL be able to tell a completed wave from one that stopped short from the exit status alone. A run whose drive loop escalated any permission prompt (per *Every live permission prompt is escalated without blocking the wave*) is reported `escalated-for-review`, so it SHALL exit `3` even when every agent's work completed and every branch merged: a prompt went to a human, and the wrapper must not read the run as clean. The summary printed to stdout SHALL be unchanged by this requirement.

#### Scenario: A completed wave exits 0

- **GIVEN** an unattended run whose wave completes with every branch merged and whose drive loop recorded no escalation
- **WHEN** the process exits
- **THEN** the exit status SHALL be `0`

Test: `tests/unattended_drive_e2e.rs::unattended_reports_completed_when_branch_is_merged`

#### Scenario: A heartbeat exit is non-zero and distinct from a failure

- **GIVEN** an unattended run that exits with outcome `heartbeat`
- **WHEN** the process exits
- **THEN** the summary SHALL be printed as before
- **AND** the exit status SHALL be `3`

Test: `tests/unattended_drive_e2e.rs::unattended_heartbeat_exits_with_summary`

#### Scenario: Escalations awaiting review exit 3

- **GIVEN** an unattended run that exits `escalated-for-review` because a branch is unmerged
- **WHEN** the process exits
- **THEN** the exit status SHALL be `3`

Test: `tests/unattended_drive_e2e.rs::unattended_reports_escalated_for_review_when_branch_unmerged`

#### Scenario: An escalated permission prompt exits 3 even when the work completed

- **GIVEN** an unattended run whose drive loop escalated a live permission prompt for one agent
- **AND** every agent later publishes a terminal status with its branch merged
- **WHEN** the process exits
- **THEN** the summary SHALL report `escalated-for-review` and list the `permission-prompt` escalation
- **AND** the exit status SHALL be `3`

Test: `tests/unattended_drive_e2e.rs::escalated_prompt_exits_three_even_when_every_branch_merged`

#### Scenario: Each outcome maps to its status

- **GIVEN** each `DriveOutcome`
- **WHEN** the exit status is derived
- **THEN** `completed` SHALL map to `0`, and `escalated-for-review`, `stuck`, `heartbeat` and `session-torn-down` to `3`

Test: `supervisor::drive::tests::outcome_exit_status_table` (table-driven, pure)
