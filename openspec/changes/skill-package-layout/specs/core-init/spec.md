## MODIFIED Requirements

### Requirement: Init installs the shared shell preamble

The system SHALL install a bundled shared shell preamble at
`<repo>/.git-paw/scripts/_paw_common.sh` when `git paw init` runs, alongside the
`broker.sh`, `sweep.sh`, and `docs-fetch.sh` helpers. The file SHALL be written
with executable mode `0o755` on Unix and SHALL be overwritten on re-run
(binary-managed content). Init SHALL report whether the file was created or
updated, consistent with the other bundled scripts.

Each bundled helper (`broker.sh`, `sweep.sh`, `docs-fetch.sh`) SHALL obtain its
shared functions — repository-root resolution, Python-3 interpreter detection,
config-value reading, broker-URL / docs-base-URL discovery, and branch slugify —
by sourcing `_paw_common.sh`, and SHALL NOT re-define those functions inline. A
helper SHALL resolve the sourced path relative to its own script location, and
the preamble SHALL be installed as a sibling of every helper wherever that
helper is installed, so the source succeeds from any installed location of the
helper. The preamble SHALL have exactly one source file in the bundled package
tree; every package that ships a helper SHALL carry that same content. The
observable behavior of every helper SHALL be unchanged from the pre-refactor
inline implementation.

The shared preamble SHALL remain project-agnostic: it SHALL source the broker URL
and docs base URL from resolved configuration and SHALL NOT hard-code any
consumer's toolchain.

#### Scenario: Init installs _paw_common.sh in a fresh repo

- **WHEN** `git paw init` is run in a git repo with no `.git-paw/` directory
- **THEN** `<repo>/.git-paw/scripts/_paw_common.sh` SHALL exist
- **AND** its first line SHALL be a bash shebang
- **AND** on Unix it SHALL have the user/group/other execute bits set

#### Scenario: Init overwrites a stale _paw_common.sh

- **GIVEN** a `<repo>/.git-paw/scripts/_paw_common.sh` containing stale local
  content
- **WHEN** `git paw init` is run
- **THEN** the file SHALL be overwritten with the bundled preamble content
- **AND** init SHALL report that `.git-paw/scripts/_paw_common.sh` was updated

#### Scenario: Bundled helpers source the shared preamble and do not re-define it

- **GIVEN** the bundled `broker.sh`, `sweep.sh`, and `docs-fetch.sh`
- **WHEN** their content is inspected
- **THEN** each SHALL source `_paw_common.sh` (resolved relative to its own
  location)
- **AND** none SHALL contain its own inline definition of a function provided by
  `_paw_common.sh` (`repo_root`, the interpreter detector, `discover_broker_url`,
  `discover_docs_base_url`, `slugify`)

#### Scenario: A deployed helper resolves shared functions from the sourced file

- **GIVEN** `_paw_common.sh` and a helper are deployed together in
  `<repo>/.git-paw/scripts/`
- **WHEN** the helper runs a command that uses a shared function (for example
  `broker.sh` resolving the broker URL)
- **THEN** the shared function SHALL resolve from `_paw_common.sh`
- **AND** the result SHALL equal what the pre-refactor inline definition produced
  for the same inputs

#### Scenario: A package installed into an arbitrary directory runs

- **GIVEN** a helper-shipping package's scripts installed together into an empty
  temporary directory
- **WHEN** the helper is invoked from there
- **THEN** it SHALL source the sibling `_paw_common.sh` and run

#### Scenario: The preamble has one source

- **WHEN** the bundled package tree is inspected
- **THEN** exactly one `_paw_common.sh` source file SHALL exist
