## ADDED Requirements

### Requirement: Status JSON exposes attributed usage

`git paw status --json` SHALL include, on each worktree object, a `usage` object with the worker's
token totals (and `cost` when the CLI reports one) or `null` when unknown, and a top-level
`usage_total` summing the known figures of the workers currently in the session, with `tokens`
(all four kinds summed, the cap's formula) and the count of unknown workers. Existing fields and the human-readable
output SHALL be unchanged. Sessions git-paw did not launch SHALL be excluded.

#### Scenario: A worker with a usage report shows figures

- **GIVEN** an active session whose `feat/x` worker has a parseable usage report
- **WHEN** `git paw status --json` is run
- **THEN** the `feat/x` worktree object SHALL contain a `usage` object with its token totals
- **AND** `usage_total.tokens` SHALL equal its input, output, cache-read and cache-write tokens summed

#### Scenario: Unknown usage is null and counted

- **GIVEN** an active session with one worker whose usage is unknown
- **WHEN** `git paw status --json` is run
- **THEN** that worktree object SHALL contain `"usage": null`
- **AND** `usage_total` SHALL report one worker with unknown usage

#### Scenario: Human-readable status is unchanged by usage

- **WHEN** `git paw status` is run without `--json`
- **THEN** the output SHALL be byte-identical to the output before this requirement for the same session
