## ADDED Requirements

### Requirement: Event wait tool

The server SHALL expose a read-only tool `wait_for_events` with parameters `since` (a broker log sequence number, default `0`) and `timeout_seconds` (default `30`, capped at `60`) that long-polls the session broker's `GET /log` cursor and returns `{entries, last_seq, wave}` — the entries published after `since`, the new cursor, and the broker's wave-staleness object. When no session broker is reachable, the tool SHALL return a successful response with empty `entries`, `last_seq` equal to `since`, and `no_session: true`, without waiting. The tool SHALL advertise a JSON Schema for its parameters and return shape and SHALL NOT mutate broker or repository state.

#### Scenario: Waiting returns new entries

- **GIVEN** a live session broker at sequence 4 and an MCP client calling `wait_for_events({ "since": 4, "timeout_seconds": 30 })`
- **WHEN** an agent publishes a message during the wait
- **THEN** the tool SHALL return that entry with `last_seq = 5` before the timeout

#### Scenario: A timed-out wait still reports the wave

- **GIVEN** a live session broker whose wave is stalled and no publisher
- **WHEN** the client calls `wait_for_events({ "since": 9, "timeout_seconds": 2 })`
- **THEN** the tool SHALL return empty `entries`, `last_seq = 9`, and a `wave` object with `stalled = true`

#### Scenario: No broker returns immediately

- **GIVEN** no active session for the repository
- **WHEN** the client calls `wait_for_events({ "since": 0, "timeout_seconds": 30 })`
- **THEN** the tool SHALL return at once with `no_session: true` and empty `entries`

### Requirement: Session status reports wave staleness

`get_session_status` SHALL include the session broker's `wave` object (`last_progress_unix_secs`, `idle_seconds`, `stall_threshold_seconds`, `stalled`) when a broker is reachable, and SHALL omit it otherwise.

#### Scenario: Stalled wave reaches the orchestrator

- **GIVEN** an active session whose broker reports `stalled = true`
- **WHEN** the client calls `get_session_status()`
- **THEN** the response SHALL contain `wave.stalled = true`
