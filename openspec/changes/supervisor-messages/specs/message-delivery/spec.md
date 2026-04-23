## ADDED Requirements

### Requirement: Verified messages are broadcast to all agents

When a `BrokerMessage::Verified` is published, the system SHALL enqueue the message in every known agent's inbox EXCEPT the sender's own inbox. This follows the same broadcast pattern as `agent.artifact`.

#### Scenario: Verified broadcast reaches all peers

- **GIVEN** agents `"feat-errors"`, `"feat-detect"`, and `"supervisor"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.verified` message from `"supervisor"` for `agent_id = "feat-errors"`
- **THEN** `poll_messages` for `"feat-errors"` returns the verified message
- **AND** `poll_messages` for `"feat-detect"` returns the verified message

#### Scenario: Verified broadcast skips the sender

- **GIVEN** agents `"feat-errors"` and `"supervisor"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.verified` from `"supervisor"`
- **THEN** `poll_messages` for `"supervisor"` returns no new messages from this publish

### Requirement: Feedback messages are delivered to the target agent only

When a `BrokerMessage::Feedback` is published, the system SHALL enqueue the message in the inbox of the agent identified by `agent_id` (the agent receiving feedback). This follows the same targeted delivery pattern as `agent.blocked`.

#### Scenario: Feedback reaches the target agent

- **GIVEN** agents `"feat-errors"` and `"supervisor"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.feedback` message with `agent_id = "feat-errors"`
- **THEN** `poll_messages` for `"feat-errors"` returns the feedback message

#### Scenario: Feedback does not reach other agents

- **GIVEN** agents `"feat-errors"`, `"feat-detect"`, and `"supervisor"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.feedback` for `agent_id = "feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` returns no new messages

### Requirement: Agent record updated for new message types

When `agent.verified` or `agent.feedback` is published, the sender's agent record SHALL be updated (last_seen, status, last_message) following the same pattern as existing message types.

#### Scenario: Verified updates sender record

- **WHEN** `publish_message` is called with an `agent.verified` from `"supervisor"`
- **THEN** the agent record for `"supervisor"` has its `last_seen` updated
