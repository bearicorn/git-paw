## MODIFIED Requirements

### Requirement: Init generates default config.toml

The system SHALL create `.git-paw/config.toml` with sensible defaults and commented-out v0.2.0 fields when no config exists. The generated config SHALL include an active `worktree_placement = "child"` field so that new repositories use the contained (in-repo) worktree layout by default.

#### Scenario: Config created with defaults
- **WHEN** `git paw init` is run and no `.git-paw/config.toml` exists
- **THEN** `.git-paw/config.toml` SHALL be created with `default_cli` and `mouse` fields and commented examples for `default_spec_cli`, `branch_prefix`, `[specs]`, and `[logging]`

#### Scenario: Config created with child worktree placement
- **WHEN** `git paw init` is run and no `.git-paw/config.toml` exists
- **THEN** `.git-paw/config.toml` SHALL contain `worktree_placement = "child"`

#### Scenario: Existing config is not overwritten
- **WHEN** `git paw init` is run and `.git-paw/config.toml` already exists
- **THEN** the existing config SHALL NOT be modified

### Requirement: Init appends logs directory to .gitignore

The system SHALL ensure the repo's `.gitignore` lists the git-paw
runtime/scratch entries: `.git-paw/logs/`, `.git-paw/tmp/`,
`.git-paw/worktrees/`, and `.git-paw/session-summary.md`. Each entry SHALL
appear at most once and SHALL be added only if absent.

#### Scenario: Gitignore includes the repo-local tmp scratch after init

- **GIVEN** `git paw init` is run in a repo without `.git-paw/tmp/` in
  `.gitignore`
- **WHEN** init completes
- **THEN** `.gitignore` SHALL contain `.git-paw/tmp/`

#### Scenario: Gitignore includes the worktrees directory after init

- **GIVEN** `git paw init` is run in a repo without `.git-paw/worktrees/` in
  `.gitignore`
- **WHEN** init completes
- **THEN** `.gitignore` SHALL contain `.git-paw/worktrees/`

#### Scenario: Gitignore includes session-summary.md after init

- **GIVEN** `git paw init` is run in a repo without `.git-paw/session-summary.md` in `.gitignore`
- **WHEN** init completes
- **THEN** `.gitignore` SHALL contain `.git-paw/session-summary.md`

#### Scenario: Gitignore not duplicated on repeated init

- **GIVEN** `.gitignore` already contains `.git-paw/tmp/`,
  `.git-paw/worktrees/`, and `.git-paw/session-summary.md`
- **WHEN** `git paw init` is run again
- **THEN** each managed entry SHALL appear exactly once in `.gitignore`
