## ADDED Requirements

### Requirement: The drive loop detects a host sleep from a wall-clock gap

Each tick the drive loop SHALL record both a monotonic and a wall-clock reading. When the wall clock advanced more than the monotonic clock by over `[drive] wake_gap_seconds` (default 120) since the previous tick, it SHALL treat the host as woken from sleep and print one line naming the gap. `wake_gap_seconds = 0` SHALL disable the check. A slow tick SHALL NOT count as a sleep.

#### Scenario: A sleep between ticks is detected

- **GIVEN** a drive loop whose clock advances 15 s monotonic and 6 h wall-clock between two ticks
- **WHEN** the second tick runs
- **THEN** the loop SHALL report a wake after about 6 h and run the wake re-engagement

#### Scenario: A slow tick is not a sleep

- **GIVEN** a tick that took 10 minutes on both clocks
- **WHEN** the next tick runs
- **THEN** no wake SHALL be reported

#### Scenario: Disabled check

- **GIVEN** `[drive] wake_gap_seconds = 0` and a 6 h wall-clock gap
- **WHEN** the next tick runs
- **THEN** no wake SHALL be reported

### Requirement: After a wake the drive loop re-engages idle and exited workers

On the tick a wake is detected, the drive loop SHALL relaunch each worker pane whose CLI exited (bare shell), using recovery's resume decision and fallback, and send it the matching message once ready. It SHALL submit the wake message once to each worker idle at its prompt, unless `[workers] resume_nudge = false`. A finished worker SHALL NOT be nudged.

#### Scenario: Idle worker is nudged once

- **GIVEN** a wake detected and worker `feat/a` idle at its CLI prompt
- **WHEN** the tick re-engages workers
- **THEN** `feat/a` SHALL receive the wake message exactly once

#### Scenario: Exited worker is relaunched

- **GIVEN** a wake detected and worker `feat/b`'s pane at a bare shell, its conversation present
- **WHEN** the tick re-engages workers
- **THEN** the pane SHALL be relaunched with its resume arguments and receive the resume message once ready

### Requirement: After a wake the drive loop reports busy workers instead of interrupting them

On the tick a wake is detected, a worker pane that is busy SHALL NOT be typed into or interrupted. The drive loop SHALL publish one escalation to the orchestrator naming each such worker and the length of the sleep, so a command hung by the sleep can be judged by the orchestrator.

#### Scenario: Busy worker is escalated, not touched

- **GIVEN** a wake detected and worker `feat/c` busy running a test command
- **WHEN** the tick re-engages workers
- **THEN** no keystroke SHALL be sent to `feat/c`
- **AND** one escalation addressed to the orchestrator SHALL name `feat/c` and the sleep length
