## Why

v0.3.0 needs a running HTTP server inside each git-paw session so parallel agents can publish status, share artifacts, and discover what their peers are doing. The `message-types` change defined the wire format; this change provides the server that hosts that wire format and the lifecycle that ties it to the session. Without this, no other v0.3.0 coordination feature has anywhere to talk to.

## What Changes

- Add a new `[broker]` section to `.git-paw/config.toml` with three fields: `enabled` (bool, default `false` in v0.3.0), `port` (u16, default `9119`), `bind` (string, default `"127.0.0.1"`)
- Spin up an axum HTTP server inside the dashboard process in pane 0 when `[broker] enabled = true`
- Create `BrokerState` — the shared state held in `Arc<RwLock<…>>` and accessible from both the server handlers and the dashboard
- Implement three HTTP endpoints:
  - `POST /publish` — accept a `BrokerMessage`, validate via `BrokerMessage::from_json`, hand to delivery
  - `GET /messages/:agent_id` — return queued messages addressed to the given agent
  - `GET /status` — return all known agent states (used by dashboard and as a liveness probe)
- Bind the listener with `SO_REUSEADDR` so a quick restart after crash does not fail with `EADDRINUSE`
- On `git paw start`, probe `GET /status` against the configured `bind:port` before binding; if a previous broker responds, reattach instead of starting a new one
- Trap `SIGINT` in the broker process so accidental Ctrl+C in pane 0 does not kill the session; only `git paw stop` from outside terminates cleanly
- Wrap broker tokio tasks so a panic in one handler does not bring down the runtime or the dashboard
- Inject `GIT_PAW_BROKER_URL=http://<bind>:<port>` into every tmux pane's environment at session launch via `tmux set-environment` so skill templates remain portable across repos with different ports
- Create stub functions in `src/broker/delivery.rs` (e.g. `publish_message`, `poll_messages`, `agent_status`) with stable signatures but `todo!()` or empty-return bodies. `peer-messaging` will fill these in during Wave 2.
- Add new dependencies: `tokio` (with minimal features), `axum`. Approved per the v0.3.0 dependency stack decision.

## Capabilities

### New Capabilities

- `broker-server`: Lifecycle, runtime, configuration, and shared state of the in-process HTTP broker. Covers config schema, server bring-up and shutdown, port binding semantics, signal handling, panic isolation, environment variable injection, and the contract by which `peer-messaging` extends delivery behavior.
- `broker-endpoints`: HTTP request and response contracts for the three v0.3.0 endpoints — `POST /publish`, `GET /messages/:agent_id`, `GET /status`. Defines status codes, content types, success and error response shapes, and validation behavior at the HTTP boundary.

### Modified Capabilities

- `configuration`: Add the `[broker]` section with `enabled`, `port`, `bind` fields and `BrokerConfig` struct with `url()` helper. Update default config generation to include commented broker examples.
- `error-handling`: Add `BrokerError` type with `PortInUse`, `ProbeTimeout`, `BindFailed`, `RuntimeFailed` variants, wrappable inside `PawError`.

## Impact

- **New files (owned by this change):**
  - `src/broker/server.rs` — axum router, handlers, runtime spawn
  - `src/broker/delivery.rs` — stub delivery functions with stable signatures (bodies filled in by `peer-messaging`)
- **Extended files (owned by this change):**
  - `src/broker/mod.rs` — extends the `message-types` stub to declare `pub mod server;`, `pub mod delivery;`, and define `BrokerState`
- **Modified files:**
  - `src/config.rs` — add the `BrokerConfig` struct and `[broker]` parsing; default-on-absent so existing v0.2.0 configs still load
- **New runtime dependencies:** `tokio` (features `rt-multi-thread`, `macros`, `net`, `sync`, `time`), `axum`. Both MIT-licensed, cross-platform on all supported targets.
- **No CLI surface changes in this change.** Wiring the broker into `git paw start`/`stop`/`purge` lifecycle and surfacing it in `git paw status` belongs to `broker-integration` in Wave 2.
- **Dependents:** `dashboard-tui` (reads `BrokerState`), `peer-messaging` (replaces `delivery.rs` stubs), `broker-integration` (lifecycle wiring), `skill-templates` (uses `GIT_PAW_BROKER_URL` env var).
- **Async boundary:** This change introduces tokio to the codebase for the first time. The runtime is contained to the broker module — `main.rs`, `tmux.rs`, `git.rs`, and other existing modules remain synchronous. No `#[tokio::main]` at the entry point.
