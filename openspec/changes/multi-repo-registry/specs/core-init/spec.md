## ADDED Requirements

### Requirement: Init registers the repository in the repository registry

`git paw init` SHALL register the repository's canonical root in the repository registry after it has set up `.git-paw/`.

It SHALL NOT add an entry when the repository root is a linked worktree. It SHALL NOT add an entry when the repository's merged configuration sets `[mcp] register = false`; in that case it SHALL remove any existing entry for the repository.

Init SHALL print one line on stdout when it adds an entry, naming the entry's id. It SHALL print one line when it removes an entry.

A failure to update the registry (lock timeout, unwritable directory, or a registry that is refused as unsafe, newer or unparseable) SHALL be reported as a warning on stderr. The warning SHALL name the registry file and the remedy. Such a failure SHALL NOT change init's exit status or any of init's other effects.

Re-running init in a registered repository SHALL leave its entry unchanged.

#### Scenario: Fresh init registers the repository and reports its id

- **GIVEN** a git repository named `proj` with no `.git-paw/` directory and an empty registry, with `HOME` overridden to a temporary directory
- **WHEN** `git paw init` is run
- **THEN** the registry SHALL contain one entry whose `path` is the repository's canonical root and whose id is `proj`
- **AND** stdout SHALL contain a line reporting the registration with the id `proj`

#### Scenario: Re-running init leaves the entry unchanged

- **GIVEN** a repository that `git paw init` has already initialized and registered
- **WHEN** `git paw init` is run again
- **THEN** the registry file SHALL be byte-identical to its prior content
- **AND** stdout SHALL report `Already initialized. Nothing to do.`

#### Scenario: Init in a linked worktree does not register it

- **GIVEN** a linked worktree of an initialized repository
- **WHEN** `git paw init` is run inside the worktree
- **THEN** the registry SHALL contain no entry for the worktree path

#### Scenario: Opted-out repository is removed on init

- **GIVEN** a registered repository whose `.git-paw/config.toml` now sets `[mcp] register = false`
- **WHEN** `git paw init` is run in it
- **THEN** the registry SHALL contain no entry for the repository
- **AND** stdout SHALL report the removal

#### Scenario: Registry failure does not fail init

- **GIVEN** a registry file containing invalid JSON
- **WHEN** `git paw init` is run in a fresh repository
- **THEN** the command SHALL exit 0, and `.git-paw/` and `.git-paw/config.toml` SHALL exist
- **AND** stderr SHALL contain a warning naming the registry file
- **AND** the registry file SHALL be byte-identical to its prior content
