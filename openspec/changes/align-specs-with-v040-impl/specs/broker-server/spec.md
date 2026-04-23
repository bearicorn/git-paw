## MODIFIED Requirements

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
