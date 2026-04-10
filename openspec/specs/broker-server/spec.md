# broker-server Specification

## Purpose
TBD - created by archiving change http-broker. Update Purpose after archive.
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

The system SHALL provide a synchronous function with the signature `pub fn start_broker(config: BrokerConfig, state: BrokerState) -> Result<BrokerHandle, BrokerError>`. This function SHALL:

1. Construct a multi-threaded tokio runtime owned by the returned handle
2. Spawn the axum server on that runtime, bound to `config.bind:config.port`
3. Return a `BrokerHandle` value that, when dropped, shuts the runtime down

The function MUST be callable from synchronous Rust code without any surrounding `#[tokio::main]` or other runtime context. The function MUST NOT panic on any expected failure (port in use, invalid bind address, runtime construction failure); it SHALL return a `BrokerError` variant instead.

#### Scenario: start_broker succeeds with default config on a free port

- **GIVEN** a `BrokerConfig` with `enabled = true`, `bind = "127.0.0.1"`, and a port known to be free
- **WHEN** `start_broker(config, state)` is called from synchronous test code
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

The system SHALL define `BrokerState` as a cheaply-cloneable handle around shared inner state protected by `std::sync::RwLock`. The type SHALL satisfy:

- `BrokerState: Clone + Send + Sync + 'static`
- Cloning the value SHALL be O(1) and SHALL share the same underlying state
- All public methods on `BrokerState` SHALL be callable from both async and synchronous code without requiring a tokio context
- Holding a read or write guard across an `.await` point SHALL be statically discouraged (clippy lint `clippy::await_holding_lock` enabled in `Cargo.toml`)

#### Scenario: BrokerState clones share underlying state

- **GIVEN** a `BrokerState` value `s1` with one registered agent record
- **WHEN** `s1.clone()` is called producing `s2`
- **THEN** queries against `s2` see the same agent record
- **AND** mutations through `s2` are observable from `s1`

#### Scenario: BrokerState is accessible from synchronous code

- **WHEN** a synchronous function reads agent status via `BrokerState`
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

