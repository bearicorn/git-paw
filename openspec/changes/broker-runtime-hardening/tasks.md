# Tasks — broker-runtime-hardening

## 1. Regression tests FIRST (must fail before the fix)

- [ ] H1 — add a **multi-thread** (`#[tokio::test(flavor = "multi_thread", worker_threads = N)]`
      or a `start_broker`-backed) regression test that builds `BrokerState` with an
      **active `RoleGatingContext`** (closing the `role_gating = None` coverage gap), fires
      a burst of `agent.artifact { status: "committed" }` publishes that each trip the
      blocking `git` read, and concurrently issues `GET /status`; assert `/status`
      completes within a generous bound. Confirm it FAILS (stalls) against the current code.
- [ ] H2 — add a **multi-thread** regression test that poisons the state lock (a test-only
      route/task that panics while holding the write guard), then issues a follow-up request
      that acquires the lock; assert the follow-up responds `200`. Confirm it FAILS (panics)
      against the current code.
- [ ] Run both new tests with `cargo test --no-fail-fast` and record the pre-fix failures
      (verify by real exit code, not piped output).

## 2. H1 — move blocking `git` off the async worker threads

- [ ] In `src/broker/server.rs` `async fn publish`, run `delivery::publish_message` via
      `tokio::task::spawn_blocking` (capturing an `Arc::clone` of state + the owned message)
      and `.await` the join handle, so the blocking `git` in the role-gating path no longer
      executes on a tokio worker thread.
- [ ] Leave `delivery::publish_message`, `role_guard::run_guard`, `head_commit_info`, and
      all lock/await/spawn ordering unchanged (only the execution context moves).
- [ ] Confirm the H1 regression test now PASSES; confirm `just lint`
      (`await_holding_lock`) stays clean.

## 3. H2 — recover a poisoned broker-state lock

- [ ] In `src/broker/mod.rs`, change `BrokerState::read` and `BrokerState::write` to recover
      from a poisoned lock (`unwrap_or_else(std::sync::PoisonError::into_inner)`) instead of
      `.expect("broker state lock poisoned")`.
- [ ] Update the `# Panics` doc sections on `read`/`write` to state they recover from poison
      rather than panic.
- [ ] Confirm the H2 regression test now PASSES.

## 4. Verification (five gates — supervisor-run)

- [ ] Gate 1 — Testing: `cargo test --no-fail-fast` green for the broker tree incl. the two
      new regression tests.
- [ ] Gate 2 — Regression: full suite green vs the merge-base; broker E2E run serially.
- [ ] Gate 3 — Spec audit: every `broker-server` scenario added by this change maps to a
      test (H1 non-stall, H2 poison-recovery).
- [ ] Gate 4 — Doc audit: `read`/`write` `# Panics` docs match the new behavior; confirm no
      `--help`/README/config-reference change is required (no user-facing surface).
- [ ] Gate 5 — Security: no new shell/path handling, no secrets; least-privilege preserved
      (git args unchanged, only relocated to the blocking pool).
- [ ] `just check` green; `cargo fmt` before commit.
- [ ] `openspec validate broker-runtime-hardening --strict` passes.
