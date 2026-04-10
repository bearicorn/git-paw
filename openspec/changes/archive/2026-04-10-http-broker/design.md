## Context

This change is the second in Wave 1 of v0.3.0 and the first to introduce async runtime, an HTTP server, and a long-lived background subsystem to git-paw. Up to v0.2.0 the codebase has been entirely synchronous and short-lived: every command executes, prints output, and exits. The broker breaks both assumptions — it lives for the entire session, accepts concurrent requests from N agents, and shares mutable state between request handlers and the dashboard render loop.

Architecturally the broker is unusual in two ways:

1. **It runs inside the dashboard process in pane 0.** Per the v0.3.0 architecture decision, the broker is not a daemon, has no PID file, and is not a separate binary. The same process that draws the ratatui dashboard also hosts the axum server. When `git paw stop` kills tmux, the broker dies with pane 0 — no separate cleanup needed.
2. **It is opt-in via config.** v0.3.0 ships with `[broker] enabled = false` so existing v0.2.0 sessions keep working untouched. Users opt in per repo by editing `.git-paw/config.toml`.

This change owns the server, its lifecycle, and its configuration, but explicitly does NOT own the message routing logic — that lives in `delivery.rs` which `peer-messaging` will fill in during Wave 2. The supervision plan from Phase 4 says this change scaffolds `delivery.rs` with stub functions that have stable signatures so peer-messaging only fills in bodies.

## Goals / Non-Goals

**Goals:**

- Provide a working HTTP server reachable at `http://<bind>:<port>` for the lifetime of an enabled git-paw session
- Define `BrokerState` as the shared state type that the dashboard and the server handlers both access
- Implement the three v0.3.0 endpoints with correct status codes, content negotiation, and validation at the HTTP boundary
- Make port reuse robust across rapid restarts (no `EADDRINUSE` after a crash)
- Make stale-broker detection graceful (probe `/status`, reattach if alive, clean up if dead)
- Contain async to the broker module so the rest of the codebase stays synchronous
- Establish stable function signatures in `delivery.rs` so `peer-messaging` is a pure body-fill exercise

**Non-Goals:**

- Authentication or authorization. The broker binds to `127.0.0.1` only and trusts every local caller. Auth is out of scope for v0.3.0 and may be reconsidered in v2.0 alongside A2A.
- TLS. Local HTTP only. Never TLS.
- Public binding. The `bind` config field exists for flexibility (e.g. binding to a specific localhost interface) but documentation will state firmly that public binds are unsupported.
- Message persistence. The broker queue is in-memory only. Stop/start loses queued messages. A separate session log captures messages for `git paw replay`, but that's the logger's job, not the queue's.
- Lifecycle wiring into `git paw start`/`stop`/`purge`. That belongs to `broker-integration` in Wave 2.
- Dashboard rendering. That belongs to `dashboard-tui` in Wave 1 (parallel).
- Delivery semantics — who receives what, polling order, message retention. `peer-messaging` owns this.

## Decisions

### Decision 1: tokio runtime is built explicitly inside `start_broker`, not via `#[tokio::main]`

The broker exposes a single entry point:

```rust
pub fn start_broker(config: BrokerConfig, state: Arc<RwLock<BrokerState>>) -> Result<BrokerHandle, BrokerError>
```

Internally, this builds a tokio multi-threaded runtime via `tokio::runtime::Builder::new_multi_thread().enable_all().build()?`, spawns the axum server on it, and returns a `BrokerHandle` that the dashboard process holds for the duration of pane 0's lifetime.

**Why:**
- Keeps `main.rs` synchronous. No `#[tokio::main]` infection at the binary entry point.
- The dashboard process can decide whether to start a broker at all (based on `[broker] enabled`) without dragging tokio into every code path.
- The runtime is tied to a value, so `Drop` on `BrokerHandle` cleanly shuts down the runtime.
- Tests can construct a broker without going through any global state.

**Alternatives considered:**
- *`#[tokio::main]` on `fn main`*. Would force every existing sync command (`init`, `stop`, `purge`, etc.) to run inside a tokio runtime they don't need. Rejected.
- *Lazy global runtime*. A `OnceLock<Runtime>` initialized on first use. Hides ownership, makes shutdown harder, complicates testing. Rejected.

### Decision 2: `BrokerState` is `Arc<RwLock<Inner>>` with a private inner struct

```rust
pub struct BrokerState {
    inner: Arc<RwLock<BrokerStateInner>>,
}

struct BrokerStateInner {
    agents: HashMap<String, AgentRecord>,        // agent_id → latest known state
    queues: HashMap<String, VecDeque<BrokerMessage>>,  // agent_id → pending inbox
    started_at: Instant,
    // ... fields filled in or extended by peer-messaging
}
```

`BrokerState` is `Clone` (cheap — just clones the `Arc`). Both the axum handlers and the dashboard hold their own clones of the same `Arc`. Read-mostly operations (like `/status` and dashboard frame draws) take `read()`. Mutations (publish, dequeue) take `write()`.

**Why:**
- `RwLock` over `Mutex` because `/status` polls and dashboard renders dominate; writes are rarer
- `Arc` because the value crosses thread boundaries (axum's tokio worker threads ↔ dashboard's main thread)
- Inner struct is private so peer-messaging can add fields without breaking external API. Public methods on `BrokerState` are the stable interface.

**Alternatives considered:**
- *Channel-based actor model* (a single task owns the state, all access via mpsc messages). More principled but ~3x more code, harder for the synchronous dashboard render loop to integrate with. Rejected for v0.3.0; could revisit if contention becomes a problem.
- *`Arc<Mutex<…>>`*. Simpler but worse for the read-heavy access pattern. Rejected.
- *`tokio::sync::RwLock` instead of `std::sync::RwLock`*. The dashboard reads from a sync context, so a tokio lock would force `block_on`. Use `std::sync::RwLock` so both sides can access without an async context. Tradeoff: holding a sync lock across `.await` is forbidden — handlers must drop the guard before awaiting. Document this in the module.

### Decision 3: The three endpoints are implemented as plain async functions, not via axum extractors-heavy patterns

```rust
async fn publish(State(state): State<BrokerState>, body: String) -> Response { ... }
async fn messages(State(state): State<BrokerState>, Path(agent_id): Path<String>) -> Response { ... }
async fn status(State(state): State<BrokerState>) -> Response { ... }
```

Bodies are read as raw `String` and parsed via `BrokerMessage::from_json` rather than via `Json<BrokerMessage>` extractor.

**Why:**
- `BrokerMessage::from_json` already does serde + validation in one pass and returns a unified `MessageError`. Using axum's `Json<>` extractor would deserialize first and validate second, producing two different error paths for the same conceptual failure.
- We get to control the exact HTTP status code and error body for each `MessageError` variant, rather than letting axum produce its own 400 with a different shape.
- Smaller dependency surface — no need for axum's `json` feature beyond what's already pulled in.

**Alternatives considered:**
- *`Json<BrokerMessage>` extractor*. Splits the validation story, harder to test. Rejected.
- *Custom extractor wrapping `from_json`*. Possible but more code than calling `from_json` inline. Rejected for now; can refactor later if more endpoints need the same pattern.

### Decision 4: Bind with `SO_REUSEADDR` via `tokio::net::TcpSocket`

Instead of `tokio::net::TcpListener::bind`, the broker uses:

```rust
let socket = tokio::net::TcpSocket::new_v4()?;
socket.set_reuseaddr(true)?;
socket.bind(addr)?;
let listener = socket.listen(1024)?;
```

**Why:**
- After a crash, the OS holds the port in TIME_WAIT for ~60s. Without `SO_REUSEADDR`, restart fails. With it, restart succeeds immediately.
- One extra line vs. `TcpListener::bind`; no downsides on local-only binds.

### Decision 5: Stale broker detection via `/status` probe, not PID files

On startup, before binding, the broker probes `GET http://<bind>:<port>/status` with a short timeout (~250ms). Three outcomes:

| Probe result | Action |
|---|---|
| Connect refused / no listener | Port is free; bind and start |
| Responds with valid `/status` JSON | A live broker exists; reattach (return its handle, do not bind) |
| Responds but with garbage / wrong shape | Something else is on this port; fail with a clear error pointing at `[broker] port` |
| Timeout | Treat as "stuck, probably dead"; log a warning, attempt to bind (will fail with `EADDRINUSE` if it's actually alive, which is fine — clear error) |

**Why:**
- Avoids PID file management and the cross-platform syscalls (`kill -0`, Windows process handles, etc.) that come with it
- The `/status` endpoint already exists for the dashboard; reusing it for liveness costs nothing
- Distinguishes "our broker is alive" from "something else has the port" for better error messages
- Self-cleaning: if the broker crashed without writing a state file, the next start does the right thing automatically

**Alternatives considered:**
- *PID file in session state*. More work, more failure modes (stale file from kill -9), and requires platform-specific process-alive checks. Rejected.
- *Fixed retry with exponential backoff*. Hides the cause from the user. Rejected.

### Decision 6: SIGINT trap inside the broker process so accidental Ctrl+C does not kill the session

The broker registers a SIGINT handler that converts the signal into a no-op for the broker (or, optionally, into a "clear pending input" hint that the dashboard handles via its own input loop). Pane 0 can only be terminated cleanly by:

1. `git paw stop` killing tmux from the outside
2. The dashboard explicitly handling a quit keybind (e.g. `q`)
3. SIGTERM / SIGKILL

**Why:**
- Users who reach for Ctrl+C in pane 0 expect to interrupt their input, not destroy the entire session and lose all queued broker messages
- Standard TUI behavior — every ratatui app does this
- `git paw stop` remains the only "official" shutdown path, which keeps the lifecycle simple

**Open question:** does SIGINT go to the broker tokio task or to the dashboard's input loop first? Implementation detail — both observers run in the same process, so order doesn't matter as long as neither calls `std::process::exit`.

### Decision 7: Panic isolation via `tokio::spawn` per request

axum's default behavior is to catch panics in handlers and return 500. We rely on that and additionally wrap any background tokio tasks (e.g. queue cleanup, future supervisor heartbeat) in `tokio::spawn(async move { … })` so a panic in one task does not poison the runtime.

**Why:**
- A bug in delivery logic should not crash the entire pane 0 process
- Standard tokio practice; no extra deps
- The dashboard render loop is already on the main thread separate from tokio worker threads; a tokio task panic does not reach it

**What we do NOT do:** wrap the dashboard render loop in `catch_unwind`. Ratatui has its own panic hook for terminal restore, and adding `catch_unwind` on top would interfere. Trust ratatui's hook plus a clean error message.

### Decision 8: `delivery.rs` functions are stubs with stable signatures

This change creates `src/broker/delivery.rs` with these public functions:

```rust
pub(crate) fn publish_message(state: &BrokerState, msg: BrokerMessage) { todo!("peer-messaging") }
pub(crate) fn poll_messages(state: &BrokerState, agent_id: &str, since: u64) -> (Vec<BrokerMessage>, u64) { (Vec::new(), 0) }
pub(crate) fn agent_status_snapshot(state: &BrokerState) -> Vec<AgentStatusEntry> { Vec::new() }
```

Server handlers call these functions. In v0.3.0 Wave 1, `publish_message` panics if called (`todo!`), `poll_messages` returns `(Vec::new(), 0)`, `agent_status_snapshot` returns empty. The handlers therefore "work" — they accept input, call the stub, and return successfully — but no actual delivery happens until `peer-messaging` lands.

`poll_messages` accepts a `since: u64` cursor parameter and returns a `(Vec<BrokerMessage>, u64)` tuple of `(messages, last_seq)`. This supports cursor-based non-destructive polling — agents track their own position and never lose messages. The stub returns `(empty, 0)` for all inputs.

**Why:**
- Server handler signatures are frozen at Wave 1 merge. `peer-messaging` does not edit `server.rs`.
- Tests for `broker-server` can verify the HTTP shape (status codes, content types, validation) without any delivery logic.
- File ownership is clean: `delivery.rs` is created by this change but its bodies are owned by `peer-messaging`. Per the supervision plan agreed earlier, this is acceptable — it's a single-direction handoff, not co-editing.

**Subtle point:** `publish_message` uses `todo!()` rather than empty body so any code that accidentally tries to use a Wave 1 broker for real work fails loudly instead of silently. `poll_messages` and `agent_status_snapshot` return empty because handlers need to produce valid HTTP responses even before delivery exists, and an empty result is a meaningful response.

### Decision 9: `GIT_PAW_BROKER_URL` injection happens in `broker-integration`, not here

This change defines the **format** of the env var (`http://<bind>:<port>`) and exposes a method `BrokerConfig::url(&self) -> String` that builds it. The actual `tmux set-environment` call belongs to `broker-integration` in Wave 2 because that's where session launch lives.

**Why:**
- This change owns the broker, not tmux session orchestration
- Wave 1 can complete and merge without touching `tmux.rs` or `main.rs`
- `broker-integration` will call `BrokerConfig::url` to compute the value to inject

## Risks / Trade-offs

- **First async code in the codebase** → New mental model, new failure modes (panics in tasks, holding sync locks across awaits, runtime shutdown ordering). **Mitigation:** contain to the broker module, document the `RwLock` discipline at the top of `mod.rs`, add unit tests that exercise concurrent access, lean on tokio's well-trodden patterns.

- **Stub `delivery.rs` could rot** → If `peer-messaging` slips or never lands, `publish_message`'s `todo!()` becomes a footgun. **Mitigation:** v0.3.0 ships with `[broker] enabled = false` by default. Users who explicitly enable it before Wave 2 lands are on the hook. The release notes for v0.3.0 will not mention the broker as a usable feature unless all of Wave 1+2 ship together.

- **`std::sync::RwLock` held across `.await`** → The compiler does not catch this; it's a runtime deadlock waiting to happen. **Mitigation:** explicit doc comment on `BrokerState` warning against it; clippy lint `clippy::await_holding_lock` is enabled in `Cargo.toml` lint config and will catch the obvious cases. Consider adding a wrapper method that takes a closure and guarantees the guard drops before any `.await`.

- **Port collision across multiple repos** → Documented in the proposal; users editing two repos must pick distinct ports. **Mitigation:** the stale-broker probe fails fast with a clear error pointing at `[broker] port`; the mdBook configuration chapter will document the multi-repo workflow.

- **`tokio::net::TcpSocket` API differences across platforms** → On Windows it requires extra dance; on Unix it's straightforward. **Mitigation:** git-paw only supports macOS, Linux, and WSL — all Unix. No Windows native support, so no issue.

- **Binary size growth** → tokio + axum + their transitive deps add ~5-8 MB to the release binary. **Mitigation:** acceptable for a developer tool; cargo-deny update may be needed for new transitive licenses.

- **`SIGINT` handler conflicts with dashboard** → The dashboard process may want to handle `Ctrl+C` itself (e.g. as "cancel current input"). If both the broker and the dashboard install handlers, the second one wins. **Mitigation:** the broker installs a no-op handler only if the dashboard has not installed one; coordinate via a shared "signal handler installed" flag in `BrokerState`. This is fragile — flag for revisit during integration.

## Migration Plan

No data migration. Rollback is `git revert` of this change plus the `message-types` change if needed (they merge in order). No on-disk state is created until `broker-integration` lands in Wave 2.

For v0.2.0 → v0.3.0 users, no action is required. The new `[broker]` config section is optional and defaults to disabled. Existing `.git-paw/config.toml` files load and run without modification.

## Open Questions

- **Should `BrokerHandle::shutdown()` wait for in-flight requests to complete (graceful) or drop them (abrupt)?** Likely graceful with a short timeout (~2s). Decide during implementation; not blocking for the spec.
- **Does the broker log to stdout, to a file, or to nowhere?** Pane 0 is a TUI — stdout is owned by ratatui. Likely log to a file under `.git-paw/logs/broker.log` (or rely on the existing session logger), but the exact path and format can be settled when `broker-integration` wires the log setup. Not blocking.
- **Should `/status` include broker uptime and version?** Useful for the stale-broker probe to confirm "this is our broker, not something else". The probe could check for a `git_paw: true` field in the response. Lean yes; document in the spec.
