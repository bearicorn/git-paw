## Why

Two latent broker hazards live on the real multi-threaded dogfood path and are invisible
to the `clippy::await_holding_lock = deny` lint that otherwise guards the broker's lock
discipline. Both were surfaced by the v0.13.0 principal-engineer code analysis (§5 H1/H2,
§6 CF2). Neither is exercised by the current test suite (broker tests are single-threaded,
router-`oneshot`, and build state with `role_gating = None`), so both can ship into the
v1.0.0 freeze undetected.

- **H1 — blocking `git` on a tokio worker thread.** The async `POST /publish` handler runs
  a synchronous `std::process::Command::new("git")` inline (call chain below) when a
  `committed` artifact arrives on a role-gating session. A burst of such publishes can
  occupy every tokio worker in a blocking `git` call, stalling *all* HTTP — `/status`,
  `/messages`, further `/publish` — until the git processes return.
- **H2 — a poisoned state lock bricks the broker.** `BrokerState::read`/`write`
  `.expect("broker state lock poisoned")`. If any thread panics while holding the `RwLock`
  guard, the lock is poisoned and every subsequent handler and background task panics on
  its next lock acquisition. The server socket stays bound but no request can ever be
  served again — "alive but permanently dead."

This is a targeted correctness/robustness fix, not a broker refactor. Per the analysis,
lock/await/spawn ordering elsewhere is load-bearing and MUST NOT be touched.

## What Changes

- **H1:** Move the blocking `git` invocation off the async runtime's worker threads. The
  role-gating guard's git read (and the synchronous `publish_message` work that reaches it)
  runs via `tokio::task::spawn_blocking` (or equivalent) so a publish burst can no longer
  saturate the workers and stall the HTTP server. Behavior of the guard itself (what it
  publishes, when it fires) is unchanged — only *where* the blocking call executes changes.
- **H2:** Recover from a poisoned broker-state lock instead of propagating a panic.
  `read`/`write` take the guard out of the `PoisonError` (via `into_inner`) and keep
  serving. A single panic while a guard is held degrades to at most one lost/observed
  inconsistent write, never a permanently dead server.
- **Regression tests FIRST:** a multi-thread (`flavor = "multi_thread"`) test that
  reproduces each hazard before the fix — a role-gating publish burst that stalls `/status`
  for H1 (the missing coverage: existing tests use `role_gating = None`), and a
  panic-while-holding-the-lock followed by a still-served request for H2.

## Capabilities

### Modified Capabilities
- `broker-server`: adds two robustness requirements to the broker runtime contract —
  blocking subprocess calls run off the async worker threads, and a poisoned state lock is
  recovered rather than fatal. No wire-format, config-schema, or public-signature change.

## Impact

- **Code:** `src/broker/server.rs` (`publish` handler — the async→sync boundary where the
  blocking work is offloaded), `src/broker/delivery.rs` (`publish_message` — the sync entry
  the offload wraps), `src/opsx/role_guard.rs` (`run_guard` / `head_commit_info` — the
  blocking `git` site), and `src/broker/mod.rs` (`BrokerState::read`/`write` poison
  recovery + their `# Panics` doc sections). No new dependencies.
- **NOT enum-variant ripple:** touches neither the `BrokerMessage` nor `SpecBackendKind`
  variant set, so the AGENTS.md exhaustive-match hazard does not apply.
- **Frozen-surface safe:** no change to the broker wire format, the `[broker]` config
  schema, or any public function signature. `BrokerState::read`/`write` return the same
  guard types; only their poison behavior changes (panic → recover).
- **Tests:** new multi-thread regression tests in the broker tree (H1 publish-burst
  non-stall with a role-gating context; H2 poison-recovery). Existing single-threaded and
  router-`oneshot` tests pass unchanged.
- **Docs:** the `# Panics` doc comments on `read`/`write` are updated (they no longer panic
  on poison). No `--help`, README, or config-reference change (no user-facing surface).
