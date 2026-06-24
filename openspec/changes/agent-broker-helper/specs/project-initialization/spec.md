## ADDED Requirements

### Requirement: Init installs the agent-broker helper script

The system SHALL install the bundled agent-broker helper
(`agent-broker-helper` capability) at
`<repo>/.git-paw/scripts/broker.sh` when `git paw init` runs, alongside
the supervisor `sweep.sh` helper. The script SHALL be written with
executable mode `0o755` on Unix and SHALL be overwritten on re-run
(binary-managed content). Init SHALL report whether the file was created
or updated.

#### Scenario: Init installs broker.sh in a fresh repo

- **WHEN** `git paw init` is run in a git repo with no `.git-paw/`
  directory
- **THEN** `<repo>/.git-paw/scripts/broker.sh` SHALL exist
- **AND** its first line SHALL be a bash shebang
- **AND** on Unix it SHALL have the user/group/other execute bits set

#### Scenario: Init overwrites a stale broker.sh

- **GIVEN** a `<repo>/.git-paw/scripts/broker.sh` containing stale local
  content
- **WHEN** `git paw init` is run
- **THEN** the file SHALL be overwritten with the bundled helper content
- **AND** init SHALL report that `.git-paw/scripts/broker.sh` was updated

#### Scenario: Init reports broker.sh creation

- **WHEN** `git paw init` is run in a repo with no prior
  `.git-paw/scripts/broker.sh`
- **THEN** stdout SHALL report creation of
  `.git-paw/scripts/broker.sh`
