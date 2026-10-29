## MODIFIED Requirements

### Requirement: The broker host wakes an idle headless worker for a directed message

For every session that holds a headless worker, whatever the session's backend, the broker host SHALL, on a cadence of at most 15 seconds, deliver the bundled wake text through the terminal port to every headless worker whose surface is awaiting input and whose broker inbox holds a message addressed to that worker that is newer than the last message it was woken for. A message SHALL count as addressed to a worker when it is an `agent.feedback` or `agent.answer` naming that worker, or an `agent.blocked` routed to that worker's inbox; broadcast messages and messages bound for the orchestrator SHALL NOT wake a worker. A worker SHALL be woken at most once per addressed message, whether or not it reads its inbox. No worker SHALL be woken after a budget-exhausted escalation has been logged for the session. The wake SHALL run in attended and unattended sessions alike. It SHALL NOT deliver the wake text to a tmux worker or a remote worker, and SHALL NOT run for a session that holds no headless worker; a remote worker receives an addressed message only as the remote follow sends it. The decision of whom to wake SHALL be a pure function of the observed worker backends and activity, inbox sequence numbers and logged escalations.

#### Scenario: An answer wakes an idle worker

- **GIVEN** a headless session whose worker `feat-x` is awaiting input
- **WHEN** an `agent.answer` for `feat-x` is published
- **THEN** within 15 seconds a turn SHALL start for `feat-x` whose message is the wake text

#### Scenario: A broadcast does not wake

- **GIVEN** a headless session whose worker `feat-x` is awaiting input
- **WHEN** another worker publishes `agent.artifact` and `agent.intent`
- **THEN** no turn SHALL start for `feat-x`

#### Scenario: One wake per message

- **GIVEN** a worker woken for an `agent.feedback` that ended its next turn without reading its inbox
- **WHEN** the wake decision runs again with no new message for that worker
- **THEN** it SHALL return no wake for that worker

#### Scenario: A busy worker is not woken

- **GIVEN** a worker whose surface is busy and whose inbox holds a new `agent.feedback`
- **WHEN** the wake decision runs
- **THEN** it SHALL return no wake for that worker

#### Scenario: No wakes after a budget breach

- **GIVEN** a session whose log holds a budget-exhausted escalation and an idle worker with a new `agent.answer`
- **WHEN** the wake decision runs
- **THEN** it SHALL return no wake

#### Scenario: tmux sessions are never woken

- **GIVEN** a session on the tmux backend with an idle worker and a new `agent.answer` for it
- **WHEN** the broker host runs
- **THEN** it SHALL deliver no wake text to any surface

#### Scenario: A headless worker in a tmux session is woken

- **GIVEN** a tmux session whose headless worker `feat-h` is awaiting input
- **WHEN** an `agent.answer` for `feat-h` is published
- **THEN** within 15 seconds a turn SHALL start for `feat-h` whose message is the wake text

#### Scenario: A remote worker is never sent the wake text

- **GIVEN** a session with a remote worker `feat-r` and an `agent.feedback` addressed to it
- **WHEN** the wake decision runs
- **THEN** it SHALL return no wake for `feat-r`
