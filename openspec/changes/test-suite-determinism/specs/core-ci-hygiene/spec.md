## MODIFIED Requirements

### Requirement: Tmux-spawning tests SHALL use a dedicated tmux socket

Every integration test in `tests/` that directly invokes `tmux` (via `std::process::Command::new("tmux")`) or indirectly spawns tmux through a `git paw` subcommand (e.g. `git paw start`, which reaches tmux through the `src/tmux/` module) SHALL configure its `std::process::Command` builder so the spawned tmux process uses a tempdir-rooted tmux socket directory, NOT the user's default tmux socket.

The mechanism SHALL be:

- Set the `TMUX_TMPDIR` environment variable on the `Command` builder to the path of a `tempfile::TempDir` owned by the test or its helper.
- Remove the `TMUX` environment variable from the `Command` builder via `env_remove("TMUX")` so the child tmux process does not detect itself as "running inside an existing tmux session" and accidentally attach to the parent's server.
- Remove the `TMUX_PANE` environment variable via `env_remove("TMUX_PANE")` for the same reason.
- Pin the shell the test-owned tmux server starts in new panes to `/bin/sh`, so a pane never runs the developer's interactive shell or its per-user startup files and a command typed into a pane behaves the same on every machine.

A shared helper `tests/helpers/mod.rs::tmux_test_env()` (returning a struct that owns the `TempDir` and exposes an `apply(&mut Command)` method, or an equivalent shape) SHALL be the canonical mechanism. Every tmux-spawning test SHALL use that helper rather than open-coding the env-var setup, so the invariant is grep-able and easy to enforce on new tests. A test that addresses an explicitly named private socket (`tmux -L <name>` or `tmux -S <path>`) on every tmux invocation also counts as isolated.

The `TempDir` SHALL outlive the tmux server process it isolates. In practice this means the helper's owning struct SHALL be bound to a local variable in the test (not dropped immediately after construction), and tests that spawn multiple `Command` builders SHALL reuse the same helper instance for all of them when they intend the spawned tmux processes to share a server.

Conformance SHALL be enforced by a standing test rather than by a one-time audit. The suite SHALL fail when a file in `tests/` spawns `tmux`, or invokes the compiled binary with a subcommand that reaches tmux, without applying the helper (or an explicit private socket) and without an accompanying comment documenting why it needs the default socket. The rule regressed after its original audit passed: five files reached the default socket again, and `just verify` documents the suite as socket-isolated on the strength of it.

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
- **AND** no test-created session (matching `paw-unit-test-*`, `paw-e2e-*`, `paw-aa-*`, `paw-test-*`, `paw-rcv-*`, `paw-bootblk-*`, `paw-pause-*`, or `paw-repo`) SHALL appear on the user's default socket at any point during or after the test run

Test: a maintainer-run smoke check during the change rollout — capture `tmux ls` output before/after a full `cargo test --quiet` with the live session attached. Documented in `tasks.md` as the rollout verification step.

#### Scenario: A test file that reaches the default socket fails the suite

- **GIVEN** a file in `tests/` that runs `Command::new("tmux")`, or invokes the binary with `start`, `pause`, `stop`, `add`, `remove` or `attach`, without applying `tmux_test_env` or an explicit private socket
- **AND** no comment documenting that it needs the default socket
- **WHEN** the suite runs
- **THEN** the tmux-isolation convention test SHALL fail
- **AND** its failure message SHALL name the offending file and the construction it matched

Test: `tests/tmux_isolation_conventions.rs::every_tmux_spawn_is_socket_isolated`, verified to fail when the helper is removed from one file.

#### Scenario: Previously non-conforming files conform

- **GIVEN** `tests/pause_e2e.rs`, `tests/agent_pane_liveness_e2e.rs`, `tests/stop_confirmation_test.rs`, `tests/default_branch_fallback_integration.rs` and `tests/rebase_integration.rs`
- **WHEN** the tmux-isolation convention test runs
- **THEN** it SHALL pass for every one of them
- **AND** running their tests next to a live `paw-*` session SHALL create no session on the default socket

Test: `tests/tmux_isolation_conventions.rs::every_tmux_spawn_is_socket_isolated`.

#### Scenario: A pane in a test-owned server runs the pinned shell

- **GIVEN** a test that applies `tmux_test_env` and starts a session whose pane receives a typed command
- **AND** a developer whose login shell and per-user startup files differ from `/bin/sh`
- **WHEN** the pane starts
- **THEN** the pane SHALL run `/bin/sh` without the developer's per-user startup files
- **AND** a command typed into the pane SHALL run with the environment the test set

Test: `tests/helpers/mod.rs::tests::isolated_pane_runs_the_pinned_shell` — starts a pane on a private server and reads back the shell it runs.

## ADDED Requirements

### Requirement: Test waits are condition-based with a single hang ceiling

An integration test that waits for an asynchronous outcome SHALL wait on the observable condition the assertion depends on — a file appearing, a process exiting, an HTTP endpoint answering, a subprocess's own outcome line — and SHALL proceed the moment it holds. A shared helper `tests/helpers/mod.rs::wait_until` SHALL implement this: it SHALL return as soon as the condition holds and, if a single suite-wide hang ceiling elapses first, SHALL fail with a message naming the awaited condition, the time waited, and any diagnostics the caller attached (for example a pane capture). No test assertion SHALL depend on an operation finishing inside a wall-clock budget; the ceiling exists only to turn a hang into a failure. An `assert_cmd` `.timeout()` SHALL be set to the same ceiling, and a subprocess killed by it SHALL be reported as hung rather than as a failed assertion on its output.

A test that asserts a component shut down SHALL infer the shutdown from the owning process exiting, then check the consequence (a refused connection, a removed file), rather than by sampling the consequence on a timer. A test SHALL assert the precondition it relies on (for example that a broker answers `GET /status`) before driving the operation under test.

#### Scenario: The wait returns as soon as the condition holds

- **GIVEN** a condition that becomes true after a short delay
- **WHEN** a test waits on it with `wait_until`
- **THEN** the call SHALL return once the condition holds, without waiting for the ceiling

Test: `tests/helpers/mod.rs::tests::wait_until_returns_when_condition_holds` — uses an injected clock.

#### Scenario: A hang is reported with what was awaited

- **GIVEN** a condition that never becomes true
- **WHEN** a test waits on it with `wait_until` and a description
- **THEN** the call SHALL fail once the hang ceiling elapses
- **AND** the failure message SHALL contain the description and the time waited

Test: `tests/helpers/mod.rs::tests::wait_until_names_the_condition_on_hang` — uses an injected clock, so it runs instantly.

#### Scenario: A slow but correct supervisor launch passes

- **GIVEN** `git paw start --supervisor` taking longer than ten seconds to build its session on a loaded machine, and returning on its own with the attach hint
- **WHEN** `cmd_supervisor_returns_immediately_with_attach_hint` runs
- **THEN** the test SHALL pass
- **AND** if the launch blocked on an attach instead, the test SHALL fail because the process exited non-zero or was reported hung, not because a ten-second budget elapsed

Test: `tests/e2e_supervisor_returns.rs::cmd_supervisor_returns_immediately_with_attach_hint`.

#### Scenario: Pause is asserted through the broker host's exit

- **GIVEN** a session whose broker answers `GET /status`
- **WHEN** `git paw pause` runs and the test waits for the broker host process to exit
- **THEN** a connection to the broker port SHALL then be refused
- **AND** the test SHALL NOT decide the broker stopped from a connect attempt that merely timed out

Test: `tests/pause_e2e.rs::pause_detaches_and_stops_broker`.

#### Scenario: The provisioning-order test waits on the probe, not on a budget

- **GIVEN** the env-provisioning launch test whose probe CLI records its worktree listing when it starts
- **WHEN** the machine is loaded and the probe starts late
- **THEN** the test SHALL wait for the recorded listing and assert on it once it exists
- **AND** if the probe never starts, the failure SHALL include the probe pane's capture

Test: `tests/worktree_env_provisioning.rs::provisioned_files_are_present_before_the_agent_cli_starts`.

### Requirement: Tests that run a bundled script pin the git-paw it resolves

A test that executes a bundled helper script which resolves `git paw` from `PATH` (today `sweep.sh`, through `_paw_common.sh::discover_prompt_profile_field`, which runs `git paw __prompt-profile`) SHALL control which binary the script resolves: either the binary under test, by prepending its directory to the child's `PATH`, or no `git-paw` at all, by removing every directory holding one, when the test exercises the script's fallback. A test SHALL NOT let the script resolve a `git-paw` installed on the developer's machine, whose version and behaviour differ from the code under test. A shared helper in `tests/helpers/mod.rs` SHALL provide both forms, and a standing test SHALL fail when a test runs such a script without one of them.

#### Scenario: Results do not depend on an installed git-paw

- **GIVEN** a developer machine with an older `git-paw` release installed on `PATH`
- **AND** a CI runner with no `git-paw` on `PATH`
- **WHEN** the `sweep_sh_*` tests run on both
- **THEN** each test SHALL produce the same result on both

Test: `tests/sweep_sh_stuck_detection.rs` and the other `sweep_sh_*` files, run with a decoy `git-paw` placed first on the ambient `PATH`.

#### Scenario: A script test without a pinned binary fails the suite

- **GIVEN** a test that runs `sweep.sh` with the ambient `PATH`
- **WHEN** the suite runs
- **THEN** the script-binary convention test SHALL fail and SHALL name the offending file

Test: `tests/script_binary_conventions.rs::every_script_test_pins_git_paw`.

### Requirement: Test stub servers live for their fixture and frame requests completely

A stub HTTP server a test uses in place of the broker SHALL accept connections until its owning fixture is dropped and SHALL NOT stop on a timer of its own. It SHALL read each request's headers and then exactly `Content-Length` body bytes before recording the body, and SHALL respond without waiting on anything else. A single shared stub in `tests/helpers/mod.rs` SHALL replace the per-file copies. A test that asserts on recorded bodies SHALL first assert the client's own report of the request's outcome (for `sweep.sh`, its `published`, `deduped` or `publish failed` line), so a transport failure is reported as a transport failure. A test that needs time to have passed (a dedup or progress window) SHALL pin it through seeded state, not by waiting.

#### Scenario: A request late in a slow test is recorded

- **GIVEN** a stub created at the start of a test that runs for longer than any former stub lifetime
- **WHEN** the client publishes near the end of the test
- **THEN** the stub SHALL record the body

Test: `tests/helpers/mod.rs::tests::stub_broker_records_until_dropped` — publishes after the fixture has been idle, without sleeping for the old lifetime (it asserts the stub has no timer, by dropping it and observing a refused connection only then).

#### Scenario: A request delivered in pieces is recorded whole

- **GIVEN** a client that sends the headers and the body of one request in separate writes
- **WHEN** the stub reads the request
- **THEN** the recorded body SHALL equal the full body the client sent

Test: `tests/helpers/mod.rs::tests::stub_broker_reads_the_full_body`.

#### Scenario: The dedup test asserts its precondition

- **GIVEN** `repeated_detection_is_deduped`
- **WHEN** the first sweep runs
- **THEN** the test SHALL assert the sweep reported `published` before running the second sweep
- **AND** the second sweep SHALL report `deduped` with exactly one body recorded

Test: `tests/sweep_sh_stuck_detection.rs::repeated_detection_is_deduped`.

#### Scenario: A window expiry is pinned through seeded state

- **GIVEN** a dedup entry seeded with a timestamp older than the dedup window
- **WHEN** a sweep detects the same shape again
- **THEN** it SHALL publish again
- **AND** the test SHALL NOT wait for any real time to pass

Test: `tests/sweep_sh_stuck_detection.rs::expired_dedup_entry_republishes`.

### Requirement: Processes a test starts exit before the test returns

Every process a test starts — directly, or inside a test-owned tmux server — SHALL have exited before the test returns. The tmux isolation helper SHALL, when dropped, terminate its private server and wait (by the condition-based wait) until every process that ran in the server's panes has exited; a process still alive at the hang ceiling SHALL be killed and reported by name. No instrumented `git-paw` process started by a test SHALL outlive the test binary that started it.

#### Scenario: A dashboard pane is gone when the helper is dropped

- **GIVEN** a test-owned server whose pane runs `git-paw __dashboard`
- **WHEN** the isolation helper is dropped
- **THEN** that `git-paw` process SHALL have exited before the drop returns

Test: `tests/helpers/mod.rs::tests::dropping_the_env_reaps_pane_processes`.

#### Scenario: A process that ignores hang-up is reported

- **GIVEN** a test-owned pane running a process that ignores `SIGHUP`
- **WHEN** the isolation helper is dropped
- **THEN** the process SHALL be killed once the hang ceiling elapses
- **AND** the helper SHALL report the process it had to kill

Test: `tests/helpers/mod.rs::tests::reaping_reports_a_process_it_had_to_kill` — uses a short ceiling override.

### Requirement: Coverage is collected and reported in separate phases

The `just coverage` recipe SHALL run the test suite to collect coverage profiles and SHALL merge and report them only after every test binary has exited. Profiles SHALL be written one per process, so a malformed profile cannot corrupt another, and the recipe SHALL name any malformed profile it encounters rather than failing with an unattributed merge error. The coverage figure SHALL be the same whether or not a `paw-*` session is live on the developer's machine.

#### Scenario: Coverage next to a live session

- **GIVEN** a developer with a live `paw-git-paw` session and `GIT_PAW_ALLOW_LIVE_SESSION=1`
- **WHEN** `just coverage` runs from a clean coverage target
- **THEN** the profile merge SHALL succeed
- **AND** the reported line coverage SHALL match a cold run of the same commit

Test: maintainer-run rollout check (live vs cold on one commit), recorded in `tasks.md`.

#### Scenario: A malformed profile is named

- **GIVEN** a coverage run in which one profile file is truncated
- **WHEN** the recipe merges profiles
- **THEN** its output SHALL name the truncated file

Test: maintainer-run rollout check — truncate one profile between the collect and report phases, recorded in `tasks.md`.

### Requirement: The suite passes cold and the cold-start job gates merges

`just check` SHALL pass on a machine with no tmux server running, `TMUX` unset, no broker running and no `git-paw` on `PATH`, at the same commit at which it passes next to a live session. The CI cold-start job (headless Linux, `TERM=dumb`, `TMUX` and `GIT_PAW_ALLOW_LIVE_SESSION` unset) SHALL be a blocking job: its failure SHALL fail the workflow.

#### Scenario: A cold failure blocks the pull request

- **GIVEN** a pull request whose tests pass next to a live session but fail cold
- **WHEN** CI runs
- **THEN** the cold-start job SHALL fail
- **AND** the workflow SHALL report failure

Test: `.github/workflows/ci.yml` `smoke-headless` without `continue-on-error`; verified by the job itself.

#### Scenario: Cold and live runs agree

- **GIVEN** one commit
- **WHEN** `just check` runs cold, and the full suite runs next to a live session with `GIT_PAW_ALLOW_LIVE_SESSION=1`
- **THEN** both SHALL report the same pass/fail result for every test

Test: maintainer-run rollout check, recorded in `tasks.md`.
