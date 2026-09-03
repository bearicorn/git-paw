## MODIFIED Requirements

### Requirement: purge --stale flag

`git paw purge` SHALL accept a `--stale` flag. When passed, the system SHALL purge only sessions whose receipt is stale per the liveness probe, and by default SHALL scope the sweep to the CURRENT repository — it SHALL NOT purge or touch a session belonging to any other repository. A machine-wide sweep of stale receipts across all repositories SHALL require the explicit `--all-repos` flag. In every case the guarantee is fail-safe: a session that is live per the probe — or whose liveness cannot be positively confirmed — SHALL NEVER be purged by `--stale`, in the current repository or any other. The flag is additive to existing `--force`.

#### Scenario: --stale is scoped to the current repository by default

- **GIVEN** a stale session belonging to the current repository AND a stale session belonging to a different repository
- **WHEN** the user runs `git paw purge --stale` (without `--all-repos`)
- **THEN** only the current repository's stale session's worktrees + branches + receipt SHALL be purged
- **AND** the other repository's session SHALL NOT be touched

#### Scenario: --stale never purges a live or liveness-indeterminate session

- **GIVEN** a live session (in the current or another repository), or a session whose liveness cannot be positively confirmed
- **WHEN** the user runs `git paw purge --stale` (with or without `--all-repos`)
- **THEN** that session SHALL NOT be purged

#### Scenario: --all-repos sweeps stale receipts across repositories

- **GIVEN** stale sessions belonging to multiple repositories
- **WHEN** the user runs `git paw purge --stale --all-repos`
- **THEN** every repository's stale session SHALL be purged
- **AND** every live session SHALL remain intact

#### Scenario: --stale with nothing stale exits cleanly

- **GIVEN** no stale receipts in scope
- **WHEN** the user runs `git paw purge --stale`
- **THEN** the command SHALL exit 0 with a "nothing to purge" message, and SHALL NOT touch any active session

#### Scenario: --stale + --force is well-defined

- **GIVEN** a stale receipt in the current repository
- **WHEN** the user runs `git paw purge --stale --force`
- **THEN** the system SHALL behave equivalently to `--stale` alone (the `--force` flag is redundant in this combination; explicitly documented as a no-op pairing)
