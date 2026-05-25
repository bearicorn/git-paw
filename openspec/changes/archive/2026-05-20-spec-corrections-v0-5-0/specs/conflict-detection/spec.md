## MODIFIED Requirements

### Requirement: Auto-emitted message conventions

Auto-emitted messages from the detector SHALL conform to the following conventions:

- `agent.feedback` messages SHALL set `payload.from = "supervisor"` and SHALL place at least one error string in `payload.errors` whose first non-whitespace token is `[conflict-detector]`.
- `agent.question` messages emitted to the supervisor inbox SHALL set `agent_id = "supervisor"` (the recipient — and, by the auto-emitted-detector convention, the sender-identification slot for this variant, since `QuestionPayload` has no `from` field), and SHALL include `[conflict-detector]` as a token in the question text.

These conventions SHALL apply to forward, in-flight, and ownership message paths.

#### Scenario: Auto-emitted feedback uses supervisor as the from field

- **WHEN** the detector emits any `agent.feedback`
- **THEN** the message has `payload.from = "supervisor"`
- **AND** at least one error string starts with the token `[conflict-detector]`

#### Scenario: Auto-emitted question is addressed to the supervisor inbox

- **WHEN** the detector emits any `agent.question`
- **THEN** the message has `agent_id = "supervisor"`
- **AND** the question text contains the token `[conflict-detector]`

#### Scenario: Auto-emitted question payload has no from field

- **WHEN** the detector emits any `agent.question`
- **THEN** the serialized JSON payload contains a `question` field
- **AND** the serialized JSON payload does NOT contain a `from` field (the `QuestionPayload` type has no such field)
- **AND** the sender-identification information is carried by the envelope `agent_id = "supervisor"`, not by a payload field
