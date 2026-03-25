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

The system SHALL create git worktrees in the parent directory of the repository root using the derived directory name convention.

#### Scenario: Worktree created at correct path
- **GIVEN** a repository with a branch `feature/test`
- **WHEN** `create_worktree()` is called
- **THEN** a worktree SHALL be created at `../<project>-feature-test` containing the repository files

Test: `git::tests::create_worktree_at_correct_path`

#### Scenario: Creating worktree for currently checked-out branch fails
- **GIVEN** the current branch is checked out in the main repo
- **WHEN** `create_worktree()` is called for that branch
- **THEN** it SHALL return `Err(PawError::WorktreeError)`

Test: `git::tests::create_worktree_errors_on_checked_out_branch`

### Requirement: Remove worktrees and prune stale entries

The system SHALL force-remove a worktree and prune stale git worktree metadata.

#### Scenario: Worktree fully cleaned up after removal
- **GIVEN** an existing worktree
- **WHEN** `remove_worktree()` is called
- **THEN** the directory SHALL be deleted and git SHALL no longer track it

Test: `git::tests::remove_worktree_cleans_up_fully`

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
