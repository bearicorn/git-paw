## ADDED Requirements

### Requirement: Declared repo-local caches are shared into each worktree by symlink

git-paw SHALL accept an optional `[worktree.shared_deps]` table with `links`, a list of repository-relative directory paths. The effective entries SHALL be exactly the `links` entries. When a worktree is created, after env-file copy and port allocation and before the `on_create` hook, git-paw SHALL create, for each effective entry that passes the safety rules, a symlink at that path in the worktree pointing at the same path in the repository's main working tree, creating the main-tree directory first when it does not exist. A worktree's link SHALL be created only when nothing exists at that path in the worktree. When the table is absent, or `links` is empty, worktree creation SHALL be unchanged from prior versions and a config without the table SHALL load unchanged.

#### Scenario: Declared cache is linked

- **GIVEN** `[worktree.shared_deps] links = ["cache/pkgs"]`, `cache/pkgs` gitignored and populated in the main working tree
- **WHEN** a worktree is created
- **THEN** `cache/pkgs` in the worktree SHALL be a symlink resolving to the main working tree's `cache/pkgs`
- **AND** it SHALL exist before the `on_create` hook runs

#### Scenario: Missing main-tree cache is created empty

- **GIVEN** a declared, gitignored entry that does not exist in the main working tree
- **WHEN** a worktree is created
- **THEN** the main-tree directory SHALL be created empty and the worktree link SHALL point at it

#### Scenario: Absent table changes nothing

- **GIVEN** no `[worktree.shared_deps]` table
- **WHEN** a worktree is created
- **THEN** no symlink SHALL be created and the worktree SHALL be identical to one created by the previous version

#### Scenario: Only declared links are shared

- **GIVEN** `[worktree.shared_deps] links = ["cache/pkgs"]`
- **WHEN** the effective entries are resolved
- **THEN** they SHALL be exactly `cache/pkgs`
- **AND** with `links = []` they SHALL be empty

### Requirement: Shared-cache entries never reach tracked files or build output

Each effective entry SHALL be refused — skipped with a warning naming the entry and the reason, never failing worktree creation — when any of the following holds: it is absolute or escapes the repository (`..`); it is not ignored by git in the main working tree; any tracked file lies under it; it equals, lies inside, or contains a build or install output on git-paw's built-in forbidden-output list; or something already exists at that path in the worktree. The forbidden-output list SHALL name outputs of several stacks (such as `target`, `node_modules`, `.venv`) and SHALL apply to every entry whatever the project's stack. git-paw SHALL never delete or replace an existing file or directory to create a link.

#### Scenario: Build output is refused

- **GIVEN** `links = ["node_modules"]` or `links = ["target/debug"]`
- **WHEN** a worktree is created
- **THEN** no link SHALL be created for that entry and a warning SHALL name it as build or install output

#### Scenario: Tracked content is refused

- **GIVEN** `links = ["vendor"]` where `vendor/` contains tracked files
- **WHEN** a worktree is created
- **THEN** no link SHALL be created and the tracked `vendor/` checkout SHALL be unchanged

#### Scenario: Escaping path is refused

- **GIVEN** `links = ["../shared"]`
- **WHEN** a worktree is created
- **THEN** no link SHALL be created and a warning SHALL be printed

#### Scenario: Existing path is never replaced

- **GIVEN** a declared entry that an earlier step already created as a directory in the worktree
- **WHEN** the shared-cache step runs
- **THEN** the directory SHALL be left untouched and the entry skipped with a warning

### Requirement: Worktree teardown never deletes a shared cache

When a worktree is torn down (`git paw remove`, `git paw purge`, or any command reusing the shared per-worktree teardown), git-paw SHALL remove every shared-cache symlink in that worktree that points into the main working tree before running the `on_remove` hook and before removing the worktree directory. The shared cache's contents in the main working tree SHALL be unchanged by the teardown.

#### Scenario: Removing a worktree keeps the cache

- **GIVEN** a worktree with a shared-cache link to the main tree's `cache/pkgs` holding 10 files
- **WHEN** the user runs `git paw remove <branch>` (or `git paw purge --force`)
- **THEN** the worktree SHALL be removed
- **AND** the main tree's `cache/pkgs` SHALL still hold the same 10 files

### Requirement: Shared caches are declared explicitly and export-agnostic

git-paw SHALL share only the caches the operator lists in `links`. It SHALL NOT derive shared-cache entries from any stack or preset, SHALL NOT enable any entry by default, and SHALL NOT treat its own toolchain as a default. The generated default config SHALL show `[worktree.shared_deps]` only as commented examples for more than one stack.

#### Scenario: Default config has no active shared-cache entry

- **WHEN** `git paw init` generates the default config
- **THEN** any `[worktree.shared_deps]` lines SHALL be commented out and SHALL show examples for more than one stack

#### Scenario: No links, no entries

- **GIVEN** a `[worktree.shared_deps]` table with no `links`
- **WHEN** a worktree is created
- **THEN** no shared-cache link SHALL be created
