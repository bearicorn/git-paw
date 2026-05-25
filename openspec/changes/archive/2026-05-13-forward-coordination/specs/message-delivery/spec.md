## ADDED Requirements

### Requirement: Intent messages are broadcast to all other agents

When a `BrokerMessage::Intent` is published, the system SHALL enqueue the message in every known agent's inbox EXCEPT the sender's own inbox. Agents whose inboxes do not yet exist (not yet registered via a publish) SHALL NOT receive the broadcast. This follows the same broadcast pattern as `agent.artifact` and `agent.verified`.

#### Scenario: Intent broadcast reaches all peers

- **GIVEN** three agents `"feat-auth"`, `"feat-detect"`, and `"supervisor"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.intent` message from `"feat-auth"`
- **THEN** `poll_messages` for `"feat-detect"` returns the intent message
- **AND** `poll_messages` for `"supervisor"` returns the intent message

#### Scenario: Intent broadcast skips the sender

- **GIVEN** agents `"feat-auth"` and `"feat-detect"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.intent` message from `"feat-auth"`
- **THEN** `poll_messages` for `"feat-auth"` returns no new messages from this publish

#### Scenario: Intent broadcast skips agents not yet registered

- **GIVEN** agent `"feat-auth"` has an existing inbox but `"feat-detect"` has never published
- **WHEN** `publish_message` is called with an `agent.intent` message from `"feat-auth"`
- **THEN** no inbox is created for `"feat-detect"`
- **AND** no error occurs

### Requirement: Agent record updated for Intent variant

When `agent.intent` is published, the sender's agent record SHALL be updated (last_seen, status, last_message) following the same pattern as existing message types. The `status` field on the agent record SHALL be set to the value returned by `status_label()` for the `Intent` variant (i.e. `"intent"`).

#### Scenario: Intent updates sender record last_seen

- **WHEN** `publish_message` is called with an `agent.intent` from `"feat-auth"`
- **THEN** the agent record for `"feat-auth"` has its `last_seen` updated

#### Scenario: Intent updates sender record status to "intent"

- **WHEN** `publish_message` is called with an `agent.intent` from `"feat-auth"`
- **THEN** the agent record for `"feat-auth"` has its `status` set to `"intent"`
