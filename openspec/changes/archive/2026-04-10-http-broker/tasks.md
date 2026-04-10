## 1. Dependencies

- [ ] 1.1 Add `tokio` to `[dependencies]` in `Cargo.toml` with features `["rt-multi-thread", "macros", "net", "sync", "time"]` and no default features
- [ ] 1.2 Add `axum` to `[dependencies]` in `Cargo.toml` with default features
- [ ] 1.3 Run `cargo build` and confirm both crates resolve and compile
- [ ] 1.4 Update `deny.toml` if `cargo deny check` flags any new transitive licenses; allow only OSI-approved permissive licenses (MIT, Apache-2.0, BSD variants, ISC, Unicode-DFS, Zlib)
- [ ] 1.5 Enable clippy lint `clippy::await_holding_lock` in `Cargo.toml` `[lints.clippy]` section to catch sync-lock-across-await bugs

## 2. BrokerConfig

- [ ] 2.1 Add `BrokerConfig` struct to `src/config.rs` with fields `enabled: bool`, `port: u16`, `bind: String`
- [ ] 2.2 Implement `Default for BrokerConfig` returning `{ enabled: false, port: 9119, bind: "127.0.0.1".to_string() }`
- [ ] 2.3 Add `broker: BrokerConfig` field to `PawConfig` with `#[serde(default)]` so missing `[broker]` sections fall back to defaults
- [ ] 2.4 Implement `BrokerConfig::url(&self) -> String` returning `format!("http://{}:{}", self.bind, self.port)`
- [ ] 2.5 Add unit tests in `src/config.rs` covering: empty config, full broker section, partial broker section, URL helper
- [ ] 2.6 Verify existing v0.2.0 config tests still pass

## 3. broker module skeleton

- [ ] 3.1 Extend `src/broker/mod.rs` (created by `message-types`) to declare `pub mod server;`, `pub mod delivery;`, and re-export the public API
- [ ] 3.2 Define `pub struct BrokerState { inner: Arc<RwLock<BrokerStateInner>> }` and a private `BrokerStateInner` with fields `agents: HashMap<String, AgentRecord>`, `queues: HashMap<String, VecDeque<BrokerMessage>>`, `started_at: Instant`
- [ ] 3.3 Implement `BrokerState::new() -> Self`, `BrokerState::clone()` (auto via `derive(Clone)` since `Arc` is clone), and accessor methods that take a closure receiving `&BrokerStateInner` or `&mut BrokerStateInner` so guards drop before any `.await`
- [ ] 3.4 Add module-level doc comment warning that `RwLock` guards MUST NOT be held across `.await` boundaries; reference the clippy lint
- [ ] 3.5 Define `pub struct AgentRecord { agent_id: String, last_seen: Instant, last_message: Option<BrokerMessage> }` (or similar — minimal viable shape; `peer-messaging` may extend)
- [ ] 3.6 Define `pub struct AgentStatusEntry` matching the JSON shape returned by `/status`
- [ ] 3.7 Define `BrokerError` enum via `thiserror` with variants `PortInUse { port: u16, source: std::io::Error }`, `ProbeTimeout { port: u16 }`, `BindFailed(std::io::Error)`, `RuntimeFailed(std::io::Error)`
- [ ] 3.8 Wire `BrokerError` into `PawError` (`error.rs`) as a wrapped variant

## 4. Delivery stubs

- [ ] 4.1 Create `src/broker/delivery.rs` with module-level doc comment explaining this is a stub owned for body-fill by `peer-messaging` in Wave 2
- [ ] 4.2 Implement `pub(crate) fn publish_message(state: &BrokerState, msg: BrokerMessage)` with body `todo!("peer-messaging will implement delivery")`
- [ ] 4.3 Implement `pub(crate) fn poll_messages(state: &BrokerState, agent_id: &str) -> Vec<BrokerMessage>` returning `Vec::new()`
- [ ] 4.4 Implement `pub(crate) fn agent_status_snapshot(state: &BrokerState) -> Vec<AgentStatusEntry>` returning `Vec::new()`
- [ ] 4.5 Add doc comments to all three functions describing their post-Wave-2 contract
- [ ] 4.6 Add a `#[cfg(test)] mod tests` block with unit tests for the two non-panicking stubs

## 5. axum server

- [ ] 5.1 Create `src/broker/server.rs` with module-level doc comment
- [ ] 5.2 Define `async fn publish(State(state): State<BrokerState>, headers: HeaderMap, body: String) -> Response` that:
  - Returns 415 if `Content-Type` is missing or not `application/json`
  - Returns 400 with JSON error body if body is empty
  - Calls `BrokerMessage::from_json(&body)`; on error returns 400 with JSON error body
  - On success calls `delivery::publish_message(&state, msg)` and returns 202 with empty body
- [ ] 5.3 Define `async fn messages(State(state): State<BrokerState>, Path(agent_id): Path<String>) -> Response` that:
  - Validates `agent_id` matches `[a-z0-9-_]+` (regex or hand-rolled char check); returns 400 with JSON error body otherwise
  - Calls `delivery::poll_messages(&state, &agent_id)`
  - Returns 200 with JSON body `{"messages": [...]}`
- [ ] 5.4 Define `async fn status(State(state): State<BrokerState>) -> Response` that returns 200 with JSON body containing `git_paw: true`, `version: env!("CARGO_PKG_VERSION")`, `uptime_seconds`, and `agents` from `delivery::agent_status_snapshot`
- [ ] 5.5 Build the axum `Router` with the three routes: `.route("/publish", post(publish))`, `.route("/messages/:agent_id", get(messages))`, `.route("/status", get(status))`, then `.with_state(state)`
- [ ] 5.6 Confirm axum's default behavior produces 404 for unknown paths and 405 for wrong methods (no extra config needed)
- [ ] 5.7 Verify no handler holds a `BrokerState` lock guard across an `.await` (clippy will catch this if violated)

## 6. Stale broker probe

- [ ] 6.1 Implement `fn probe_existing_broker(url: &str) -> ProbeResult` that:
  - Uses a synchronous HTTP client (a tiny hand-rolled `TcpStream` + manual HTTP/1.1 GET to avoid pulling in `reqwest` for one call) OR uses `tokio` with `block_on` against a temporary single-thread runtime
  - Times out after 500ms
  - Returns one of `ProbeResult::NoListener`, `ProbeResult::LiveBroker`, `ProbeResult::ForeignServer`, `ProbeResult::Timeout`
- [ ] 6.2 Define `enum ProbeResult` privately in `src/broker/server.rs` or a new `src/broker/probe.rs`
- [ ] 6.3 Parse the `/status` response body (if any) to detect the `git_paw: true` marker; treat any other shape as `ForeignServer`
- [ ] 6.4 Add unit tests for the probe using a test HTTP server (axum on a random port) covering all four outcomes

## 7. Broker entry point and handle

- [ ] 7.1 Implement `pub fn start_broker(config: BrokerConfig, state: BrokerState) -> Result<BrokerHandle, BrokerError>`:
  - Call `probe_existing_broker(&config.url())`
  - If `LiveBroker` → return a `BrokerHandle` in "reattached" mode that does not own a runtime
  - If `ForeignServer` → return `Err(BrokerError::PortInUse { ... })`
  - If `Timeout` → return `Err(BrokerError::ProbeTimeout { ... })`
  - If `NoListener` → proceed to bind
- [ ] 7.2 Construct the runtime: `tokio::runtime::Builder::new_multi_thread().enable_all().build()?`
- [ ] 7.3 Build the listener via `tokio::net::TcpSocket::new_v4()?` + `set_reuseaddr(true)?` + `bind(addr)?` + `listen(1024)?`
- [ ] 7.4 Spawn the axum server on the runtime via `runtime.spawn(async move { axum::serve(listener, router).await })`
- [ ] 7.5 Define `pub struct BrokerHandle { runtime: Option<tokio::runtime::Runtime>, shutdown_tx: Option<oneshot::Sender<()>> }` (or equivalent)
- [ ] 7.6 Implement `Drop for BrokerHandle` that signals shutdown and waits briefly for in-flight requests to drain (~2 second timeout), then drops the runtime
- [ ] 7.7 Add unit tests for `start_broker` covering: success on free port, error on occupied non-broker port, reattach on existing broker

## 8. Signal handling

- [ ] 8.1 Inside `start_broker`, install a `SIGINT` handler via `tokio::signal::ctrl_c` spawned as a background task that does nothing (or logs a message) — the goal is to swallow the signal so it does not terminate the process
- [ ] 8.2 Document in the broker's module doc comment that the dashboard process is responsible for any user-facing Ctrl+C handling (e.g. clearing input)
- [ ] 8.3 Add a unit test that sends `SIGINT` to the current process via `nix` or `libc::raise` and confirms the broker continues to respond (skip on Windows; we don't support it natively)

## 9. Panic isolation

- [ ] 9.1 Confirm axum's default `IntoResponse` panic handler returns 500 (no extra config needed; verify with a test that installs a deliberate-panic route)
- [ ] 9.2 Ensure any background tasks spawned by the broker use `tokio::spawn` so panics are confined to the task
- [ ] 9.3 Add a test that asserts a panicking handler returns 500 and subsequent requests to other routes still succeed

## 10. Integration tests

- [ ] 10.1 Create `tests/broker.rs` (new integration test file)
- [ ] 10.2 Helper: `fn spawn_test_broker() -> (BrokerHandle, String)` that picks a random free port (bind to port 0, read assigned port back), starts a broker, returns the handle and the URL
- [ ] 10.3 Test: `POST /publish` with valid `agent.status` returns 202
- [ ] 10.4 Test: `POST /publish` with invalid JSON returns 400 with error body
- [ ] 10.5 Test: `POST /publish` with validation failure (empty agent_id) returns 400
- [ ] 10.6 Test: `POST /publish` with wrong content type returns 415
- [ ] 10.7 Test: `POST /publish` with empty body returns 400
- [ ] 10.8 Test: `GET /messages/feat-x` returns 200 with `{"messages":[]}`
- [ ] 10.9 Test: `GET /messages/feat%2Fx` (invalid char after URL-decode) returns 400
- [ ] 10.10 Test: `GET /status` returns 200 with `git_paw: true`, `version`, `uptime_seconds`, `agents: []`
- [ ] 10.11 Test: ten concurrent `GET /status` requests all return 200 with the marker
- [ ] 10.12 Test: `GET /unknown/route` returns 404
- [ ] 10.13 Test: `GET /publish`, `POST /status`, `POST /messages/feat-x` all return 405
- [ ] 10.14 Test: dropping `BrokerHandle` shuts down the broker (subsequent connect fails)
- [ ] 10.15 Test: starting two brokers on the same port — the second one's probe sees the first and reattaches

## 11. Quality gates

- [ ] 11.1 `cargo fmt` clean
- [ ] 11.2 `cargo clippy --all-targets -- -D warnings` clean (no `unwrap`/`expect` outside tests, all public items documented, `await_holding_lock` clean)
- [ ] 11.3 `cargo test` — all unit and integration tests pass
- [ ] 11.4 `cargo doc --no-deps` builds without warnings for the new module
- [ ] 11.5 `just deny` clean (license/advisory/duplicate-dep checks pass with new tokio + axum tree)
- [ ] 11.6 `just audit` clean
- [ ] 11.7 `just check` — full pipeline green

## 12. Handoff readiness

- [ ] 12.1 Confirm `src/broker/server.rs` and `src/broker/delivery.rs` exist with the documented signatures
- [ ] 12.2 Confirm `src/broker/mod.rs` exposes `BrokerState`, `BrokerConfig`, `BrokerHandle`, `BrokerError`, `start_broker`, `AgentStatusEntry`
- [ ] 12.3 Confirm no `#[tokio::main]` was added to `src/main.rs`
- [ ] 12.4 Confirm no edits outside `Cargo.toml`, `deny.toml` (if needed), `src/config.rs`, `src/error.rs`, `src/broker/`, `tests/broker.rs`
- [ ] 12.5 Confirm `delivery.rs` stubs are unchanged from the contract: `publish_message` is `todo!()`, `poll_messages` and `agent_status_snapshot` return empty
- [ ] 12.6 Update root `AGENTS.md` to add the `broker` module to the architecture diagram (Wave 1 deliverable)
- [ ] 12.7 Commit with message: `feat(broker): add http server, config, and delivery stubs`
