## MODIFIED Requirements

### Requirement: `git paw selftest` subcommand

The CLI SHALL provide a `selftest` subcommand that runs an isolated, end-to-end session lifecycle against a throwaway repository and a dummy CLI, then reports a single pass/fail verdict. The subcommand SHALL parse with no required arguments and SHALL exit `0` when the lifecycle completes successfully and non-zero when any lifecycle step fails.

The subcommand SHALL be an **internal diagnostic**: it SHALL be hidden from the command list `git paw --help` prints, and its `long_about` SHALL frame it as the **live arm of `git paw doctor`**, whose static preflight checks it complements. It SHALL remain directly invocable — hiding affects discovery, not availability — and SHALL still carry an `about` string and a `long_about` string with at least one usage example, matching the project's CLI help conventions.

The same lifecycle SHALL additionally be reachable through `git paw doctor --live`, which runs this harness and folds its verdict into doctor's grouped report rather than printing selftest's own summary line. Direct invocation of `git paw selftest` SHALL keep its own output unchanged.

The lifecycle SHALL NOT require a real AI CLI backend (LLM), SHALL NOT require an interactive terminal, and SHALL NOT touch the user's default tmux socket, real sessions directory, or live `paw-*` sessions.

#### Scenario: selftest parses with no arguments

- **GIVEN** `selftest` is passed to the CLI
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Selftest`

#### Scenario: selftest is hidden from the command list

- **WHEN** `git paw --help` is run
- **THEN** stdout SHALL NOT list a `selftest` subcommand
- **AND** `git paw doctor` SHALL be listed instead as the diagnostic entry point

#### Scenario: selftest stays invocable while hidden

- **GIVEN** `selftest` does not appear in the command list
- **WHEN** `git paw selftest` is run
- **THEN** the command SHALL execute normally rather than erroring as unknown

#### Scenario: selftest help text describes the isolated lifecycle

- **WHEN** `git paw selftest --help` is run
- **THEN** stdout SHALL describe that the command runs an isolated session lifecycle with a dummy CLI and no real LLM backend
- **AND** stdout SHALL identify it as an internal diagnostic and the live arm of `git paw doctor`
- **AND** stdout SHALL contain at least one usage example

#### Scenario: selftest reports pass and exits zero on a healthy build

- **GIVEN** tmux is available on PATH
- **WHEN** `git paw selftest` is run
- **THEN** the command SHALL exit with status `0`
- **AND** stdout SHALL contain a pass indication (e.g. "selftest passed")

#### Scenario: selftest reports failure and exits non-zero when a lifecycle step fails

- **GIVEN** a lifecycle step (start, roster check, or stop) fails during the run
- **WHEN** `git paw selftest` completes
- **THEN** the command SHALL exit with a non-zero status
- **AND** stderr SHALL name the failing step
