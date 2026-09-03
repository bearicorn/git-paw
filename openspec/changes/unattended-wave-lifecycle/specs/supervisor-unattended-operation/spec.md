## MODIFIED Requirements

### Requirement: Exit summary

On exit the drive loop SHALL print a human-readable summary that states the overall outcome (completed / escalated-for-review / stuck / heartbeat), each agent's RESOLVED final state, the deduped list of escalated prompts awaiting human review, and a pointer to the broker log and captured learnings. Each agent's final state SHALL be resolved from terminal artifacts — a verified and/or merged branch, using the `WorkerPhase` lifecycle — rather than the agent's last broker-recorded `agent.status`, so a wave whose branches were all verified and merged is not reported as still "working." When per-agent terminal resolution is unavailable, the summary SHALL still print a wave-level outcome line (e.g. "N/M branches merged").

#### Scenario: Summary reports outcome and escalations

- **GIVEN** a drive loop that exits after escalating one risky prompt and completing the rest of the wave
- **WHEN** the summary is printed
- **THEN** the summary SHALL state the overall outcome
- **AND** SHALL list the per-agent final state
- **AND** SHALL include the escalated prompt awaiting human review

#### Scenario: A completed wave is not reported as working

- **GIVEN** a drive loop exiting after every branch was verified and merged
- **WHEN** the summary is printed
- **THEN** no agent's final state SHALL read `working`
- **AND** the summary SHALL report the wave as completed (e.g. an "N/M branches merged" outcome line)

## ADDED Requirements

### Requirement: The drive loop is bound to a specific session instance

The drive loop SHALL bind to the specific session instance it began driving — identified by a stable instance token (e.g. the session's start receipt, PID, or start-timestamp), not merely the session name — and SHALL stop acting once that instance is purged, stopped, or replaced by a newer session of the same name. A drive loop whose bound session instance no longer exists SHALL exit rather than continue sweeping the session name, so it never sends approvals or nudges to a subsequently-created same-named session's panes.

#### Scenario: A purged session's drive loop does not drive the next same-named session

- **GIVEN** a running unattended drive loop bound to session `paw-x`
- **WHEN** `paw-x` is purged (or stopped) and a new `paw-x` session is later started
- **THEN** the original drive loop SHALL NOT send approvals or nudges to the new session's panes
- **AND** the original drive loop SHALL exit once its bound session instance is gone

#### Scenario: The loop keeps driving its own live session

- **GIVEN** a running drive loop bound to session `paw-x` that is still alive
- **WHEN** the loop ticks
- **THEN** it SHALL continue driving `paw-x` normally
