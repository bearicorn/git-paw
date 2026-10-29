## MODIFIED Requirements

### Requirement: Actionable error messages for each variant

Each user-facing error SHALL produce a message that states what failed, names the object it failed on (a path, branch, session, CLI, port or config key) when there is one, and suggests a next step — a command to run, a flag to pass, or a config key to change — when one exists. This SHALL hold for the fixed-message variants and for every construction site of a message-wrapping variant whose error can reach the user. Every user-reachable failure class SHALL be covered by a table-driven end-to-end test that drives the compiled binary into the failure and asserts the exit code and that stderr names the object and the next step.

#### Scenario: NotAGitRepo is actionable
- **GIVEN** `PawError::NotAGitRepo`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL mention "git repository" and name the tool

Test: `error::tests::test_not_a_git_repo_is_actionable`

#### Scenario: TmuxNotInstalled includes install instructions
- **GIVEN** `PawError::TmuxNotInstalled`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL include both `brew install` and `apt install` hints

Test: `error::tests::test_tmux_not_installed_includes_install_instructions`

#### Scenario: NoCLIsFound suggests add-cli
- **GIVEN** `PawError::NoCLIsFound`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL suggest the `add-cli` command

Test: `error::tests::test_no_clis_found_suggests_add_cli`

#### Scenario: UserCancelled has a message
- **GIVEN** `PawError::UserCancelled`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL not be empty

Test: `error::tests::test_user_cancelled_is_not_empty`

#### Scenario: An unknown CLI names the CLI and how to find a valid one
- **GIVEN** a repository where `git paw start --cli no-such-cli --branches a` is run
- **WHEN** the error is printed
- **THEN** stderr SHALL name `no-such-cli`
- **AND** SHALL point at `git paw list-clis` or `git paw add-cli`

Test: `tests/error_matrix_e2e.rs::every_user_reachable_failure_is_actionable` (row `unknown-cli`)

#### Scenario: Every user-reachable failure is actionable
- **GIVEN** the table of user-reachable failures — outside a git repository, tmux missing, no CLI on `PATH`, an unknown `--cli`, an unknown preset, invalid config TOML, an I/O failure while provisioning a worktree, `purge` with no terminal, an argument error
- **WHEN** each is driven through the compiled binary
- **THEN** each SHALL exit with the code the exit-code contract assigns it
- **AND** its stderr SHALL name the failing object and a next step, asserted by stable anchors rather than exact prose

Test: `tests/error_matrix_e2e.rs::every_user_reachable_failure_is_actionable`

### Requirement: Exit codes distinguish cancellation from errors

The process exit code SHALL follow one contract, which every command honours:

- `0` — the command did what was asked.
- `1` — the command was attempted and failed. Every `PawError` not listed under `2` or `3` SHALL exit `1`, including a `git paw doctor` run with a failing check and an error that stops an unattended run's drive loop.
- `2` — nothing was attempted: an invalid or conflicting argument (including clap's own argument errors), a cancelled interactive selection or prompt (`UserCancelled`), or a condition for proceeding that was not met (`NotAttempted`). `UserCancelled` and `NotAttempted` SHALL exit `2`.
- `3` — the command ran but stopped short of its goal and reported the outcome, without leaving anything half-done: a `git paw start --unattended` run that ended without completing its wave (`WaveIncomplete`), or an integration that `git paw finish` declined under its merge policy (not fast-forwardable, or a conflicting merge) after changing nothing (`IntegrationRefused`). An unattended run whose drive loop escalated any permission prompt ends `escalated-for-review`, and so exits `3` even when every branch merged.

`NotAttempted` reports that a command wrote nothing because a condition for proceeding was not met — a declined confirmation, a confirmation that could not be shown because standard input is not a terminal, or a requested item that is not supported or not enabled for its target. Its message SHALL state that condition and, where one exists, name the flag or command that proceeds, and SHALL NOT read only `Cancelled.`. A declined proceed-confirmation (such as `purge`'s) and a required confirmation that could not be shown because stdin is not a terminal SHALL be reported as `NotAttempted`, not as `UserCancelled`.

`--help` and `--version` SHALL exit `0`. The contract SHALL be documented, with the same four codes and meanings, in the CLI reference's Exit Codes section, the architecture chapter and the root `--help` text.

#### Scenario: UserCancelled exit code
- **GIVEN** `PawError::UserCancelled`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `2`

Test: `error::tests::test_user_cancelled_exit_code`

#### Scenario: NotAttempted exit code and message
- **GIVEN** `PawError::NotAttempted` built for a confirmation that could not be shown, naming `--yes`
- **WHEN** `exit_code()` is called and the error is formatted with `Display`
- **THEN** `exit_code()` SHALL return `2`
- **AND** the message SHALL name `--yes`
- **AND** the message SHALL NOT be `Cancelled.`

Test: `error::tests::test_not_attempted_exit_code_and_message`

#### Scenario: General errors exit code
- **GIVEN** any error variant other than `UserCancelled`, `NotAttempted`, `WaveIncomplete` and `IntegrationRefused`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `1`

Test: `error::tests::test_general_errors_exit_code`

#### Scenario: An argument error exits 2
- **GIVEN** `git paw start --no-such-flag`
- **WHEN** the binary runs
- **THEN** it SHALL exit `2` without changing any state

Test: `tests/error_matrix_e2e.rs::every_user_reachable_failure_is_actionable` (row `unknown-flag`)

#### Scenario: Declining purge exits 2
- **GIVEN** a session and `git paw purge` run on a terminal
- **WHEN** the user answers no to the confirmation
- **THEN** the process SHALL report that the purge was not run because the confirmation was declined
- **AND** SHALL exit `2`
- **AND** no worktree, branch or session file SHALL be removed

Test: `tests/cli_prompt_matrix.rs::purge_declined_exits_two` (PTY harness)

#### Scenario: Purge without a terminal names --force
- **GIVEN** a session and `git paw purge` run with stdin not a terminal and no `--force`
- **WHEN** the binary runs
- **THEN** it SHALL exit `2`
- **AND** stderr SHALL name `--force`
- **AND** stderr SHALL NOT read only `Cancelled.`
- **AND** the session file and worktrees SHALL be left in place

Test: `tests/cli_tests.rs::test_purge_cancel_preserves_worktrees`

#### Scenario: An incomplete unattended run exits 3
- **GIVEN** `PawError::WaveIncomplete` for any of the outcomes `escalated-for-review`, `stuck`, `heartbeat`, `session-torn-down`
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `3`

Test: `error::tests::test_exit_code_contract_table` (table-driven over every variant shape)

#### Scenario: IntegrationRefused exit code
- **GIVEN** `PawError::IntegrationRefused` with any message
- **WHEN** `exit_code()` is called
- **THEN** it SHALL return `3`

Test: `error::tests::test_exit_code_contract_table`

#### Scenario: The documented table matches the contract
- **GIVEN** the CLI reference Exit Codes section, the architecture chapter and `git paw --help`
- **WHEN** they are read
- **THEN** each SHALL list codes `0`, `1`, `2` and `3` with the meanings above and no other code
- **AND** each SHALL state that an unattended run that escalated a permission prompt exits `3` even when its work completed

Test: `tests/cli_dx_docs.rs::exit_code_table_is_consistent_across_layers`

## ADDED Requirements

### Requirement: I/O failures name the operation and the path

An error caused by a filesystem or I/O operation SHALL name the operation and the path it was applied to, followed by the operating-system reason. A bare conversion from `std::io::Error` that discards that context SHALL NOT exist: the I/O variant SHALL be constructed with the operation and path, so code that propagates an I/O result without adding them does not compile.

#### Scenario: A failed copy names the file
- **GIVEN** a worktree configured to copy `.env` whose source cannot be read
- **WHEN** `git paw start` provisions the worktree and the copy fails
- **THEN** stderr SHALL name the copy operation and the `.env` path
- **AND** SHALL include the operating-system reason
- **AND** the process SHALL exit `1`

Test: `tests/error_matrix_e2e.rs::every_user_reachable_failure_is_actionable` (row `provision-io`)

#### Scenario: The I/O variant renders its context
- **GIVEN** an I/O error constructed for operation `read` on path `/x/y`
- **WHEN** formatted with `Display`
- **THEN** the message SHALL contain `read`, `/x/y` and the source reason

Test: `error::tests::io_error_names_operation_and_path`
