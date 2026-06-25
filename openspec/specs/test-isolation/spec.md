# test-isolation Specification

## Purpose
TBD - created by archiving change test-tmux-isolation. Update Purpose after archive.
## Requirements
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

- **GIVEN** a test in `tests/auto_approve_integration.rs` or `tests/prompt_inbox_integration.rs` that constructs a `std::process::Command::new("tmux")` to run `new-session`
- **WHEN** the test sets up its `Command` builder
- **THEN** the builder SHALL apply `tmux_test_env()` exactly as a `git paw start` test would
- **AND** the resulting session SHALL appear only on the test-owned socket

Test: `tests/auto_approve_integration.rs::*` and `tests/prompt_inbox_integration.rs::*` — every direct `tmux new-session` site applies the helper.

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

### Requirement: Tests SHALL refuse to run when a live paw-* session exists on the user's default tmux socket

A helper `tests/helpers/mod.rs::guard_against_live_session()` SHALL be called as the first action of `tests/helpers/mod.rs::setup_test_repo()`. The guard SHALL:

- Spawn `tmux ls` against the user's default socket. The `tmux ls` subprocess SHALL have `TMUX_TMPDIR`, `TMUX`, and `TMUX_PANE` explicitly removed via `Command::env_remove` so the guard always inspects the real default socket regardless of whether a parallel test in the same binary has mutated those env vars via `apply_to_process()`. This is required because Rust's test runner shares `std::env` across parallel test threads — without `env_remove`, a `#[serial]` test elsewhere in the binary that has called `apply_to_process()` would cause the guard to inspect its isolated socket instead, falsely reporting in-flight test sessions as default-socket leaks.
- Parse the stdout for any line whose session-name field starts with `paw-` (e.g. `paw-git-paw`, `paw-other-project`).
- If at least one such session is found, panic with a message that:
  - Names every offending session.
  - Recommends either killing the live session(s) with `tmux kill-session -t <name>` or running a targeted `cargo test --test <name>` invocation that doesn't depend on `setup_test_repo()`.
  - Documents the escape hatch `GIT_PAW_ALLOW_LIVE_SESSION=1` for maintainers who explicitly accept the risk.
- If no such session is found, or if `GIT_PAW_ALLOW_LIVE_SESSION=1` is set in the test process's environment, return without panicking.

The guard's `tmux ls` invocation SHALL use the user's default socket (not the per-test socket), because the purpose of the guard is to detect live sessions on the *real* host, not on a per-test isolated socket.

The guard SHALL NOT panic when `tmux ls` exits non-zero with the "no server running" message — that is the expected case when the user has no live sessions.

#### Scenario: A live paw-* session causes setup_test_repo to panic

- **GIVEN** a developer with `paw-git-paw` running on the user's default tmux socket
- **AND** `GIT_PAW_ALLOW_LIVE_SESSION` is not set
- **WHEN** any integration test that calls `setup_test_repo()` runs
- **THEN** the test SHALL panic during setup with a message naming `paw-git-paw`
- **AND** the panic message SHALL include the kill-session command and the `cargo test --test <name>` recommendation
- **AND** no tmux subprocess SHALL be spawned by the test

Test: `tests/helpers/mod.rs::tests::guard_panics_when_live_paw_session_exists` — uses a test-owned tmux socket dir, creates a `paw-guard-test` session on it, sets `TMUX_TMPDIR` to that dir for the guard call only (to simulate "the user's default socket has a paw-* session" without actually touching the user's default socket), asserts the guard panics with the expected message.

#### Scenario: No live paw-* session lets setup_test_repo proceed

- **GIVEN** a developer with no `paw-*` sessions on the user's default tmux socket
- **WHEN** an integration test that calls `setup_test_repo()` runs
- **THEN** the guard SHALL return without panicking
- **AND** `setup_test_repo()` SHALL proceed to create the test repo as before

Test: every integration test in `tests/` continues to pass on CI (where no `paw-*` sessions exist by construction).

#### Scenario: No tmux server running lets setup_test_repo proceed

- **GIVEN** a developer with no tmux server running on the user's default socket at all
- **WHEN** an integration test that calls `setup_test_repo()` runs
- **THEN** `tmux ls` SHALL exit non-zero with the "no server running" message
- **AND** the guard SHALL interpret that as "no live sessions" and return without panicking

Test: `tests/helpers/mod.rs::tests::guard_returns_when_no_tmux_server` — uses an empty test-owned socket dir, calls the guard, asserts no panic.

#### Scenario: GIT_PAW_ALLOW_LIVE_SESSION=1 bypasses the guard

- **GIVEN** a developer with `paw-git-paw` running on the user's default socket
- **AND** `GIT_PAW_ALLOW_LIVE_SESSION=1` is set in the test process's environment
- **WHEN** an integration test that calls `setup_test_repo()` runs
- **THEN** the guard SHALL return without panicking
- **AND** the test SHALL proceed (relying on per-test `TMUX_TMPDIR` isolation to protect the live session)

Test: `tests/helpers/mod.rs::tests::guard_honours_allow_live_session_env`.

### Requirement: Tests invoking git paw subcommands SHALL override HOME and XDG_DATA_HOME

Every integration test that constructs an `assert_cmd::Command` (via the test-helper `cmd()` function or equivalent) to run a `git paw` subcommand against the compiled binary SHALL configure the `Command` builder to:

- Set `HOME` to a `tempfile::TempDir`-rooted path that is unique to the test and whose lifetime exceeds the subprocess.
- Remove `XDG_DATA_HOME` via `env_remove("XDG_DATA_HOME")`.

The pattern SHALL be the existing one demonstrated by `tests/cli_tests.rs:278-279`:

```rust
let fake_home = TempDir::new().expect("home tempdir");
let output = cmd()
    .current_dir(repo_path)
    .env("HOME", fake_home.path())
    .env_remove("XDG_DATA_HOME")
    .args(["<subcommand>", ...])
    .output()
    .expect("run <subcommand>");
```

This ensures the subprocess resolves `crate::dirs::data_dir()` (and the sessions directory below it) into the test-owned `TempDir`, not into the user's real `~/Library/Application Support/git-paw/` (macOS) or `~/.local/share/git-paw/` (Linux).

The audit SHALL cover every `cmd().args(["<subcommand>", ...])` invocation in `tests/` and SHALL specifically fix `tests/e2e_tests.rs::broker_session_full_lifecycle` (around line 874 in the v0.5.0 baseline, the `["start", "--cli", "echo", "--branches", ...]` call), the matching `stop` and `purge --force` calls in the same test, and any other call sites the audit surfaces. Calls that already follow the pattern (e.g. `tests/cli_tests.rs::test_purge_no_unmerged_runs_without_warning`) SHALL be left as-is.

#### Scenario: `broker_session_full_lifecycle` does not write into the real sessions directory

- **GIVEN** a developer with `~/Library/Application Support/git-paw/sessions/` populated with the live `paw-git-paw.json` (and other real session files)
- **WHEN** `cargo test --test e2e_tests broker_session_full_lifecycle` runs
- **THEN** the test SHALL apply the HOME/XDG override pattern to every `cmd()` builder in the test (start, stop, purge)
- **AND** the test SHALL NOT create, modify, or delete any file inside the user's real `~/Library/Application Support/git-paw/sessions/`
- **AND** the live `paw-git-paw.json` file SHALL be byte-identical before and after the test runs

Test: maintainer-run smoke check during the change rollout — record the directory contents before/after the test runs. Documented in `tasks.md` as the rollout verification step.

#### Scenario: Every `git paw start` test in `tests/` applies the override

- **GIVEN** the v0.5.0 source tree after this change is applied
- **WHEN** the audit step in `tasks.md` is complete
- **THEN** every `assert_cmd::Command` builder in `tests/*.rs` that invokes a `git paw` subcommand SHALL either (a) have `.env("HOME", <tempdir>)` and `.env_remove("XDG_DATA_HOME")` applied, or (b) be explicitly documented in a code comment as needing the user's real HOME (no such case exists today and none is expected)
- **AND** the audit's grep query `grep -n 'cmd()\|args(\["start"\|args(\["stop"\|args(\["purge"' tests/*.rs` SHALL be cross-checked against the test bodies to confirm

Test: covered by `cargo test --test e2e_tests` plus the spec-audit grep documented in `tasks.md` step 3.

### Requirement: Tests SHALL select the broker port via an OS-assigned ephemeral port

Every integration test and the `selftest` harness that needs a free TCP port for the broker (or any helper named `pick_broker_port`, `broker_port`, `pick_port`, or equivalent) SHALL obtain that port by binding `127.0.0.1:0`, reading back the OS-assigned local port, and releasing the listener so the broker can claim the port. The helper SHALL NOT derive the port from the process id via a `BASE + (std::process::id() % N)` scheme.

The former scheme `24_000 + (std::process::id() % 200)` (and its siblings such as `BASE + (process::id() % 100)`, `% 1000`, `% 5000`) keyed the port on the process id modulo a small constant, yielding at most `N` distinct ports. `N` concurrent `cargo test` runs collided modulo that constant, the broker failed to bind with "address already in use", and the verify run reported a false-negative failure. This was the real cause of the in-session verify flakes — not a live-session collision. An OS-assigned ephemeral port is collision-proof at any concurrency because the kernel guarantees each `bind 127.0.0.1:0` returns a port not currently in use.

The canonical implementation SHALL be the helper already present at `tests/e2e_supervisor_stop.rs::pick_broker_port`:

```rust
fn pick_broker_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}
```

Tests that previously used a PID-derived port base SHALL be migrated to this helper (or an equivalent ephemeral-bind call). There is an inherent, accepted race window between releasing the listener and the broker binding the port; because the window is microseconds and the port is OS-assigned (not contended by other test workers), this is dramatically less collision-prone than the PID-mod scheme and is the same trade-off the broker's own free-port discovery already makes.

#### Scenario: The broker-port helper returns a free, OS-assigned port

- **GIVEN** a test (or the `selftest` harness) needs a broker port
- **WHEN** it calls the ephemeral-port helper
- **THEN** the helper SHALL bind `127.0.0.1:0`, read back the kernel-assigned port, and release the listener
- **AND** the returned port SHALL be one the broker can immediately bind

#### Scenario: Concurrent test runs do not collide on the broker port

- **GIVEN** two or more `cargo test` invocations run concurrently on the same host (e.g. under `cargo llvm-cov` or parallel CI shards)
- **WHEN** each invocation allocates a broker port via the ephemeral-port helper
- **THEN** each invocation SHALL receive a distinct OS-assigned port
- **AND** no invocation SHALL fail the broker bind with "address already in use" caused by a PID-modulo collision

#### Scenario: No test relies on the PID-modulo port scheme

- **GIVEN** the source tree after this change is applied
- **WHEN** the broker-port helpers across `tests/` are audited
- **THEN** no broker-port helper SHALL compute its port as `BASE + (std::process::id() % N)`
- **AND** every broker-port helper SHALL obtain its port by binding `127.0.0.1:0`

