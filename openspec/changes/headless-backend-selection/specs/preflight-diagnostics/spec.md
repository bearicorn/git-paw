## MODIFIED Requirements

### Requirement: Environment checks

The command SHALL verify that `git` and `tmux` are present on PATH and meet minimum
versions, and that the working directory is inside a git repository. Any missing tool or a
non-repository directory SHALL be reported ✗ with a remedy, except that when the effective
configuration sets `default_backend = "headless"` a missing or too-old `tmux` SHALL be
reported ⚠ with a remedy that states only the headless backend is usable without it.

#### Scenario: tmux missing

- **GIVEN** `tmux` is not on PATH and no `default_backend` is configured
- **WHEN** `git paw doctor` is run
- **THEN** the Environment group SHALL report a ✗ for tmux with a remedy to install it

#### Scenario: tmux missing under a headless default

- **GIVEN** `tmux` is not on PATH and the configuration sets `default_backend = "headless"`
- **WHEN** `git paw doctor` is run
- **THEN** the Environment group SHALL report a ⚠ for tmux stating that only the headless backend is usable
- **AND** that check alone SHALL NOT make the command exit non-zero

#### Scenario: Not a git repository

- **GIVEN** the working directory is not inside a git repository
- **WHEN** `git paw doctor` is run
- **THEN** the Environment group SHALL report a ✗ with a remedy

## ADDED Requirements

### Requirement: Headless eligibility check

When the effective configuration sets `default_backend = "headless"`, the command SHALL report, for each worker CLI the configuration would launch, the result of the same eligibility decision a headless launch uses: ✓ when eligible with matrix evidence, ⚠ when eligible as user-declared or with a recorded gap, and ✗ listing every failed condition with the configuration key that satisfies it. The check SHALL spawn no CLI. When `default_backend` is absent or `"tmux"`, the check SHALL NOT be reported, so a tmux user's report is unchanged.

#### Scenario: An ineligible CLI is reported with its fix

- **GIVEN** `default_backend = "headless"` and a worker CLI at `full-auto` with no `sandbox` declared
- **WHEN** `git paw doctor` is run
- **THEN** it SHALL report ✗ for that CLI naming the missing sandbox and `[clis.<name>].sandbox`

#### Scenario: A user-declared CLI warns

- **GIVEN** `default_backend = "headless"` and an eligible CLI whose headless templates come from `[clis.<name>]`
- **WHEN** `git paw doctor` is run
- **THEN** it SHALL report ⚠ for that CLI stating that its headless operation is user-declared

#### Scenario: No headless check for tmux users

- **GIVEN** a configuration with no `default_backend`
- **WHEN** `git paw doctor` is run
- **THEN** no headless eligibility check SHALL appear in the report
