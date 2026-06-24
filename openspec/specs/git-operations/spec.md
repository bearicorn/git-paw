## Purpose

Validate git repositories, list branches, create and remove worktrees, and derive worktree directory names. Provides the git plumbing that underpins parallel branch sessions.
## Requirements
### Requirement: Validate that a path is inside a git repository

The system SHALL confirm a path is inside a git repository and return the repository root.

#### Scenario: Path is inside a git repository
- **GIVEN** a path inside an initialized git repository
- **WHEN** `validate_repo()` is called
- **THEN** it SHALL return `Ok` with the absolute path to the repository root

Test: `git::tests::validate_repo_returns_root_inside_repo`

#### Scenario: Path is not inside a git repository
- **GIVEN** a path that is not inside any git repository
- **WHEN** `validate_repo()` is called
- **THEN** it SHALL return `Err(PawError::NotAGitRepo)`

Test: `git::tests::validate_repo_returns_not_a_git_repo_outside`

### Requirement: List branches sorted and deduplicated

The system SHALL list all local and remote branches, deduplicated, sorted, with remote prefixes stripped and HEAD pointers excluded.

#### Scenario: Branches are returned sorted
- **GIVEN** a repository with multiple branches
- **WHEN** `list_branches()` is called
- **THEN** it SHALL return branches sorted alphabetically

Test: `git::tests::list_branches_returns_sorted_branches`

#### Scenario: Local and remote branches are deduplicated with prefix stripping
- **GIVEN** a repository cloned from a remote, with branches existing both locally and as remote-tracking refs
- **WHEN** `list_branches()` is called
- **THEN** each branch SHALL appear exactly once, with `origin/` prefixes stripped

Test: `git_integration::list_branches_strips_remote_prefix_and_deduplicates`

### Requirement: Derive project name from repository path

The system SHALL extract the project name from the final component of the repository root path, falling back to `"project"` for root paths.

#### Scenario: Normal repository path
- **GIVEN** a repository at `/Users/jie/code/git-paw`
- **WHEN** `project_name()` is called
- **THEN** it SHALL return `"git-paw"`

Test: `git::tests::project_name_from_path`

#### Scenario: Root path fallback
- **GIVEN** a repository at `/`
- **WHEN** `project_name()` is called
- **THEN** it SHALL return `"project"`

Test: `git::tests::project_name_fallback_for_root`

### Requirement: Build worktree directory names

The system SHALL generate worktree directory names as `<project>-<sanitized-branch>`, replacing `/` with `-` and stripping unsafe characters.

#### Scenario: Branch with single slash
- **GIVEN** project `"git-paw"` and branch `"feature/auth-flow"`
- **WHEN** `worktree_dir_name()` is called
- **THEN** it SHALL return `"git-paw-feature-auth-flow"`

Test: `git::tests::worktree_dir_name_replaces_slash_with_dash`

#### Scenario: Branch with multiple slashes
- **GIVEN** project `"git-paw"` and branch `"feat/auth/v2"`
- **WHEN** `worktree_dir_name()` is called
- **THEN** it SHALL return `"git-paw-feat-auth-v2"`

Test: `git::tests::worktree_dir_name_handles_multiple_slashes`

#### Scenario: Branch with special characters
- **GIVEN** project `"my-proj"` and branch `"fix/issue#42"`
- **WHEN** `worktree_dir_name()` is called
- **THEN** unsafe characters SHALL be stripped, returning `"my-proj-fix-issue42"`

Test: `git::tests::worktree_dir_name_strips_special_chars`

#### Scenario: Simple branch name
- **GIVEN** project `"git-paw"` and branch `"main"`
- **WHEN** `worktree_dir_name()` is called
- **THEN** it SHALL return `"git-paw-main"`

Test: `git::tests::worktree_dir_name_simple_branch`

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

### Requirement: Remove worktrees and prune stale entries

The system SHALL force-remove a worktree and prune stale git worktree metadata. `remove_worktree` SHALL pass `--force` to `git worktree remove` so a worktree containing uncommitted modifications, untracked files, or both is still deleted; the function is only called from the destructive `purge` path, where leaving worktree directories on disk after the user already opted into a destructive operation is the wrong behaviour.

#### Scenario: Worktree fully cleaned up after removal
- **GIVEN** an existing worktree
- **WHEN** `remove_worktree()` is called
- **THEN** the directory SHALL be deleted and git SHALL no longer track it

Test: `git::tests::remove_worktree_cleans_up_fully`

#### Scenario: Dirty worktree is force-removed
- **GIVEN** an existing worktree containing both a modified tracked file and an untracked file
- **WHEN** `remove_worktree()` is called
- **THEN** the call SHALL succeed
- **AND** the worktree directory SHALL be deleted from disk
- **AND** git SHALL no longer track the worktree

Test: `git_integration::remove_worktree_force_removes_dirty_worktree`

### Requirement: Repository validation SHALL work against real git repos

#### Scenario: Succeeds inside a real git repo
- **GIVEN** a temporary git repository with an initial commit
- **WHEN** `validate_repo()` is called
- **THEN** it SHALL return the canonicalized repo root

Test: `git_integration::validate_repo_succeeds_inside_git_repo`

#### Scenario: Fails outside a git repo
- **GIVEN** a temporary directory that is not a git repo
- **WHEN** `validate_repo()` is called
- **THEN** it SHALL return an error

Test: `git_integration::validate_repo_fails_outside_git_repo`

### Requirement: Branch listing SHALL work against real git repos

#### Scenario: Lists created branches
- **GIVEN** a repo with branches `feature/auth` and `fix/db`
- **WHEN** `list_branches()` is called
- **THEN** both branches SHALL appear in the result

Test: `git_integration::list_branches_includes_created_branches`

#### Scenario: Branches are sorted
- **GIVEN** branches created in non-alphabetical order
- **WHEN** `list_branches()` is called
- **THEN** results SHALL be alphabetically sorted

Test: `git_integration::list_branches_returns_sorted`

#### Scenario: Deduplicates local and remote
- **GIVEN** a repository with a default branch
- **WHEN** `list_branches()` is called
- **THEN** each branch SHALL appear exactly once

Test: `git_integration::list_branches_deduplicates_local_and_remote`

### Requirement: Worktree lifecycle SHALL work against real git repos

#### Scenario: Create and remove worktree
- **GIVEN** a branch in a temporary repo
- **WHEN** `create_worktree()` then `remove_worktree()` are called
- **THEN** the worktree SHALL exist after creation and be gone after removal

Test: `git_integration::create_and_remove_worktree`

#### Scenario: Worktree placed as sibling of repo
- **GIVEN** a repo at `<sandbox>/test-repo/`
- **WHEN** `create_worktree()` is called
- **THEN** the worktree SHALL be in the same parent directory

Test: `git_integration::worktree_placed_as_sibling_of_repo`

#### Scenario: Fails for checked-out branch
- **GIVEN** the currently checked-out branch
- **WHEN** `create_worktree()` is called for it
- **THEN** it SHALL fail

Test: `git_integration::create_worktree_fails_for_checked_out_branch`

### Requirement: Directory naming SHALL be correct in integration tests

#### Scenario: Project name from real repo path
- **GIVEN** a repo at `.../test-repo/`
- **WHEN** `project_name()` is called
- **THEN** it SHALL return `"test-repo"`

Test: `git_integration::project_name_from_repo_path`

#### Scenario: Worktree dir name replaces slashes
- **WHEN** `worktree_dir_name("my-project", "feature/auth-flow")` is called
- **THEN** it SHALL return `"my-project-feature-auth-flow"`

Test: `git_integration::worktree_dir_name_replaces_slashes`

#### Scenario: Worktree dir name strips unsafe chars
- **WHEN** `worktree_dir_name("proj", "feat/special@chars!")` is called
- **THEN** `@` and `!` SHALL be stripped

Test: `git_integration::worktree_dir_name_strips_unsafe_chars`

#### Scenario: Worktree dir name handles nested slashes
- **WHEN** `worktree_dir_name("proj", "feature/deep/nested/branch")` is called
- **THEN** it SHALL return `"proj-feature-deep-nested-branch"`

Test: `git_integration::worktree_dir_name_handles_nested_slashes`

### Requirement: Worktree creation produces a usable worktree path

The `create_worktree` function SHALL create a git worktree for the given branch and return its path. Callers MAY perform post-creation setup (such as AGENTS.md generation) using the returned path.

#### Scenario: Worktree created at correct path
- **GIVEN** a git repo and a branch name
- **WHEN** `create_worktree()` is called
- **THEN** it SHALL return the path to the new worktree as a sibling of the repo directory

Test: `git::tests::create_worktree_at_correct_path`

#### Scenario: Worktree creation fails for checked-out branch
- **GIVEN** a branch that is currently checked out
- **WHEN** `create_worktree()` is called
- **THEN** it SHALL return a `PawError::WorktreeError`

Test: `git::tests::create_worktree_errors_on_checked_out_branch`

