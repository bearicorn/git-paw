## MODIFIED Requirements

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
