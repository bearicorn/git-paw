## ADDED Requirements

### Requirement: Answer delivery routes to the target agent

The delivery layer SHALL route an `agent.answer` message to the inbox of the agent named by the envelope's `agent_id` (the target), mirroring `agent.feedback` routing. The message's sender for roster purposes SHALL be the payload's `from` field, so publishing an answer SHALL NOT create a phantom roster entry for the target.

#### Scenario: Answer lands in the target agent's inbox

- **GIVEN** a published `agent.answer` with `agent_id = "feat-x"` and `payload.from = "supervisor"`
- **WHEN** agent `feat-x` polls its inbox
- **THEN** the answer message SHALL be delivered to `feat-x`
- **AND** other agents' inboxes SHALL NOT receive it

#### Scenario: Answer publish does not distort the roster

- **GIVEN** the same published answer
- **WHEN** the broker roster is inspected
- **THEN** the publish SHALL be attributed to `supervisor` (the `from` sender), not to `feat-x`
