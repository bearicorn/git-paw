## ADDED Requirements

### Requirement: Registry file location and versioned schema

The system SHALL keep the registry of git-paw-managed repositories in one JSON file at `<data dir>/git-paw/repos.json`, where `<data dir>` is the platform data directory resolved by the built-in platform-directory helper (`crate::dirs::data_dir()`): `~/Library/Application Support` on macOS, `$XDG_DATA_HOME` or `~/.local/share` on Linux. The document SHALL carry a top-level integer `version`, equal to `1` for this schema, and a `repos` array. Each entry SHALL carry an `id`, the repository's absolute canonical `path`, and `registered_at` as an ISO 8601 UTC timestamp. A missing registry file SHALL be treated as an empty registry. A writer SHALL preserve every top-level and per-entry field it does not recognise.

#### Scenario: Registry is written to the platform data directory

- **GIVEN** `HOME` points at a temporary directory and `XDG_DATA_HOME` is unset
- **WHEN** a repository is registered
- **THEN** `<data dir>/git-paw/repos.json` SHALL exist
- **AND** it SHALL parse as a document with `version` equal to `1` and one `repos` entry carrying `id`, `path` and `registered_at`

#### Scenario: Missing registry reads as empty

- **GIVEN** no `repos.json` exists
- **WHEN** the registry is loaded
- **THEN** the result SHALL be an empty registry and SHALL NOT be an error

#### Scenario: Unrecognised fields survive a rewrite

- **GIVEN** a version-1 registry whose top level and one entry each carry a field this binary does not define
- **WHEN** another repository is registered
- **THEN** both unrecognised fields SHALL be present, unchanged, in the rewritten file

### Requirement: Repository identifiers

The system SHALL give each registry entry an `id` derived from the repository's directory name. The name is lowercased (ASCII), every character outside `[a-z0-9_-]` becomes `-`, runs of `-` collapse to one, and leading and trailing `-` are removed. An empty result becomes `repo`. The `id` SHALL be unique within the registry: when the derived id is taken, the system SHALL append the smallest free suffix `-2`, `-3`, and so on. Every `id`, including its suffix, SHALL be at most 64 characters. An existing entry's `id` SHALL NOT change while the entry exists.

#### Scenario: Directory names are slugified

- **WHEN** repositories whose directory names are `My Project`, `api.v2`, `snake_case` and `...` are registered into an empty registry
- **THEN** their ids SHALL be `my-project`, `api-v2`, `snake_case` and `repo` respectively

#### Scenario: Colliding names get a numeric suffix

- **GIVEN** a registry holding a repository with id `api`
- **WHEN** a different repository whose directory is also named `api` is registered
- **THEN** the new entry's id SHALL be `api-2`
- **AND** the existing entry's id SHALL still be `api`

#### Scenario: Re-registering keeps the id

- **GIVEN** a registered repository with id `api-2`
- **WHEN** the same repository is registered again
- **THEN** the registry SHALL still hold exactly one entry for that path, with id `api-2`

### Requirement: Only main working trees with a git-paw directory are registered

The registry SHALL contain only repositories whose canonical root is a main working tree, meaning its `.git` entry is a directory, and that contain a `.git-paw/` directory. The system SHALL register the canonical form of the root, so a repository reached through a symbolic link has a single entry. A linked worktree, meaning a directory whose `.git` entry is a file, SHALL never be registered.

#### Scenario: Linked worktree is not registered

- **GIVEN** a linked worktree created with `git worktree add` that contains a `.git-paw/` directory
- **WHEN** registration is attempted for the worktree root
- **THEN** the registry SHALL NOT contain an entry for the worktree path

#### Scenario: Symlinked path registers the canonical path once

- **GIVEN** a repository at `/real/proj` and a symbolic link `/link/proj` pointing at it
- **WHEN** the repository is registered once through each path
- **THEN** the registry SHALL hold exactly one entry, whose `path` is the canonical `/real/proj`

### Requirement: Stale entries are pruned on write and refused on use

Every registry update SHALL first remove each entry whose `path` is no longer a directory that contains both a `.git` directory and a `.git-paw/` directory. This removal happens before the entry being added is placed, so a re-registered repository can take back a freed id. Before serving an entry, any consumer SHALL re-validate it: its `path` must canonicalize to itself, it must still meet the registration conditions, and it must not be excluded by its configuration. A configuration that cannot be loaded counts as a failure, because exclusion cannot be ruled out. An entry that fails re-validation SHALL be treated as unavailable, with a human-readable reason, and SHALL NOT be served.

#### Scenario: Deleted repository is pruned at the next registration

- **GIVEN** a registry with entries for repositories `a` and `b`, and `b`'s directory has since been deleted
- **WHEN** repository `c` is registered
- **THEN** the registry SHALL contain entries for `a` and `c` only

#### Scenario: Moved repository keeps its id when re-registered

- **GIVEN** a registered repository with id `proj` at `/old/proj` that has been moved to `/new/proj`
- **WHEN** the repository is registered from `/new/proj`
- **THEN** the registry SHALL hold one entry with id `proj` and path `/new/proj`
- **AND** no entry for `/old/proj` SHALL remain

#### Scenario: Stale entry is unavailable before any write prunes it

- **GIVEN** a registry entry whose directory no longer exists and no registry write has happened since
- **WHEN** a consumer resolves the entry
- **THEN** the entry SHALL be reported unavailable, with a reason stating the path no longer exists
- **AND** nothing under that path SHALL be read

#### Scenario: Swapped symbolic link is refused

- **GIVEN** a registered path that has since been replaced by a symbolic link to a different directory
- **WHEN** a consumer resolves the entry
- **THEN** the entry SHALL be reported unavailable, because its path no longer canonicalizes to itself

#### Scenario: Unloadable configuration makes the entry unavailable

- **GIVEN** a registered repository whose `.git-paw/config.toml` is not valid TOML
- **WHEN** a consumer resolves the entry
- **THEN** the entry SHALL be reported unavailable, with a reason stating its configuration could not be loaded

### Requirement: Concurrent registry updates are serialized and atomic

A registry update SHALL hold an exclusive operating-system advisory lock on the sidecar file `repos.json.lock` for its whole read-modify-write. It SHALL wait at most a bounded time for that lock and SHALL fail without modifying the registry when the wait expires. It SHALL replace `repos.json` by writing a temporary file in the same directory and renaming it over the registry. Readers SHALL NOT need the lock, and SHALL always observe either the complete previous document or the complete new one. A lock held by a process that has exited SHALL NOT block later updates and SHALL NOT need manual cleanup.

#### Scenario: Two concurrent inits both register

- **GIVEN** two initialized repositories and an empty registry
- **WHEN** `git paw init` runs in both at the same time
- **THEN** the registry SHALL parse, and SHALL contain an entry for each repository

#### Scenario: Readers never see a partial document

- **GIVEN** a loop of registry updates running concurrently
- **WHEN** the registry file is read repeatedly during the updates
- **THEN** every read SHALL parse as a complete version-1 document

#### Scenario: Lock held past the bound fails without modification

- **GIVEN** another process holds the registry lock for longer than the wait bound
- **WHEN** a registry update is attempted
- **THEN** the update SHALL fail with an error naming the lock
- **AND** `repos.json` SHALL be byte-identical to its prior content

#### Scenario: A lock left by a killed process does not block

- **GIVEN** a process acquired the registry lock and was then killed
- **WHEN** a registry update is attempted
- **THEN** the update SHALL succeed without any lock file being removed by hand

### Requirement: Registry file integrity and permissions

On Unix the system SHALL create the registry file, its temporary files and its lock file with mode `0600`. Readers and writers SHALL refuse a registry file that is group-writable or world-writable. A reader SHALL refuse a registry whose `version` is newer than it supports. A writer SHALL NOT overwrite a registry that is newer than it supports or that it cannot parse. Every refusal SHALL name the file and the remedy. Readers SHALL skip, and writers SHALL drop, any entry whose `id` breaks the identifier rules or whose `path` is not absolute.

#### Scenario: Registry is created owner-only

- **WHEN** the registry is first written on Unix
- **THEN** `repos.json` and `repos.json.lock` SHALL each have mode `0600`

#### Scenario: Group-writable registry is refused

- **GIVEN** `repos.json` has mode `0664`
- **WHEN** the registry is loaded
- **THEN** loading SHALL fail with an error naming the file and telling the user to restrict its permissions or delete it

#### Scenario: Newer schema version is neither read nor overwritten

- **GIVEN** `repos.json` declares `version` equal to `2`
- **WHEN** a repository is registered
- **THEN** the registration SHALL fail with an error naming the file and the unsupported version
- **AND** the file SHALL be byte-identical to its prior content

#### Scenario: Unparseable registry is not overwritten

- **GIVEN** `repos.json` contains invalid JSON
- **WHEN** a repository is registered
- **THEN** the registration SHALL fail with an error naming the file
- **AND** the file SHALL be byte-identical to its prior content

#### Scenario: Malformed entries are ignored

- **GIVEN** a version-1 registry with one valid entry, one entry whose `path` is relative, and one entry whose `id` contains `/`
- **WHEN** the registry is loaded
- **THEN** only the valid entry SHALL be returned

### Requirement: Repositories can opt out of the registry

A repository whose merged configuration sets `[mcp] register = false` SHALL NOT be registered. A registration attempt for such a repository SHALL remove any existing entry for its path. Any consumer SHALL treat an existing entry for an excluded repository as excluded: it SHALL neither list nor serve that entry.

#### Scenario: Opting out removes the entry

- **GIVEN** a registered repository whose configuration is changed to `[mcp] register = false`
- **WHEN** registration is attempted for that repository
- **THEN** the registry SHALL contain no entry for its path

#### Scenario: Consumers honour an opt-out before the entry is removed

- **GIVEN** a registry entry for a repository whose configuration now sets `[mcp] register = false`, and no registry write has happened since
- **WHEN** a consumer lists or resolves registry entries
- **THEN** that repository SHALL be neither listed nor resolvable
