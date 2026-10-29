## ADDED Requirements

### Requirement: Worktree entry records the worker profile

The `WorktreeEntry` struct SHALL include an optional field `profile: Option<String>` naming the worker profile the worker was launched with, annotated `#[serde(default, skip_serializing_if = "Option::is_none")]` so session files written before this requirement load with `None` and an entry launched without a profile serializes byte-identically to before. The entry's `cli` field SHALL still record the CLI id the profile resolved to. `git paw status --json` SHALL include each worktree's `profile` (a string, or `null`). The human-readable `git paw status` output SHALL be unchanged.

#### Scenario: The profile round-trips

- **GIVEN** a worker launched with profile `routine` on CLI `beta`
- **WHEN** the session is saved and loaded back
- **THEN** its entry SHALL have `profile = Some("routine")` and `cli = "beta"`

#### Scenario: Older session files load with None

- **GIVEN** a session JSON file whose worktree entries have no `profile` field
- **WHEN** the session is loaded
- **THEN** every entry's `profile` SHALL be `None` and loading SHALL succeed

#### Scenario: Status JSON carries the profile

- **GIVEN** an active session with one worker launched with `routine` and one with a plain CLI
- **WHEN** `git paw status --json` is run
- **THEN** the first worktree object SHALL contain `"profile": "routine"` and the second `"profile": null`

### Requirement: Recovery relaunches a worker with its recorded profile

When `git paw start` recovers a stopped session, a worktree whose entry records a `profile` SHALL be relaunched as that worker profile, resolved from the current configuration — its CLI, `args` and approval level — with the resume behaviour of *Recovery relaunches a worker with its recorded session id* unchanged. When the recorded profile no longer exists or is no longer a worker profile, the worktree SHALL be relaunched with its recorded `cli` and the default worker approval, and recovery SHALL print one warning naming the branch and the profile. Recovery SHALL NOT fail because a recorded profile is missing.

#### Scenario: A recovered worker keeps its profile's arguments

- **GIVEN** a stopped session whose `feat/x` entry records `profile = "routine"`, and `[profiles.routine]` with `args = "--model small"`
- **WHEN** the session is recovered
- **THEN** the pane command for `feat/x` SHALL contain `--model small`

#### Scenario: A removed profile falls back with a warning

- **GIVEN** a stopped session whose `feat/x` entry records `profile = "gone"` and `cli = "beta"`, and no `[profiles.gone]`
- **WHEN** the session is recovered
- **THEN** `feat/x` SHALL be relaunched with `beta`
- **AND** recovery SHALL print one warning naming `feat/x` and `gone`

### Requirement: Status JSON attributes usage per profile

`git paw status --json` SHALL include a top-level `usage_by_profile` object keyed by profile name. Each value SHALL carry the profile's `kind`, the number of workers and of delegate runs attributed to it, the sum of their known usage in the same fields as a worktree's `usage` object, and the number of those workers and runs whose usage is unknown. Worker usage SHALL be attributed to the profile recorded on the worktree entry; delegate usage SHALL be attributed to the profile of each run recorded in `.git-paw/logs/delegates/runs.jsonl` that started at or after the session's creation, resolved through the run's recorded CLI session id. Workers launched without a profile SHALL NOT appear in `usage_by_profile`. With no profile in use the object SHALL be empty, and every other status field SHALL keep its name and value.

#### Scenario: Worker and delegate usage are grouped by profile

- **GIVEN** an active session with two workers on profile `routine` with known usage, one delegate run of profile `spec-audit` with known usage, and one delegate run of `explorer` whose CLI reports no usage
- **WHEN** `git paw status --json` is run
- **THEN** `usage_by_profile.routine` SHALL report 2 workers and the sum of their usage
- **AND** `usage_by_profile["spec-audit"]` SHALL report 1 delegate run and its usage
- **AND** `usage_by_profile.explorer` SHALL report 1 delegate run with unknown usage

#### Scenario: Runs before the session are excluded

- **GIVEN** a delegate run recorded before the current session was created
- **WHEN** `git paw status --json` is run
- **THEN** that run SHALL NOT be counted in `usage_by_profile`

#### Scenario: No profiles yields an empty object

- **GIVEN** a session in which no worker or delegate used a profile
- **WHEN** `git paw status --json` is run
- **THEN** `usage_by_profile` SHALL be `{}`
