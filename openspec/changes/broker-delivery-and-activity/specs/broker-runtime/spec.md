## ADDED Requirements

### Requirement: POST /publish returns the assigned sequence number

On accepting a message, `POST /publish` SHALL answer `202` with a JSON body `{"seq": N}`, where `N` is the sequence number the message was logged under. Clients that ignore the body SHALL keep working, and every refusal SHALL keep its existing status and body.

#### Scenario: The publisher learns the sequence number

- **GIVEN** a running broker whose last logged sequence number is 41
- **WHEN** a valid `agent.feedback` is published
- **THEN** the response SHALL be `202` with body `{"seq": 42}`
- **AND** `GET /log?since=41` SHALL return that message with `seq` 42

### Requirement: The broker records how far each agent has read its inbox

When `GET /messages/<agent_id>?since=N` is served, the broker SHALL record `N` as that agent's read cursor if it exceeds the recorded one. `GET /status` SHALL report, per agent, the cursor and the count, oldest `seq` and oldest age of messages in its inbox above the cursor sent by the orchestrator.

#### Scenario: An unpolled message is unread

- **GIVEN** orchestrator feedback for `feat-x` logged as `seq` 42 and `feat-x` last polled with `since=40`
- **WHEN** `GET /status` is requested
- **THEN** `feat-x` SHALL show one unread orchestrator message with oldest `seq` 42

#### Scenario: Polling past a message marks it read

- **GIVEN** the state above
- **WHEN** `feat-x` polls with `since=42`
- **THEN** `GET /status` SHALL show no unread orchestrator message for `feat-x`

### Requirement: A commit made over unread orchestrator messages is escalated

When a `committed` artifact arrives from an agent that has unread orchestrator `agent.feedback` or `orchestrator.freeze`, the broker SHALL put one `agent.question` tagged `[delivery]` into the orchestrator inbox, naming the agent, the artifact's `sha` when present, and each unread `seq`. It SHALL NOT repeat for the same unread set.

#### Scenario: Unread rebase feedback followed by a commit is flagged

- **GIVEN** orchestrator feedback `seq` 42 telling `feat-x` to rebase, unread by `feat-x`
- **WHEN** `feat-x` publishes a `committed` artifact with `sha = ccc333`
- **THEN** the orchestrator inbox SHALL receive one `[delivery]` question naming `feat-x`, `ccc333` and `42`

#### Scenario: A second commit over the same unread set is not re-flagged

- **GIVEN** the `[delivery]` question above was emitted
- **WHEN** `feat-x` commits again without polling
- **THEN** no further `[delivery]` question SHALL be emitted for `seq` 42

### Requirement: A visibly busy agent keeps the wave from reading stalled

The broker host SHALL sample each registered agent's pane at a fixed interval. A pane classified mid-response whose capture changed since the previous sample SHALL count as activity, not progress. `wave` SHALL add `last_activity_unix_secs` and `busy_agents`, and `stalled` SHALL also require no activity within the threshold.

#### Scenario: A long silent turn is busy, not stalled

- **GIVEN** a stall threshold of 60 seconds and agent `feat-x` mid-response with a changing pane for 5 minutes, publishing nothing
- **WHEN** `GET /status` is requested
- **THEN** `wave.stalled` SHALL be `false`, `idle_seconds` SHALL be at least 300, and `busy_agents` SHALL contain `feat-x`

#### Scenario: A frozen pane still stalls

- **GIVEN** a stall threshold of 60 seconds and agent `feat-x` showing a mid-response screen that has not changed for 5 minutes
- **WHEN** `GET /status` is requested
- **THEN** `wave.stalled` SHALL be `true`
