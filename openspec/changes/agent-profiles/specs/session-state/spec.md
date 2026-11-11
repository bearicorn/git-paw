## ADDED Requirements

### Requirement: Worktree entry records the worker profile

The `WorktreeEntry` struct SHALL include an optional field `profile: Option<String>` naming the worker profile the worker was launched with, annotated `#[serde(default, skip_serializing_if = "Option::is_none")]`, so older session files load with `None` and an entry launched without a profile serializes as before. The entry's `cli` SHALL still record the CLI id the profile resolved to.

#### Scenario: The profile round-trips

- **GIVEN** a worker launched with profile `routine` on CLI `beta`
- **WHEN** the session is saved and loaded back
- **THEN** its entry SHALL have `profile = Some("routine")` and `cli = "beta"`

#### Scenario: Older session files load with None

- **GIVEN** a session JSON file whose worktree entries have no `profile` field
- **WHEN** the session is loaded
- **THEN** every entry's `profile` SHALL be `None` and loading SHALL succeed

### Requirement: Status JSON exposes each worker's profile

`git paw status --json` SHALL include each worktree's `profile` (a string, or `null`). The human-readable `git paw status` output SHALL be unchanged.

#### Scenario: Status JSON carries the profile

- **GIVEN** an active session with one worker launched with `routine` and one with a plain CLI
- **WHEN** `git paw status --json` is run
- **THEN** the first worktree object SHALL contain `"profile": "routine"` and the second `"profile": null`

### Requirement: Recovery relaunches a worker with its recorded profile

When `git paw start` recovers a stopped session, a worktree whose entry records a `profile` SHALL be relaunched as that worker profile, resolved from the current configuration (its CLI, `args` and approval level), with the resume behaviour of *Recovery relaunches a worker with its recorded session id* unchanged.

#### Scenario: A recovered worker keeps its profile's arguments

- **GIVEN** a stopped session whose `feat/x` entry records `profile = "routine"`, and `[profiles.routine]` with `args = "--model small"`
- **WHEN** the session is recovered
- **THEN** the pane command for `feat/x` SHALL contain `--model small`

### Requirement: Recovery tolerates a profile that no longer exists

When a recorded profile no longer exists or is no longer a worker profile, recovery SHALL relaunch the worktree with its recorded `cli` at the default worker approval, SHALL print one warning naming the branch and the profile, and SHALL NOT fail because of it.

#### Scenario: A removed profile falls back with a warning

- **GIVEN** a stopped session whose `feat/x` entry records `profile = "gone"` and `cli = "beta"`, and no `[profiles.gone]`
- **WHEN** the session is recovered
- **THEN** `feat/x` SHALL be relaunched with `beta`
- **AND** recovery SHALL print one warning naming `feat/x` and `gone`

### Requirement: Status JSON attributes usage per profile

`git paw status --json` SHALL include a top-level `usage_by_profile` object keyed by profile name. Each value SHALL carry the profile's `kind`, its number of workers and of delegate runs, the sum of their known usage in the fields of a worktree's `usage` object, and how many of them have unknown usage. Workers without a profile SHALL NOT appear in it; with no profile in use it SHALL be `{}`. `usage_total` and every other field SHALL keep its name and value.

#### Scenario: Worker and delegate usage are grouped by profile

- **GIVEN** an active session with two workers on profile `routine` with known usage, one delegate run of profile `spec-audit` with known usage, and one delegate run of `explorer` whose CLI reports no usage
- **WHEN** `git paw status --json` is run
- **THEN** `usage_by_profile.routine` SHALL report 2 workers and the sum of their usage
- **AND** `usage_by_profile["spec-audit"]` SHALL report 1 delegate run and its usage
- **AND** `usage_by_profile.explorer` SHALL report 1 delegate run with unknown usage
- **AND** `usage_total` SHALL count only the workers, as before

#### Scenario: No profiles yields an empty object

- **GIVEN** a session in which no worker or delegate used a profile
- **WHEN** `git paw status --json` is run
- **THEN** `usage_by_profile` SHALL be `{}`

### Requirement: Delegate usage is attributed from the run ledger

Delegate usage SHALL be attributed to the profile of each run recorded in `.git-paw/logs/delegates/runs.jsonl` that started at or after the session's creation, resolved through the CLI usage facet with the run's recorded CLI session id and the repository root as the run's working directory. A run without a recorded session id SHALL count as unknown.

#### Scenario: Runs before the session are excluded

- **GIVEN** a delegate run recorded before the current session was created
- **WHEN** `git paw status --json` is run
- **THEN** that run SHALL NOT be counted in `usage_by_profile`
