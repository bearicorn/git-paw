## ADDED Requirements

### Requirement: POST /publish accepts and validates broker messages

The system SHALL expose `POST /publish` accepting an `application/json` request body. The handler SHALL parse the body via `BrokerMessage::from_json` and SHALL behave as follows:

- **Valid `BrokerMessage`** → call `publish_message(&state, msg)`, respond with HTTP `202 Accepted` and an empty body
- **Invalid JSON or validation failure** → respond with HTTP `400 Bad Request` and an `application/json` body containing `{ "error": "<message>" }` describing the failure
- **Wrong content type** → respond with HTTP `415 Unsupported Media Type`
- **Empty body** → respond with HTTP `400 Bad Request` with an error explaining a JSON body is required

The handler SHALL NOT log message bodies to standard output. The handler MUST complete in bounded time (no synchronous blocking I/O) and MUST NOT hold any `BrokerState` lock guard across an `.await` boundary.

#### Scenario: Valid agent.status message returns 202

- **GIVEN** a running broker
- **WHEN** `POST /publish` is sent with body `{"type":"agent.status","agent_id":"feat-x","payload":{"status":"working","modified_files":[],"message":null}}` and `Content-Type: application/json`
- **THEN** the response status is `202`
- **AND** the response body is empty

#### Scenario: Invalid JSON returns 400 with error body

- **GIVEN** a running broker
- **WHEN** `POST /publish` is sent with body `{not-json` and `Content-Type: application/json`
- **THEN** the response status is `400`
- **AND** the response body is JSON containing an `error` field with a human-readable message

#### Scenario: Validation failure returns 400

- **GIVEN** a running broker
- **WHEN** `POST /publish` is sent with body `{"type":"agent.status","agent_id":"","payload":{"status":"working","modified_files":[],"message":null}}`
- **THEN** the response status is `400`
- **AND** the response body's `error` field mentions `agent_id`

#### Scenario: Unknown message type returns 400

- **GIVEN** a running broker
- **WHEN** `POST /publish` is sent with a JSON body whose `type` is `"agent.unknown"`
- **THEN** the response status is `400`

#### Scenario: Missing content type returns 415

- **GIVEN** a running broker
- **WHEN** `POST /publish` is sent without a `Content-Type` header
- **THEN** the response status is `415`

#### Scenario: Wrong content type returns 415

- **GIVEN** a running broker
- **WHEN** `POST /publish` is sent with `Content-Type: text/plain`
- **THEN** the response status is `415`

#### Scenario: Empty body returns 400

- **GIVEN** a running broker
- **WHEN** `POST /publish` is sent with an empty body and `Content-Type: application/json`
- **THEN** the response status is `400`

### Requirement: GET /messages/:agent_id returns queued messages with cursor

The system SHALL expose `GET /messages/:agent_id` returning messages addressed to the specified agent. The endpoint SHALL support cursor-based pagination via an optional `since` query parameter. The handler SHALL:

- Validate that `agent_id` matches the slug character set `[a-z0-9-_]+`; if not, respond with HTTP `400`
- Parse the optional `since` query parameter as a `u64` sequence number; if absent, default to `0` (return all messages)
- Call `poll_messages(&state, agent_id, since)` to retrieve messages with sequence numbers strictly greater than `since`
- Respond with HTTP `200 OK` and an `application/json` body of shape `{ "messages": [<BrokerMessage>, ...], "last_seq": <u64> }`
- `last_seq` SHALL be the highest sequence number across all messages returned, or `0` if no messages are returned
- Return an empty `messages` array with `"last_seq": 0` when no messages match, NOT a 404

Messages SHALL NOT be drained on read. Polling is non-destructive — the same messages are returned on repeated polls with the same `since` value. Agents track their own cursor by passing the `last_seq` from the previous response as the next request's `since` value.

The handler SHALL NOT mutate any broker state. (In Wave 1, the stub returns empty; in Wave 2, `peer-messaging` implements the cursor logic.)

#### Scenario: Polling an agent with no queued messages returns empty array

- **GIVEN** a running broker (Wave 1, stub `poll_messages` returns empty)
- **WHEN** `GET /messages/feat-x` is sent
- **THEN** the response status is `200`
- **AND** the response body is `{"messages":[],"last_seq":0}`

#### Scenario: Polling without since parameter returns all messages

- **GIVEN** a running broker with messages queued for agent `feat-x`
- **WHEN** `GET /messages/feat-x` is sent without a `since` parameter
- **THEN** the response contains all messages addressed to `feat-x`
- **AND** the response contains a `last_seq` field with the highest sequence number

#### Scenario: Polling with since parameter returns only newer messages

- **GIVEN** a running broker with messages at sequence numbers 1, 2, 3, 4, 5 queued for agent `feat-x`
- **WHEN** `GET /messages/feat-x?since=3` is sent
- **THEN** the response contains only messages with sequence numbers 4 and 5
- **AND** `last_seq` is `5`

#### Scenario: Polling with since equal to last_seq returns empty

- **GIVEN** a running broker with messages up to sequence 5 for agent `feat-x`
- **WHEN** `GET /messages/feat-x?since=5` is sent
- **THEN** the response is `{"messages":[],"last_seq":0}`

#### Scenario: Repeated polls with same since return same messages

- **GIVEN** a running broker with messages for agent `feat-x`
- **WHEN** `GET /messages/feat-x?since=0` is sent twice
- **THEN** both responses contain the same messages and the same `last_seq`

#### Scenario: Invalid since parameter returns 400

- **WHEN** `GET /messages/feat-x?since=abc` is sent
- **THEN** the response status is `400`
- **AND** the response body's `error` field mentions the invalid `since` parameter

#### Scenario: Polling with invalid agent_id returns 400

- **WHEN** `GET /messages/feat%2Fx` is sent (URL-decoded: `feat/x`)
- **THEN** the response status is `400`
- **AND** the response body's `error` field mentions the invalid character set

#### Scenario: Polling with empty agent_id segment returns 404

- **WHEN** `GET /messages/` is sent (no agent_id segment)
- **THEN** the response status is `404` (route does not match)

### Requirement: GET /status returns broker and agent state

The system SHALL expose `GET /status` returning the current state of the broker and all known agents. The response SHALL be HTTP `200 OK` with an `application/json` body containing at least these fields:

- `git_paw: bool` — always `true`; serves as the marker the stale-broker probe checks
- `version: String` — the git-paw crate version (`env!("CARGO_PKG_VERSION")`)
- `uptime_seconds: u64` — seconds since the broker started
- `agents: Array<AgentStatusEntry>` — the list returned by `agent_status_snapshot`

The handler MUST be safe to call concurrently. The handler MUST NOT block for more than a few milliseconds.

#### Scenario: Status response contains the marker field

- **GIVEN** a running broker
- **WHEN** `GET /status` is sent
- **THEN** the response status is `200`
- **AND** the response body is JSON
- **AND** the body contains `"git_paw": true`

#### Scenario: Status response contains version and uptime

- **GIVEN** a running broker
- **WHEN** `GET /status` is sent
- **THEN** the response body contains a `version` string field
- **AND** the response body contains a `uptime_seconds` numeric field

#### Scenario: Status response contains empty agents array in Wave 1

- **GIVEN** a running broker (Wave 1, stub `agent_status_snapshot` returns empty)
- **WHEN** `GET /status` is sent
- **THEN** the response body contains `"agents": []`

#### Scenario: Status endpoint is reachable concurrently

- **GIVEN** a running broker
- **WHEN** ten concurrent `GET /status` requests are sent
- **THEN** all ten responses are `200`
- **AND** all ten bodies contain `"git_paw": true`

### Requirement: Unknown routes return 404

The system SHALL respond with HTTP `404 Not Found` for any request whose path does not match one of the three documented routes (`POST /publish`, `GET /messages/:agent_id`, `GET /status`).

#### Scenario: Unknown path returns 404

- **WHEN** `GET /unknown/route` is sent
- **THEN** the response status is `404`

### Requirement: Wrong HTTP methods return 405

The system SHALL respond with HTTP `405 Method Not Allowed` for requests where the path matches a documented route but the method does not.

#### Scenario: GET /publish returns 405

- **WHEN** `GET /publish` is sent
- **THEN** the response status is `405`

#### Scenario: POST /status returns 405

- **WHEN** `POST /status` is sent
- **THEN** the response status is `405`

#### Scenario: POST /messages/feat-x returns 405

- **WHEN** `POST /messages/feat-x` is sent
- **THEN** the response status is `405`
