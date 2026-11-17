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

### Requirement: Start warns once about tracked generated files

When doctor's tracked-generated-files check would flag a file, each `git paw start` run that launches or recovers a session SHALL print one stderr warning, before attaching. It SHALL carry the same entries and `git rm -r --cached <paths>` remedy as the doctor row. With nothing tracked, or a failed git query, start SHALL print nothing for it. The warning SHALL NOT change the exit code, the launch or the index.

#### Scenario: Committed helper copies are named at launch

- **GIVEN** a repository where `.git-paw/scripts/broker.sh` and `.git-paw/scripts/sweep.sh` are tracked
- **WHEN** `git paw start` launches a session with two agents
- **THEN** stderr SHALL contain exactly one tracked-generated-files warning
- **AND** the warning SHALL contain `git rm -r --cached .git-paw/scripts/`
- **AND** stdout SHALL NOT contain the warning

#### Scenario: A clean repository launches without the warning

- **GIVEN** a repository where only `.git-paw/config.toml` is tracked under `.git-paw/`
- **WHEN** `git paw start` launches a session
- **THEN** stderr SHALL NOT contain a tracked-generated-files warning

#### Scenario: The warning does not change the outcome

- **GIVEN** a repository with tracked generated files
- **WHEN** `git paw start` launches a session that would otherwise succeed
- **THEN** the session SHALL launch and the exit code SHALL be the same as without tracked files
- **AND** the index SHALL be unchanged afterwards
