## ADDED Requirements

### Requirement: CI builds every target on the declared MSRV

The CI workflow SHALL include a job that installs the toolchain named by `rust-version` in `Cargo.toml` and runs `cargo +<rust-version> check --all-targets --locked`. A failure of that job SHALL fail the workflow. The job SHALL NOT be marked `continue-on-error`.

#### Scenario: Code that needs a newer compiler fails CI

- **GIVEN** `rust-version = "1.96"` and a change that calls a std API stabilised after 1.96
- **WHEN** CI runs on the change
- **THEN** the MSRV job SHALL fail and the workflow SHALL report failure

Test: the `msrv` job in `.github/workflows/ci.yml` (verified at implementation by a throwaway branch that calls a post-1.96 API)

#### Scenario: Test and bench targets are checked too

- **WHEN** the MSRV job runs
- **THEN** its command SHALL include `--all-targets`, so integration tests and binary-crate test modules compile on the MSRV

Test: `tests/ci_workflow_conventions.rs::the_msrv_job_checks_all_targets_on_the_declared_rust_version`

### Requirement: The MSRV job and rust-version cannot drift

A convention test SHALL read `rust-version` from `Cargo.toml` and the MSRV job's toolchain from `.github/workflows/ci.yml`, and SHALL fail when they differ, naming both values.

#### Scenario: Raising rust-version alone fails the suite

- **GIVEN** `Cargo.toml` changed to `rust-version = "1.97"` while the MSRV job still installs 1.96
- **WHEN** the suite runs
- **THEN** the convention test SHALL fail naming `1.97` and `1.96`

Test: `tests/ci_workflow_conventions.rs::the_msrv_job_checks_all_targets_on_the_declared_rust_version`

### Requirement: Known timing flakes are recorded in a register

The repository SHALL keep one register under `tests/` of tests known to fail intermittently under load. Each entry SHALL name the test by file and function, describe the observed failure, and give the date first seen. A convention test SHALL fail when an entry names a test function that does not exist.

#### Scenario: The known tmux timing flakes are registered

- **WHEN** the register is read
- **THEN** it SHALL list `tests/e2e_supervisor_approval_flags.rs::coding_agents_resolve_flags_from_agent_approval`, `tests/e2e_supervisor_stop.rs::stop_revokes_the_drive_loop_binding` and `tests/agent_pane_liveness_e2e.rs::doctor_warns_that_the_agent_panes_exited_with_the_start_remedy`, until each meets the exit criterion

Test: `tests/flake_register.rs::every_registered_test_exists`

#### Scenario: A stale entry fails the suite

- **GIVEN** a register entry naming a test function that was renamed or deleted
- **WHEN** the suite runs
- **THEN** the convention test SHALL fail naming the entry

Test: `tests/flake_register.rs::every_registered_test_exists`

### Requirement: A registered flake leaves the register only when fixed

A test SHALL be removed from the flake register only when every wait in it polls the condition it asserts, with no fixed sleep standing in for that condition, and it has passed ten consecutive runs of its test binary while the rest of the suite runs in parallel. The change that removes the entry SHALL record those ten runs.

#### Scenario: A test with a fixed sleep stays registered

- **GIVEN** a registered test that still sleeps a fixed time before reading a pane
- **WHEN** a change proposes removing its register entry
- **THEN** review SHALL keep the entry until the sleep is replaced by a condition wait

Test: review rule recorded in `CONTRIBUTING.md`; checked when an entry is removed

### Requirement: Processes left by a killed test are reaped by the next run

Every test-owned tmux socket directory, and every long-lived `git-paw` child a test spawns directly, SHALL record the process id of the test process that owns it. Creating a `TmuxTestEnv` or spawning such a child SHALL first kill every recorded tmux server and child whose owner process no longer exists. It SHALL NOT signal a process whose owner is alive, and SHALL NOT touch the user's default tmux socket.

#### Scenario: A killed test's dashboard does not survive the next run

- **GIVEN** a test killed with SIGKILL while its private tmux server runs a `git-paw __dashboard` pane
- **WHEN** the next test creates a `TmuxTestEnv`
- **THEN** that tmux server and its `__dashboard` process SHALL have exited

Test: `tests/helpers/mod.rs::tests::a_dead_owners_tmux_server_is_reaped`

#### Scenario: A killed test's broker host does not survive the next run

- **GIVEN** a test killed with SIGKILL while a `git-paw __broker` child it spawned is running
- **WHEN** the next test spawns a long-lived `git-paw` child
- **THEN** the orphaned `__broker` process SHALL have exited

Test: `tests/helpers/mod.rs::tests::a_dead_owners_child_is_reaped`

#### Scenario: A concurrent run is left alone

- **GIVEN** two `cargo test` runs in progress on the same host
- **WHEN** either creates a `TmuxTestEnv`
- **THEN** no tmux server or child owned by the other run SHALL be signalled

Test: `tests/helpers/mod.rs::tests::a_live_owners_tmux_server_is_left_alone`

### Requirement: Fixtures wait for a pane's stand-in command through one shared helper

`tests/helpers` SHALL provide one function that polls a pane's current command until it equals an expected command or a timeout passes, then fails the test naming the pane and the last command seen. A fixture that starts a stand-in process in a pane and then reads session liveness SHALL call it before the first liveness read, and SHALL NOT carry its own copy of the poll.

#### Scenario: A pause fixture reads liveness only after its stand-in runs

- **GIVEN** a fixture that types `sleep 3600` into an agent pane, as `tests/pause_e2e.rs` does
- **WHEN** the machine is loaded enough that the pane's shell starts late
- **THEN** the fixture SHALL wait until the pane's command is `sleep` before the test runs `git paw pause` or reads status

Test: `tests/pause_e2e.rs` (all fixtures), via `helpers::wait_for_pane_command`

#### Scenario: A stand-in fixture without the shared wait fails the suite

- **GIVEN** a test file in `tests/` that starts a `sleep` stand-in in a tmux pane and never calls the shared helper
- **WHEN** the convention test scans `tests/`
- **THEN** it SHALL fail naming the file

Test: `tests/ci_workflow_conventions.rs::stand_in_fixtures_use_the_shared_pane_wait`
