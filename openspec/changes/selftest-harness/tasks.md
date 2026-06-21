## 1. CLI surface

- [ ] 1.1 Add `Selftest` variant to `Command` in `src/cli.rs` with `about` + `long_about` (including a usage example) and no required args
- [ ] 1.2 Add the `selftest` dispatch arm in `src/main.rs` that calls into the new harness and maps its verdict to the process exit code (0 = pass, non-zero = fail)
- [ ] 1.3 Add unit tests for parsing: `selftest` parses to `Command::Selftest`; `selftest --help` shows the description + example

## 2. Selftest harness module

- [ ] 2.1 Create `src/selftest.rs` with a module-level doc comment and a public entry point returning a pass/fail verdict that names the failing step on failure
- [ ] 2.2 Implement the isolation recipe: per-run private tmux socket (tempdir-rooted `TMUX_TMPDIR`), `env_remove("TMUX")` + `env_remove("TMUX_PANE")` on every child `Command`
- [ ] 2.3 Implement `pick_broker_port()` in the harness binding `127.0.0.1:0` and reading back the OS-assigned port (no PID-mod scheme)
- [ ] 2.4 Implement throwaway-repo creation under `.git-paw/tmp/` (git init + one base commit) with a stale-dir sweep before creation
- [ ] 2.5 Launch the session in detached/non-TTY mode with a dummy CLI (`cat`/`sh`), asserting no real AI CLI is spawned
- [ ] 2.6 Drive the lifecycle: start → add an agent worktree → observe roster grows → remove → observe roster shrinks → stop/purge
- [ ] 2.7 Implement cleanup on BOTH success and failure paths (kill the private-socket session, remove the `.git-paw/tmp/` throwaway repo)
- [ ] 2.8 Skip-with-message when tmux is unavailable; exit non-zero only on an actual lifecycle failure

## 3. F8 correction: ephemeral broker port across tests

- [ ] 3.1 Audit `tests/` for every broker-port helper using `BASE + (std::process::id() % N)` (e.g. `learnings_mode_integration`, `mcp_e2e`, `hook_integration`, `e2e_qualitative_learnings`, `broker`, `broker_integration`, `broker_agent_id_validation`, `conflict_detection_integration`, `e2e_learnings_aggregator_disabled`, `e2e_learnings_multi_session`)
- [ ] 3.2 Migrate each to the canonical ephemeral helper (`bind 127.0.0.1:0` → read back port), matching `tests/e2e_supervisor_stop.rs::pick_broker_port`
- [ ] 3.3 Grep-verify no broker-port helper computes `BASE + (process::id() % N)` after migration

## 4. Tests

- [ ] 4.1 Integration test: `git paw selftest` exits 0 and prints a pass indication on a healthy build (skips if tmux unavailable)
- [ ] 4.2 Integration test: `selftest` session appears only on its private socket, never on the default socket
- [ ] 4.3 Integration test: `selftest` boots with a dummy CLI and spawns no real AI CLI; throwaway repo lives under `.git-paw/tmp/` and is removed afterward
- [ ] 4.4 Integration test: `selftest` observes the roster grow on add and shrink on remove
- [ ] 4.5 Integration test: `selftest` reports non-zero + names the failing step when a lifecycle step fails (inject a forced failure)
- [ ] 4.6 Test: the ephemeral-port helper returns a free, immediately-bindable port and two helper calls yield distinct ports under concurrency
- [ ] 4.7 Run two `cargo test` shards concurrently (or under `cargo llvm-cov`) and confirm no "address already in use" broker-port failure

## 5. Docs

- [ ] 5.1 Update `git paw --help` / root `after_help` quick-start to mention `selftest`
- [ ] 5.2 Add `selftest` to the README CLI table with a one-line description
- [ ] 5.3 Add an mdBook chapter (or section) under `docs/src/` documenting `git paw selftest` and the isolation recipe; `mdbook build docs/` succeeds
- [ ] 5.4 Update the configuration/architecture docs if the new module changes the documented module map

## 6. Quality gates

- [ ] 6.1 `just check` (fmt + clippy + all tests) passes; no `unwrap()`/`expect()` in non-test code; all public items documented
- [ ] 6.2 `just deny` passes (no new dependencies introduced)
- [ ] 6.3 Every spec scenario in `specs/selftest/spec.md` and `specs/test-isolation/spec.md` maps to at least one test
