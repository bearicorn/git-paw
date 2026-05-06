## Purpose

Bootstrap a repository for git-paw by creating the `.git-paw/` directory structure, generating a default config file, updating `.gitignore`, and reporting actions taken, with idempotent behavior on repeated runs.
## Requirements
### Requirement: Init creates .git-paw directory structure

The system SHALL create the `.git-paw/` directory and `.git-paw/logs/` subdirectory in the repository root when `git paw init` is run.

#### Scenario: Init in a fresh repo
- **WHEN** `git paw init` is run in a git repo with no `.git-paw/` directory
- **THEN** `.git-paw/` and `.git-paw/logs/` SHALL exist

#### Scenario: Init when .git-paw already exists
- **WHEN** `git paw init` is run in a repo that already has `.git-paw/`
- **THEN** the existing directory SHALL be preserved and `.git-paw/logs/` SHALL be created if missing

### Requirement: Init generates default config.toml

The system SHALL create `.git-paw/config.toml` with sensible defaults and commented-out v0.2.0 fields when no config exists.

#### Scenario: Config created with defaults
- **WHEN** `git paw init` is run and no `.git-paw/config.toml` exists
- **THEN** `.git-paw/config.toml` SHALL be created with `default_cli` and `mouse` fields and commented examples for `default_spec_cli`, `branch_prefix`, `[specs]`, and `[logging]`

#### Scenario: Existing config is not overwritten
- **WHEN** `git paw init` is run and `.git-paw/config.toml` already exists
- **THEN** the existing config SHALL NOT be modified

### Requirement: Init appends logs directory to .gitignore

The system SHALL also ensure `.git-paw/session-summary.md` is listed in the repo's `.gitignore`, in addition to `.git-paw/logs/`.

#### Scenario: Gitignore includes session-summary.md after init

- **GIVEN** `git paw init` is run in a repo without `.git-paw/session-summary.md` in `.gitignore`
- **WHEN** init completes
- **THEN** `.gitignore` SHALL contain `.git-paw/session-summary.md`

#### Scenario: Gitignore not duplicated on repeated init

- **GIVEN** `.gitignore` already contains `.git-paw/session-summary.md`
- **WHEN** `git paw init` is run again
- **THEN** `.git-paw/session-summary.md` SHALL appear exactly once in `.gitignore`

### Requirement: Init is idempotent

Running `git paw init` multiple times SHALL produce the same result as running it once.

#### Scenario: Double init produces identical state
- **WHEN** `git paw init` is run twice in the same repo
- **THEN** the resulting `.git-paw/` directory, config, and `.gitignore` SHALL be identical to a single run

### Requirement: Init requires a git repository

The system SHALL fail with an actionable error when run outside a git repository.

#### Scenario: Init outside git repo
- **WHEN** `git paw init` is run in a directory that is not a git repository
- **THEN** the command SHALL fail with an error containing "Not a git repository"

### Requirement: Init reports what it created

The system SHALL print a summary of actions taken (directories created, files written, files skipped).

#### Scenario: Init in fresh repo shows all actions
- **WHEN** `git paw init` is run in a repo with no prior setup
- **THEN** stdout SHALL report creation of `.git-paw/`, `config.toml`, `logs/`, and `.gitignore` update

#### Scenario: Init in already-initialized repo shows skips
- **WHEN** `git paw init` is run in an already-initialized repo
- **THEN** stdout SHALL report that existing files were skipped

### Requirement: Init merges new config fields without mutating existing ones

When `git paw init` runs on a repo with an existing `.git-paw/config.toml`, the system SHALL compare the generated default config against the existing file and append only sections that are absent. The system SHALL NOT modify or remove any existing content.

This makes `init` a safe upgrade path for every version bump — users run `git paw init` after upgrading and new config sections are added without touching their customized settings.

#### Scenario: Init preserves existing broker config while adding supervisor

- **GIVEN** a `.git-paw/config.toml` with `[broker]` section containing `port = 9200` (non-default) but no `[supervisor]` section
- **WHEN** `git paw init` is run
- **THEN** the `[broker]` section SHALL still contain `port = 9200`
- **AND** a `[supervisor]` section SHALL be appended

#### Scenario: Init does not duplicate existing sections

- **GIVEN** a `.git-paw/config.toml` that already has both `[broker]` and `[supervisor]` sections
- **WHEN** `git paw init` is run
- **THEN** no sections SHALL be added or modified
- **AND** the file content SHALL be unchanged

#### Scenario: Init on a completely empty config file adds all sections

- **GIVEN** a `.git-paw/config.toml` that exists but is empty
- **WHEN** `git paw init` is run
- **THEN** all default sections SHALL be appended (commented out)

