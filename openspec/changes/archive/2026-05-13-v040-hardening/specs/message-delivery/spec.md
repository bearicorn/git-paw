## ADDED Requirements

### Requirement: Question messages are routed to the supervisor inbox

When a `BrokerMessage::Question` is published, the system SHALL enqueue the message in the inbox of the agent whose `agent_id` is exactly `"supervisor"`. If a `"supervisor"` inbox does not yet exist at delivery time, the system SHALL create it before enqueuing — `Question` is the only variant whose delivery creates a new inbox if missing.

The system SHALL NOT enqueue the message in the sender's inbox or in any other agent's inbox.

This routing differs from `Blocked` (which silently drops if the target inbox is missing) because the supervisor is a singleton recipient and may not have published any prior message at the time the first `Question` arrives.

#### Scenario: Question routed to existing supervisor inbox

- **GIVEN** agents `"feat-x"` and `"supervisor"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.question` message from `"feat-x"`
- **THEN** `poll_messages` for `"supervisor"` returns the question message
- **AND** `poll_messages` for `"feat-x"` returns no new messages from this publish

#### Scenario: Question creates supervisor inbox when absent

- **GIVEN** agent `"feat-x"` has an existing inbox AND no inbox exists for `"supervisor"`
- **WHEN** `publish_message` is called with an `agent.question` message from `"feat-x"`
- **THEN** a new inbox is created for `"supervisor"` containing the question message
- **AND** subsequent `poll_messages` for `"supervisor"` returns the question

#### Scenario: Question does not reach unrelated agents

- **GIVEN** agents `"feat-x"`, `"feat-y"`, and `"supervisor"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.question` from `"feat-x"`
- **THEN** `poll_messages` for `"feat-y"` returns no new messages from this publish

### Requirement: Agent record updated for Question variant

When `agent.question` is published, the sender's agent record SHALL be updated (last_seen, status, last_message) following the same pattern as existing message types. The `status` field on the agent record SHALL be set to the value returned by `status_label()` for the `Question` variant (i.e. `"question"`).

#### Scenario: Question updates sender record last_seen

- **WHEN** `publish_message` is called with an `agent.question` from `"feat-x"`
- **THEN** the agent record for `"feat-x"` has its `last_seen` updated

#### Scenario: Question updates sender record status to "question"

- **WHEN** `publish_message` is called with an `agent.question` from `"feat-x"`
- **THEN** the agent record for `"feat-x"` has its `status` set to `"question"`
