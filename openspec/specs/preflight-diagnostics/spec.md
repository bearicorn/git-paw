# preflight-diagnostics Specification

## Purpose
The read-only `git paw doctor` preflight command: grouped ✓/⚠/✗ diagnostics across environment, CLIs, config, spec system, bundled scripts, broker, supervisor, and hygiene — each non-passing check carrying a remedy — with a `--json` renderer, an optional `--live` session-lifecycle smoke arm, and an exit code that reflects the worst check. Doctor diagnoses but never repairs.

## Requirements
### Requirement: `git paw doctor` runs read-only preflight checks

The system SHALL provide a `git paw doctor` command that inspects the environment,
configuration, and repository state and prints a grouped diagnostic report. Each check
SHALL be reported with one of three statuses — ✓ (pass), ⚠ (warning), or ✗ (failure) —
and every non-✓ check SHALL print an actionable remedy. The command SHALL NOT mutate any
file, config, session, or other persistent state.

#### Scenario: Doctor prints a grouped report

- **GIVEN** a git repository
- **WHEN** `git paw doctor` is run
- **THEN** it SHALL print checks grouped under Environment, CLIs, Config, Spec system, Bundled scripts, Broker, Supervisor, and Hygiene
- **AND** each check SHALL carry a ✓, ⚠, or ✗ status
- **AND** every ⚠ or ✗ check SHALL include a remedy line

#### Scenario: Doctor does not mutate state

- **GIVEN** a repository with a `.git-paw/` directory
- **WHEN** `git paw doctor` is run
- **THEN** no file under `.git-paw/` (config, scripts, session state) SHALL be created, modified, or deleted

### Requirement: Exit code reflects the worst check

The command SHALL exit with a non-zero status when any check is ✗, and with zero when all
checks are ✓ or ⚠. A ⚠ alone SHALL NOT cause a non-zero exit.

#### Scenario: All checks pass

- **GIVEN** an environment where every check resolves to ✓
- **WHEN** `git paw doctor` is run
- **THEN** the process SHALL exit 0

#### Scenario: A hard failure is present

- **GIVEN** an environment with at least one ✗ check
- **WHEN** `git paw doctor` is run
- **THEN** the process SHALL exit non-zero

#### Scenario: Only warnings are present

- **GIVEN** an environment with ⚠ checks but no ✗ checks
- **WHEN** `git paw doctor` is run
- **THEN** the process SHALL exit 0

### Requirement: Machine-readable `--json` output

When `--json` is passed, the command SHALL emit a single JSON document enumerating every
check with at least its group, name, status, detail, and remedy fields, and SHALL suppress
the human-readable rendering. The exit-code contract SHALL be identical to the
human-readable mode.

#### Scenario: `--json` emits parseable output

- **WHEN** `git paw doctor --json` is run
- **THEN** stdout SHALL be a single parseable JSON document
- **AND** each check entry SHALL include `group`, `name`, `status`, and (for non-✓ checks) `remedy`
- **AND** the exit code SHALL match what the human-readable mode would return for the same environment

### Requirement: Environment checks

The command SHALL verify that `git` and `tmux` are present on PATH and meet minimum
versions, and that the working directory is inside a git repository. Any missing tool or a
non-repository directory SHALL be reported ✗ with a remedy.

#### Scenario: tmux missing

- **GIVEN** `tmux` is not on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the Environment group SHALL report a ✗ for tmux with a remedy to install it

#### Scenario: Not a git repository

- **GIVEN** the working directory is not inside a git repository
- **WHEN** `git paw doctor` is run
- **THEN** the Environment group SHALL report a ✗ with a remedy

### Requirement: CLI availability check

The command SHALL report the AI CLIs detected on PATH (the known roster plus any
`[clis.*]` custom entries). When no CLI resolves, it SHALL report ⚠ (surfacing the
`NoCLIsFound` launch condition early) with a remedy to install a CLI or add a `[clis.*]`
entry.

#### Scenario: No CLIs resolve

- **GIVEN** no known or custom CLI resolves on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the CLIs group SHALL report ⚠ with a remedy

#### Scenario: CLIs detected

- **GIVEN** at least one CLI resolves
- **WHEN** `git paw doctor` is run
- **THEN** the CLIs group SHALL report ✓ and list the detected CLIs

### Requirement: Config check

The command SHALL report whether `.git-paw/config.toml` is present and parses, report the
resolved `worktree_placement`, and flag unknown or deprecated fields as ⚠. An unparseable
config SHALL be ✗.

#### Scenario: Config parses

- **GIVEN** a valid `.git-paw/config.toml`
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✓ and state the resolved `worktree_placement`

#### Scenario: Config does not parse

- **GIVEN** a `.git-paw/config.toml` with invalid TOML
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✗ with a remedy

### Requirement: Spec-system check

The command SHALL report the explicitly resolved spec system (from `--specs-format` /
`[specs]`, per the v0.12.0 explicit-only rule) and the count of specs scanned. When no spec
system is configured, it SHALL report ⚠ with the actionable "add `[specs]` or pass
`--specs-format`" guidance rather than a bare error.

#### Scenario: Spec system configured

- **GIVEN** `[specs]` is configured (or the equivalent flag would be passed)
- **WHEN** `git paw doctor` is run
- **THEN** the Spec-system group SHALL report ✓ with the resolved format and scanned spec count

#### Scenario: Spec system unconfigured

- **GIVEN** no `[specs]` configuration
- **WHEN** `git paw doctor` is run
- **THEN** the Spec-system group SHALL report ⚠ with guidance to add `[specs]` or pass `--specs-format`

### Requirement: Bundled-scripts check

The command SHALL verify that the bundled helper scripts (`sweep.sh`, `broker.sh`,
`docs-fetch.sh`) exist under `.git-paw/scripts/`, are executable, and match the running
binary's embedded version. A missing or stale script SHALL be reported with a remedy to run
`git paw init`. The command SHALL also verify that a Python 3 interpreter is available on
PATH (`python3`, or `python` reporting major version 3), since every bundled helper script
requires one to run; when none is found it SHALL report ⚠ with a remedy to install Python 3.
Because core git-paw orchestration (start/add/remove) does not require Python, a missing
interpreter SHALL be ⚠ and SHALL NOT be ✗.

#### Scenario: A bundled script is missing

- **GIVEN** `.git-paw/scripts/sweep.sh` does not exist
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ✗ for `sweep.sh` with a remedy to run `git paw init`

#### Scenario: A bundled script is stale

- **GIVEN** a bundled script exists but its content differs from the running binary's embedded version
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report a non-✓ status with a remedy to run `git paw init`

#### Scenario: No Python 3 interpreter on PATH

- **GIVEN** neither `python3` nor a version-3 `python` is on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ⚠ for the Python 3 interpreter with a remedy to install Python 3 (the bundled helper scripts require it)

### Requirement: Broker check

When `[broker]` is enabled, the command SHALL check that the configured `bind`/`port` is
free or reachable. When the broker is disabled, it SHALL note the pure-manual baseline as an
informational ✓.

#### Scenario: Broker enabled and port free

- **GIVEN** `[broker] enabled = true` and the configured port is free
- **WHEN** `git paw doctor` is run
- **THEN** the Broker group SHALL report ✓

#### Scenario: Broker disabled

- **GIVEN** `[broker] enabled = false`
- **WHEN** `git paw doctor` is run
- **THEN** the Broker group SHALL report an informational ✓ noting the pure-manual baseline

### Requirement: Supervisor check

When `[supervisor]` is enabled, the command SHALL verify that the configured gate commands
resolve to binaries on PATH and that `sweep.sh` is installed. The gate-command verbs SHALL
be sourced from the resolved stack preset, NOT from a hard-coded git-paw toolchain, so the
check stays project-agnostic.

#### Scenario: A gate-command binary is missing

- **GIVEN** `[supervisor] enabled = true` and a configured gate command whose binary is not on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the Supervisor group SHALL report ✗ for that gate command with a remedy

#### Scenario: Supervisor disabled

- **GIVEN** `[supervisor] enabled = false`
- **WHEN** `git paw doctor` is run
- **THEN** the Supervisor group SHALL report an informational ✓

### Requirement: Hygiene check

The command SHALL verify required `.gitignore` entries are present (including
`.git-paw/worktrees/` being ignored under child placement) and detect stale session state or
orphaned worktree registrations, reporting ⚠ with a remedy to run `git paw purge --stale`.

#### Scenario: Missing gitignore entry

- **GIVEN** a required `.gitignore` entry is absent
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ with a remedy

#### Scenario: Stale session detected

- **GIVEN** session state that claims active but whose tmux session is gone
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ with a remedy to run `git paw purge --stale`

### Requirement: `--live` smoke arm

The command SHALL accept a `--live` flag that additionally runs the `selftest`
session-lifecycle harness and folds its verdict into the report as a Live-smoke check,
so static preflight and live smoke sit under one diagnostic surface. Without `--live` the
command SHALL remain static — it SHALL NOT spawn a tmux session or an AI CLI, and the
Live-smoke group SHALL be absent. The `selftest` command SHALL remain available as the
harness's direct entry point, with its own output unchanged. Because the static checks
already report a missing prerequisite (notably tmux) as ✗, a smoke run that could not
start SHALL be reported ⚠ rather than ✗.

#### Scenario: `--live` adds the smoke verdict

- **GIVEN** an environment where the session lifecycle completes
- **WHEN** `git paw doctor --live` is run
- **THEN** the report SHALL carry a Live-smoke check reporting ✓

#### Scenario: A failing lifecycle is a hard failure

- **GIVEN** an environment where a lifecycle step fails
- **WHEN** `git paw doctor --live` is run
- **THEN** the Live-smoke check SHALL report ✗ naming the failing step
- **AND** the process SHALL exit non-zero

#### Scenario: A smoke run that cannot start warns

- **GIVEN** the lifecycle cannot run because tmux is unavailable
- **WHEN** `git paw doctor --live` is run
- **THEN** the Live-smoke check SHALL report ⚠ with a remedy rather than ✗

#### Scenario: Static runs carry no Live-smoke group

- **WHEN** `git paw doctor` is run without `--live`
- **THEN** the report SHALL NOT contain a Live-smoke group

#### Scenario: `--json --live` stays parseable

- **WHEN** `git paw doctor --json --live` is run
- **THEN** stdout SHALL remain a single parseable JSON document, with the harness's
  per-step progress output suppressed

### Requirement: Diagnose-only (no `--fix` in v0.13.0)

The command SHALL be diagnose-only in v0.13.0; it SHALL NOT expose a repair mode. (`--fix`
is deferred to a later cycle.)

#### Scenario: No repair flag

- **WHEN** `git paw doctor --help` is inspected
- **THEN** it SHALL NOT advertise a `--fix` or other repair option

