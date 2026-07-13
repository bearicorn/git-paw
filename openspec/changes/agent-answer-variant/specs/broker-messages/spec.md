## ADDED Requirements

### Requirement: agent.answer message type

The broker SHALL accept an `agent.answer` message variant carrying a non-error supervisor→agent reply. The envelope's `agent_id` SHALL name the TARGET agent (the one being answered), and the payload SHALL contain:

- `from: String` — the sender (typically `"supervisor"`); required non-empty
- `answer: String` — the reply text; required non-empty
- `re: Option<String>` — an optional short reference to the question being answered; omitted from serialization when absent

Validation SHALL reject an empty `from` or an empty `answer` with a named error, mirroring `agent.feedback`'s field validation. The variant SHALL serialize with `type = "agent.answer"`.

#### Scenario: Valid answer round-trips through serde

- **GIVEN** the JSON `{"type":"agent.answer","agent_id":"feat-x","payload":{"from":"supervisor","answer":"Use the existing helper; do not add a dependency","re":"add crate X?"}}`
- **WHEN** it is parsed and re-serialized
- **THEN** parsing SHALL succeed and the round-trip SHALL preserve all fields

#### Scenario: Empty answer is rejected

- **GIVEN** an `agent.answer` payload with `answer = ""`
- **WHEN** the message is validated
- **THEN** validation SHALL fail with an error naming the empty answer field

#### Scenario: Empty from is rejected

- **GIVEN** an `agent.answer` payload with `from = ""`
- **WHEN** the message is validated
- **THEN** validation SHALL fail with an error naming the empty from field

#### Scenario: re is optional

- **GIVEN** an `agent.answer` payload with no `re` field
- **WHEN** the message is validated and serialized
- **THEN** validation SHALL pass and the serialized JSON SHALL omit `re`

#### Scenario: Answer is not an error channel

- **WHEN** the coordination skill's message documentation is rendered
- **THEN** it SHALL describe `agent.answer` as an authoritative supervisor reply to act on — distinct from `agent.feedback`, which carries corrective errors
