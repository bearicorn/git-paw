## RENAMED Requirements

- FROM: `### Requirement: Supervisor check`
- TO: `### Requirement: Coordination check`

## MODIFIED Requirements

### Requirement: `git paw doctor` runs read-only preflight checks

The system SHALL provide a `git paw doctor` command that inspects the environment,
configuration, repository, and session state and prints a grouped diagnostic report. Each check
SHALL be reported with one of three statuses — ✓ (pass), ⚠ (warning), or ✗ (failure) —
and every non-✓ check SHALL print an actionable remedy. Without `--fix`, the command SHALL NOT
mutate any file, config, session, worktree registration, or other persistent state. With
`--fix` it SHALL mutate only what the "`git paw doctor --fix` repairs only git-paw's own session
records" requirement allows.

#### Scenario: Doctor prints a grouped report

- **GIVEN** a git repository
- **WHEN** `git paw doctor` is run
- **THEN** it SHALL print checks grouped under Environment, CLIs, Config, Spec system, Bundled scripts, Broker, Coordination, Session, and Hygiene
- **AND** no group SHALL be headed Supervisor
- **AND** each check SHALL carry a ✓, ⚠, or ✗ status
- **AND** every ⚠ or ✗ check SHALL include a remedy line

#### Scenario: Doctor does not mutate state without --fix

- **GIVEN** a repository with a `.git-paw/` directory and a drifted session record
- **WHEN** `git paw doctor` is run without `--fix`
- **THEN** no file under `.git-paw/`, no session file, and no git worktree registration SHALL be created, modified, or deleted

### Requirement: Coordination check

When coordinated mode is the configured default (`default_mode = "coordinated"`, including a version-1 `[supervisor] enabled = true`), the command SHALL verify, in a group headed Coordination, that the configured `[orchestrator.gates]` commands resolve to binaries on PATH and that `sweep.sh` is installed. The gate-command verbs SHALL be sourced from the resolved stack preset, NOT from a hard-coded git-paw toolchain, so the check stays project-agnostic. A failing gate's remedy SHALL name its `[orchestrator.gates]` key. The checks' machine ids SHALL begin with `coordination.`.

#### Scenario: A gate-command binary is missing

- **GIVEN** `default_mode = "coordinated"` and an `[orchestrator.gates]` command whose binary is not on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the Coordination group SHALL report ✗ for that gate command with a remedy naming its `[orchestrator.gates]` key

#### Scenario: Coordinated mode not the default

- **GIVEN** `default_mode = "bare"`
- **WHEN** `git paw doctor` is run
- **THEN** the Coordination group SHALL report an informational ✓

#### Scenario: Check ids carry the new group name

- **WHEN** `git paw doctor --json` is run with coordinated mode as the default
- **THEN** every check in the Coordination group SHALL have an `id` beginning with `coordination.`
- **AND** no `id` or `group` in the document SHALL contain `supervisor`

## ADDED Requirements

### Requirement: Version-1 coordination keys are reported with their version-2 names

`git paw config check` SHALL report each version-1 `[supervisor]` path in a version-1 file as one warning finding naming the path, its version-2 path and the deprecating release, taken from the deprecation registry; the `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` tables, which version 2 drops, SHALL each stay one warning finding naming the table, release 0.17.0 and that it has no effect, from their registry entries, with no version-2 path; a `[supervisor]` table in a file that declares `version = 2`, and a version-1 path set together with its version-2 counterpart, SHALL each be an error finding naming both paths. The file's `config_version` SHALL be reported as 1 for a file that declares no version. `git paw doctor`'s Config group SHALL report ⚠ when a config file uses version-1 paths, with a remedy pointing at `git paw config check`.

#### Scenario: Each old key is a finding with its new name

- **GIVEN** a repo config with `[supervisor] test_command = "just check"` and `agent_approval = "manual"`
- **WHEN** `git paw config check --json` runs
- **THEN** `findings` SHALL contain a warning for `supervisor.test_command` naming `orchestrator.gates.test_command` and one for `supervisor.agent_approval` naming `workers.approval`
- **AND** the repo file's `config_version` SHALL be 1
- **AND** the process SHALL exit 0, and 1 under `--strict`

#### Scenario: A dropped table has no version-2 name

- **GIVEN** a repo config with `[supervisor.common_dev_allowlist] stacks = ["rust"]`
- **WHEN** `git paw config check --json` runs
- **THEN** `findings` SHALL contain one warning for `supervisor.common_dev_allowlist` naming release 0.17.0 and stating that it has no effect
- **AND** no finding SHALL name a `workers.` path

#### Scenario: A conflict is an error finding

- **GIVEN** a repo config with `[supervisor] cli = "a"` and `[orchestrator] cli = "b"`
- **WHEN** `git paw config check` runs
- **THEN** it SHALL report an error naming `supervisor.cli` and `orchestrator.cli`
- **AND** the process SHALL exit 1

#### Scenario: Doctor flags a version-1 file

- **GIVEN** a repo config with `[supervisor] enabled = true`
- **WHEN** `git paw doctor` runs
- **THEN** the Config group SHALL report ⚠ with a remedy naming `git paw config check`
- **AND** the exit code SHALL be unaffected by this ⚠
