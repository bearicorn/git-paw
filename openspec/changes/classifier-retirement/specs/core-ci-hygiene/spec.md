## MODIFIED Requirements

### Requirement: Tmux-spawning tests SHALL use a dedicated tmux socket

Every integration test in `tests/` that directly invokes `tmux` (via `std::process::Command::new("tmux")`) or indirectly spawns tmux through a `git paw` subcommand (e.g. `git paw start`, which calls `src/tmux.rs::TmuxCommand::execute()`) SHALL configure its `std::process::Command` builder so the spawned tmux process uses a tempdir-rooted tmux socket directory, NOT the user's default tmux socket.

The mechanism SHALL be:

- Set the `TMUX_TMPDIR` environment variable on the `Command` builder to the path of a `tempfile::TempDir` owned by the test or its helper.
- Remove the `TMUX` environment variable from the `Command` builder via `env_remove("TMUX")` so the child tmux process does not detect itself as "running inside an existing tmux session" and accidentally attach to the parent's server.
- Remove the `TMUX_PANE` environment variable via `env_remove("TMUX_PANE")` for the same reason.

A shared helper `tests/helpers/mod.rs::tmux_test_env()` (returning a struct that owns the `TempDir` and exposes an `apply(&mut Command)` method, or an equivalent shape) SHALL be the canonical mechanism. Every tmux-spawning test SHALL use that helper rather than open-coding the env-var setup, so the invariant is grep-able and easy to enforce on new tests.

The `TempDir` SHALL outlive the tmux server process it isolates. In practice this means the helper's owning struct SHALL be bound to a local variable in the test (not dropped immediately after construction), and tests that spawn multiple `Command` builders SHALL reuse the same helper instance for all of them when they intend the spawned tmux processes to share a server.

#### Scenario: A test that runs `git paw start` uses a dedicated socket

- **GIVEN** a test in `tests/e2e_tests.rs` (or any other integration test file) that constructs an `assert_cmd::Command` to run `git paw start`
- **WHEN** the test sets up its `Command` builder
- **THEN** the builder SHALL have `TMUX_TMPDIR` set to a path inside a test-owned `TempDir`
- **AND** the builder SHALL have `TMUX` and `TMUX_PANE` removed from its environment
- **AND** the tmux session created by the subprocess SHALL appear on the test-owned socket (`tmux -S <socket_dir>/tmux-<uid>/default ls`) and SHALL NOT appear on the user's default socket (`tmux ls`)

Test: `tests/e2e_tests.rs::broker_session_full_lifecycle` (and every other `git paw start` test) — adapted to apply the helper; verified by listing sessions on both sockets after the test has started the session.

#### Scenario: A test that calls `tmux new-session` directly uses a dedicated socket

- **GIVEN** a test in `tests/sweep_sh_approve.rs` (or any other integration test) that constructs a `std::process::Command::new("tmux")` to run `new-session`
- **WHEN** the test sets up its `Command` builder
- **THEN** the builder SHALL apply `tmux_test_env()` exactly as a `git paw start` test would
- **AND** the resulting session SHALL appear only on the test-owned socket

Test: `tests/sweep_sh_approve.rs::*` — every direct `tmux new-session` site applies the helper.

#### Scenario: Two `Command` builders in the same test share a socket when intended

- **GIVEN** a test that first runs `git paw start` and then runs `git paw stop` against the same session
- **WHEN** the test applies the same `tmux_test_env()` instance to both `Command` builders
- **THEN** both subprocesses SHALL see the same `TMUX_TMPDIR`
- **AND** the second subprocess SHALL be able to find the session the first subprocess created (proving they share a tmux server)

Test: `tests/e2e_tests.rs::broker_session_full_lifecycle` (which runs start → stop → purge against one session).

#### Scenario: A `cargo test` run does not touch the user's default tmux socket

- **GIVEN** a user with a `paw-git-paw` session running on the default tmux socket
- **AND** the collision-guard from the requirement below is satisfied (e.g. the user passes `GIT_PAW_ALLOW_LIVE_SESSION=1` to opt out of the guard for this scenario)
- **WHEN** a full `cargo test --quiet` run completes (pass or fail)
- **THEN** `tmux ls` on the user's default socket SHALL still list `paw-git-paw` with the same window count and pane count it had before the test run
- **AND** no test-created session (matching `paw-unit-test-*`, `paw-e2e-*`, `paw-aa-*`, `paw-test-*`, `paw-rcv-*`, `paw-bootblk-*`, or `paw-repo`) SHALL appear on the user's default socket at any point during or after the test run

Test: a maintainer-run smoke check during the change rollout — capture `tmux ls` output before/after a full `cargo test --quiet` with the live session attached. Documented in `tasks.md` as the rollout verification step.
