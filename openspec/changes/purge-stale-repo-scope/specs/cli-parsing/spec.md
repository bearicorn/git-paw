## ADDED Requirements

### Requirement: Purge --all-repos flag for a machine-wide stale sweep

`git paw purge` SHALL accept an `--all-repos` flag that broadens the `--stale` sweep from the current repository (the default) to every repository with a receipt on the machine. `--all-repos` SHALL be meaningful only in combination with `--stale`; used without `--stale` it SHALL be rejected with an actionable error (or treated as inert) per the CLI's flag rules. The machine-wide sweep SHALL still only ever purge stale receipts and SHALL never purge a live session.

#### Scenario: --all-repos requires --stale

- **WHEN** the user runs `git paw purge --all-repos` without `--stale`
- **THEN** the CLI SHALL reject the combination with an actionable error (or treat `--all-repos` as inert), per the flag rules

#### Scenario: --stale --all-repos broadens the sweep

- **GIVEN** stale receipts in multiple repositories
- **WHEN** the user runs `git paw purge --stale --all-repos`
- **THEN** the sweep SHALL cover every repository's stale receipts
- **AND** live sessions SHALL remain intact
