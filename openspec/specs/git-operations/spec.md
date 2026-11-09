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

The system SHALL create a worktree at the path `worktree_placement` resolves: `<repo_root>/.git-paw/worktrees/<branch-slug>` for `"child"` (creating the directory), else `<repo_parent>/<project>-<branch-slug>`. A worktree registered at that path for the branch SHALL be reused without `git worktree add`. Any other existing path SHALL reach `git worktree add`, so git's `already exists` error shows. A new branch SHALL be created from HEAD.

The `rebase_onto_main` parameter SHALL rebase an existing branch onto the default branch as the creation-time rebase requirements describe; with `false`, no rebase SHALL run.

#### Scenario: Worktree created at correct path

- **GIVEN** a repository with a branch `feature/test`
- **WHEN** `create_worktree()` is called with `rebase_onto_main = false`
- **THEN** a worktree SHALL be created at `../<project>-feature-test` containing the repository files

Test: `src/git.rs::create_worktree_sibling_placement_creates_beside_repo`, `tests/git_integration.rs::create_and_remove_worktree`

#### Scenario: Worktree created under child placement

- **GIVEN** a repository whose effective config has `worktree_placement = "child"` and a branch `feature/test`
- **WHEN** `create_worktree()` is called with `rebase_onto_main = false`
- **THEN** a worktree SHALL be created at `<repo_root>/.git-paw/worktrees/feature-test` containing the repository files

Test: `src/git.rs::create_worktree_child_placement_creates_inside_repo`

#### Scenario: Worktree created under sibling placement

- **GIVEN** a repository whose effective config has `worktree_placement = "sibling"` and a branch `feature/test`
- **WHEN** `create_worktree()` is called with `rebase_onto_main = false`
- **THEN** a worktree SHALL be created at `../<project>-feature-test`

Test: `src/git.rs::create_worktree_sibling_placement_creates_beside_repo`

#### Scenario: Worktree created under absent placement defaults to sibling

- **GIVEN** a repository whose effective config has no `worktree_placement` field and a branch `feature/test`
- **WHEN** `create_worktree()` is called with `rebase_onto_main = false`
- **THEN** a worktree SHALL be created at `../<project>-feature-test`

Test: `src/config/tests.rs::worktree_placement_parses_and_resolves`, `src/git.rs::create_worktree_sibling_placement_creates_beside_repo`

#### Scenario: Creating worktree for currently checked-out branch fails

- **GIVEN** the current branch is checked out in the main repo
- **WHEN** `create_worktree()` is called for that branch
- **THEN** it SHALL return `Err(PawError::WorktreeError)`

Test: `tests/git_integration.rs::create_worktree_fails_for_checked_out_branch`, `src/git.rs::create_worktree_refuses_a_branch_checked_out_in_the_main_checkout`

#### Scenario: Resume of an existing worktree returns success without re-running git worktree add

- **GIVEN** a worktree already exists at `../<project>-feature-test` for branch `feature/test` from a prior session
- **AND** `rebase_onto_main = false` is passed
- **WHEN** `create_worktree()` is called for `feature/test`
- **THEN** the function SHALL return `Ok(WorktreeCreation { path: <expected>, branch_created: false })`
- **AND** the existing worktree SHALL remain unchanged (HEAD SHA, working tree files, and uncommitted changes preserved)
- **AND** no second `git worktree add` SHALL be executed

Test: `tests/git_integration.rs::create_worktree_resume_returns_success_when_worktree_already_exists`

#### Scenario: Path exists but is not a git worktree

- **GIVEN** the expected worktree path `../<project>-feature-test` exists as a regular directory (not registered with git)
- **WHEN** `create_worktree()` is called for branch `feature/test`
- **THEN** the function SHALL return `Err(PawError::WorktreeError)` whose message contains the substring `already exists`

Test: `tests/git_integration.rs::create_worktree_resume_falls_through_when_path_exists_but_unrelated`

#### Scenario: Path exists as a worktree but for a different branch

- **GIVEN** a worktree already exists at `../<project>-feature-test` but registered for branch `feature/other`
- **WHEN** `create_worktree()` is called for branch `feature/test`
- **THEN** the function SHALL fall through to `git worktree add` and return `Err(PawError::WorktreeError)`

Test: `tests/git_integration.rs::create_worktree_resume_falls_through_when_path_registered_for_different_branch`

#### Scenario: Rebase-on-resume happy path advances branch onto current main

- **GIVEN** the default branch `main` has advanced by N commits since branch `feat/example` was created
- **AND** `feat/example` exists locally and is behind `main` by exactly N commits with no diverging commits of its own
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = true`
- **THEN** the function SHALL run `git rebase <main>` inside `feat/example`'s own worktree, never in the main checkout
- **AND** the rebase SHALL succeed
- **AND** `feat/example`'s HEAD SHA after the call SHALL be reachable from `main` (i.e. include the N new commits)
- **AND** the function SHALL return `Ok(WorktreeCreation { path: <expected>, branch_created: false })`

Test: `src/git.rs::create_worktree_rebases_branch_when_behind_main`, `src/git.rs::create_worktree_rebases_a_branch_without_a_worktree_in_its_new_worktree`

#### Scenario: Rebase skipped when branch is already up-to-date

- **GIVEN** branch `feat/example` exists locally and is at the same SHA as `main` (no divergence)
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = true`
- **THEN** `git rebase <main>` SHALL be invoked in the branch's worktree and SHALL exit zero with no rewrite
- **AND** `feat/example`'s HEAD SHA SHALL be unchanged
- **AND** the function SHALL return `Ok(WorktreeCreation { path: <expected>, branch_created: false })`
- **AND** no error SHALL be returned

Test: `src/git.rs::create_worktree_rebase_noop_when_branch_up_to_date`

#### Scenario: Rebase conflict aborts cleanly and surfaces error

- **GIVEN** branch `feat/example` and `main` both modify the same line of the same file with different content
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = true`
- **THEN** `git rebase <main>` SHALL be invoked in the branch's worktree and SHALL exit non-zero with conflict markers
- **AND** the function SHALL invoke `git rebase --abort` in that same worktree for the rebase it started
- **AND** `feat/example`'s HEAD SHA after the call SHALL equal its pre-call HEAD SHA
- **AND** no rebase state SHALL remain in that worktree's git dir
- **AND** the function SHALL return `Err(PawError::WorktreeError(msg))` where `msg` contains the substring `rebase onto main failed`
- **AND** a worktree created for the call SHALL have been removed again (one that existed from a prior session SHALL be unchanged)

Test: `src/git.rs::create_worktree_rebase_conflict_aborts_and_errors`

#### Scenario: rebase_onto_main = false preserves v0.5 no-rebase behaviour

- **GIVEN** branch `feat/example` exists locally and is behind `main` by 3 commits
- **WHEN** `create_worktree()` is called for `feat/example` with `rebase_onto_main = false`
- **THEN** no `git rebase` invocation SHALL occur
- **AND** `feat/example`'s HEAD SHA after the call SHALL equal its pre-call HEAD SHA
- **AND** the function SHALL proceed to the existence check and, if applicable, `git worktree add`

Test: `src/git.rs::create_worktree_no_rebase_preserves_v0_5_behaviour`

#### Scenario: New branch creation skips rebase regardless of flag

- **GIVEN** branch `feat/new` does NOT exist in the local repository
- **AND** `rebase_onto_main = true` is passed
- **WHEN** `create_worktree()` is called for `feat/new`
- **THEN** no `git rebase` invocation SHALL occur (there is nothing to rebase)
- **AND** the function SHALL invoke `git worktree add -b feat/new <path>` to create the branch from current HEAD
- **AND** the function SHALL return `Ok(WorktreeCreation { path, branch_created: true })`

Test: `src/git.rs::create_worktree_new_branch_skips_rebase_regardless_of_flag`, `src/git.rs::create_worktree_skips_the_rebase_when_the_branch_does_not_exist_locally`

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

Repository validation SHALL work against real git repos.

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

Branch listing SHALL work against real git repos.

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

Worktree lifecycle SHALL work against real git repos.

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

Directory naming SHALL be correct in integration tests.

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

### Requirement: Default-branch resolution is single-source and tolerant of a missing origin/HEAD

The system SHALL resolve the repository's default branch through a single resolver used by every launch path (initial `start`, resume, `add`, and the rebase step), so no two paths can disagree. When `refs/remotes/origin/HEAD` is not set — e.g. a repository with local commits but no configured remote HEAD — the resolver SHALL fall back deterministically (a local `main`, else a local `master`, else the checked-out branch via `git symbolic-ref --short HEAD`) rather than aborting. A launch that succeeded initially SHALL therefore also succeed on resume in the same repository state.

#### Scenario: Resolution succeeds without origin/HEAD

- **GIVEN** a repository with local commits and no `refs/remotes/origin/HEAD` set
- **WHEN** the default branch is resolved
- **THEN** it SHALL return a valid local default branch (via the fallback) rather than aborting with an error

#### Scenario: First launch and resume resolve identically

- **GIVEN** a repository whose first `git paw start` resolved a default branch
- **WHEN** the session is later resumed (a second `git paw start`) in the same repository state
- **THEN** the resume SHALL resolve the same default branch through the same resolver and SHALL NOT fail where the first launch succeeded

#### Scenario: origin/HEAD is still honoured when present

- **GIVEN** a repository with `refs/remotes/origin/HEAD` pointing at `origin/main`
- **WHEN** the default branch is resolved
- **THEN** it SHALL return `main` (the configured remote HEAD), unchanged from prior behaviour

### Requirement: Mid-session rebase of a live worker branch

The system SHALL provide a rebase entry point that rebases a named branch onto the repository's default branch while that branch is checked out in a live worktree, separate from `create_worktree`'s creation-time `rebase_onto_main` path. The entry point SHALL run the rebase inside the worktree where the branch is currently checked out. On a non-zero `git rebase` exit it SHALL invoke `git rebase --abort` and return an error, leaving the branch at its pre-rebase HEAD. A branch already at or ahead of the default branch SHALL be a successful no-op.

The creation-time rebase SHALL follow its own requirements, unaffected by this entry point.

#### Scenario: Rebase runs in the worktree holding the branch

- **GIVEN** a branch checked out in a live worktree
- **WHEN** the mid-session rebase entry point is invoked for that branch
- **THEN** the rebase SHALL be run inside that worktree

Test: `src/git.rs::rebase_onto_default_runs_inside_the_worktree_holding_the_branch`

#### Scenario: Failed mid-session rebase aborts and restores

- **GIVEN** a mid-session rebase that exits non-zero
- **WHEN** the failure is detected
- **THEN** `git rebase --abort` SHALL be invoked
- **AND** the branch SHALL be left at its pre-rebase HEAD
- **AND** an error SHALL be returned

Test: `src/git.rs::rebase_onto_default_aborts_and_restores_head_on_conflict`

#### Scenario: Branch already current is a successful no-op

- **GIVEN** a branch already at or ahead of the default branch
- **WHEN** the mid-session rebase entry point is invoked for that branch
- **THEN** it SHALL return success
- **AND** the branch SHALL NOT be rewritten

Test: `src/git.rs::rebase_onto_default_mid_session_already_current_is_a_successful_noop`, `src/git.rs::rebase_branch_onto_default_delegates_to_the_with_variant`

#### Scenario: Creation-time rebase contract is unaffected

- **WHEN** `create_worktree()` is called with `rebase_onto_main = true` for an existing branch
- **THEN** it SHALL rebase in the branch's own worktree, skip a dirty worktree or a stopped rebase, and refuse a branch the main checkout holds, as the creation-time rebase requirements state

Test: `src/git.rs::create_worktree_rebases_a_branch_without_a_worktree_in_its_new_worktree`, `src/git.rs::create_worktree_skips_the_rebase_over_a_dirty_worktree`, `src/git.rs::create_worktree_never_aborts_a_rebase_already_in_progress`, `src/git.rs::create_worktree_refuses_a_branch_checked_out_in_the_main_checkout`

### Requirement: Creation-time rebase never runs over uncommitted work

`create_worktree` SHALL rebase an existing branch only inside the branch's own worktree, never in or by switching the main checkout. A branch no worktree holds SHALL get its worktree first and be rebased there; if that rebase fails, the new worktree SHALL be removed. When the worktree already holding the branch has uncommitted changes, it SHALL NOT rebase, stash, commit, reset, check out over or discard anything, and SHALL proceed on the branch's current HEAD.

#### Scenario: A surviving dirty worktree is reused without a rebase

- **GIVEN** no saved session, and a worktree for `feat/x` left on disk with a modified tracked file and an untracked file
- **AND** the default branch has gained commits since `feat/x` was based on it
- **WHEN** the user runs `git paw start` for `feat/x` without `--no-rebase`
- **THEN** the command SHALL succeed and the agent SHALL be launched in that worktree
- **AND** `feat/x`'s HEAD SHALL equal its HEAD before the command
- **AND** both files SHALL be byte-identical to their state before the command and `git stash list` SHALL be unchanged

#### Scenario: A branch without a worktree is rebased in its own worktree

- **GIVEN** a local branch `feat/y` that no worktree holds, and a main checkout with a modified tracked file and an untracked file
- **AND** the default branch has gained commits since `feat/y` was based on it
- **WHEN** the user runs `git paw start --branches feat/y`
- **THEN** the command SHALL succeed, create the worktree for `feat/y` and rebase `feat/y` onto the default branch in it
- **AND** the main checkout's branch, HEAD and both files SHALL be unchanged and no skip notice SHALL be printed

#### Scenario: A failed rebase in a new worktree leaves no worktree behind

- **GIVEN** a local branch `feat/y` that no worktree holds and that conflicts with the advanced default branch
- **WHEN** `create_worktree` is called for `feat/y` with `rebase_onto_main = true`
- **THEN** it SHALL return an error containing `rebase onto main failed`
- **AND** no worktree SHALL hold `feat/y`, `feat/y`'s tip SHALL be unchanged and the main checkout SHALL stay on its branch

#### Scenario: A clean worktree on a moved default branch is still rebased

- **GIVEN** a surviving worktree for `feat/x` with no uncommitted changes
- **AND** the default branch has gained commits since `feat/x` was based on it
- **WHEN** the user runs `git paw start` for `feat/x`
- **THEN** `feat/x` SHALL be rebased onto the default branch and no skip notice SHALL be printed

#### Scenario: A conflicting rebase on a clean tree still fails

- **GIVEN** a clean surviving worktree for `feat/x` whose branch conflicts with the advanced default branch
- **WHEN** the user runs `git paw start` for `feat/x`
- **THEN** the command SHALL exit non-zero with an error containing `rebase onto main failed`
- **AND** `feat/x`'s HEAD SHALL equal its HEAD before the command

### Requirement: Creation-time rebase never touches the main checkout or a stopped rebase

When the main checkout holds the branch, `create_worktree` SHALL fail before any rebase with an error saying the branch is checked out in the main checkout and to switch it to another branch. It SHALL rebase only in the worktree path it holds or just created, never re-discovering it. A worktree whose git dir has a `rebase-merge` or `rebase-apply` state SHALL NOT be rebased or aborted; the skip reason SHALL say a rebase is already in progress.

#### Scenario: A branch checked out in the main checkout is refused

- **GIVEN** `feat/x` checked out in the main checkout and an advanced default branch
- **WHEN** `create_worktree` is called for `feat/x` with `rebase_onto_main = true`
- **THEN** it SHALL return an error naming the main checkout and telling the user to switch it to another branch
- **AND** `feat/x`'s tip SHALL be unchanged, the main checkout SHALL stay on `feat/x` and no worktree SHALL be created

#### Scenario: A new worktree is rebased by its own path

- **GIVEN** a branch `feat/x` no worktree holds, and a `git worktree list` that fails after the worktree is created
- **WHEN** `create_worktree` is called for `feat/x` with `rebase_onto_main = true`
- **THEN** the rebase SHALL run in the new worktree and the main checkout's branch SHALL never be switched

#### Scenario: A stopped rebase is left in place

- **GIVEN** a surviving worktree for `feat/x` with a clean status and a rebase stopped part-way in it
- **AND** the default branch has gained commits
- **WHEN** `create_worktree` is called for `feat/x` with `rebase_onto_main = true`
- **THEN** the worktree SHALL be reused, no new rebase and no `git rebase --abort` SHALL run, and the stopped rebase SHALL still be in progress
- **AND** the returned skip reason SHALL say a rebase is already in progress

### Requirement: Creation-time rebase judges uncommitted work as remove does

The creation-time rebase check SHALL use the uncommitted-work detection `remove` and `add` use: a tree is dirty when NUL-delimited `git status --porcelain -z` reports at least one entry that is not git-paw-managed. A tree whose only entries are git-paw-managed SHALL be clean. A tree whose status check fails SHALL be treated as dirty, not as an error. The skip reason SHALL name the worktree it checked.

#### Scenario: git-paw-managed files alone do not block a start rebase

- **GIVEN** a surviving worktree for `feat/x` whose only uncommitted entry is the injected `.git-paw/AGENTS.local.md` sidecar
- **AND** the default branch has gained commits
- **WHEN** the user runs `git paw start` for `feat/x`
- **THEN** `feat/x` SHALL be rebased onto the default branch and no skip notice SHALL be printed

#### Scenario: An unreadable worktree is not rebased

- **GIVEN** a worktree for `feat/x` still registered with git whose status check fails
- **WHEN** `create_worktree` is called for `feat/x` with `rebase_onto_main = true`
- **THEN** no `git rebase` SHALL run and `feat/x`'s tip SHALL be unchanged
- **AND** the returned skip reason SHALL say the check failed

### Requirement: Every launch path reports a skipped rebase

`create_worktree` SHALL return the reason a rebase was skipped. `start` (plain, `--supervisor`, spec-driven) and `add` SHALL print it as one stderr notice naming the branch and `--no-rebase`, never on stdout. `add` SHALL check before creating the worktree, so the notice prints even if creation fails, and SHALL also print a skip `create_worktree` returns. MCP `start_session` SHALL return the reason in `warnings` without naming `--no-rebase`. With `--no-rebase` no check or notice SHALL occur.

#### Scenario: start prints the skip notice on stderr

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw start` for `feat/x`
- **THEN** stderr SHALL contain one notice naming `feat/x` and `--no-rebase`
- **AND** stdout SHALL contain nothing about the skip

Test: `tests/start_dirty_rebase_e2e.rs::start_reuses_a_dirty_surviving_worktree_without_a_rebase`

#### Scenario: A supervised launch completes over a dirty worktree

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw start --supervisor` for `feat/x`
- **THEN** the session SHALL launch with `feat/x` on its current HEAD
- **AND** stderr SHALL contain the skip notice naming `feat/x` and `--no-rebase`

Test: `tests/start_dirty_rebase_e2e.rs::supervised_start_completes_over_a_dirty_worktree`

#### Scenario: MCP start_session reports the skip as a warning

- **GIVEN** a root MCP session and a surviving worktree for `feat/x` with uncommitted changes
- **WHEN** the client calls `start_session({ "branches": ["feat/x"], "cli": "<cli>" })`
- **THEN** the response SHALL be successful and its `warnings` SHALL contain one skip notice naming `feat/x` and its reason
- **AND** that notice SHALL NOT name `--no-rebase`, which the tool does not accept
- **AND** the MCP server's stdout SHALL carry only protocol messages

Test: `tests/mcp_lifecycle_e2e.rs::start_session_reports_a_skipped_rebase_as_a_warning`

#### Scenario: --no-rebase on a dirty worktree prints no notice

- **GIVEN** a surviving worktree for `feat/x` with uncommitted changes
- **WHEN** the user runs `git paw start --no-rebase` for `feat/x`
- **THEN** the branch SHALL NOT be rebased, the changes SHALL be unchanged and no skip notice SHALL be printed

Test: `tests/start_dirty_rebase_e2e.rs::start_no_rebase_on_a_dirty_worktree_prints_no_notice`

#### Scenario: add keeps its notice text

- **GIVEN** a kept worktree for `feat/x` with uncommitted changes and an advanced default branch
- **WHEN** the user runs `git paw add feat/x`
- **THEN** stderr SHALL contain `notice: rebase skipped for 'feat/x': its worktree has uncommitted changes, which are left untouched. Pass --no-rebase to attach without a rebase.`

Test: `tests/add_remove_e2e.rs::add_skips_the_rebase_over_a_modified_tracked_file`
