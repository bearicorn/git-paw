## Why

Three gaps between "the suite passes" and "the suite can be trusted" surfaced in the v0.19.0
release review and the v0.20.0 dogfood:

- **The MSRV is declared but never built.** `Cargo.toml` says `rust-version = "1.96"`, and
  README, CONTRIBUTING and AGENTS.md repeat it, but every CI job installs `stable`. A change that
  uses a newer std API passes CI and breaks anyone building on 1.96, including the cargo-dist
  release runner the MSRV is pinned to.
- **Three tmux timing tests flake under load**, each passing alone:
  `coding_agents_resolve_flags_from_agent_approval`, `stop_revokes_the_drive_loop_binding`, and
  `agent_pane_liveness_e2e::doctor_warns_that_the_agent_panes_exited_with_the_start_remedy`
  (DV-18). Nothing records them, so each failure is re-diagnosed from scratch or re-run until it
  goes green.
- **The e2e suite leaks processes (DV-21).** About twenty `git-paw __dashboard` / `__broker`
  processes from earlier test runs were still alive days later. `TmuxTestEnv`'s `Drop` kills its
  tmux server, but a test that is killed or times out never runs `Drop`. A related race was fixed at
  the v0.20.0 release (c286737): two fixtures read liveness while their pane still showed a shell
  instead of `sleep`. The fix was copied into each fixture, and other fixtures that start a
  stand-in process in a pane (`pause_e2e`, `agent_pane_liveness_e2e`) still have the race or their
  own copy.

## What Changes

- **An MSRV CI job.** `.github/workflows/ci.yml` gains a job that installs the toolchain named by
  `rust-version` and runs `cargo +1.96 check --all-targets --locked`. A convention test fails when
  the job's toolchain and `rust-version` disagree, so raising one without the other fails the suite.
- **A register of known timing flakes.** One file under `tests/` lists each known flaky test with
  its symptom and the date it was first seen. It starts with the three tests above. A convention
  test fails when an entry names a test that no longer exists. A test leaves the register only when
  its waits poll the condition it asserts and it passes ten consecutive runs under parallel load.
- **Leaked test processes are reaped.** Every test-owned tmux socket directory and every directly
  spawned `git-paw` child records the test process that owns it. The next test run kills the tmux
  servers and children whose owner is dead. A live concurrent run's processes, and the user's
  default socket, are never touched.
- **One shared wait for a pane's stand-in command.** `tests/helpers` gains
  `wait_for_pane_command`. Every fixture that starts a stand-in process in a pane and then reads
  liveness calls it first. The two copies added in c286737 and the inline wait in
  `agent_pane_liveness_e2e` move to it, and `pause_e2e` gains it.

No production code changes. No CLI, config or persisted-format change.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `core-ci-hygiene`: ADDED the MSRV job and its drift check, the flake register and its exit
  criterion, reaping of processes left by a killed test, and the shared pane-command wait.

## Impact

- **CI:** `.github/workflows/ci.yml` (new `msrv` job). This adds one job of about a minute on a
  warm cache. It gets its own cache key, because a 1.96 build cannot share artifacts with stable.
- **Tests:**
  - `tests/helpers/mod.rs`: owner marker in `TmuxTestEnv`, the reaper, `wait_for_pane_command`, and
    a shared `wait_until` that replaces the copies in six test files;
  - a shared spawn helper for long-lived `git-paw` children (`tests/broker_host_e2e.rs::spawn_host`,
    the two `spawn_unattended` copies in `tests/e2e_supervisor_stop.rs` and
    `tests/unattended_drive_e2e.rs`, and the MCP server fixtures move onto it);
  - the flake register, plus convention tests for the register, the MSRV job, and the
    stand-in wait;
  - the fixtures in `tests/cmd_golden_output.rs`, `tests/command_library.rs`,
    `tests/agent_pane_liveness_e2e.rs` and `tests/pause_e2e.rs`.
- **Docs:** `CONTRIBUTING.md` and `docs/src/contributing.md`: the MSRV job, the flake register,
  and what to do when a registered test fails.
- **Production code:** none. `src/dashboard.rs::orphaned` (exit when reparented to init) is left
  as is; it does not fire for a pane process whose tmux server is still alive, which is exactly
  the leak.
- **Security / safety:** the reaper only signals processes recorded in a test-owned marker under
  the system temp directory, and only when the marker's owner process is gone. It never lists or
  kills by process name.
- **Cross-version dependencies:** none. `session-state-locking` (this release) adds concurrency
  e2e tests that should use the shared spawn helper and pane-command wait from the start.
