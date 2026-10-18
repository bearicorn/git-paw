## MODIFIED Requirements

### Requirement: Heartbeat re-engages the human after prolonged inactivity

When the drive loop has observed no progress for approximately 25 minutes without reaching a completion condition, it SHALL re-engage the human by exiting with a status summary (outcome `heartbeat`) rather than running forever silently. The interval SHALL be measured from the most recent progress the loop observed — or from the loop's start, if it has observed none — and NOT from the loop's start alone, so a wave that keeps progressing is never ended by the heartbeat. Progress SHALL be either of: a coding agent's branch tip moving between two ticks, or a coding agent's phase advancing (its resolved `WorkerPhase` rising). Nothing the drive loop does itself SHALL count as progress: in particular, an escalation it records, its re-escalation of a permission prompt that is still unanswered, and a learning it records SHALL NOT count. A change in pane output alone, or a broker message that does not advance the agent's phase, SHALL NOT count as progress. A wave whose agents all sit on unanswered permission prompts therefore exits with `heartbeat` one interval after its last progress. The decision whether the heartbeat is due SHALL be a pure function of the last-progress instant, the current instant and the interval. The behaviour SHALL NOT depend on whether the process has a controlling terminal.

#### Scenario: Heartbeat fires after prolonged run without progress

- **GIVEN** an unattended drive loop whose agents have made no progress for approximately 25 minutes and have not completed
- **WHEN** the heartbeat interval elapses
- **THEN** the loop SHALL exit and surface a status summary with outcome `heartbeat` to re-engage the human
- **AND** SHALL NOT continue running indefinitely without surfacing status

Test: `supervisor::drive::tests::loop_exits_on_heartbeat_when_never_completing` (fake clock); `tests/unattended_drive_e2e.rs::unattended_heartbeat_exits_with_summary`.

#### Scenario: A progressing wave is not ended by the heartbeat

- **GIVEN** an unattended drive loop that has run longer than the heartbeat interval
- **AND** a coding agent's branch tip moved less than one interval ago
- **WHEN** the loop evaluates the heartbeat
- **THEN** the loop SHALL keep driving the wave
- **AND** SHALL exit with `heartbeat` only once a full interval passes with no further progress

Test: `supervisor::drive::tests::branch_tip_movement_defers_the_heartbeat` (fake clock).

#### Scenario: Only a tip move or a phase advance resets the interval

- **GIVEN** the heartbeat decision with a last-progress instant and the current instant
- **WHEN** the tick observes a branch tip move or a phase advance
- **THEN** the last-progress instant SHALL become the current instant
- **AND** a tick that observes only changed pane output, a repeated `working` status, the loop's re-escalation of an unanswered prompt, or a learning the loop recorded SHALL leave it unchanged

Test: `supervisor::drive::tests::progress_kinds_reset_the_heartbeat` (table-driven, pure).

#### Scenario: Re-escalating an unanswered prompt does not defer the heartbeat

- **GIVEN** an unattended drive loop whose only coding agent sits on an unanswered live permission prompt
- **AND** the loop re-escalates that prompt each dedup window and records a learning for it
- **WHEN** a full heartbeat interval passes with no branch tip move and no phase advance
- **THEN** the loop SHALL exit with outcome `heartbeat`

Test: `supervisor::drive::tests::re_escalations_do_not_defer_the_heartbeat` (fake clock).

#### Scenario: A detached run reaches completion

- **GIVEN** `git paw start --unattended` launched with no controlling terminal and stdin not a TTY, with the heartbeat interval shortened for the test
- **AND** coding agents that keep committing for longer than that interval, then publish a terminal status with their branches merged
- **WHEN** the run proceeds
- **THEN** the loop SHALL NOT exit with `heartbeat`
- **AND** the run SHALL end with outcome `completed`

Test: `tests/unattended_drive_e2e.rs::detached_unattended_run_completes_past_the_heartbeat`.
