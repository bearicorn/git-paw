## Context

git-paw orchestrates AI coding CLIs across git worktrees via tmux and a broker HTTP server. The session-management surface (`start`, `add`, `remove`, `stop`, `purge`) is the riskiest plumbing because it spans modules — `git.rs` (worktrees), `tmux.rs` (panes/sockets), `broker/` (HTTP roster), and `session.rs` (state JSON). Today the only way to exercise the full path is to boot a real session with a real AI CLI in an interactive terminal. CI cannot do that, and developers fall back to ad-hoc dogfood shell incantations.

The dogfood recipe for safe isolation is well established and already encoded piecemeal in the test suite:
- `tests/helpers/mod.rs::tmux_test_env()` strips `TMUX`/`TMUX_PANE` and sets a tempdir-rooted `TMUX_TMPDIR` (per the existing `test-isolation` spec).
- `tests/e2e_supervisor_stop.rs::pick_broker_port()` already binds `127.0.0.1:0` for an ephemeral broker port.
- `git paw start` already supports detached (non-TTY) launch and accepts `--cli <command>`, so a dummy CLI like `cat` boots a session without an LLM.

This change packages that recipe as a shipped `git paw selftest` subcommand and, as a rider, fixes the F8 root cause: the remaining PID-mod-N broker-port helpers that cause "address already in use" verify flakes under concurrency.

## Goals / Non-Goals

**Goals:**
- Ship `git paw selftest` that runs start → add → remove → stop against a throwaway repo and a dummy CLI, with no LLM and no TTY, returning a clear pass/fail.
- Isolate every external resource: private tmux socket, ephemeral broker port, throwaway repo under `.git-paw/tmp/`.
- Make the add/remove roster transitions observable, closing git-paw-add's deferred live-verification tasks.
- Replace PID-mod-N broker-port selection in the test harness with OS-assigned ephemeral ports.

**Non-Goals:**
- No real AI CLI integration, no network calls to any model provider.
- Not a replacement for the unit/integration test suite — `selftest` is a runtime smoke check of the orchestration plumbing, complementary to `cargo test`.
- No new TUI/dashboard surface; `selftest` is non-interactive and prints a textual verdict.
- Not changing the broker's own production free-port discovery — only the test/selftest port helper.

## Decisions

### Decision: dummy CLI is a plain command (`cat`/`sh`), launched in detached mode
`git paw start --cli cat` boots a tmux session whose panes run `cat`, which holds the pane open without producing output or requiring input. Combined with the existing non-TTY launch path (stdin redirected from `/dev/null`), the session boots and exits cleanly with the attach hint, no agent process and no terminal required.

*Alternative considered:* a bespoke fake-LLM binary. Rejected — adds a build artifact and a dependency for zero benefit; `cat`/`sh` are universally present on supported platforms (macOS/Linux/WSL).

### Decision: throwaway repo lives under `.git-paw/tmp/`
Namespacing under the repo's own `.git-paw/` working tree keeps selftest artifacts discoverable and gitignorable, mirrors the v0.8.0 embedded-worktree convention, and makes cleanup a single directory removal. The harness `git init`s a minimal repo there with one commit so worktree operations have a base.

*Alternative considered:* a system tempdir. Rejected — less discoverable for debugging, and inconsistent with git-paw's embedded-placement direction.

### Decision: reuse the env-stripping + private-socket recipe, do not invent a new one
The harness applies the same `TMUX`/`TMUX_PANE` removal and per-run `TMUX_TMPDIR` that `tests/helpers/mod.rs::tmux_test_env()` proved out. Keeping one recipe means the invariant stays grep-able and the selftest exercises the same isolation users' own tests rely on. The harness lives in `src/selftest.rs` (shipped code, so the recipe is reusable at runtime, not just in `tests/`).

### Decision: ephemeral broker port via `bind 127.0.0.1:0`
The canonical helper is the one already in `tests/e2e_supervisor_stop.rs`. The kernel guarantees each bind returns a currently-unused port; releasing the listener immediately before the broker binds leaves a microsecond race window that is acceptable (and strictly better than PID-mod-N, which deterministically collides at concurrency ≥ N-cycle).

*Alternative considered:* widen the PID-mod constant (e.g. `% 50000`). Rejected — still keyed on PID, still collides when two runs share a PID-low-bits value; ephemeral binding removes the failure mode entirely.

### Decision: roster observation goes through the broker/session state, not log scraping
After start, the harness adds an agent and reads the observable roster (broker `/status` roster or session-state roster) to assert growth, then removes and asserts shrink. This makes the add/remove scenarios behavioral (observable roster in → roster out) rather than asserting internal calls.

## Risks / Trade-offs

- [Ephemeral-port release-then-bind race] → The window between releasing the `127.0.0.1:0` listener and the broker binding is microseconds and the port is not contended by sibling test workers; this is the same trade-off the broker's own discovery makes. Accepted.
- [`cat`/`sh` availability on exotic platforms] → git-paw supports macOS/Linux/WSL only, where `cat`/`sh` are guaranteed present. The harness can fall back between them if needed.
- [Leftover `.git-paw/tmp/` artifacts if the harness is killed mid-run] → Cleanup runs on both success and failure paths; a stale-dir sweep at harness start removes any prior aborted run's directory before creating a fresh one.
- [selftest masking a real failure by skipping when tmux is absent] → Like the rest of the e2e suite, selftest reports "skipped: tmux not available" and exits non-zero only on an actual lifecycle failure; CI runners have tmux installed, so a skip never silently passes there.

## Migration Plan

1. Add `Command::Selftest` to `src/cli.rs` and dispatch in `src/main.rs`.
2. Implement `src/selftest.rs` reusing the isolation recipe.
3. Migrate the remaining PID-mod-N broker-port helpers in `tests/` to the ephemeral helper; `tests/e2e_supervisor_stop.rs::pick_broker_port` is the reference.
4. Add docs (`--help`, README CLI table, mdBook chapter) and the OpenSpec test mappings.

Rollback is trivial: the subcommand is additive and the port-helper change only affects test code, so reverting either is independent and non-breaking.
