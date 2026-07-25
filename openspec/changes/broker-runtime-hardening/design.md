# Design — broker-runtime-hardening

## Context

The broker is a tokio/axum HTTP server on a `new_multi_thread` runtime (`src/broker/mod.rs`
`start_broker_with`). Its single concurrency bet — every state lock is `std::sync` and no
guard is ever held across `.await` — is sound and lint-enforced (`await_holding_lock =
deny`). But two hazards sit *outside* what that lint can see, on the real multi-threaded
dogfood path, and neither is covered by the current tests (single-threaded, router
`oneshot`, `role_gating = None`).

### H1 — blocking `git` inside the async `/publish` handler

Exact call chain, all on a tokio worker thread:

1. `src/broker/server.rs` — `async fn publish` (~line 198) validates the body, then at
   ~line 241 calls `delivery::publish_message(&state, &msg)` **inline** (no
   `spawn_blocking`).
2. `src/broker/delivery.rs` — `publish_message` (~line 201): after the write lock is
   released, when a role-gating context is attached and the message is an
   `Artifact { status: "committed" }`, calls
   `crate::opsx::role_guard::run_guard(state, agent_id, payload, ctx)`.
3. `src/opsx/role_guard.rs` — `run_guard` (~line 295) calls `head_commit_info(worktree)`.
4. `src/opsx/role_guard.rs` — `head_commit_info` (~line 370) runs
   `std::process::Command::new("git").args(["log", "-1", ...]).output()` — a **blocking**
   subprocess spawn + wait, executed directly on the tokio worker.

A burst of `committed` publishes on a role-gating session therefore consumes tokio workers
in blocking `git` waits. With enough concurrent publishes to occupy every worker, all other
HTTP (`/status`, `/messages`, further `/publish`) stalls until the git processes exit. This
is latent only because tests build state with `role_gating = None`, so step 2 never reaches
the git call.

### H2 — lock-poison bricks the broker

`src/broker/mod.rs` — `BrokerState::read` (~line 295) and `write` (~line 304):

```rust
self.inner.read().expect("broker state lock poisoned")
```

`std::sync::RwLock` becomes *poisoned* if a thread panics while holding a guard. After that,
every `read()`/`write()` `.expect(...)` panics too. axum catches the per-request panic
(the socket stays bound), but the state is now permanently unreachable — every later handler
and every background task (watcher, flush, detector) panics on its next lock acquisition.
The server is alive but can never serve a real response again. The existing
`panic_in_handler_is_isolated` test panics in a handler that holds **no** lock, so poison
propagation is untested.

## Decisions

### D1 — H1: offload the blocking publish work via `spawn_blocking`

`publish_message` is a synchronous function that performs blocking work (a state-lock
critical section plus, on the role-gating path, a `git` subprocess and learnings file I/O).
The clean, surgical fix is at the async→sync boundary: the `publish` handler runs
`delivery::publish_message` inside `tokio::task::spawn_blocking` and `.await`s the join
handle. `Arc<BrokerState>` is `Send + Sync + 'static` and `BrokerMessage` is `Clone + Send`,
so the closure captures an `Arc::clone` and an owned message — no lifetime friction. The
blocking `git` (and the rest of the synchronous publish path) then runs on tokio's dedicated
blocking pool, which is sized for exactly this and does not starve the async workers.

Rejected alternative — making `head_commit_info` `async` / pushing `spawn_blocking` down
into `role_guard`: that would spread tokio into `src/opsx/`, violating the async-containment
requirement ("tokio confined to `src/broker/`") and touching more surface than the hazard
warrants. Offloading at the handler keeps async strictly inside `src/broker/` and leaves the
guard, delivery routing, and lock ordering byte-for-byte unchanged. No lock/await/spawn
ordering elsewhere is reordered — the only change is that the existing synchronous call now
runs on the blocking pool.

### D2 — H2: recover a poisoned lock via `PoisonError::into_inner`

Replace the `.expect("broker state lock poisoned")` in `read`/`write` with recovery:

```rust
self.inner.read().unwrap_or_else(std::sync::PoisonError::into_inner)
```

`into_inner` yields the guard regardless of poison, so a single panic while a guard was held
degrades to at most one observed-inconsistent or lost write — never a permanently dead
server. Broker state is a best-effort in-memory roster/log/queue set that is rebuilt fresh
on every `git paw start`; there is no on-disk invariant a poisoned view could corrupt, so
continuing to serve is strictly better than wedging. Both `read` and `write` are changed
identically, and their `# Panics` doc sections are updated to state they recover from poison
rather than panic.

Rejected alternative — swap `std::sync::RwLock` for `parking_lot` (no poisoning): adds a
dependency outside the approved set and is a broader change than the hazard needs.

### D3 — Regression tests reproduce each hazard first (multi-thread)

Both tests use a real multi-threaded runtime so the hazard is observable (a single-threaded
test cannot exhibit worker starvation, and router `oneshot` never crosses a thread boundary).

- **H1:** start a broker (or drive the router on a `flavor = "multi_thread"` runtime with a
  bounded worker count) whose `BrokerState` carries an **active `RoleGatingContext`** —
  closing the `role_gating = None` gap. Fire a burst of `Artifact { status: "committed" }`
  publishes that each trip the blocking `git` read, and concurrently issue `GET /status`.
  Before the fix the status request cannot complete within a generous bound while the
  workers are saturated; after the fix `/status` returns promptly because the git work is on
  the blocking pool. Assert the concurrent `/status` completes within the bound.
- **H2:** on a multi-thread runtime, cause a panic while a write guard is held (a
  test-only route/task that panics mid-critical-section, poisoning the lock), then issue a
  subsequent request that takes the lock. Before the fix it panics/500s forever; after the
  fix the broker still responds `200`. Assert the post-poison request succeeds.

The H1 test's timing assertion uses a wide margin (the fixed path returns in milliseconds
while the stalled path blocks for the full git burst) to stay robust under CI load, per the
local flakiness learnings around load-sensitive broker timing.

### D4 — Scope fence

Only H1 and H2. The analysis's M3 (seq ordering), M4 (learnings mutex across file I/O), M5
(unbounded `message_log`), M6 (task cancellation), L7/L8 are explicitly **out of scope** —
they change behavior/perf and belong to their own changes. This change reorders nothing in
the lock/await/spawn discipline and performs no structural broker refactor.

## Non-goals

- No broker structural refactor (the analysis rates `broker/*` restructuring Medium-risk,
  post-freeze).
- No change to the broker wire format, `[broker]` config schema, or any public signature.
- No fix for M3/M4/M5/M6/L7/L8 (separate changes).

## Risks

- **H1 offload correctness:** `spawn_blocking` moves the publish path onto the blocking
  pool; the multi-thread regression test plus the unchanged single-threaded delivery tests
  confirm ordering and routing semantics are preserved (`publish_message` remains internally
  synchronous; only its execution context moves).
- **H1 test flakiness:** timing-based; mitigated by a wide margin and by asserting the
  *non-stall* direction (the fixed path is orders of magnitude faster), and by running
  broker E2E serially per the project's serialize-E2E convention.
- **H2 semantics:** continuing after poison could surface a partially-updated in-memory view
  once; acceptable because broker state is ephemeral (rebuilt each `git paw start`) with no
  persisted invariant, and the alternative is a dead server. Low risk.
