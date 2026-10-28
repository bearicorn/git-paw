## ADDED Requirements

### Requirement: Status JSON exposes attributed usage

`git paw status --json` SHALL include, on each worktree object, a `usage` object carrying the
worker's token totals (and `cost` when the CLI reports one) or `null` when its usage is unknown, and
a top-level `usage_total` summing the known figures with a count of workers whose usage is unknown.
The fields SHALL be additive: every existing field SHALL keep its name and value. The human-readable
`git paw status` output SHALL be unchanged. The usage SHALL exclude any session git-paw did not
launch.

#### Scenario: A worker with a usage report shows figures

- **GIVEN** an active session whose `feat/x` worker has a parseable usage report
- **WHEN** `git paw status --json` is run
- **THEN** the `feat/x` worktree object SHALL contain a `usage` object with its token totals

#### Scenario: Unknown usage is null and counted

- **GIVEN** an active session with one worker whose usage is unknown
- **WHEN** `git paw status --json` is run
- **THEN** that worktree object SHALL contain `"usage": null`
- **AND** `usage_total` SHALL report one worker with unknown usage

#### Scenario: Human-readable status is unchanged by usage

- **WHEN** `git paw status` is run without `--json`
- **THEN** the output SHALL be byte-identical to the output before this requirement for the same session
