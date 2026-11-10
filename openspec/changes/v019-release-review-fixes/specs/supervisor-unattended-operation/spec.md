## ADDED Requirements

### Requirement: Recording a learning never holds the drive loop

The drive loop SHALL bound each `sweep.sh learn` it runs: a run still going after the bound (longer than the helper's own broker-call bound) SHALL be killed, a warning printed and the loop SHALL continue. It SHALL pass the session's recorded broker as `GIT_PAW_BROKER_URL`, so a learning never goes to a `[broker]` port edited after launch.

#### Scenario: A learning that never finishes does not hold the loop

- **GIVEN** a `sweep.sh learn` that never exits
- **WHEN** the loop records a learning with a 300 ms bound
- **THEN** the call SHALL return within 5 seconds

Test: `src/supervisor/drive.rs::a_hung_learning_is_dropped_at_the_bound`

#### Scenario: A learning carries the session's recorded broker

- **GIVEN** a loop whose session recorded broker `http://127.0.0.1:4242`
- **WHEN** it records a learning that finishes inside the bound
- **THEN** `sweep.sh learn` SHALL run to completion with `GIT_PAW_BROKER_URL` set to that address

Test: `src/supervisor/drive.rs::a_fast_learning_runs_with_the_session_broker_url`

#### Scenario: A re-bound loop records learnings to the recorded broker

- **GIVEN** a live session with the root helpers installed whose `[broker] port` was edited after launch to a port that accepts and never answers
- **WHEN** `git paw start --unattended` binds a loop that escalates a prompt
- **THEN** the loop SHALL exit on its heartbeat
- **AND** nothing SHALL reach the edited port

Test: `tests/unattended_drive_e2e.rs::unattended_rebind_uses_the_recorded_broker_endpoint`
