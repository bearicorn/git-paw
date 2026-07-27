## MODIFIED Requirements

### Requirement: GET /messages/:agent_id returns queued messages with cursor

The system SHALL expose `GET /messages/:agent_id` returning messages addressed to the specified agent. The endpoint SHALL support cursor-based pagination via an optional `since` query parameter. The handler SHALL:

- Validate that `agent_id` matches the slug character set `[a-z0-9-_]+`; if not, respond with HTTP `400`
- Parse the optional `since` query parameter as a `u64` sequence number; if absent, default to `0` (return all messages)
- Call `poll_messages(&state, agent_id, since)` to retrieve messages with sequence numbers strictly greater than `since`
- Respond with HTTP `200 OK` and an `application/json` body of shape `{ "messages": [<BrokerMessage>, ...], "last_seq": <u64> }`
- `last_seq` SHALL be the highest sequence number across all messages returned, or — when no messages are returned — the `since` cursor value the caller supplied (the cursor is **held at `since`**, NOT reset to `0`)

The hold-at-`since` semantics are the v0.11.0 re-serve fix: resetting an empty poll's `last_seq` to `0` would rewind an agent that has already advanced its cursor and cause it to re-fetch the entire backlog (and could wedge a re-published question). Holding at `since` keeps the cursor monotonic so a caller that passes back the previous `last_seq` neither rewinds nor loses a later-arriving message. This spec and `message-delivery`'s "Cursor-based message polling" requirement MUST agree on this behaviour.

Messages SHALL NOT be drained on read. Polling is non-destructive — the same messages are returned on repeated polls with the same `since` value. Agents track their own cursor by passing the `last_seq` from the previous response as the next request's `since` value.

The handler SHALL NOT mutate any broker state.

#### Scenario: Polling an agent with no queued messages returns empty array

- **GIVEN** a running broker with no messages queued for agent `feat-x`
- **WHEN** `GET /messages/feat-x` is sent (no `since` parameter, so `since` defaults to `0`)
- **THEN** the response status is `200`
- **AND** the response body is `{"messages":[],"last_seq":0}` (the held cursor equals the supplied `since` of `0`)

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

#### Scenario: Polling with since equal to the latest seq holds the cursor at since

- **GIVEN** a running broker with messages up to sequence 5 for agent `feat-x`
- **WHEN** `GET /messages/feat-x?since=5` is sent
- **THEN** the response is `{"messages":[],"last_seq":5}` (empty, and the cursor is held at the supplied `since` of `5`, not reset to `0`)

Test: `broker::delivery::tests::poll_since_latest_holds_cursor_at_since`

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
