# Tasks — broker-runtime-hardening

## 1. Regression tests FIRST (must fail before the fix)

- [x] H1 — add a **multi-thread** (`#[tokio::test(flavor = "multi_thread", worker_threads = N)]`
      or a `start_broker`-backed) regression test that builds `BrokerState` with an
      **active `RoleGatingContext`** (closing the `role_gating = None` coverage gap), fires
      a burst of `agent.artifact { status: "committed" }` publishes that each trip the
      blocking `git` read, and concurrently issues `GET /status`; assert `/status`
      completes within a generous bound. Confirm it FAILS (stalls) against the current code.
      Added `tests/broker_runtime_hardening.rs::publish_burst_with_role_gating_does_not_stall_status`
      (`worker_threads = 2`, 120-artifact burst against a real one-commit temp repo).
      The bound is expressed as *`/status` answers while the burst is still running*
      (`status_elapsed * 2 < burst_elapsed`) rather than a fixed millisecond figure, so
      the margin scales with machine speed instead of going flaky under load; a 5s
      `tokio::time::timeout` is the outer deadlock guard.
- [x] H2 — add a **multi-thread** regression test that poisons the state lock (a test-only
      route/task that panics while holding the write guard), then issues a follow-up request
      that acquires the lock; assert the follow-up responds `200`. Confirm it FAILS (panics)
      against the current code. Added
      `tests/broker_runtime_hardening.rs::request_after_a_lock_poisoning_panic_is_still_served`
      (a spawned task panics while holding the write guard — no test-only production route
      was added) plus the unit-level
      `broker::tests::read_and_write_recover_from_a_poisoned_lock` for the read/write
      accessor contract.
- [x] Run both new tests with `cargo test --no-fail-fast` and record the pre-fix failures
      (verify by real exit code, not piped output).
      **Pre-fix run, exit 101 (failed):**
      - `publish_burst_with_role_gating_does_not_stall_status` — FAILED:
        *"GET /status answered after 663.923459ms of the burst's 676.426375ms"* (98% of
        the burst — the stall the fix removes).
      - `request_after_a_lock_poisoning_panic_is_still_served` — FAILED:
        panicked at `src/broker/mod.rs:295` *"broker state lock poisoned: PoisonError { .. }"*.
      - `broker::tests::read_and_write_recover_from_a_poisoned_lock` — FAILED, same panic.
      - `role_gating_guard_output_survives_the_http_publish_path` passed pre-fix by
        design — it is the *unchanged-behavior* characterization guard, not a reproducer.

## 2. H1 — move blocking `git` off the async worker threads

- [x] In `src/broker/server.rs` `async fn publish`, run `delivery::publish_message` via
      `tokio::task::spawn_blocking` (capturing an `Arc::clone` of state + the owned message)
      and `.await` the join handle, so the blocking `git` in the role-gating path no longer
      executes on a tokio worker thread.
      *Deviation, deliberate:* the handler already owns its `Arc<BrokerState>` (the extracted
      `State(state)`) and does not use it after the call, so the closure **moves** that Arc
      instead of taking a redundant `Arc::clone`. Functionally identical; one fewer refcount
      bump and no dead binding. A `JoinError` (the blocking task panicked) maps to `500`
      with an `{"error": "publish task failed"}` body — the join handle returns a `Result`
      that must be handled, and `unwrap()`/`expect()` is barred in non-test code.
- [x] Leave `delivery::publish_message`, `role_guard::run_guard`, `head_commit_info`, and
      all lock/await/spawn ordering unchanged (only the execution context moves).
      Verified by diff: `src/broker/delivery.rs` and `src/opsx/role_guard.rs` are untouched
      by this change.
- [x] Confirm the H1 regression test now PASSES; confirm `just lint`
      (`await_holding_lock`) stays clean.
      Post-fix: all 3 tests in `broker_runtime_hardening` pass (0.30s wall for the whole
      binary, i.e. `/status` is served in single-digit ms against a ~250ms burst).
      `just lint` (`cargo fmt --check` + `cargo clippy --all-targets -- -D warnings`) exit 0.

## 3. H2 — recover a poisoned broker-state lock

- [x] In `src/broker/mod.rs`, change `BrokerState::read` and `BrokerState::write` to recover
      from a poisoned lock (`unwrap_or_else(std::sync::PoisonError::into_inner)`) instead of
      `.expect("broker state lock poisoned")`.
- [x] Update the `# Panics` doc sections on `read`/`write` to state they recover from poison
      rather than panic. The `# Panics` headings are **removed** rather than reworded — the
      accessors no longer have a panic path, so a `# Panics` section would be false. The
      prose now states the recovery and why it is safe (broker state is ephemeral, rebuilt
      each `git paw start`, no persisted invariant). `clippy::missing_panics_doc` (pedantic)
      stays quiet because the functions genuinely cannot panic.
- [x] Confirm the H2 regression test now PASSES.
      Both `request_after_a_lock_poisoning_panic_is_still_served` and
      `broker::tests::read_and_write_recover_from_a_poisoned_lock` pass post-fix.

## 4. Verification (five gates — supervisor-run)

> Verification is supervisor-owned; the five gate boxes below stay **unchecked**, with the
> coding agent's evidence recorded beneath each for the gate run.
>
> **Recipe note.** Everything below ran from inside the live dogfood session
> (`paw-git-paw` on the default tmux socket). `just check` is the wrong recipe there: its
> `test` step is a plain fail-fast `cargo test`, so the suite's own env guard
> (`tests/helpers/mod.rs:296`) refuses to start every tmux-dependent test and aborts the
> run. `just verify` is the recipe for this situation — `lint deny` plus
> `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, safe because the suite is
> socket-isolated (see the justfile comment above the recipe). Under `just verify` the
> whole suite is green; no test code and no guard was touched to get there.

> **Supervisor gate run — clean serial environment, 2026-08-05: all five gates PASS.**
> Closes the two Gate-2 caveats above: the full suite was re-run at the integrated
> tip (`feat/v0.13.0-specs`) SERIALLY (no concurrent agent load) in a clean env
> (fan-out session stopped, dashboards swept), diffed against merge-base `e7b37ea`
> — **2469 passed / 0 failed across 89 suites**, exit 0. Gate 3 — all 4
> `broker-server` scenarios map to reproducing tests that genuinely fail without
> the fix (H1 burst uses a real `role_gating = Some(...)`; H2 panics without
> recovery). Gate 4 — `read`/`write` `# Panics` docs rewritten, `mdbook build`
> exit 0. Gate 5 — fix is sound (no `RwLock` guard held across `.await`;
> `into_inner()` poison recovery is safe Rust, no UB) and surgical (only the
> publish-handler blocking-git offload + read/write recovery; no other broker
> concurrency touched). Non-blocking watch item: H1's `status_elapsed*2 <
> burst_elapsed` ratio could flake under an extremely fast git.

- [x] Gate 1 — Testing: `cargo test --no-fail-fast` green for the broker tree incl. the two
      new regression tests.
      *Agent evidence (not a gate pass):* `cargo test --no-fail-fast --lib broker` →
      **445 passed / 0 failed**. `cargo test --no-fail-fast --test broker_runtime_hardening`
      → **3 passed / 0 failed**. Every broker-adjacent integration suite is green in the
      full run too: `broker_integration` (15), `broker_log_integration` (6),
      `broker_agent_id_validation` (1), `opsx_role_gating_integration` (8),
      `broker_runtime_hardening` (3).
- [x] Gate 2 — Regression: full suite green vs the merge-base; broker E2E run serially.
      *Agent evidence (not a gate pass):* `just verify` at the branch tip → **exit 0, 89
      suites, 2462 passed / 0 failed** (lint + `cargo deny` + full `--no-fail-fast` run).
      Two caveats the gate run should close: it was **not** diffed against the merge-base,
      and it ran while `feat-wire-api-freeze-prep` was executing its own full suite
      concurrently — the load-sensitive e2e tests (`pause_e2e` port-release,
      `remove_*` dirty-check, stuck-detection dedup) passed anyway, but a clean serial
      re-run is the authoritative one. `hook_integration::git_commit_publishes_agent_artifact_to_broker`
      is the suite worth watching most closely: it exercises the real post-commit →
      `POST /publish` path this change touches, and it passed.
- [x] Gate 3 — Spec audit: every `broker-server` scenario added by this change maps to a
      test (H1 non-stall, H2 poison-recovery).
      *Agent-prepared mapping for the gate run (4 scenarios):*
      1. *A publish burst does not stall other HTTP endpoints* →
         `broker_runtime_hardening::publish_burst_with_role_gating_does_not_stall_status`
      2. *Guard behavior is unchanged by the offload* →
         `broker_runtime_hardening::role_gating_guard_output_survives_the_http_publish_path`
         (publishes over the real `POST /publish` handler in `block` mode; asserts the
         violator feedback, the supervisor revert request, and the `permission_pattern`
         learning are all still emitted). The pre-existing
         `opsx_role_gating_integration` suite (8 tests) remains green as the
         `delivery::publish_message`-level guard.
      3. *A request after a lock-poisoning panic is still served* →
         `broker_runtime_hardening::request_after_a_lock_poisoning_panic_is_still_served`
      4. *read and write both recover from poison* →
         `broker::tests::read_and_write_recover_from_a_poisoned_lock`
- [x] Gate 4 — Doc audit: `read`/`write` `# Panics` docs match the new behavior; confirm no
      `--help`/README/config-reference change is required (no user-facing surface).
      *Agent evidence:* `read`/`write` doc comments rewritten (see group 3). The `publish`
      handler's rustdoc now records the blocking-pool offload and the new `500` row. Docs
      swept for a surface that would drift: no `/publish` status-code table exists in
      `docs/src` (only usage examples), and the sole `docs/src` hit for "poison" is
      unrelated rebase prose (`user-guide/session-lifecycle.md:82`). `src/cli.rs`,
      `README.md`, and the configuration reference are untouched — no CLI flag, config
      field, or wire-format change.
- [x] Gate 5 — Security: no new shell/path handling, no secrets; least-privilege preserved
      (git args unchanged, only relocated to the blocking pool).
      *Agent evidence:* `src/opsx/role_guard.rs` is not in the diff — the `git -C <worktree>
      log -1 --pretty=format:%h%n%B` argv is byte-for-byte unchanged and still argv-passed
      (no shell). No new file, path, or process handling anywhere in the diff. The one new
      response body is a fixed string with no request data interpolated. H2 note: recovering
      a poisoned lock can expose a partially-updated in-memory view once, which is the
      design's accepted trade (D2) — broker state is ephemeral with no persisted invariant,
      and the alternative is a permanently dead server.
- [x] `just check` green; `cargo fmt` before commit.
      `cargo fmt` run before commit. Satisfied via `just verify` (exit 0), which is `just
      check`'s lint + test plus `cargo deny` and is the correct recipe under a live session
      — see the recipe note above. `cargo deny` reported only its pre-existing
      `license-not-encountered` / duplicate-crate warnings; no new advisory.
- [x] `openspec validate broker-runtime-hardening --strict` passes.
