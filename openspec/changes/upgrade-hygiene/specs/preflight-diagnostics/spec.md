## ADDED Requirements

### Requirement: Tracked generated files are flagged

`git paw doctor` SHALL report in the Hygiene group, as ⚠, every file tracked in the main checkout's index under the paths `git paw init` adds to `.gitignore` (everything git-paw writes under `.git-paw/` except `config.toml`). The remedy SHALL name the exact `git rm -r --cached <paths>` command followed by a commit. The check SHALL be read-only and SHALL NOT change the index.

#### Scenario: Committed helper copies are flagged

- **GIVEN** a repository where `.git-paw/scripts/broker.sh` and `.git-paw/scripts/sweep.sh` are tracked
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ naming the tracked files
- **AND** the remedy SHALL contain `git rm -r --cached .git-paw/scripts/`
- **AND** the index SHALL be unchanged afterwards

#### Scenario: A tracked config is not flagged

- **GIVEN** a repository where only `.git-paw/config.toml` is tracked under `.git-paw/`
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ✓ for tracked generated files

### Requirement: Doctor names one command for an outdated repository setup

When any finding that `git paw init` repairs is present — a missing `.gitignore` entry, a missing or stale bundled helper or compatibility copy, or a stale orchestrator pointer — `git paw doctor` SHALL add one Hygiene ⚠ stating that the repository setup is behind this git-paw version, with the count of such findings and the remedy `git paw init`. The individual findings SHALL still be reported.

#### Scenario: A repository set up by an older release

- **GIVEN** a repository whose `.gitignore` lacks five of the managed entries and whose `.git-paw/scripts/sweep.sh` differs from the bundled copy
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report one ⚠ naming `git paw init` as the fix for the outdated setup
- **AND** the `.gitignore` and stale-helper findings SHALL still be listed

#### Scenario: An up-to-date repository has no upgrade row

- **GIVEN** a repository on which this git-paw version ran `git paw init` with no later change
- **WHEN** `git paw doctor` is run
- **THEN** no outdated-setup ⚠ SHALL be reported
