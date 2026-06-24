## MODIFIED Requirements

### Requirement: Create worktrees as siblings of the repository

The system SHALL create git worktrees at a path resolved from the
configured `worktree_placement` setting. When `worktree_placement` is
`"child"`, the target path SHALL be
`<repo_root>/.git-paw/worktrees/<branch-slug>` and the
`.git-paw/worktrees/` directory SHALL be created if absent. When
`worktree_placement` is `"sibling"` OR the setting is absent, the target
path SHALL be `<repo_parent>/<project>-<branch-slug>` using the derived
directory name convention, matching the v0.7.0 sibling layout. Only the
resolved target path varies with placement; all other behaviour described
below is unchanged.

The `create_worktree` function SHALL accept a `rebase_onto_main: bool` parameter. When `rebase_onto_main` is `true` AND the target branch already exists in the local repository, the function SHALL rebase the target branch onto the repository's default branch (as returned by `default_branch()`) BEFORE performing the existence check for the worktree directory. The rebase SHALL be performed by invoking `git rebase <default-branch>` from the repository root. When the branch is already at or ahead of the default branch, `git rebase` exits zero with no rewrite; the function SHALL treat that as success.

If `git rebase` exits non-zero (rebase conflict or any other failure), the function SHALL invoke `git rebase --abort` in the repository root and return `Err(PawError::WorktreeError("rebase onto main failed: <stderr>"))`. The branch SHALL be left at its pre-rebase HEAD after the abort; the function SHALL NOT proceed to the existence check or `git worktree add` when the rebase failed.

If `rebase_onto_main` is `false`, the function SHALL skip the rebase block entirely and behave identically to the post-`worktree-resume-fix` v0.5.0 contract (idempotent existence check followed by `git worktree add`).

If the target branch does NOT exist in the local repository at the time `create_worktree` is invoked, the function SHALL skip the rebase block regardless of the `rebase_onto_main` value and proceed to the existing `git worktree add -b <branch>` fallback, which creates the branch from current HEAD (already at the default branch tip by construction).

The system SHALL be idempotent in the resume case: when `create_worktree()` is invoked for a branch whose worktree already exists at the expected path AND is registered with git for that branch, the function SHALL return `Ok(WorktreeCreation { path, branch_created: false })` without re-running `git worktree add`. Idempotency is verified by parsing `git worktree list --porcelain` output and matching both the worktree path and the `refs/heads/<branch>` line. When `rebase_onto_main` is `true`, the rebase block runs BEFORE this idempotency check, so a surviving worktree's branch ref SHALL be updated to the rebased SHA before the function returns.

If the expected path exists on disk but is NOT a git worktree registered for the specified branch (e.g. an unrelated directory, or a worktree for a different branch), the function SHALL fall through to the existing `git worktree add` call so the user sees the actionable `fatal: '<path>' already exists` error from git directly.

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
- **THEN** the function SHALL fall through to `git worktree add` and return `Err(PawError::WorktreeError)` (preserving the v0.4 contract for unrelated path collisions)

#### Scenario: Rebase-on-resume happy path advances branch onto current main

- **GIVEN** the default branch `main` has advanced by N commits since branch `feat/example` was created
- **AND** `feat/example` exists locally and is behind `main` by exactly N commits with no diverging commits of its own
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = true`
- **THEN** the function SHALL invoke `git rebase <main>` against `feat/example` in the repository root
- **AND** the rebase SHALL succeed
- **AND** `feat/example`'s HEAD SHA after the call SHALL be reachable from `main` (i.e. include the N new commits)
- **AND** the function SHALL return `Ok(WorktreeCreation { path: <expected>, branch_created: false })`

#### Scenario: Rebase skipped when branch is already up-to-date

- **GIVEN** branch `feat/example` exists locally and is at the same SHA as `main` (no divergence)
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = true`
- **THEN** `git rebase <main>` SHALL be invoked and SHALL exit zero with no rewrite
- **AND** `feat/example`'s HEAD SHA SHALL be unchanged
- **AND** the function SHALL return `Ok(WorktreeCreation { path: <expected>, branch_created: false })`
- **AND** no error SHALL be returned

#### Scenario: Rebase conflict aborts cleanly and surfaces error

- **GIVEN** branch `feat/example` and `main` both modify the same line of the same file with different content
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = true`
- **THEN** `git rebase <main>` SHALL be invoked and SHALL exit non-zero with conflict markers
- **AND** the function SHALL invoke `git rebase --abort`
- **AND** `feat/example`'s HEAD SHA after the call SHALL equal its pre-call HEAD SHA
- **AND** no `.git/rebase-merge` or `.git/rebase-apply` directory SHALL remain in the repository
- **AND** the function SHALL return `Err(PawError::WorktreeError(msg))` where `msg` contains the substring `rebase onto main failed`
- **AND** the worktree directory at `../<project>-feat-example` SHALL NOT have been created (or, if it existed from a prior session, SHALL be unchanged)

#### Scenario: rebase_onto_main = false preserves v0.5 no-rebase behaviour

- **GIVEN** branch `feat/example` exists locally and is behind `main` by 3 commits
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = false`
- **THEN** no `git rebase` invocation SHALL occur
- **AND** `feat/example`'s HEAD SHA after the call SHALL equal its pre-call HEAD SHA
- **AND** the function SHALL proceed to the existence check and (if applicable) `git worktree add`, matching the post-`worktree-resume-fix` v0.5.0 contract exactly

#### Scenario: New branch creation skips rebase regardless of flag

- **GIVEN** branch `feat/new` does NOT exist in the local repository
- **AND** `rebase_onto_main = true` is passed
- **WHEN** `create_worktree()` is called for `feat/new`
- **THEN** no `git rebase` invocation SHALL occur (there is nothing to rebase)
- **AND** the function SHALL invoke `git worktree add -b feat/new <path>` to create the branch from current HEAD
- **AND** the function SHALL return `Ok(WorktreeCreation { path, branch_created: true })`
