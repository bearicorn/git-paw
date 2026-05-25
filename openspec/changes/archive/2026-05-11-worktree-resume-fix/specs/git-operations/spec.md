## MODIFIED Requirements

### Requirement: Create worktrees as siblings of the repository

The system SHALL create git worktrees in the parent directory of the repository root using the derived directory name convention.

The system SHALL be idempotent in the resume case: when `create_worktree()` is invoked for a branch whose worktree already exists at the expected path AND is registered with git for that branch, the function SHALL return `Ok(WorktreeCreation { path, branch_created: false })` without re-running `git worktree add`. Idempotency is verified by parsing `git worktree list --porcelain` output and matching both the worktree path and the `refs/heads/<branch>` line.

If the expected path exists on disk but is NOT a git worktree registered for the specified branch (e.g. an unrelated directory, or a worktree for a different branch), the function SHALL fall through to the existing `git worktree add` call so the user sees the actionable `fatal: '<path>' already exists` error from git directly.

#### Scenario: Worktree created at correct path

- **GIVEN** a repository with a branch `feature/test`
- **WHEN** `create_worktree()` is called
- **THEN** a worktree SHALL be created at `../<project>-feature-test` containing the repository files

#### Scenario: Creating worktree for currently checked-out branch fails

- **GIVEN** the current branch is checked out in the main repo
- **WHEN** `create_worktree()` is called for that branch
- **THEN** it SHALL return `Err(PawError::WorktreeError)`

#### Scenario: Resume of an existing worktree returns success without re-running git worktree add

- **GIVEN** a worktree already exists at `../<project>-feature-test` for branch `feature/test` from a prior session
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
