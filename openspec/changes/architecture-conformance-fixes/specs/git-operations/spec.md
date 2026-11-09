## MODIFIED Requirements

### Requirement: Create worktrees as siblings of the repository

The system SHALL create a worktree at the path `worktree_placement` resolves: `<repo_root>/.git-paw/worktrees/<branch-slug>` for `"child"` (creating the directory), else `<repo_parent>/<project>-<branch-slug>`. A worktree registered at that path for the branch SHALL be reused without `git worktree add`. Any other existing path SHALL reach `git worktree add`, so git's `already exists` error shows. A new branch SHALL be created from HEAD.

The `rebase_onto_main` parameter SHALL rebase an existing branch onto the default branch as the creation-time rebase requirements describe; with `false`, no rebase SHALL run.

#### Scenario: Worktree created at correct path

- **GIVEN** a repository with a branch `feature/test`
- **WHEN** `create_worktree()` is called with `rebase_onto_main = false`
- **THEN** a worktree SHALL be created at `../<project>-feature-test` containing the repository files

#### Scenario: Worktree created under child placement

- **GIVEN** a repository whose effective config has `worktree_placement = "child"` and a branch `feature/test`
- **WHEN** `create_worktree()` is called with `rebase_onto_main = false`
- **THEN** a worktree SHALL be created at `<repo_root>/.git-paw/worktrees/feature-test` containing the repository files

#### Scenario: Worktree created under sibling placement

- **GIVEN** a repository whose effective config has `worktree_placement = "sibling"` and a branch `feature/test`
- **WHEN** `create_worktree()` is called with `rebase_onto_main = false`
- **THEN** a worktree SHALL be created at `../<project>-feature-test`

#### Scenario: Worktree created under absent placement defaults to sibling

- **GIVEN** a repository whose effective config has no `worktree_placement` field and a branch `feature/test`
- **WHEN** `create_worktree()` is called with `rebase_onto_main = false`
- **THEN** a worktree SHALL be created at `../<project>-feature-test`

#### Scenario: Creating worktree for currently checked-out branch fails

- **GIVEN** the current branch is checked out in the main repo
- **WHEN** `create_worktree()` is called for that branch
- **THEN** it SHALL return `Err(PawError::WorktreeError)`

#### Scenario: Resume of an existing worktree returns success without re-running git worktree add

- **GIVEN** a worktree already exists at `../<project>-feature-test` for branch `feature/test` from a prior session
- **AND** `rebase_onto_main = false` is passed
- **WHEN** `create_worktree()` is called for `feature/test`
- **THEN** the function SHALL return `Ok(WorktreeCreation { path: <expected>, branch_created: false })`
- **AND** the existing worktree SHALL remain unchanged (HEAD SHA, working tree files, and uncommitted changes preserved)
- **AND** no second `git worktree add` SHALL be executed

#### Scenario: Path exists but is not a git worktree

- **GIVEN** the expected worktree path `../<project>-feature-test` exists as a regular directory (not registered with git)
- **WHEN** `create_worktree()` is called for branch `feature/test`
- **THEN** the function SHALL return `Err(PawError::WorktreeError)` whose message contains the substring `already exists`

#### Scenario: Path exists as a worktree but for a different branch

- **GIVEN** a worktree already exists at `../<project>-feature-test` but registered for branch `feature/other`
- **WHEN** `create_worktree()` is called for branch `feature/test`
- **THEN** the function SHALL fall through to `git worktree add` and return `Err(PawError::WorktreeError)`

#### Scenario: Rebase-on-resume happy path advances branch onto current main

- **GIVEN** the default branch `main` has advanced by N commits since branch `feat/example` was created
- **AND** `feat/example` exists locally and is behind `main` by exactly N commits with no diverging commits of its own
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = true`
- **THEN** the function SHALL run `git rebase <main>` inside `feat/example`'s own worktree, never in the main checkout
- **AND** the rebase SHALL succeed
- **AND** `feat/example`'s HEAD SHA after the call SHALL be reachable from `main` (i.e. include the N new commits)
- **AND** the function SHALL return `Ok(WorktreeCreation { path: <expected>, branch_created: false })`

#### Scenario: Rebase skipped when branch is already up-to-date

- **GIVEN** branch `feat/example` exists locally and is at the same SHA as `main` (no divergence)
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = true`
- **THEN** `git rebase <main>` SHALL be invoked in the branch's worktree and SHALL exit zero with no rewrite
- **AND** `feat/example`'s HEAD SHA SHALL be unchanged
- **AND** the function SHALL return `Ok(WorktreeCreation { path: <expected>, branch_created: false })`
- **AND** no error SHALL be returned

#### Scenario: Rebase conflict aborts cleanly and surfaces error

- **GIVEN** branch `feat/example` and `main` both modify the same line of the same file with different content
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = true`
- **THEN** `git rebase <main>` SHALL be invoked in the branch's worktree and SHALL exit non-zero with conflict markers
- **AND** the function SHALL invoke `git rebase --abort` in that same worktree for the rebase it started
- **AND** `feat/example`'s HEAD SHA after the call SHALL equal its pre-call HEAD SHA
- **AND** no rebase state SHALL remain in that worktree's git dir
- **AND** the function SHALL return `Err(PawError::WorktreeError(msg))` where `msg` contains the substring `rebase onto main failed`
- **AND** a worktree created for the call SHALL have been removed again (one that existed from a prior session SHALL be unchanged)

#### Scenario: rebase_onto_main = false preserves v0.5 no-rebase behaviour

- **GIVEN** branch `feat/example` exists locally and is behind `main` by 3 commits
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = false`
- **THEN** no `git rebase` invocation SHALL occur
- **AND** `feat/example`'s HEAD SHA after the call SHALL equal its pre-call HEAD SHA
- **AND** the function SHALL proceed to the existence check and, if applicable, `git worktree add`

#### Scenario: New branch creation skips rebase regardless of flag

- **GIVEN** branch `feat/new` does NOT exist in the local repository
- **AND** `rebase_onto_main = true` is passed
- **WHEN** `create_worktree()` is called for `feat/new`
- **THEN** no `git rebase` invocation SHALL occur (there is nothing to rebase)
- **AND** the function SHALL invoke `git worktree add -b feat/new <path>` to create the branch from current HEAD
- **AND** the function SHALL return `Ok(WorktreeCreation { path, branch_created: true })`

### Requirement: Mid-session rebase of a live worker branch

The system SHALL provide a rebase entry point that rebases a named branch onto the repository's default branch while that branch is checked out in a live worktree, separate from `create_worktree`'s creation-time `rebase_onto_main` path. The entry point SHALL run the rebase inside the worktree where the branch is currently checked out. On a non-zero `git rebase` exit it SHALL invoke `git rebase --abort` and return an error, leaving the branch at its pre-rebase HEAD. A branch already at or ahead of the default branch SHALL be a successful no-op.

The creation-time rebase SHALL follow its own requirements, unaffected by this entry point.

#### Scenario: Rebase runs in the worktree holding the branch

- **GIVEN** a branch checked out in a live worktree
- **WHEN** the mid-session rebase entry point is invoked for that branch
- **THEN** the rebase SHALL be run inside that worktree

#### Scenario: Failed mid-session rebase aborts and restores

- **GIVEN** a mid-session rebase that exits non-zero
- **WHEN** the failure is detected
- **THEN** `git rebase --abort` SHALL be invoked
- **AND** the branch SHALL be left at its pre-rebase HEAD
- **AND** an error SHALL be returned

#### Scenario: Branch already current is a successful no-op

- **GIVEN** a branch already at or ahead of the default branch
- **WHEN** the mid-session rebase entry point is invoked for that branch
- **THEN** it SHALL return success
- **AND** the branch SHALL NOT be rewritten

#### Scenario: Creation-time rebase contract is unaffected

- **WHEN** `create_worktree()` is called with `rebase_onto_main = true` for an existing branch
- **THEN** it SHALL rebase in the branch's own worktree, skip a dirty worktree or a stopped rebase, and refuse a branch the main checkout holds, as the creation-time rebase requirements state

### Requirement: Every launch path reports a skipped rebase

`create_worktree` SHALL return the reason a rebase was skipped. `start` (plain, `--supervisor`, spec-driven) and `add` SHALL print it as one stderr notice naming the branch and `--no-rebase`, never on stdout. `add` SHALL check before creating the worktree, so the notice prints even if creation fails, and SHALL also print a skip `create_worktree` returns. MCP `start_session` SHALL return the reason in `warnings` without naming `--no-rebase`. With `--no-rebase` no check or notice SHALL occur.

#### Scenario: start prints the skip notice on stderr

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw start` for `feat/x`
- **THEN** stderr SHALL contain one notice naming `feat/x` and `--no-rebase`
- **AND** stdout SHALL contain nothing about the skip

#### Scenario: A supervised launch completes over a dirty worktree

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw start --supervisor` for `feat/x`
- **THEN** the session SHALL launch with `feat/x` on its current HEAD
- **AND** stderr SHALL contain the skip notice naming `feat/x` and `--no-rebase`

#### Scenario: MCP start_session reports the skip as a warning

- **GIVEN** a root MCP session and a surviving worktree for `feat/x` with uncommitted changes
- **WHEN** the client calls `start_session({ "branches": ["feat/x"], "cli": "<cli>" })`
- **THEN** the response SHALL be successful and its `warnings` SHALL contain one skip notice naming `feat/x` and its reason
- **AND** that notice SHALL NOT name `--no-rebase`, which the tool does not accept
- **AND** the MCP server's stdout SHALL carry only protocol messages

#### Scenario: --no-rebase on a dirty worktree prints no notice

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes
- **WHEN** the user runs `git paw start --no-rebase` for `feat/x`
- **THEN** the branch SHALL NOT be rebased, the changes SHALL be unchanged and no skip notice SHALL be printed

#### Scenario: add keeps its notice text

- **GIVEN** a kept worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw add feat/x`
- **THEN** stderr SHALL contain `notice: rebase skipped for 'feat/x': its worktree has uncommitted changes, which are left untouched. Pass --no-rebase to attach without a rebase.`
