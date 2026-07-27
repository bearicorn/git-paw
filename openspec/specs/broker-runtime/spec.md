# broker-runtime Specification

## Purpose

The broker runtime is git-paw's in-process coordination server. This capability defines its configuration and lifecycle (the optional `[broker]` config section, the synchronous `start_broker` entry point that owns a tokio runtime and spawns the axum server plus per-worktree watcher tasks, stale-broker detection, `SO_REUSEADDR` restart, `Arc`-shared `BrokerState`, SIGINT and panic isolation, and the stable `delivery.rs` contract — with tokio confined to `src/broker/` so every other module stays synchronous); its complete HTTP surface (`POST /publish` message validation, `GET /messages/:agent_id` cursor-based inbox polling, `GET /status`, the `GET /log` replay seam, and `POST /watch` live watch-target registration); and its integration into the session lifecycle (the hidden `__dashboard` subcommand, the start/stop/pause/resume/purge flows, `GIT_PAW_BROKER_URL` injection into the tmux session, and how `git paw status` surfaces broker and paused-session state).

## Requirements

### Requirement: Broker configuration schema

The system SHALL extend `.git-paw/config.toml` with a new optional `[broker]` section containing exactly three fields:

- `enabled: bool` — defaults to `false` when the field or the entire section is absent
- `port: u16` — defaults to `9119` when absent
- `bind: String` — defaults to `"127.0.0.1"` when absent

Loading a `.git-paw/config.toml` that omits the `[broker]` section SHALL succeed and produce a `BrokerConfig` with the documented defaults. The system MUST NOT change behavior of existing v0.2.0 sessions when this field is absent.

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
- **HTTP 200 with a response body containing the marker field `"git_paw": true`** → return a `BrokerHandle` that reattaches to the existing broker without binding
- **HTTP response without the marker field** → return `Err(BrokerError::PortInUse { .. })` indicating the port is occupied by a foreign process
- **Timeout** → return `Err(BrokerError::ProbeTimeout { .. })` so the user can investigate

#### Scenario: Probe finds no listener and proceeds to bind

- **GIVEN** no process is listening on `127.0.0.1:9119`
- **WHEN** `start_broker` is called with `port = 9119`
- **THEN** the probe returns connection-refused
- **AND** `start_broker` proceeds to bind and returns `Ok(BrokerHandle)`

#### Scenario: Probe finds a live git-paw broker and reattaches

- **GIVEN** a live git-paw broker is already running on `127.0.0.1:9119`
- **WHEN** `start_broker` is called with the same config
- **THEN** the probe receives an HTTP 200 with `"git_paw": true` in the body
- **AND** `start_broker` returns `Ok(BrokerHandle)` without binding a new socket

#### Scenario: Probe finds a foreign HTTP server and refuses

- **GIVEN** a non-git-paw HTTP server is bound to `127.0.0.1:9119` and responds to `GET /status` with `404`
- **WHEN** `start_broker` is called with `port = 9119`
- **THEN** the probe receives a response without the `"git_paw"` marker
- **AND** `start_broker` returns `Err(BrokerError::PortInUse { .. })` mentioning the port

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

The broker process SHALL install a `SIGINT` handler that prevents accidental Ctrl+C in pane 0 from terminating the broker or the dashboard process. The handler SHALL NOT call `std::process::exit` and SHALL NOT trigger broker shutdown. Clean shutdown of the broker SHALL only occur via:

- The `BrokerHandle` being dropped
- An explicit dashboard quit keybind
- `SIGTERM` or `SIGKILL` from outside the process (e.g. `git paw stop` killing tmux)

#### Scenario: SIGINT does not terminate the broker

- **GIVEN** a running broker
- **WHEN** `SIGINT` is delivered to the broker process
- **THEN** the broker continues to respond to HTTP requests
- **AND** the process does not exit

#### Scenario: Dropping BrokerHandle still shuts down cleanly

- **GIVEN** a running broker
- **WHEN** `BrokerHandle` is dropped
- **THEN** the broker shuts down and the process owning the handle continues running

### Requirement: Panic isolation

A panic in any single HTTP request handler or background tokio task MUST NOT terminate the broker runtime or the dashboard process. The system SHALL rely on axum's default panic-catching for handlers and SHALL spawn any background broker tasks via `tokio::spawn` so panics remain isolated to a single task.

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

The system SHALL respond with HTTP `404 Not Found` for any request whose path does not match one of the four documented routes (`POST /publish`, `GET /messages/:agent_id`, `GET /status`, `GET /log`).

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

### Requirement: Live watch-target registration endpoint

The broker SHALL expose `POST /watch` accepting a JSON body with an
agent id, worktree path, and cli label, and SHALL add that path to its
live filesystem-watch-target set so the watcher begins surfacing the
worktree's activity without a broker restart. The endpoint SHALL be
idempotent: registering an already-watched path SHALL NOT create a
duplicate target. It SHALL bind to loopback only, consistent with the
other broker endpoints.

A hot-added agent (via `git paw add`) registered through `POST /watch`
SHALL appear in `/status` from worktree activity on the same terms as
an agent seeded at `git paw start`, independent of whether its CLI has
self-published a status.

#### Scenario: Registering a target surfaces the worktree via the watcher

- **GIVEN** a running broker and a worktree not among its start-time
  watch targets
- **WHEN** a client `POST`s the worktree (agent id + path + cli) to
  `/watch`
- **AND** that worktree subsequently has uncommitted changes
- **THEN** the watcher SHALL surface the agent in `/status` without a
  broker restart and without requiring the agent's CLI to have
  self-published

#### Scenario: Registration is idempotent

- **GIVEN** a worktree already registered as a watch target
- **WHEN** the same path is `POST`ed to `/watch` again
- **THEN** the broker SHALL NOT create a duplicate target and SHALL
  return success

#### Scenario: Endpoint binds to loopback only

- **WHEN** the broker starts with `/watch` enabled
- **THEN** the endpoint SHALL be reachable only on the loopback
  interface, consistent with `/publish`, `/status`, and `/messages`

### Requirement: Dashboard subcommand starts broker and dashboard

The system SHALL handle the hidden `__dashboard` subcommand by:

1. Loading `BrokerConfig` from `.git-paw/config.toml`
2. Constructing a `BrokerState` with `log_path` set to `<session_state_dir>/broker.log`
3. Calling `start_broker(config, state.clone())` to obtain a `BrokerHandle`
4. Calling `run_dashboard(state, handle)` which blocks until the user presses `q`
5. Returning `Ok(())` on clean exit

The subcommand SHALL refuse to run if the `$TMUX` environment variable is not set, returning an error indicating it is an internal command intended to run inside tmux.

#### Scenario: Dashboard subcommand starts broker and blocks

- **GIVEN** `$TMUX` is set and `[broker]` config is valid
- **WHEN** `git paw __dashboard` is executed
- **THEN** a broker starts listening on the configured port
- **AND** the dashboard renders in the terminal

#### Scenario: Dashboard subcommand refuses outside tmux

- **GIVEN** `$TMUX` is not set
- **WHEN** `git paw __dashboard` is executed
- **THEN** it returns an error mentioning "internal command" and "tmux"

### Requirement: Start flow conditionally creates dashboard pane

When `[broker] enabled = true` in config, the `start` flow SHALL insert a dashboard pane that runs `git paw __dashboard`. The dashboard pane's index depends on whether supervisor mode is active:

- **Bare `git paw start` and `git paw start --from-specs` (no supervisor):** dashboard at pane 0; coding agent panes start at pane 1. Same as v0.4.
- **`git paw start --supervisor` (or `--from-specs --supervisor`):** dashboard at pane 1; supervisor agent at pane 0; coding agent panes start at pane 2. Updated in this change per the new supervisor-as-pane layout.

When `[broker] enabled = false` (or absent), the start flow SHALL behave identically to v0.2.0 with no dashboard pane (and supervisor mode is not meaningful since auto-approve, dashboard, and broker-status all require the broker).

The dashboard pane is in both cases a non-interactive TUI process; it does NOT receive a `tmux send-keys` boot block injection.

#### Scenario: Broker enabled in bare-start mode adds dashboard as pane 0

- **GIVEN** `[broker]\nenabled = true` in config and no supervisor mode
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 4 panes: pane 0 running `git paw __dashboard`, panes 1-3 running coding CLIs

#### Scenario: Broker enabled in supervisor mode places dashboard at pane 1

- **GIVEN** `[broker]\nenabled = true` and `[supervisor]\nenabled = true` in config
- **WHEN** `git paw start --supervisor` launches a session with 3 branches
- **THEN** the tmux session has 5 panes
- **AND** pane 0 SHALL be the supervisor agent (Claude with the supervisor skill as AGENTS.md)
- **AND** pane 1 SHALL be the dashboard (`git paw __dashboard`)
- **AND** panes 2-4 SHALL be the coding CLIs

#### Scenario: Broker disabled produces no dashboard pane

- **GIVEN** no `[broker]` section in config (or `enabled = false`)
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 3 panes, all running coding CLIs (same as v0.2.0)

#### Scenario: Dashboard pane title

- **GIVEN** broker enabled (in either bare-start or supervisor mode)
- **WHEN** the session is created
- **THEN** the dashboard pane has the title `"dashboard"` regardless of its index

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

The `stop` flow SHALL kill the tmux session via `tmux::kill_session`. Killing the session kills every pane including the dashboard pane, which causes `run_dashboard` to exit, which drops `BrokerHandle`, which triggers graceful broker shutdown including the final log flush.

The stop flow SHALL render an interactive confirmation prompt before killing the session when stdin is a TTY AND `--force` is not set. The prompt SHALL:

- Name the destructive consequences (CLI processes killed, agent conversation context lost).
- Point at `git paw pause` as the soft-stop alternative.
- Point at `git paw purge` as the full-reset alternative.
- Default to `n` (no) — the user SHALL confirm with `y` to proceed.

When `--force` is set OR stdin is not a TTY, the prompt SHALL be skipped and the kill SHALL proceed immediately. This preserves CI / automation back-compat (non-TTY contexts behave as in v0.4) and gives scripts a `--force` opt-out for TTY contexts.

When the session's recorded status is `Paused`, the confirmation prompt SHALL additionally inform the user that the session is currently paused and that continuing will kill the still-running CLI processes.

#### Scenario: Stop kills tmux and broker shuts down

- **GIVEN** an active session with broker enabled
- **WHEN** `git paw stop --force` is executed
- **THEN** the tmux session SHALL be killed
- **AND** the broker port SHALL be freed within 5 seconds
- **AND** `broker.log` SHALL contain a final flush of all messages

#### Scenario: Stop from TTY without --force prompts before killing

- **GIVEN** an active session and stdin attached to a TTY
- **WHEN** `git paw stop` is executed (no `--force`)
- **THEN** a confirmation prompt SHALL appear
- **AND** the prompt SHALL mention `git paw pause` as the soft alternative
- **AND** the prompt SHALL default to `no`

#### Scenario: Stop from non-TTY without --force does not prompt

- **GIVEN** an active session and stdin not attached to a TTY (e.g. CI)
- **WHEN** `git paw stop` is executed (no `--force`)
- **THEN** no interactive prompt SHALL be rendered
- **AND** the stop SHALL proceed immediately (v0.4 back-compat)

#### Scenario: Stop after pause kills remaining CLI panes

- **GIVEN** a session with `status == Paused` and tmux still alive
- **WHEN** `git paw stop --force` is executed
- **THEN** the tmux session SHALL be killed
- **AND** every previously-still-running CLI process SHALL be terminated
- **AND** the session state SHALL be `status == Stopped`

#### Scenario: Stop after pause from TTY prompt mentions paused state

- **GIVEN** a session with `status == Paused` and stdin attached to a TTY
- **WHEN** `git paw stop` is executed (no `--force`)
- **THEN** the confirmation prompt SHALL inform the user the session is currently paused
- **AND** the prompt SHALL state that continuing will kill the still-running CLIs

### Requirement: Purge flow cleans up broker log

The `purge` flow SHALL delete `broker.log` from the session state directory if the session state contains a `broker_log_path` field. Deletion SHALL be best-effort — missing or already-deleted log files SHALL NOT cause an error.

#### Scenario: Purge deletes broker.log

- **GIVEN** an active session with broker enabled and a `broker.log` file
- **WHEN** `git paw purge --force` is executed
- **THEN** the `broker.log` file is deleted
- **AND** the session state file is deleted
- **AND** worktrees are removed

#### Scenario: Purge succeeds when broker.log does not exist

- **GIVEN** a session state with `broker_log_path` pointing to a nonexistent file
- **WHEN** `git paw purge --force` is executed
- **THEN** purge completes successfully

### Requirement: Status shows broker information

When a session is active and has broker fields in its state, `git paw status` SHALL display broker information including the configured URL. The system SHALL attempt to probe `GET /status` against the broker URL:

- If the probe succeeds: display the broker URL, agent count, and uptime from the response.
- If the probe fails AND the session's effective status is `Paused`: display the broker URL with `(paused — run 'git paw start' to resume)`.
- If the probe fails AND the session's effective status is `Active` or `Stopped`: display the broker URL with `(not responding)`.

`git paw status` SHALL render the three session statuses with distinguishable visual treatment (e.g. different emoji / labels for `active`, `paused`, `stopped`). The paused row SHALL include a one-line restart hint pointing at `git paw start`.

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

- **GIVEN** a session state with `status == Active` and broker fields but the dashboard pane has crashed
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL contain the broker URL and `not responding`

#### Scenario: Status shows no broker info when disabled

- **GIVEN** a session without broker fields in state
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL NOT mention broker, port, or agents

### Requirement: Auto-approve thread location in dashboard subprocess

When supervisor mode is active AND `[supervisor.auto_approve] enabled = true`, the auto-approve poll thread SHALL run inside the dashboard's `__dashboard` subprocess (the long-lived process running in the dashboard pane), NOT inside the `cmd_supervisor` process (which returns immediately after launching the session per the new supervisor-as-pane architecture).

The auto-approve thread's responsibilities are unchanged: poll `/status` every `stall_threshold_seconds`, capture stalled panes, classify pending commands, dispatch approve keystrokes for safe commands, escalate unknowns via `agent.question`. Only the host process changes.

#### Scenario: Auto-approve thread runs inside the dashboard subprocess

- **GIVEN** an active supervisor mode session with auto-approve enabled
- **WHEN** the dashboard's `__dashboard` subprocess starts
- **THEN** it spawns the auto-approve poll thread alongside the broker + TUI rendering
- **AND** `cmd_supervisor` SHALL NOT spawn a parallel auto-approve thread

#### Scenario: Auto-approve thread terminates when dashboard pane is killed

- **GIVEN** an active supervisor mode session with auto-approve running
- **WHEN** the user kills the dashboard pane (via `tmux kill-pane` or pane exit)
- **THEN** the `__dashboard` subprocess exits
- **AND** the auto-approve thread terminates with it
- **AND** the broker shuts down (per the existing "Stop flow" requirement)

### Requirement: Pause flow detaches client and stops broker without killing tmux

`git paw pause` SHALL perform a soft-stop that:

1. Detaches every client currently attached to the session by running `tmux detach-client -s <session-name>`. With no clients attached, the command SHALL be a no-op and SHALL NOT error.
2. Stops the broker by killing the dashboard pane only (`tmux kill-pane -t <session-name>:0.<dashboard-pane-index>`). The dashboard subprocess receives SIGHUP, the `BrokerHandle` drop runs, the broker shuts down gracefully, and `broker.log` flushes.
3. Updates the on-disk session state's `status` field from `Active` to `Paused` (see the session-state delta in this change).
4. Leaves the tmux session and every coding-agent CLI pane running.
5. Prints a one-line confirmation: `"Session '<name>' paused. <N> CLI pane(s) still running. Run 'git paw start' to resume."`

The dashboard pane index SHALL be read from the saved session's `dashboard_pane` field (see the session-state delta). For sessions saved by v0.4.0 (where the field is absent and defaults to `None`), the index SHALL default to `0` (the bare-start dashboard location).

The pause flow SHALL NOT call `tmux::kill_session` at any point.

#### Scenario: Pause detaches the client

- **GIVEN** an active session with a tmux client attached
- **WHEN** `git paw pause` is executed
- **THEN** `tmux list-clients -t <session>` SHALL return no clients
- **AND** the tmux session SHALL still be alive (`tmux has-session -t <session>` exits 0)

#### Scenario: Pause stops the broker

- **GIVEN** an active session with broker enabled and listening on port P
- **WHEN** `git paw pause` is executed
- **THEN** within 5 seconds, port P SHALL be free (no listener)
- **AND** the broker's `broker.log` SHALL contain a final flush of all messages

#### Scenario: Pause leaves coding-agent panes alive

- **GIVEN** an active session with 3 coding-agent panes
- **WHEN** `git paw pause` is executed
- **THEN** the tmux session SHALL still report 3 panes (dashboard pane removed)
- **AND** each coding-agent CLI process SHALL still be running (PID alive)

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

1. Recreate the dashboard pane at the saved `dashboard_pane` index (or `0` if absent — v0.4 fallback) by running `tmux split-window` / `tmux new-window` / equivalent layout-restore tmux invocation appropriate to the original pane arrangement.
2. Send the `git paw __dashboard` command into the new dashboard pane via `tmux send-keys`.
3. Update the session state's `status` field from `Paused` to `Active`.
4. Attach to the tmux session via `tmux attach -t <session-name>`.

The restart-from-pause flow SHALL NOT create worktrees, SHALL NOT spawn coding-agent CLI processes, and SHALL NOT inject boot prompts. Coding-agent panes are already running and retain their in-memory conversation state.

#### Scenario: Start against paused session reattaches and restarts broker

- **GIVEN** a session with `status == Paused` and tmux alive
- **WHEN** `git paw start` is executed
- **THEN** the broker SHALL be listening on its configured port within 5 seconds
- **AND** the user's tmux client SHALL be attached to the session
- **AND** the session state SHALL be `status == Active`

#### Scenario: Start against paused session does not respawn CLIs

- **GIVEN** a paused session whose coding-agent CLI processes have PIDs P1..Pn
- **WHEN** `git paw start` is executed
- **THEN** the coding-agent panes SHALL still hold processes with PIDs P1..Pn
- **AND** no `tmux send-keys` SHALL be issued to the coding-agent panes during the restart

#### Scenario: Start against paused-but-tmux-dead falls through to recover

- **GIVEN** a session with recorded `status == Paused` but `tmux has-session` exits non-zero
- **WHEN** `git paw start` is executed
- **THEN** `effective_status` SHALL evaluate to `Stopped`
- **AND** the start flow SHALL run the existing cold-recovery path (fresh CLI spawn), NOT the restart-from-pause path
