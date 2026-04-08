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

The system SHALL ensure `.git-paw/logs/` is listed in the repo's `.gitignore`.

#### Scenario: Gitignore does not exist
- **WHEN** `git paw init` is run and no `.gitignore` exists
- **THEN** `.gitignore` SHALL be created containing `.git-paw/logs/`

#### Scenario: Gitignore exists without the entry
- **WHEN** `git paw init` is run and `.gitignore` exists but does not contain `.git-paw/logs/`
- **THEN** `.git-paw/logs/` SHALL be appended to `.gitignore`

#### Scenario: Gitignore already contains the entry
- **WHEN** `git paw init` is run and `.gitignore` already contains `.git-paw/logs/`
- **THEN** `.gitignore` SHALL NOT be modified

#### Scenario: Gitignore without trailing newline
- **WHEN** `git paw init` is run and `.gitignore` exists without a trailing newline
- **THEN** a newline SHALL be prepended before appending `.git-paw/logs/`

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
