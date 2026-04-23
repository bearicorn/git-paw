## MODIFIED Requirements

### Requirement: Status messages are not routed

The existing routing rule (status messages are not enqueued) remains unchanged. The addition of `agent.question` routing SHALL NOT affect status message delivery.

#### Scenario: Status message still does not appear in any inbox after question routing added

- **GIVEN** agents `"feat-errors"` and `"feat-detect"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.status` message from `"feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` SHALL return no messages from that publish
- **AND** `poll_messages` for `"supervisor"` SHALL return no messages from that publish

### Requirement: Artifact messages are broadcast to all other agents

The existing broadcast rule for `agent.artifact` remains unchanged. Artifact messages SHALL NOT be delivered to the `"supervisor"` inbox.

#### Scenario: Artifact broadcast does not reach supervisor inbox

- **GIVEN** three agents with existing inboxes and a `"supervisor"` inbox
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` SHALL return the artifact message
- **AND** `poll_messages` for `"supervisor"` SHALL return no messages from that publish

### Requirement: Blocked messages are delivered to the target agent

The existing targeted delivery rule for `agent.blocked` remains unchanged. Blocked messages SHALL NOT be delivered to the `"supervisor"` inbox.

#### Scenario: Blocked message still reaches only the target agent

- **GIVEN** agents `"feat-config"`, `"feat-errors"`, and a `"supervisor"` inbox all exist
- **WHEN** `publish_message` is called with an `agent.blocked` message from `"feat-config"` with `payload.from = "feat-errors"`
- **THEN** `poll_messages` for `"feat-errors"` SHALL return the blocked message
- **AND** `poll_messages` for `"supervisor"` SHALL return no messages from that publish

### Requirement: Question messages are delivered to the supervisor inbox

When a `BrokerMessage::Question` is published, the system SHALL enqueue the message in the `"supervisor"` inbox. If no `"supervisor"` inbox exists, one SHALL be created automatically.

Question messages SHALL NOT be enqueued in any other agent's inbox (including the sender's own inbox).

#### Scenario: Question message is delivered to supervisor inbox

- **GIVEN** agent `"feat-config"` publishes an `agent.question` message
- **WHEN** `publish_message` is called
- **THEN** `poll_messages(&state, "supervisor", 0)` SHALL return the question message

#### Scenario: Question message does not reach other agents

- **GIVEN** agents `"feat-config"` and `"feat-detect"` both with existing inboxes
- **WHEN** agent `"feat-config"` publishes an `agent.question` message
- **THEN** `poll_messages(&state, "feat-detect", 0)` SHALL return no messages from that publish

#### Scenario: Question message creates supervisor inbox if absent

- **GIVEN** no `"supervisor"` inbox exists
- **WHEN** agent `"feat-config"` publishes an `agent.question` message
- **THEN** a `"supervisor"` inbox SHALL be created automatically
- **AND** the question message SHALL be in the supervisor inbox

#### Scenario: Question message does not appear in sender's inbox

- **GIVEN** agent `"feat-config"` has an existing inbox
- **WHEN** agent `"feat-config"` publishes an `agent.question` message
- **THEN** `poll_messages(&state, "feat-config", 0)` SHALL return no messages from that publish
