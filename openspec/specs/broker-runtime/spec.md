# broker-runtime Specification

## Purpose

The broker runtime is git-paw's in-process coordination server. This capability defines its configuration and lifecycle (the optional `[broker]` config section, the synchronous `start_broker` entry point that owns a tokio runtime and spawns the axum server plus per-worktree watcher tasks, stale-broker detection, `SO_REUSEADDR` restart, `Arc`-shared `BrokerState`, SIGINT and panic isolation, and the stable `delivery.rs` contract — with tokio confined to `src/broker/` so every other module stays synchronous); its complete HTTP surface (`POST /publish` message validation, `GET /messages/:agent_id` cursor-based inbox polling, `GET /status`, the `GET /log` replay seam, and `POST /watch` live watch-target registration); and its integration into the session lifecycle (the hidden `__broker` host subcommand that runs the broker in the session's detached `broker` service surface, the hidden `__dashboard` subcommand that renders it as a thin HTTP client, the start/stop/pause/resume/purge flows that launch, stop and recreate the host, `GIT_PAW_BROKER_URL` injection into the tmux session, and how `git paw status` surfaces broker and paused-session state).
## Requirements
### Requirement: Broker configuration schema

The system SHALL extend `.git-paw/config.toml` with a new optional `[broker]` section containing these fields:

- `enabled: bool` — defaults to `false` when the field or the entire section is absent
- `port: u16` — defaults to `9119` when absent
- `bind: String` — defaults to `"127.0.0.1"` when absent
- `wave_stall_minutes: Option<u64>` — defaults to `30` when absent; `0` disables the `stalled` flag while `idle_seconds` is still reported
- `watcher: WatcherConfig` — the filesystem-watcher sub-table (`[broker.watcher]`), itself all-optional

Loading a `.git-paw/config.toml` that omits the `[broker]` section SHALL succeed and produce a `BrokerConfig` with the documented defaults. Loading one that omits any individual field SHALL succeed and default that field. The system MUST NOT change behavior of existing v0.2.0 sessions when this field is absent.

#### Scenario: Config file with no broker section loads successfully

- **WHEN** a `.git-paw/config.toml` containing no `[broker]` section is parsed
- **THEN** the resulting config exposes a `BrokerConfig` with `enabled = false`, `port = 9119`, `bind = "127.0.0.1"`

#### Scenario: Config file with explicit broker enabled section

- **WHEN** a `.git-paw/config.toml` containing `[broker]\nenabled = true\nport = 9200\nbind = "127.0.0.1"` is parsed
- **THEN** the resulting `BrokerConfig` has `enabled = true`, `port = 9200`, `bind = "127.0.0.1"`

#### Scenario: Config file with partial broker section uses defaults for missing fields

- **WHEN** a `.git-paw/config.toml` containing only `[broker]\nenabled = true` is parsed
- **THEN** the resulting `BrokerConfig` has `enabled = true`, `port = 9119`, `bind = "127.0.0.1"`

#### Scenario: BrokerConfig URL helper produces a well-formed URL

- **WHEN** `BrokerConfig { enabled: true, port: 9200, bind: "127.0.0.1" }` calls `url()`
- **THEN** the result is `"http://127.0.0.1:9200"`

#### Scenario: Every documented field round-trips

- **WHEN** a `.git-paw/config.toml` naming every `[broker]` field, including `wave_stall_minutes` and a `[broker.watcher]` sub-table, is parsed
- **THEN** each field SHALL carry the configured value, and no field SHALL be rejected as unknown

### Requirement: Broker entry point and handle

The system SHALL provide a synchronous function with the signature:

```rust
pub fn start_broker(
    config: &BrokerConfig,
    state: BrokerState,
    watch_targets: Vec<WatchTarget>,
) -> Result<BrokerHandle, BrokerError>
```

This function SHALL:

1. Construct a multi-threaded tokio runtime owned by the returned handle
2. Spawn the axum server on that runtime, bound to `config.bind:config.port`
3. Spawn one `watcher::watch_worktree` task per `WatchTarget` so that working-tree changes trigger broker publishes
4. Return a `BrokerHandle` value that, when dropped, shuts the runtime down and signals all watcher tasks to stop

The function MUST be callable from synchronous Rust code without any surrounding `#[tokio::main]` or other runtime context. The function MUST NOT panic on any expected failure (port in use, invalid bind address, runtime construction failure); it SHALL return a `BrokerError` variant instead.

`watch_targets` MAY be empty; in that case no watcher tasks are spawned and the broker behaves as in v0.3.0.

#### Scenario: start_broker succeeds with default config on a free port

- **GIVEN** a `BrokerConfig` with `enabled = true`, `bind = "127.0.0.1"`, and a port known to be free
- **WHEN** `start_broker(&config, state, vec![])` is called from synchronous test code
- **THEN** the function returns `Ok(BrokerHandle)`
- **AND** an HTTP `GET /status` request to the configured URL succeeds within 1 second

#### Scenario: BrokerHandle drop shuts down the runtime

- **GIVEN** a successfully started broker
- **WHEN** the `BrokerHandle` is dropped
- **THEN** subsequent HTTP requests to the configured URL fail to connect within 1 second

#### Scenario: start_broker returns an error when the port is occupied by a non-broker process

- **GIVEN** a TCP listener bound to `127.0.0.1:9119` by code other than git-paw
- **WHEN** `start_broker` is called with `port = 9119`
- **THEN** the function returns `Err(BrokerError::PortInUse { .. })`
- **AND** the error message identifies the configured port

#### Scenario: Watch targets are honoured

- **GIVEN** a `BrokerConfig` with broker enabled and a `WatchTarget` describing a temporary worktree
- **WHEN** `start_broker(&config, state, vec![target])` is called
- **THEN** the broker SHALL spawn a watcher task for that worktree
- **AND** modifying a file inside the worktree SHALL eventually result in an `agent.status` message in the broker state

### Requirement: Stale broker detection

Before binding, the system SHALL probe `GET <config.url()>/status` with a timeout of at most 500 milliseconds and SHALL act on the result as follows:

- **No connection / connection refused** → bind and start a new broker
- **Connection reset, broken pipe, or EOF before any response byte** → the listener is going away, not answering slowly: re-probe once after a brief pause; if the re-probe is refused or resets again, treat the port as having no live broker and bind and start a new broker. A re-probe that receives a response is classified by that response
- **HTTP 200 with a response body containing the marker field `"git_paw": true` AND a `repo_root` equal to the caller's repository** → return a `BrokerHandle` that reattaches to the existing broker without binding
- **HTTP 200 with the marker field but a different `repo_root`** → return an error stating that the port is held by another repository's git-paw broker, naming that repository and the port, and telling the user to set a distinct `[broker] port` for one of them. The system SHALL NOT reattach, because reattaching would route this repository's agents into the other repository's broker while this repository's dashboard renders an empty, unwatched state
- **HTTP 200 with the marker field and no `repo_root`** (a broker from a git-paw version that does not report it) → treat the owner as unknown and return the same error, naming the port; the system SHALL NOT assume the broker belongs to the caller
- **HTTP response without the marker field** → return `Err(BrokerError::PortInUse { .. })` indicating the port is occupied by a foreign process
- **Timeout** (the listener accepted but sent nothing within the read timeout) → return `Err(BrokerError::ProbeTimeout { .. })` so the user can investigate. A reset or EOF SHALL NOT be classified as a timeout

Any local process can bind the port and answer the probe, so a reported `repo_root` is untrusted input. Before it is reproduced in an error message the system SHALL strip control characters from it and SHALL cap its length, and the probe SHALL bound how much of the response it reads.

#### Scenario: Probe finds no listener and proceeds to bind

- **GIVEN** no process is listening on `127.0.0.1:9119`
- **WHEN** `start_broker` is called with `port = 9119`
- **THEN** the probe returns connection-refused
- **AND** `start_broker` proceeds to bind and returns `Ok(BrokerHandle)`

#### Scenario: Probe finds a live git-paw broker and reattaches

- **GIVEN** a live git-paw broker for the same repository is already running on `127.0.0.1:9119`
- **WHEN** `start_broker` is called with the same config
- **THEN** the probe receives an HTTP 200 with `"git_paw": true` in the body
- **AND** `start_broker` returns `Ok(BrokerHandle)` without binding a new socket

#### Scenario: Probe finds a foreign HTTP server and refuses

- **GIVEN** a non-git-paw HTTP server is bound to `127.0.0.1:9119` and responds to `GET /status` with `404`
- **WHEN** `start_broker` is called with `port = 9119`
- **THEN** the probe receives a response without the `"git_paw"` marker
- **AND** `start_broker` returns `Err(BrokerError::PortInUse { .. })` mentioning the port

#### Scenario: Another repository's broker on the port is refused

- **GIVEN** a live git-paw broker for repository A on `127.0.0.1:9119`
- **WHEN** a session for repository B calls `start_broker` with `port = 9119`
- **THEN** it SHALL return an error naming repository A and port `9119`
- **AND** the error SHALL tell the user to set a distinct `[broker] port`
- **AND** no `BrokerHandle` SHALL be returned for repository B

#### Scenario: A broker that does not report its repository is not assumed to be ours

- **GIVEN** a live git-paw broker on the port whose `GET /status` body has no `repo_root`
- **WHEN** `start_broker` is called
- **THEN** it SHALL return the port-held error rather than reattaching

#### Scenario: A hostile reported repository cannot reach the terminal intact

- **GIVEN** a listener on the port whose `GET /status` reports a `repo_root` containing ANSI escape sequences and other control characters
- **WHEN** the port-held error is rendered
- **THEN** the rendered message SHALL contain no control characters
- **AND** an over-long reported repository SHALL be truncated rather than printed in full

#### Scenario: A listener that resets mid-handshake is not a live broker

- **GIVEN** a listener on the broker port that accepts each connection and closes it before sending any response byte
- **WHEN** the stale-broker probe runs against the port
- **THEN** it SHALL re-probe once
- **AND** when the re-probe also resets or is refused, it SHALL report no live broker rather than a probe timeout

#### Scenario: A silent listener is a probe timeout

- **GIVEN** a listener on the broker port that accepts a connection and never answers
- **WHEN** the stale-broker probe runs against the port
- **THEN** it SHALL report a probe timeout
- **AND** `start_broker` SHALL return `Err(BrokerError::ProbeTimeout { .. })`

### Requirement: Port reuse on restart

The broker SHALL bind its TCP listener with `SO_REUSEADDR` enabled so that a restart immediately following a crash succeeds even when the previous socket is held in `TIME_WAIT`.

#### Scenario: Restart immediately after crash succeeds

- **GIVEN** a broker that was abruptly terminated (kill -9) on `127.0.0.1:9119`
- **WHEN** `start_broker` is called within 5 seconds with the same port
- **THEN** binding succeeds and the new broker responds to `GET /status` within 1 second

### Requirement: Broker shared state

The system SHALL define `BrokerState` as a value type whose lifetime is managed by the caller. Callers SHALL share `BrokerState` across threads and async tasks by wrapping it in `std::sync::Arc<BrokerState>` and cloning the `Arc` (`O(1)`); the type itself is not required to implement `Clone`. The type SHALL satisfy:

- `BrokerState: Send + Sync + 'static`
- Cheap sharing across threads via `Arc<BrokerState>`
- All public methods on `BrokerState` SHALL be callable from both async and synchronous code without requiring a tokio context
- Holding a read or write guard across an `.await` point SHALL be statically discouraged (clippy lint `clippy::await_holding_lock` enabled in `Cargo.toml`)

#### Scenario: Arc-wrapped BrokerState shares underlying state

- **GIVEN** an `Arc<BrokerState>` value `s1` with one registered agent record
- **WHEN** `Arc::clone(&s1)` is called producing `s2`
- **THEN** queries against `s2` see the same agent record
- **AND** mutations through `s2` are observable from `s1`

#### Scenario: BrokerState is accessible from synchronous code

- **WHEN** a synchronous function reads agent status via `&BrokerState`
- **THEN** the call completes without entering a tokio runtime

### Requirement: Signal handling

The broker host process SHALL install a `SIGINT` handler that prevents an accidental Ctrl+C from terminating the broker. The handler SHALL NOT call `std::process::exit` and SHALL NOT trigger broker shutdown. Clean shutdown of the broker SHALL only occur via:

- The `BrokerHandle` being dropped
- `SIGHUP` delivered to the host (its service surface closed)
- `SIGTERM` or `SIGKILL` from outside the process (e.g. `git paw stop` killing tmux)

#### Scenario: SIGINT does not terminate the broker

- **GIVEN** a running broker host
- **WHEN** `SIGINT` is delivered to it
- **THEN** the broker continues to respond to HTTP requests
- **AND** the process does not exit

#### Scenario: Dropping BrokerHandle still shuts down cleanly

- **GIVEN** a running broker
- **WHEN** `BrokerHandle` is dropped
- **THEN** the broker shuts down and the process owning the handle continues running

### Requirement: Panic isolation

A panic in any single HTTP request handler or background tokio task MUST NOT terminate the broker runtime or the broker host process. The system SHALL rely on axum's default panic-catching for handlers and SHALL spawn any background broker tasks via `tokio::spawn` so panics remain isolated to a single task.

#### Scenario: Panic in a request handler returns 500 and broker keeps serving

- **GIVEN** a running broker whose handler implementation contains a deliberate `panic!` for one route (test-only setup)
- **WHEN** a request to that route arrives
- **THEN** the response status is 500
- **AND** subsequent requests to other routes continue to succeed

### Requirement: Delivery extension contract

The system SHALL create `src/broker/delivery.rs` and SHALL declare three crate-private functions used by the HTTP handlers:

- `publish_message(state: &BrokerState, msg: BrokerMessage)`
- `poll_messages(state: &BrokerState, agent_id: &str, since: u64) -> (Vec<BrokerMessage>, u64)`
- `agent_status_snapshot(state: &BrokerState) -> Vec<AgentStatusEntry>`

`poll_messages` SHALL accept a `since` parameter (sequence number) and return a tuple of `(messages, last_seq)` where `messages` contains only messages with sequence numbers strictly greater than `since`, and `last_seq` is the highest sequence number among the returned messages (or `0` if empty). Polling is non-destructive — messages are retained and can be re-read with the same `since` value.

In this change, these functions SHALL be stubs:

- `publish_message` SHALL panic via `todo!("peer-messaging")` if invoked at runtime, ensuring premature use is loud
- `poll_messages` SHALL return `(Vec::new(), 0)`
- `agent_status_snapshot` SHALL return an empty `Vec`

The function signatures, names, and module location SHALL be considered the stable contract that `peer-messaging` consumes in Wave 2. `peer-messaging` MAY add new fields to `BrokerStateInner` and MAY add new helper functions, but MUST NOT change these three signatures and MUST NOT edit `src/broker/server.rs`.

#### Scenario: Delivery functions exist with the documented signatures

- **WHEN** the crate is built
- **THEN** `crate::broker::delivery::publish_message`, `crate::broker::delivery::poll_messages`, and `crate::broker::delivery::agent_status_snapshot` are reachable with the signatures specified above

#### Scenario: poll_messages stub returns empty with zero sequence

- **WHEN** `poll_messages(&state, "feat-x", 0)` is called against any `BrokerState`
- **THEN** the result is `(Vec::new(), 0)`

#### Scenario: agent_status_snapshot stub returns empty

- **WHEN** `agent_status_snapshot(&state)` is called against any `BrokerState`
- **THEN** the result is an empty `Vec`

### Requirement: Async containment

The introduction of tokio SHALL be confined to the `src/broker/` module tree. The system MUST NOT add `#[tokio::main]` to `src/main.rs` and MUST NOT require a tokio runtime for any code path outside of `src/broker/`. Existing synchronous modules (`tmux`, `git`, `session`, `interactive`, etc.) MUST remain synchronous.

#### Scenario: main.rs has no tokio attribute

- **WHEN** `src/main.rs` is inspected
- **THEN** it contains no `#[tokio::main]` attribute and no `tokio::runtime::Runtime` construction

#### Scenario: Sync commands work without a tokio runtime

- **WHEN** `git paw init` (or any other v0.2.0 sync command) is invoked
- **THEN** it completes successfully without constructing a tokio runtime

### Requirement: POST /publish accepts and validates broker messages

The system SHALL expose `POST /publish` accepting an `application/json` request body. For a request that passes the listener guard (see *Broker listener validates the Host header on a loopback bind* and *Broker listener validates the Origin header*), the handler SHALL parse the body via `BrokerMessage::from_json` and SHALL behave as follows:

- **Valid `BrokerMessage`** → call `publish_message(&state, msg)`, respond with HTTP `202 Accepted` and an empty body
- **Invalid JSON or validation failure** → respond with HTTP `400 Bad Request` and an `application/json` body containing `{ "error": "<message>" }` describing the failure
- **Wrong content type** → respond with HTTP `415 Unsupported Media Type`
- **Empty body** → respond with HTTP `400 Bad Request` with an error explaining a JSON body is required

A request refused by the listener guard SHALL receive `403` instead, and SHALL NOT reach this handler.

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

#### Scenario: A publish refused by the listener guard never reaches the handler

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** a valid `POST /publish` is sent with `Host: attacker.example:9119`
- **THEN** the response status is `403`, not `202`
- **AND** no message is delivered

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

### Requirement: GET /status returns broker and agent state

The system SHALL expose `GET /status` returning the current state of the broker and all known agents. The response SHALL be HTTP `200 OK` with an `application/json` body containing at least these fields:

- `git_paw: bool` — always `true`; serves as the marker the stale-broker probe checks
- `version: String` — the git-paw crate version (`env!("CARGO_PKG_VERSION")`)
- `uptime_seconds: u64` — seconds since the broker started
- `agents: Array<AgentStatusEntry>` — the list returned by `agent_status_snapshot`
- `repo_root: String` — the canonical path of the repository this broker serves; the stale-broker probe compares it with the caller's repository

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

#### Scenario: Status response identifies the repository

- **GIVEN** a running broker started for a repository
- **WHEN** `GET /status` is sent
- **THEN** the response body contains a `repo_root` field equal to that repository's canonical path

### Requirement: GET /log returns the full broker message log

The system SHALL expose `GET /log` returning the broker's complete `message_log` filtered to entries with sequence number greater than the optional `since` query parameter (defaulting to `0`, i.e. every message).

The response body SHALL be JSON of the shape `{"entries": [...], "last_seq": N}`. Each entry SHALL have the fields `seq: u64`, `timestamp_unix_secs: u64`, and `message: BrokerMessage`. Entries SHALL appear in chronological order (oldest first) so callers can replay them into a fresh `BrokerState` to reconstruct broker state from outside the dashboard process.

This endpoint is the IPC seam used by `cmd_supervisor` (which runs in a different process from the broker) to (a) build the dependency graph for merge ordering from `agent.blocked` messages, and (b) populate broker state for the session-summary write so per-agent records reflect what actually happened during the session.

#### Scenario: GET /log returns all messages chronologically when since is absent

- **GIVEN** a broker with three published `agent.status` messages
- **WHEN** `GET /log` is sent
- **THEN** the response status is `200`
- **AND** `entries.length` is `3`
- **AND** `entries[0].seq < entries[1].seq < entries[2].seq`
- **AND** `last_seq` equals `entries[2].seq`

#### Scenario: GET /log?since=N filters out messages with seq <= N

- **GIVEN** a broker with three published messages at seq 1, 2, 3
- **WHEN** `GET /log?since=2` is sent
- **THEN** `entries.length` is `1`
- **AND** `entries[0].seq` is `3`

#### Scenario: GET /log with non-numeric since returns 400

- **WHEN** `GET /log?since=notanumber` is sent
- **THEN** the response status is `400`

### Requirement: Unknown routes return 404

The system SHALL respond with HTTP `404 Not Found` for any request that passes the listener guard and whose path does not match one of the five documented routes (`POST /publish`, `POST /watch`, `GET /messages/:agent_id`, `GET /status`, `GET /log`).

The listener guard runs before routing, so a request it refuses receives `403` rather than `404` even when its path matches no route.

#### Scenario: Unknown path returns 404

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET /unknown/route` is sent with `Host: 127.0.0.1:9119`
- **THEN** the response status is `404`

#### Scenario: Unknown path with a refused Host returns 403, not 404

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET /unknown/route` is sent with `Host: attacker.example:9119`
- **THEN** the response status is `403`

### Requirement: Wrong HTTP methods return 405

The system SHALL respond with HTTP `405 Method Not Allowed` for requests that pass the listener guard, where the path matches a documented route but the method does not.

The listener guard runs before routing, so a request it refuses receives `403` rather than `405`.

#### Scenario: GET /publish returns 405

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET /publish` is sent with `Host: 127.0.0.1:9119`
- **THEN** the response status is `405`

#### Scenario: POST /status returns 405

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `POST /status` is sent with `Host: 127.0.0.1:9119`
- **THEN** the response status is `405`

#### Scenario: POST /messages/feat-x returns 405

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `POST /messages/feat-x` is sent with `Host: 127.0.0.1:9119`
- **THEN** the response status is `405`

### Requirement: Live watch-target registration endpoint

The broker SHALL expose `POST /watch` accepting a JSON body with an agent id, worktree path, cli label, and an optional spec id, and SHALL add that path to its live filesystem-watch-target set so the watcher begins surfacing the worktree's activity without a broker restart. When a spec id is present and the learnings aggregator is attached, the broker SHALL record the agent → spec mapping for conflict classification. A spec id longer than 128 characters or containing anything but ASCII letters, digits, `.`, `_`, `/` and `-` SHALL be rejected with 400, since it is written into the learnings file the orchestrator reads. The endpoint SHALL be idempotent: registering an already-watched path SHALL NOT create a duplicate target. It SHALL bind to loopback only, consistent with the other broker endpoints. A body without a spec id SHALL be accepted exactly as before.

A coding agent registered through `POST /watch` — hot-added via `git paw add`, or registered by the launcher of a fresh `git paw start` once its pane exists — SHALL appear in `/status` from worktree activity on the same terms as an agent seeded from the saved session, independent of whether its CLI has self-published a status.

#### Scenario: Registering a target surfaces the worktree via the watcher

- **GIVEN** a running broker and a worktree not among its start-time watch targets
- **WHEN** a client `POST`s the worktree (agent id + path + cli) to `/watch`
- **AND** that worktree subsequently has uncommitted changes
- **THEN** the watcher SHALL surface the agent in `/status` without a broker restart and without requiring the agent's CLI to have self-published

#### Scenario: Registration is idempotent

- **GIVEN** a worktree already registered as a watch target
- **WHEN** the same path is `POST`ed to `/watch` again
- **THEN** the broker SHALL NOT create a duplicate target and SHALL return success

#### Scenario: Endpoint binds to loopback only

- **WHEN** the broker starts with `/watch` enabled
- **THEN** the endpoint SHALL be reachable only on the loopback interface, consistent with `/publish`, `/status`, and `/messages`

#### Scenario: A registration with a spec id maps the agent

- **GIVEN** a running broker with the learnings aggregator attached
- **WHEN** a client `POST`s `/watch` for agent `feat-x` with spec id `003-user-list`
- **THEN** a later forward conflict between `feat-x` and another agent mapped to `003-user-list` SHALL be classified `forward-conflict-intra-spec`

### Requirement: Start flow conditionally creates dashboard pane

When `[broker] enabled = true` in config, the `start` flow SHALL insert a dashboard pane that runs `git paw __dashboard` at pane 0 in every mode:

- **Bare `git paw start` and `git paw start --from-specs` (no supervisor):** dashboard at pane 0; coding agent panes start at pane 1. Same as v0.4.
- **`git paw start --supervisor` (or `--from-specs --supervisor`, or `[supervisor] enabled = true`):** dashboard at pane 0 as the full-width top row; coding agent panes start at pane 1 in the supervised grid. No orchestrator pane is created.

When `[broker] enabled = false` (or absent), the start flow SHALL behave identically to v0.2.0 with no dashboard pane (and supervised mode is not meaningful since the dashboard, broker status, and the drive loop's escalations all require the broker).

The dashboard pane is in both cases a non-interactive TUI process; it does NOT receive a `tmux send-keys` boot block injection, and it sends no keystrokes to any other pane.

The panes above are the panes of the session's window 0. When the broker is enabled the session also has a detached `broker` service window holding the broker host; it holds no agent and is not counted among the session's panes.

#### Scenario: Broker enabled in bare-start mode adds dashboard as pane 0

- **GIVEN** `[broker]\nenabled = true` in config and no supervisor mode
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 4 panes: pane 0 running `git paw __dashboard`, panes 1-3 running coding CLIs

#### Scenario: Broker enabled in supervised mode also places dashboard at pane 0

- **GIVEN** `[broker]\nenabled = true` and `[supervisor]\nenabled = true` in config
- **WHEN** `git paw start --supervisor` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 4 panes
- **AND** pane 0 SHALL be the dashboard (`git paw __dashboard`)
- **AND** panes 1-3 SHALL be the coding CLIs

#### Scenario: Broker disabled produces no dashboard pane

- **GIVEN** no `[broker]` section in config (or `enabled = false`)
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 3 panes, all running coding CLIs (same as v0.2.0)

#### Scenario: Dashboard pane title

- **GIVEN** broker enabled (in either bare-start or supervised mode)
- **WHEN** the session is created
- **THEN** the dashboard pane has the title `"dashboard"`

### Requirement: Broker URL injected into tmux environment

When broker is enabled, the `start` flow SHALL call `tmux set-environment -t <session-name> GIT_PAW_BROKER_URL <url>` before any pane CLI commands are sent. The URL SHALL be computed from `BrokerConfig::url()`.

All panes in the session SHALL inherit this environment variable automatically via tmux's session-level environment.

#### Scenario: GIT_PAW_BROKER_URL is set on the session

- **GIVEN** `[broker]\nenabled = true\nport = 9119` in config
- **WHEN** a session is created
- **THEN** `tmux show-environment -t <session> GIT_PAW_BROKER_URL` returns `GIT_PAW_BROKER_URL=http://127.0.0.1:9119`

#### Scenario: Env var is set before pane commands

- **WHEN** the tmux session builder emits commands
- **THEN** the `set-environment` command appears before any `send-keys` commands

#### Scenario: No env var when broker is disabled

- **GIVEN** broker is disabled
- **WHEN** a session is created
- **THEN** `tmux show-environment -t <session> GIT_PAW_BROKER_URL` returns "unknown variable" or empty

### Requirement: Stop flow shuts down broker via pane 0 exit

The `stop` flow SHALL kill the tmux session via `tmux::kill_session`. Killing the session terminates the broker host's service surface (the `broker` window), which delivers `SIGHUP` to `git paw __broker`, which drops `BrokerHandle`, which triggers graceful broker shutdown including the final log flush. For a session started before the host split, killing the session kills the dashboard pane that hosts the broker, with the same effect.

The stop flow SHALL NOT render a confirmation prompt, in any context. Stopping is recoverable: worktrees and session state survive, and `git paw start` recovers the session. When the session's recorded status is `Paused`, stopping SHALL kill the still-running CLI processes as for any other session.

#### Scenario: Stop kills tmux and broker shuts down

- **GIVEN** an active session with broker enabled
- **WHEN** `git paw stop` is executed
- **THEN** the tmux session SHALL be killed
- **AND** the broker port SHALL be freed within 5 seconds
- **AND** the session's broker log SHALL contain a final flush of all messages

#### Scenario: Stop never prompts

- **GIVEN** an active session, with stdin attached to a TTY or not
- **WHEN** `git paw stop` is executed
- **THEN** no interactive prompt SHALL be rendered
- **AND** the stop SHALL proceed immediately

#### Scenario: Stop after pause kills remaining CLI panes

- **GIVEN** a session with `status == Paused` and tmux still alive
- **WHEN** `git paw stop` is executed
- **THEN** the tmux session SHALL be killed
- **AND** every previously-still-running CLI process SHALL be terminated
- **AND** the session state SHALL be `status == Stopped`

### Requirement: Purge flow cleans up broker log

The `purge` flow SHALL delete the broker log recorded in the session state's `broker_log_path` field — `<session_state_dir>/<session_name>.broker.log` for a session saved by this version — and SHALL delete no other file in the session-state directory. Deletion SHALL be best-effort: a missing or already-deleted log SHALL NOT cause an error.

Because the log is named per session, purging one repository's session SHALL NOT delete another session's log, including the log of a neighbouring repository's live broker. A session saved by an earlier git-paw records the shared `broker.log` name; purge SHALL delete whatever path that session actually recorded.

#### Scenario: Purge deletes the session's broker log

- **GIVEN** an active session with broker enabled and a `<session_name>.broker.log` file
- **WHEN** `git paw purge --force` is executed
- **THEN** the session's broker log file is deleted
- **AND** the session state file is deleted
- **AND** worktrees are removed

#### Scenario: Purge succeeds when the session's broker log does not exist

- **GIVEN** a session state with `broker_log_path` pointing to a nonexistent file
- **WHEN** `git paw purge --force` is executed
- **THEN** purge completes successfully

#### Scenario: Purging one session spares a neighbouring session's log

- **GIVEN** two sessions' broker logs side by side in the session-state directory
- **WHEN** `git paw purge --force` is executed for one of them
- **THEN** that session's log SHALL be deleted
- **AND** the other session's log SHALL remain in place

#### Scenario: Purge deletes the path an older git-paw recorded

- **GIVEN** a session saved by an earlier git-paw whose `broker_log_path` is the shared `broker.log`
- **WHEN** `git paw purge --force` is executed
- **THEN** that recorded file SHALL be deleted

### Requirement: Status shows broker information

When a session is active and has broker fields in its state, `git paw status` SHALL display broker information including the configured URL. The system SHALL probe `GET /status` against the broker URL **with the session's repository**, so a broker on the port that serves a different repository is never reported as this session's broker, and SHALL act on the result as follows:

- If the probe finds a git-paw broker for this session's repository AND the session's broker host is alive — its `broker` service surface exists, as the terminal backend reports; for a session with no such surface (one started before the host split; a host that died while its terminal session lives is indistinguishable and is judged the same way, the probe still deciding whether anything answers), its terminal session is alive: display the broker URL, agent count, and uptime from the response.
- If the probe finds a git-paw broker for this session's repository but the session's broker host is not alive (for example after `git paw stop`, while a process that outlived the session still answers on the port): display the broker URL noting that a leftover broker answers with no live session host. The system SHALL NOT display it as running.
- If the probe finds a git-paw broker for a different — or unreported — repository: display the broker URL noting it is held by another repository's git-paw broker. The system SHALL NOT display it as running.
- If the probe fails AND the session's effective status is `Paused`: display the broker URL with `(paused — run 'git paw start' to resume)`.
- If the probe fails AND the session's effective status is `Active` or `Stopped`: display the broker URL with `(not responding)`.

The broker's state SHALL be taken from the probe and the host's liveness, never from the session receipt alone. `git paw status` SHALL render the three session statuses with distinguishable visual treatment (e.g. different emoji / labels for `active`, `paused`, `stopped`). The paused row SHALL include a one-line restart hint pointing at `git paw start`.

#### Scenario: Status shows running broker with agents

- **GIVEN** an active session with broker enabled and 3 agents registered
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL contain the broker URL, `running`, and `3 agents`

#### Scenario: Status shows paused session with broker offline

- **GIVEN** a session with `status == Paused`, tmux alive, broker stopped
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL show the paused state distinctly from `active` and `stopped`
- **AND** the output SHALL contain a restart hint mentioning `git paw start`
- **AND** the broker line SHALL indicate the broker is paused, not "not responding" in error terms

#### Scenario: Status shows broker not responding for crashed active session

- **GIVEN** a session state with `status == Active` and broker fields but the broker host has crashed
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL contain the broker URL and `not responding`

#### Scenario: Status after stop does not report the broker running

- **GIVEN** a session stopped with `git paw stop`
- **WHEN** `git paw status` is executed right afterwards, whether or not a leftover process still answers on the broker port
- **THEN** the broker line SHALL NOT report the broker as running

#### Scenario: Status shows no broker info when disabled

- **GIVEN** a session without broker fields in state
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL NOT mention broker, port, or agents

#### Scenario: Status reports a port held by another repository

- **GIVEN** a session in repository B whose configured port is held by a live git-paw broker for repository A
- **WHEN** `git paw status` is executed in repository B
- **THEN** the broker line SHALL say the port is held by another repository
- **AND** the output SHALL NOT report the broker as running

### Requirement: Pause flow detaches client and stops broker without killing tmux

`git paw pause` SHALL perform a soft-stop that:

1. Detaches every client currently attached to the session by running `tmux detach-client -s <session-name>`. With no clients attached, the command SHALL be a no-op and SHALL NOT error.
2. Stops the broker by closing the broker host's service surface (the `broker` window). The host receives `SIGHUP`, the `BrokerHandle` drop runs, the broker shuts down gracefully, and the session's broker log flushes. When the session has no `broker` surface (started before the host split), pause SHALL instead kill the dashboard pane at the recorded `dashboard_pane` (default `0`).
3. Updates the on-disk session state's `status` field from `Active` to `Paused`.
4. Leaves the tmux session, the dashboard pane, and every coding-agent CLI pane running.
5. Prints a one-line confirmation: `"Session '<name>' paused. <N> CLI pane(s) still running. Run 'git paw start' to resume."`

The pause flow SHALL NOT call `tmux::kill_session` at any point.

#### Scenario: Pause detaches the client

- **GIVEN** an active session with a tmux client attached
- **WHEN** `git paw pause` is executed
- **THEN** `tmux list-clients -t <session>` SHALL return no clients
- **AND** the tmux session SHALL still be alive

#### Scenario: Pause stops the broker

- **GIVEN** an active session with broker enabled and listening on port P
- **WHEN** `git paw pause` is executed
- **THEN** within 5 seconds, port P SHALL be free
- **AND** the session's broker log SHALL contain a final flush of all messages

#### Scenario: Pause leaves coding-agent panes and the dashboard alive

- **GIVEN** an active session with 3 coding-agent panes and a dashboard
- **WHEN** `git paw pause` is executed
- **THEN** window 0 SHALL still report 4 panes (dashboard + 3 agents)
- **AND** each coding-agent CLI process SHALL still be running
- **AND** the dashboard SHALL render its broker-unreachable state

#### Scenario: Pause updates session state to paused

- **GIVEN** an active session
- **WHEN** `git paw pause` is executed
- **THEN** loading the session via `session::load_session` SHALL return a session with `status == SessionStatus::Paused`

#### Scenario: Pause prints a resume hint

- **WHEN** `git paw pause` completes successfully
- **THEN** stdout SHALL contain the session name
- **AND** stdout SHALL contain the phrase "Run `git paw start` to resume" (or words conveying the same)

### Requirement: Pause is idempotent

Running `git paw pause` against a session that is already in `Paused` state (no clients attached, broker stopped, tmux alive) SHALL be a no-op that exits successfully with an informational message. The second invocation SHALL NOT error, SHALL NOT re-publish broker shutdown events, and SHALL NOT alter the session state file.

#### Scenario: Pause on an already-paused session

- **GIVEN** a session with `status == Paused` and tmux alive
- **WHEN** `git paw pause` is executed
- **THEN** the command SHALL exit 0
- **AND** stdout SHALL contain a message indicating the session is already paused
- **AND** the session state file SHALL be unchanged

#### Scenario: Pause on a stopped session

- **GIVEN** a session with `status == Stopped` (tmux not alive)
- **WHEN** `git paw pause` is executed
- **THEN** the command SHALL exit 0
- **AND** stdout SHALL inform the user the session is already stopped and pause has no effect
- **AND** the session state SHALL remain `Stopped`

#### Scenario: Pause when no session exists

- **GIVEN** no session file exists for the current repo
- **WHEN** `git paw pause` is executed
- **THEN** the command SHALL exit 0
- **AND** stdout SHALL contain "No active session for this repo." (or words conveying the same)

### Requirement: Start flow restarts a paused session

When `git paw start` is invoked against a session whose effective status is `Paused` (recorded `Paused` AND tmux alive), the start flow SHALL:

1. Recreate the broker host in its detached service surface (`broker` window), handing it the session name and the re-stamped log path, and wait, bounded by 10 seconds, for `GET /status` to answer.
2. Recreate the dashboard pane at the saved `dashboard_pane` index (or `0` if absent) ONLY when no pane is currently running `git paw __dashboard`; an existing dashboard pane SHALL be left alone and SHALL reconnect on its own.
3. Update the session state's `status` field from `Paused` to `Active`.
4. Attach to the tmux session via `tmux attach -t <session-name>`.

The restart-from-pause flow SHALL NOT create worktrees, SHALL NOT spawn coding-agent CLI processes, and SHALL NOT inject boot prompts other than prompts held for agents added while paused.

#### Scenario: Start against paused session reattaches and restarts broker

- **GIVEN** a session with `status == Paused` and tmux alive
- **WHEN** `git paw start` is executed
- **THEN** the broker SHALL be listening on its configured port within 10 seconds
- **AND** the user's tmux client SHALL be attached to the session
- **AND** the session state SHALL be `status == Active`

#### Scenario: Existing dashboard pane is reused

- **GIVEN** a paused session whose dashboard pane is still running
- **WHEN** `git paw start` is executed
- **THEN** no second dashboard pane SHALL be created
- **AND** the existing dashboard SHALL render live data within 5 seconds of the broker answering

#### Scenario: Start against paused session does not respawn CLIs

- **GIVEN** a paused session whose coding-agent CLI processes have PIDs P1..Pn
- **WHEN** `git paw start` is executed
- **THEN** the coding-agent panes SHALL still hold processes with PIDs P1..Pn
- **AND** no keystrokes SHALL be sent to the coding-agent panes during the restart other than a held boot prompt

#### Scenario: Start against paused-but-tmux-dead falls through to recover

- **GIVEN** a session with recorded `status == Paused` but `tmux has-session` exits non-zero
- **WHEN** `git paw start` is executed
- **THEN** `effective_status` SHALL evaluate to `Stopped`
- **AND** the start flow SHALL run the existing cold-recovery path

### Requirement: Launch refuses a port held by another repository before creating anything

Before creating the tmux session — and therefore before any agent pane exists — every launch path that starts or rebuilds a broker-enabled session (`git paw start`, the spec-driven launch, the supervised launch, and session recovery) SHALL probe the broker port against the repository being launched, and SHALL fail the launch with the port-held error when a git-paw broker for a different — or unreported — repository holds it.

The check is required at launch time, not only when the dashboard pane starts its broker: git-paw injects `GIT_PAW_BROKER_URL` into every agent pane, so by the time the dashboard refuses, this repository's agents are already publishing into the other repository's broker. A refused launch SHALL leave no tmux session and no session receipt behind.

**The port probed SHALL be the port the agent panes will be given.** For a fresh launch that is the configured `[broker] port`. For session recovery it is the saved session's `broker_port`/`broker_bind`, which is what recovery rebuilds `GIT_PAW_BROKER_URL` from and which can differ from the current config when `[broker] port` changed after the session was saved; probing the configured port there would miss the clash the agents actually face. A session saved with no broker fields gets no broker URL at all, so recovery SHALL skip the check.

**The check SHALL run before any interactive prompt.** It belongs as early as the resolved configuration allows — ahead of the branch and CLI pickers — so the user is never asked to make selections for a launch that cannot proceed.

A disabled broker SHALL skip the check entirely, and so SHALL a `--dry-run`, which creates nothing and only prints a plan. A foreign listener or a probe timeout SHALL NOT fail the launch here; those are reported by the dashboard's own broker start-up with their existing errors.

#### Scenario: A second repository's launch is refused before any pane exists

- **GIVEN** a live git-paw broker for repository A on port P
- **AND** repository B configured with `[broker] port = P`
- **WHEN** `git paw start` is executed in repository B
- **THEN** the command SHALL exit non-zero with an error naming repository A, port P, and the `[broker] port` fix
- **AND** no tmux session SHALL have been created
- **AND** no session receipt SHALL have been written

#### Scenario: A disabled broker launches regardless of the port

- **GIVEN** `[broker] enabled = false` and a git-paw broker for another repository on the configured port
- **WHEN** a session is launched
- **THEN** the launch SHALL proceed without probing the port

#### Scenario: The refusal precedes the branch and CLI pickers

- **GIVEN** a git-paw broker for another repository on the configured port
- **WHEN** `git paw start` is executed with neither `--branches` nor `--cli`
- **THEN** it SHALL fail with the port-held error
- **AND** no branch or CLI picker SHALL have been presented

#### Scenario: Recovery probes the saved port, not the configured one

- **GIVEN** a saved session recording `broker_port = P`, where another repository's git-paw broker holds `P`
- **AND** the current `[broker] port` is a different, free port
- **WHEN** `git paw start` recovers that session
- **THEN** it SHALL fail with the port-held error naming `P`

#### Scenario: A dry run prints its plan without probing

- **GIVEN** a git-paw broker for another repository on the configured port
- **WHEN** `git paw start --dry-run` is executed
- **THEN** the command SHALL exit zero and print the session plan
- **AND** it SHALL NOT fail with the port-held error

#### Scenario: Our own broker on the port does not block a launch

- **GIVEN** a live git-paw broker for this same repository on the configured port
- **WHEN** a session is launched
- **THEN** the pre-flight check SHALL pass, leaving the reattach decision to `start_broker`

### Requirement: Resume and recovery re-stamp the broker log path

When a broker-enabled session is resumed from pause or recovered from a stopped state, the system SHALL re-stamp the saved session's `broker_log_path` to `<session_state_dir>/<session_name>.broker.log` before saving the session back.

This migrates a session saved by an earlier git-paw — which recorded the single shared `broker.log` — onto the per-session name in one step, so a long-lived session does not keep a shared log name indefinitely. Until it is re-stamped, the recorded path remains authoritative for both the broker and `purge` (see *Dashboard subcommand starts broker and dashboard*), so the two never disagree at any point in the migration.

The re-stamped session SHALL be saved **before** the dashboard pane is started. The dashboard resolves its log path from the saved session, so starting it first would let it read the pre-re-stamp path and write there while `purge` went on to delete the re-stamped one — orphaning exactly the log this requirement exists to prevent.

That early write SHALL carry the re-stamped `broker_log_path` only. On the resume-from-pause path the session's `status` SHALL remain `Paused` until the dashboard pane exists, and SHALL flip to `Active` only afterwards: recording `Active` before the pane is up would leave a failed resume — a `split-window` or `send-keys` that did not succeed — marked active with tmux still alive, which the next `git paw start` reattaches to instead of restarting the broker.

#### Scenario: Recovery re-stamps a shared broker log path

- **GIVEN** a stopped session whose `broker_log_path` is the shared `<session_state_dir>/broker.log` written by an earlier git-paw
- **WHEN** the session is recovered with `git paw start`
- **THEN** the saved session SHALL record `<session_state_dir>/<session_name>.broker.log`

#### Scenario: A session without a broker is not given a log path

- **GIVEN** a saved session with no `broker_port`
- **WHEN** it is resumed or recovered
- **THEN** `broker_log_path` SHALL remain absent

#### Scenario: A failed resume stays paused with the log path already re-stamped

- **GIVEN** a paused broker-enabled session whose `broker_log_path` is the shared `<session_state_dir>/broker.log`
- **WHEN** `git paw start` resumes it and recreating the dashboard pane fails
- **THEN** the saved session's `status` SHALL still be `Paused`, so a retry restarts the broker rather than reattaching
- **AND** the saved session's `broker_log_path` SHALL already be `<session_state_dir>/<session_name>.broker.log`

#### Scenario: The recovered dashboard writes the re-stamped log, not the old one

- **GIVEN** a stopped session whose `broker_log_path` is the shared `<session_state_dir>/broker.log`
- **WHEN** it is recovered and its broker records a message
- **THEN** the broker SHALL have written `<session_state_dir>/<session_name>.broker.log`
- **AND** the shared `broker.log` SHALL NOT have been written

### Requirement: Broker listener validates the Host header on a loopback bind

When the broker is bound to a loopback address, it SHALL check the host part of every request's `Host` header before routing the request. The accepted host parts SHALL be:

- `localhost`, compared ASCII case-insensitively;
- an IPv4 literal in `127.0.0.0/8`;
- `[::1]`.

The port's *value* SHALL NOT be checked, so tunnels and port-forwards on any port are accepted. Its *shape* SHALL be checked: whatever follows the host part SHALL be empty or a `:` followed only by ASCII digits, so `localhost:evil.com`, `127.0.0.1:80:evil` and `[::1]evil.com` are not read as accepted hosts.

A `Host` header sent more than once, or whose bytes are not valid UTF-8, SHALL be refused rather than resolved to its first value or treated as absent.

When the request target is in absolute form (`GET http://host/path`), its authority SHALL be subject to the same rule as the `Host` header, so a foreign authority cannot be smuggled past a loopback `Host`.

A request whose `Host` is missing, or whose host part is not accepted, SHALL receive HTTP `403` with the body `{"error":"forbidden host"}`. The request SHALL NOT be routed and SHALL NOT change any broker state. The check SHALL run before routing, so it covers every route and the `404`/`405` fallbacks alike; a request that passes it SHALL be routed exactly as before.

When the broker is bound to a non-loopback address, it SHALL NOT perform this check. In that case both the process that starts the broker AND every command that brings the broker up SHALL print a warning on stderr. The command SHALL print it before the broker is started, because the broker runs in the dashboard pane, which switches to tmux's alternate screen immediately afterwards. This SHALL cover a fresh launch, a spec-driven launch, a supervisor launch, the recovery of a stopped session, and a resume from pause — not only the fresh-launch paths. The warning SHALL name the bind address and SHALL state that the broker is reachable from the network without authentication.

A non-loopback bind SHALL NOT be described as keeping browsers out. With the `Host` check disabled, a DNS-rebinding page is same-origin with the broker, and a same-origin `GET` carries no `Origin` header, so such a page can read `GET /log`, `GET /status` and `GET /messages/{agent_id}`. Only a loopback bind prevents that.

#### Scenario: Loopback Host is served

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET /status` is sent with `Host: 127.0.0.1:9119`
- **THEN** the response status SHALL be `200`

#### Scenario: Tunnelled loopback Host on another port is served

- **GIVEN** a broker bound to `127.0.0.1` on port 9119
- **WHEN** `GET /status` is sent with `Host: localhost:19119`
- **THEN** the response status SHALL be `200`

#### Scenario: Rebinding Host is refused without state change

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `POST /publish` with a valid `agent.answer` targeting `feat-x` is sent with `Host: attacker.example:9119`
- **THEN** the response status SHALL be `403` with the body `{"error":"forbidden host"}`
- **AND** `GET /messages/feat-x` SHALL return no `agent.answer`

#### Scenario: A Host with junk after the host part is refused

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET /status` is sent with `Host: localhost:evil.com`
- **THEN** the response status SHALL be `403` with the body `{"error":"forbidden host"}`

#### Scenario: A duplicated or unreadable Host is refused

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET /status` is sent with two `Host` headers, the first `127.0.0.1:9119` and the second `attacker.example:9119`
- **THEN** the response status SHALL be `403` with the body `{"error":"forbidden host"}`

#### Scenario: A foreign absolute-form target is refused despite a loopback Host

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET http://evil.com/status` is sent with `Host: 127.0.0.1:9119`
- **THEN** the response status SHALL be `403` with the body `{"error":"forbidden host"}`

#### Scenario: The launch command warns before the session is created

- **GIVEN** `[broker] bind = "0.0.0.0"` and `[broker] enabled = true`
- **WHEN** a session is launched
- **THEN** the launch command SHALL print a warning on stderr naming `0.0.0.0` before the session is created

#### Scenario: Recovering or resuming a session warns too

- **GIVEN** `[broker] bind = "0.0.0.0"` and `[broker] enabled = true`
- **WHEN** a stopped session is recovered, or a paused session is resumed
- **THEN** the command SHALL print the same warning on stderr before it brings the broker up

#### Scenario: Non-loopback bind skips the Host check and warns

- **GIVEN** `[broker] bind = "0.0.0.0"`
- **WHEN** the broker starts
- **THEN** stderr SHALL contain a warning naming `0.0.0.0` and stating that the broker is reachable from the network without authentication
- **AND** `GET /status` sent with `Host: 192.168.1.5:9119` SHALL return `200`

### Requirement: Broker listener validates the Origin header

The broker SHALL check every request that carries an `Origin` header before routing it. This SHALL apply whatever the bind address. The origin's host SHALL be one that the Host check accepts. A request whose origin's host is not accepted SHALL receive HTTP `403` with the body `{"error":"forbidden origin"}`, as SHALL a request whose `Origin` is the serialized value `null`, and as SHALL a request whose `Origin` header is sent more than once or whose bytes are not valid UTF-8 — such a header SHALL NOT be treated as absent. Such a request SHALL NOT be routed and SHALL NOT change any broker state. A request without an `Origin` header SHALL NOT be affected by this check.

Every in-tree broker client SHALL remain accepted by both checks on the default bind: the bundled `broker.sh` and `sweep.sh` helpers, the bundled post-commit git hook, and the Rust HTTP clients (the publish, watch-registration, log and status clients, and the stale-broker probe) that the CLI, the MCP server and the dashboard use.

#### Scenario: Foreign origin is refused even with a loopback Host

- **WHEN** `POST /publish` is sent with `Host: 127.0.0.1:9119` and `Origin: http://attacker.example`
- **THEN** the response status SHALL be `403` with the body `{"error":"forbidden origin"}`

#### Scenario: A duplicated or unreadable Origin is refused

- **WHEN** `GET /status` is sent with `Host: 127.0.0.1:9119` and an `Origin` header whose bytes are not valid UTF-8
- **THEN** the response status SHALL be `403` with the body `{"error":"forbidden origin"}`

#### Scenario: Null origin is refused

- **WHEN** `GET /log` is sent with `Origin: null`
- **THEN** the response status SHALL be `403`

#### Scenario: Loopback origin is served

- **WHEN** `GET /status` is sent with `Host: 127.0.0.1:9119` and `Origin: http://localhost:3000`
- **THEN** the response status SHALL be `200`

#### Scenario: Bundled shell clients are unaffected

- **GIVEN** a running broker on the default bind
- **WHEN** `broker.sh status "x"`, `broker.sh poll`, `sweep.sh status`, `sweep.sh status-publish "x"` and the bundled post-commit hook each talk to it
- **THEN** every publish SHALL be accepted with `202` and every read SHALL return `200`

#### Scenario: In-tree Rust clients are unaffected

- **GIVEN** a running broker on the default bind
- **WHEN** `publish_to_broker_http`, `register_watch_target_http`, `fetch_log_entries_over_http`, `fetch_log_over_http`, `fetch_status_agents_over_http`, `fetch_status_over_http` and the stale-broker probe each talk to it
- **THEN** each SHALL succeed as it does without the guard
- **AND** the probe SHALL still recognise the listener as a live git-paw broker

### Requirement: GET /log long-polls on the since cursor

The `GET /log` endpoint SHALL accept an optional `wait` query parameter (whole seconds). When `wait` is present and no log entry with a sequence number greater than `since` exists, the broker SHALL hold the request until such an entry is appended or `wait` seconds elapse, whichever comes first, and SHALL then respond with the same `{"entries": [...], "last_seq": N}` shape as an immediate request — `entries` empty when the wait elapsed with nothing new. The broker SHALL cap the wait at 60 seconds, SHALL reject a non-numeric `wait` with 400, and SHALL NOT hold any broker state lock while waiting. Whenever a `GET /log` response carries no entries — with or without `wait` — its `last_seq` SHALL equal the request's `since` (`0` when `since` is absent), so a reader's cursor never rewinds; a response with entries SHALL report the highest returned sequence number, as before. In every other respect a request without `wait` SHALL behave exactly as before. The drive loop's broker-message observer SHALL request only entries after the last sequence number it has already seen.

#### Scenario: A waiting request returns as soon as an entry is published

- **GIVEN** a broker whose last sequence number is 7
- **WHEN** a client sends `GET /log?since=7&wait=30` and another client publishes a message 2 seconds later
- **THEN** the first request SHALL complete within about 2 seconds with exactly the new entry and `last_seq = 8`

#### Scenario: A waiting request times out empty

- **GIVEN** a broker whose last sequence number is 7 and no publisher
- **WHEN** a client sends `GET /log?since=7&wait=2`
- **THEN** the response SHALL arrive after about 2 seconds with `entries` empty and `last_seq = 7`

#### Scenario: The wait is capped

- **WHEN** a client sends `GET /log?since=0&wait=600` against a broker with no entries after 0
- **THEN** the response SHALL arrive within about 60 seconds

#### Scenario: A non-numeric wait is rejected

- **WHEN** a client sends `GET /log?wait=soon`
- **THEN** the response status SHALL be 400

#### Scenario: A request without wait returns at once

- **GIVEN** a broker with three published messages
- **WHEN** a client sends `GET /log?since=2`
- **THEN** the response SHALL return immediately with the entry at sequence 3 and `last_seq = 3`

#### Scenario: An empty response without wait holds the cursor

- **GIVEN** a broker whose last sequence number is 7
- **WHEN** a client sends `GET /log?since=7` with no `wait`
- **THEN** the response SHALL return immediately with `entries` empty and `last_seq = 7`

### Requirement: The log response identifies the broker instance

Sequence numbers restart at zero every time a broker starts, so a cursor from a previous broker is meaningless against the current one — and because a cursor never rewinds, a reader holding a stale high cursor would silently skip every entry the new broker has appended and wait out every long poll. To make that detectable, every `GET /log` response SHALL carry an `epoch` field identifying the running broker instance (its start time in Unix milliseconds), and `wait_for_events` and `sweep.sh events` SHALL pass it through to their callers.

A cursor reader SHALL remember the epoch it last saw and, whenever the epoch it receives differs from that one, SHALL reset its cursor to `0` before its next request, so it re-reads the new broker's log from the beginning exactly once. The first epoch a reader sees SHALL be recorded without resetting, since there is no earlier cursor to invalidate.

#### Scenario: A restarted broker reports a new epoch

- **GIVEN** a reader that has consumed up to sequence 12 from a broker
- **WHEN** that broker is stopped, a new broker starts for the same session, and the reader polls again
- **THEN** the response's `epoch` SHALL differ from the one the reader recorded

#### Scenario: A reader resets its cursor when the epoch changes

- **GIVEN** a drive-loop observer holding cursor 12 and the previous broker's epoch
- **WHEN** it polls a broker reporting a different epoch and three entries at sequences 1 to 3
- **THEN** it SHALL observe all three entries rather than skipping them as older than its cursor

#### Scenario: An unchanged epoch leaves the cursor alone

- **GIVEN** a reader holding cursor 12 and the current broker's epoch
- **WHEN** it polls the same broker again
- **THEN** its request SHALL still carry `since=12` and it SHALL NOT re-read the log from the beginning

#### Scenario: The drive loop reads only new entries

- **GIVEN** a running drive loop whose observer has already consumed entries up to sequence 12
- **WHEN** it polls the broker for new messages
- **THEN** its request SHALL carry `since=12`

### Requirement: GET /status reports wave staleness

The broker SHALL record the time the wave last made progress and SHALL include in every `GET /status` response a `wave` object with `last_progress_unix_secs`, `idle_seconds`, `stall_threshold_seconds`, and `stalled` (true when the threshold is non-zero and `idle_seconds` exceeds it). Progress SHALL be a wave phase advancing or a branch tip moving, and nothing else. A wave phase advances when the broker accepts an `agent.status` whose status or phase differs from that agent's previously recorded values, an `agent.artifact`, an `agent.verified`, an `agent.feedback`, or an `agent.advanced-main`. The bundled stuck-detection helper's synthetic phases are the exception: a status whose `phase` begins `stuck` (`stuck-on-prompt`, `stuck-stream-timeout`) or is `context-bloat`, `no-progress` or `blocked-on-supervisor` is a report *about* a wave that is not moving, not a wave that is. Treating any of them as a phase advance would let the very detection of a stall reset the stall clock — plainest for `no-progress`, which says in its own name that nothing is progressing. A status that moves an agent *out* of one of those phases is an ordinary phase advance and does count. A branch tip moves when the watcher observes that an agent's worktree `HEAD` commit differs from the one it last observed. Every other accepted message or observation SHALL NOT be progress — in particular an `agent.status` that repeats the agent's recorded status and phase (whatever its modified-file set), an `agent.question` (including the drive loop's escalation of a live permission prompt and its re-escalation of a prompt it already escalated), an `agent.learning` (including the learnings the drive loop records for its own escalations), and a change to an agent's uncommitted modified-file set. A broker that has accepted no message SHALL report its start time as the last progress. Every other field of the `/status` response SHALL be unchanged.

#### Scenario: Repeated identical heartbeats are not progress

- **GIVEN** an agent whose last recorded status is `working` with no modified files, and a stall threshold of 60 seconds
- **WHEN** the agent re-publishes the identical `working` status every 10 seconds for 2 minutes and nothing else happens
- **THEN** `/status` SHALL report `stalled = true` and `idle_seconds` of at least 120

#### Scenario: A real event resets staleness

- **GIVEN** a wave reported `stalled = true`
- **WHEN** an agent publishes an `agent.artifact`
- **THEN** the next `/status` SHALL report `stalled = false` and an `idle_seconds` near 0

#### Scenario: A moved branch tip is progress

- **GIVEN** an agent whose worktree `HEAD` the watcher last observed at commit `A`
- **WHEN** the agent commits and the watcher observes `HEAD` at commit `B`
- **THEN** `last_progress_unix_secs` SHALL advance

#### Scenario: Uncommitted file churn is not progress

- **GIVEN** an agent whose recorded status is `working` and a stall threshold of 60 seconds
- **WHEN** the watcher observes the agent's modified-file set change several times over 2 minutes, with no commit and no status or phase change
- **THEN** `/status` SHALL report `stalled = true`

#### Scenario: The drive loop's own escalations and learnings are not progress

- **GIVEN** an `--unattended` session whose only agent sits on an unanswered live permission prompt, and a stall threshold of 60 seconds
- **WHEN** over 2 minutes the drive loop publishes the prompt's escalation as an `agent.question`, re-escalates it after its dedup window, and records `agent.learning` entries for it, while no agent's phase advances and no branch tip moves
- **THEN** `/status` SHALL report `stalled = true` and an `idle_seconds` of at least 120

#### Scenario: A stuck-detection alert is not progress

- **GIVEN** an agent recorded as `working` with no phase, and a stall threshold of 60 seconds
- **WHEN** over 2 minutes the stuck-detection helper publishes synthetic `agent.status` messages for it carrying each of its alert phases — `stuck-on-prompt`, `stuck-stream-timeout`, `context-bloat`, `no-progress` and `blocked-on-supervisor` — and nothing else happens
- **THEN** `/status` SHALL report `stalled = true` and an `idle_seconds` of at least 120

#### Scenario: An ordinary phase is still progress

- **GIVEN** an agent recorded as `working` with no phase
- **WHEN** it publishes an `agent.status` with a lifecycle phase such as `sweep` or `merge`
- **THEN** that SHALL count as a wave phase advancing

#### Scenario: Recovering from an alert phase is progress

- **GIVEN** an agent whose recorded phase is `stuck-on-prompt`
- **WHEN** it publishes an `agent.status` with no phase
- **THEN** that SHALL count as a wave phase advancing

#### Scenario: A zero threshold disables the flag

- **GIVEN** `[broker] wave_stall_minutes = 0`
- **WHEN** the wave has been idle for an hour
- **THEN** `/status` SHALL report `stalled = false` and SHALL still report `idle_seconds`

### Requirement: Wave stall threshold is configurable

The `[broker]` table SHALL accept an optional `wave_stall_minutes` integer, defaulting to `30` when absent; `0` SHALL disable the `stalled` flag while keeping `idle_seconds`. A config without the key SHALL load unchanged.

#### Scenario: Absent key uses the default

- **GIVEN** a config with `[broker] enabled = true` and no `wave_stall_minutes`
- **WHEN** the broker starts
- **THEN** `/status` SHALL report `stall_threshold_seconds = 1800`

#### Scenario: Configured threshold is honoured

- **GIVEN** `[broker] wave_stall_minutes = 5`
- **WHEN** the broker starts
- **THEN** `/status` SHALL report `stall_threshold_seconds = 300`

### Requirement: Status surfaces a stalled wave

When the session's broker reports `stalled = true`, `git paw status` SHALL print a line stating that the wave has made no progress and for how long, and `git paw status --json` SHALL include the broker's `wave` object. When the wave is not stalled, the human output SHALL be unchanged.

#### Scenario: A stalled wave is visible in status

- **GIVEN** an active session whose broker reports `stalled = true` with `idle_seconds = 11520`
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL contain a line stating the wave has made no progress for 3h12m

#### Scenario: JSON status carries the wave object

- **GIVEN** an active session with a live broker
- **WHEN** `git paw status --json` is executed
- **THEN** the output SHALL contain a `wave` object with `stalled` and `idle_seconds`

#### Scenario: A progressing wave adds no line

- **GIVEN** an active session whose broker reports `stalled = false`
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL NOT contain a stalled-wave line

### Requirement: Broker host subcommand

The system SHALL provide a hidden `git paw __broker` subcommand that hosts the broker for the current repository's session. The launcher SHALL hand it the session name and the per-session broker log path (`<session_state_dir>/<session_name>.broker.log`) as command-line arguments — neither is secret — because on a fresh launch the session receipt is saved only after the panes exist. It SHALL load `BrokerConfig` — falling back to the defaults with a warning when the repository config cannot be loaded, as a resume does, so a config broken since launch cannot keep a resumed session's broker down — taking the port and bind address from the saved session of that name when one records them (its agents were handed that URL, which can differ from the current config); construct the `BrokerState` with that log path (one log per session), the verify-on-commit nudge setting, the seeded orchestrator CLI, the opsx role-gating context, and the learnings aggregator when enabled; when a saved `Session` with that name exists for this repository (recovery, restart from pause; a same-named receipt of another repository SHALL be ignored), seed the watch targets and the agent → spec mapping from it, while on a fresh launch the agents arrive through `POST /watch`; start the broker — with the stale-broker probe that reattaches only to this repository's own broker — using those watch targets and the conflict-detector configuration; and block until shutdown. It SHALL exit — dropping `BrokerHandle` so the final broker-log flush and the learnings flush run — on `SIGHUP`, when reparented to init, or when a controlling terminal it was given hangs up. It SHALL ignore `SIGINT`. It SHALL NOT render a TUI, SHALL NOT enter the alternate screen, and SHALL NOT require `$TMUX`. A bind or probe failure SHALL exit non-zero with a diagnostic rather than retry, and so SHALL finding this repository's broker already served by another process: the host SHALL NOT idle beside a broker it cannot flush or stop. It SHALL be hidden from `git paw --help`.

#### Scenario: Host serves the broker

- **GIVEN** a saved session with the broker enabled on a free port
- **WHEN** `git paw __broker` is started in the repository with that session's name and log path
- **THEN** `GET <url>/status` SHALL answer with the `git_paw` marker and this repository's `repo_root` within 5 seconds
- **AND** the process SHALL still be running

#### Scenario: Host of a fresh launch writes the per-session log

- **GIVEN** a fresh launch whose session receipt has not been saved yet
- **WHEN** the host, handed session name `paw-demo` and its log path, accepts a published message
- **THEN** the message SHALL be flushed to `<session_state_dir>/paw-demo.broker.log`
- **AND** nothing SHALL be written to the `git-paw.broker.log` fallback

#### Scenario: SIGHUP shuts the host down with a final flush

- **GIVEN** a running `__broker` that has accepted at least one message
- **WHEN** `SIGHUP` is delivered to it
- **THEN** the port SHALL be free within 5 seconds
- **AND** the session's broker log SHALL contain every accepted message

#### Scenario: Host renders nothing

- **WHEN** `git paw __broker` runs for 2 seconds attached to a pseudo-terminal
- **THEN** it SHALL write no alternate-screen or cursor-addressing escape sequences to that terminal

#### Scenario: Bind failure exits instead of looping

- **GIVEN** a foreign HTTP server bound to the configured port
- **WHEN** `git paw __broker` starts
- **THEN** it SHALL exit non-zero with a diagnostic naming the port
- **AND** SHALL NOT remain running

#### Scenario: Host runs outside tmux

- **GIVEN** a shell with `$TMUX` unset
- **WHEN** `git paw __broker` is started for a session
- **THEN** it SHALL start and serve `/status`

### Requirement: Dashboard subcommand runs a thin broker client

The hidden `git paw __dashboard` subcommand SHALL hold no `BrokerState`, SHALL start no broker, watcher, conflict detector or learnings aggregator, and SHALL NOT require `$TMUX`. It SHALL resolve the broker URL from the saved session's `broker_port` and `broker_bind` whatever the session's recorded status — unlike the MCP server, which resolves a URL only for an `Active` session, because restart from pause recreates the dashboard while the session is still recorded `Paused` — and, while no session is saved yet (a fresh launch saves its receipt after the panes exist), SHALL retry the resolution on its poll cadence in the degraded state. A saved session that records no broker (a supervised launch with the broker disabled) SHALL render a "broker disabled" state rather than the unreachable indicator. It SHALL render from `GET /status` and `GET /log?since=<seq>`, and continue running in a degraded state when the broker is unreachable. Killing the dashboard SHALL NOT affect the broker.

#### Scenario: Dashboard holds no broker

- **GIVEN** a running session
- **WHEN** the dashboard pane is killed
- **THEN** `GET /status` SHALL continue to answer
- **AND** watcher, conflict-detector and learnings behavior SHALL be unchanged

#### Scenario: Dashboard runs outside tmux

- **GIVEN** a shell with `$TMUX` unset and a saved session with a live broker
- **WHEN** `git paw __dashboard` is started
- **THEN** it SHALL render the agent table from `/status`

#### Scenario: Dashboard with no live broker renders degraded

- **GIVEN** a saved session whose broker is not running
- **WHEN** `git paw __dashboard` is started
- **THEN** it SHALL render with a "broker unreachable" indicator rather than exiting or busy-looping
- **AND** pressing `q` SHALL exit with status 0

#### Scenario: Dashboard of a session still recorded paused connects

- **GIVEN** a session recorded `Paused` whose broker host is answering
- **WHEN** `git paw __dashboard` is started before the session is recorded `Active`
- **THEN** it SHALL render the agent table from `/status` without being restarted

#### Scenario: Dashboard started before the receipt connects once it is saved

- **GIVEN** a fresh launch whose dashboard pane starts before the session receipt is saved
- **WHEN** the launcher saves the receipt
- **THEN** the dashboard SHALL resolve the broker URL on a later poll and render live data

### Requirement: Broker host lifecycle across start, recovery, pause, stop

When the broker is enabled, the `start` flow (bare, spec-driven and supervised) SHALL launch the broker host in a detached service surface of the session provided by the terminal backend — on tmux, a window named `broker` that is not part of window 0's pane grid — as soon as the session exists: the backend opens the session with `GIT_PAW_BROKER_URL` in its environment and the dashboard pane at pane 0 and the coding-agent panes in one step, and a service surface can only be added to a session that exists. The launcher SHALL hand the host the session name and the per-session log path, and SHALL wait, bounded by 10 seconds, for `GET /status` to answer with the `git_paw` marker and this repository's `repo_root` before it registers any agent, saves the receipt, or injects any boot prompt. The dashboard pane resolves the broker URL on its own poll cadence and renders degraded until the host answers. Once the host answers, and before any boot prompt is injected, the launcher SHALL register every coding agent with the host through `POST /watch`, carrying the agent's spec id when it was launched from a spec — the path `git paw add` uses. The launcher SHALL save the session receipt only after the panes exist and the host has answered, so a launch that fails — including a host that never answers — leaves the persisted session state unchanged (*A failed start preserves persisted session state*); a fresh launch whose host fails SHALL close the session it opened, since no receipt names it, and a recovery whose host fails SHALL close the rebuilt session and restore the receipt it had saved, so the next `git paw start` recovers again. Before starting a host the launcher SHALL wait, bounded, for a broker of this repository that still answers (a host stopped a moment ago) to release the port, and SHALL fail naming the port if it does not. Recovery of a stopped session SHALL recreate the broker host the same way; the host seeds its agents from the saved receipt. `stop` SHALL kill the session, which terminates the host's surface and triggers the graceful flush. `pause` SHALL stop the host by closing its service surface and SHALL leave the dashboard pane running. When a session started before this change (no `broker` surface) is paused, `pause` SHALL fall back to killing the dashboard pane at the recorded `dashboard_pane`. Window 0's pane indices SHALL be unaffected by the host in every flow.

#### Scenario: Start launches the host before the first boot prompt

- **GIVEN** `[broker] enabled = true`
- **WHEN** `git paw start` launches a session
- **THEN** a tmux window named `broker` SHALL exist in the session running `git paw __broker`
- **AND** `GET /status` SHALL have answered with the `git_paw` marker before the first injection into a coding-agent pane

#### Scenario: A fresh launch registers its agents with the host

- **GIVEN** `[broker] enabled = true`, learnings enabled, and a spec-driven launch of agent `feat-x` from spec `003-user-list`
- **WHEN** `git paw start --from-specs` launches the session
- **THEN** the host SHALL have accepted a `POST /watch` for `feat-x` carrying spec id `003-user-list` before `feat-x`'s boot prompt was injected
- **AND** `/status` SHALL list `feat-x` once its worktree shows activity, on the same terms as an agent seeded from a saved session

#### Scenario: A launch that fails after the host starts saves no receipt

- **GIVEN** a repository with no saved session
- **WHEN** a fresh `git paw start` starts its broker host and the host never answers `/status`
- **THEN** no session receipt SHALL have been saved for the repository
- **AND** the terminal session the launch opened SHALL be closed

#### Scenario: Pane indices are unaffected by the host

- **GIVEN** a supervised launch with 3 agents
- **WHEN** the session is built
- **THEN** window 0 SHALL contain exactly 4 panes (dashboard + 3 agents) with the dashboard at index 0

#### Scenario: Host that never answers fails the launch clearly

- **GIVEN** a broker host that cannot start (port held by a foreign process)
- **WHEN** `git paw start` waits for `/status`
- **THEN** the launch SHALL fail within the bound with an error naming the broker port
- **AND** the persisted session state SHALL be preserved for recovery

#### Scenario: Recovery recreates the host

- **GIVEN** a stopped session with `broker_port = Some(P)`
- **WHEN** `git paw start` recovers it
- **THEN** a `broker` window running `git paw __broker` SHALL exist
- **AND** `GET http://127.0.0.1:P/status` SHALL answer
- **AND** the host SHALL watch every agent's worktree of the saved session, so each appears in `/status` once its worktree shows activity

#### Scenario: Pause stops the host and keeps the dashboard

- **GIVEN** an active session with a `broker` window and a dashboard at pane 0
- **WHEN** `git paw pause` runs
- **THEN** the `broker` window SHALL be gone and the broker port free within 5 seconds
- **AND** pane 0 SHALL still be running `git paw __dashboard`

#### Scenario: Pause of a session started before the split falls back

- **GIVEN** an active session with no `broker` window whose dashboard pane hosts the broker
- **WHEN** `git paw pause` runs
- **THEN** it SHALL kill the dashboard pane at the recorded `dashboard_pane` so the broker stops as before

### Requirement: Launcher mints a per-session authority credential

Every `start` path that launches a fresh session with the broker enabled (bare, spec-driven, supervised) SHALL generate a fresh authority credential of at least 32 bytes of OS-provided randomness, hex-encoded; SHALL hand it to the broker host (`git paw __broker`) through a one-shot owner-only (`0600`) hand-off file in the user-data sessions directory, written before the host starts and removed by the host once read (and by the launcher if the host fails to start); and SHALL persist that same credential in the user-data `Session` when it saves the session receipt after the panes exist — never earlier, so a launch that fails leaves the persisted session state unchanged (session-state *A failed start preserves persisted session state*). A path that restarts the host for an existing session (recovery, restart from pause) SHALL hand the host the credential its saved `Session` carries, if any, and any later save of that `Session` in the same flow SHALL keep the credential the host persisted rather than overwrite it. The credential SHALL NOT be passed to any process as a command-line argument or environment variable, and SHALL NOT be typed into or printed to any terminal surface.

#### Scenario: Supervised start records a credential

- **GIVEN** `[broker] enabled = true`
- **WHEN** `git paw start --supervisor` saves session state
- **THEN** the saved `Session` SHALL carry a non-empty `broker_authority`
- **AND** its value SHALL be at least 64 hexadecimal characters

#### Scenario: Bare start with the broker also records a credential

- **GIVEN** `[broker] enabled = true` and no supervisor mode
- **WHEN** `git paw start` saves session state
- **THEN** the saved `Session` SHALL carry a non-empty `broker_authority`

#### Scenario: The host and the receipt hold the same credential

- **GIVEN** a fresh `git paw start` with the broker enabled has completed
- **WHEN** a client POSTs `agent.verified` with `X-Paw-Authority` equal to the saved `Session`'s `broker_authority`
- **THEN** the response SHALL be 202

#### Scenario: A failed start persists no credential

- **GIVEN** a repository with no saved session
- **WHEN** a fresh `git paw start` starts its broker host and then errors before its panes exist
- **THEN** no session receipt, and so no credential, SHALL have been saved for the repository

#### Scenario: Credential never appears on a command line or in the environment

- **WHEN** the launch flow's emitted process invocations — the broker host's included — and session-environment settings are inspected
- **THEN** none SHALL contain the credential value

### Requirement: Broker host loads and enforces the credential

The broker host process (`git paw __broker`) SHALL take the authority credential from the launcher's hand-off file when there is one (consuming it: the file is removed once read), and otherwise SHALL read `broker_authority` from the saved `Session` of the session it was named; it SHALL hand the credential to the `/publish` handler for enforcement. When it is handed no credential and the loaded `Session` carries none (a session saved before this requirement, being recovered or restarted from pause), the host SHALL mint one, persist it to the `Session` before it serves its first request, and enforce it; it SHALL NOT back-fill in any other case. A broker started with no credential at all SHALL accept no verdict message (fail closed). The host SHALL NOT log the credential. The in-process drive loop SHALL attach the credential to every verdict message it publishes.

#### Scenario: Host enforces the launcher's credential

- **GIVEN** a launcher that started the broker host with credential `C` in its hand-off file
- **WHEN** a client POSTs `agent.verified` with `X-Paw-Authority: C`
- **THEN** the response SHALL be 202
- **AND** the hand-off file SHALL no longer exist

#### Scenario: A session without a credential gains one

- **GIVEN** a `Session` file saved without `broker_authority`
- **WHEN** recovery starts the broker host for that session
- **THEN** the `Session` file SHALL be re-saved with a non-empty `broker_authority`
- **AND** `agent.verified` without that credential SHALL be refused with 403

#### Scenario: Restart from pause keeps a back-filled credential

- **GIVEN** a paused session whose `Session` file has no `broker_authority`
- **WHEN** `git paw start` restarts it from pause and the restart completes
- **THEN** the saved `Session` SHALL carry the credential the host minted
- **AND** `agent.verified` with that credential SHALL be accepted with 202

#### Scenario: The drive loop's branch-refresh notification is accepted

- **GIVEN** an `--unattended` session with branch refresh enabled
- **WHEN** the drive loop publishes its `agent.feedback` refresh notification from `supervisor`
- **THEN** the broker SHALL accept it with 202

### Requirement: Credential is confined to root-side surfaces

The authority credential SHALL be readable only from the user-data `Session` file and the hidden `git paw __authority` subcommand. It SHALL NOT appear in the per-repo discovery JSON (`.git-paw/sessions/<name>.json`), in `GET /status` or `GET /log` responses, in `git paw status --json`, in the MCP `get_session_status` response, in the session's broker log, or in any diagnostic output.

#### Scenario: Discovery file carries no credential

- **GIVEN** a running session with a credential
- **WHEN** `.git-paw/sessions/<name>.json` is inspected
- **THEN** it SHALL NOT contain the credential value or a `broker_authority` key

#### Scenario: Status surfaces carry no credential

- **GIVEN** a running session with credential `C`
- **WHEN** `GET /status`, `git paw status --json`, and MCP `get_session_status` are each invoked
- **THEN** none of the responses SHALL contain `C`

#### Scenario: Broker log carries no credential

- **GIVEN** an accepted `agent.verified` published with the credential
- **WHEN** the broker log is flushed and inspected
- **THEN** it SHALL contain the message and SHALL NOT contain the credential value

