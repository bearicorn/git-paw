## ADDED Requirements

### Requirement: Broker `/publish` enforces agent_id validation in code

The `src/broker/server.rs::publish` HTTP handler SHALL execute the validation already specified in `openspec/specs/broker-messages/spec.md` under "Broker rejects invalid agent_id strings" and "Broker rejects payload fields matching placeholder syntax" (propagated from the archived `supervisor-as-pane-followups` change). Today those spec requirements describe behaviour the binary does NOT implement; this change closes the gap.

Specifically, the handler SHALL:

1. Reject the request with HTTP 400 when the deserialized `BrokerMessage`'s top-level `agent_id` does NOT match the regular expression `^(supervisor|feat/[a-z0-9][a-z0-9-]+|feat-[a-z0-9][a-z0-9-]+)$`.
2. Reject the request with HTTP 400 when any of `payload.question`, `payload.message`, `payload.needs`, or any string element of `payload.errors[]` matches `^<.*>$` exactly.

The error body shape and error message text are as defined in the existing main-spec scenarios — no new wording.

A single compiled `OnceLock<Regex>` per pattern is acceptable; the broker's hot path SHALL NOT rebuild the regex per request.

#### Scenario: Single-letter agent_id is rejected by the running broker

- **GIVEN** a running broker on port `<P>` with the validation implemented
- **WHEN** a client POSTs `{"type":"agent.status","agent_id":"a","payload":{"status":"working","modified_files":[],"message":null}}` to `http://127.0.0.1:<P>/publish`
- **THEN** the HTTP response status SHALL be 400
- **AND** the response body SHALL be a JSON object containing the substring `"invalid agent_id"`
- **AND** a subsequent `GET /status` SHALL NOT contain an entry with `agent_id = "a"`

#### Scenario: Placeholder-shaped agent_id is rejected by the running broker

- **GIVEN** a running broker
- **WHEN** a client POSTs `{"type":"agent.question","agent_id":"<agent-id>","payload":{"question":"placeholder text"}}`
- **THEN** the HTTP response status SHALL be 400
- **AND** the response body SHALL contain the substring `"invalid agent_id"`

#### Scenario: Placeholder-shaped payload.question is rejected by the running broker

- **GIVEN** a running broker
- **WHEN** a client POSTs `{"type":"agent.question","agent_id":"feat-x","payload":{"question":"<your specific question>"}}`
- **THEN** the HTTP response status SHALL be 400
- **AND** the response body SHALL contain the substring `"unfilled placeholder"` and the substring `"question"`

#### Scenario: Valid supervisor and feat-* publishers succeed

- **GIVEN** a running broker
- **WHEN** a client POSTs a well-formed `agent.status` message with `agent_id = "supervisor"`
- **THEN** the HTTP response status SHALL be 200 or 204
- **AND** the message SHALL be appended to the supervisor's inbox

The same SHALL hold for `agent_id = "feat-test-branch"` and `agent_id = "feat/test-branch"`.

#### Scenario: Real human content passes through

- **GIVEN** a running broker
- **WHEN** a client POSTs `{"type":"agent.question","agent_id":"feat-x","payload":{"question":"Should we use bcrypt or argon2?"}}`
- **THEN** the HTTP response status SHALL be 200 or 204

#### Scenario: Existing test fixtures using non-conforming agent_ids are updated

- **WHEN** the test suite is run after this change lands
- **THEN** every `/publish` test caller in `tests/broker_integration.rs`, `tests/conflict_detection_integration.rs`, `tests/learnings_mode_integration.rs`, `tests/e2e_*.rs`, and any other broker-touching test file SHALL use an `agent_id` matching the regex (e.g. `feat-x`, `feat-test`, `supervisor`) and SHALL NOT use ad-hoc identifiers like `"test"`, `"agent1"`, or single letters
- **AND** the broker-side validation SHALL be active for all those tests (no opt-out)
