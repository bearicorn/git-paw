## MODIFIED Requirements

### Requirement: Purge warns about unmerged commits

Before destroying worktrees, `git paw purge` SHALL check each worktree branch for commits not yet merged to the default branch, and SHALL never delete a branch that carries any. The system SHALL:

1. For each worktree branch, determine whether its tip is an ancestor of the default branch (`git merge-base --is-ancestor <branch> <default-branch>`) and, when it is not, count the commits not in the default branch
2. If any branch has unmerged commits, display a warning listing each such branch with its commit count and tip SHA, and stating that those branches will be **kept** — only their worktrees (including any uncommitted changes) and the session state are removed
3. Require either the `--force` flag or interactive confirmation ("Y" response) to proceed with worktree removal
4. If the user declines, exit without destroying any worktrees
5. Whether or not `--force` is given, retain every branch that has unmerged commits or whose merge state cannot be determined; delete only a branch git-paw created whose tip is an ancestor of the default branch; and print, for each retained branch, its tip SHA and how to resume or discard it by hand

The default branch SHALL be resolved through the single default-branch resolver (see `git-operations`, "Default-branch resolution is single-source and tolerant of a missing origin/HEAD"), not a purge-local lookup. The `purge` help text (`about`, `long_about`, and the `--force` option) and the `stop` help text that describes purge SHALL state that branches with unmerged commits are kept.

#### Scenario: Purge with no unmerged commits proceeds without warning

- **GIVEN** all worktree branches have no commits beyond the default branch
- **WHEN** `git paw purge` is run
- **THEN** no unmerged commit warning SHALL be shown
- **AND** purge proceeds normally, deleting the branches git-paw created and printing each with the SHA it was at

#### Scenario: Purge with unmerged commits warns before destroying

- **GIVEN** one worktree branch has 3 commits not merged to main
- **WHEN** `git paw purge` is run without `--force`
- **THEN** a warning SHALL be displayed identifying the branch, its 3 unmerged commits, and its tip SHA
- **AND** the warning SHALL state that the branch will be kept
- **AND** the system SHALL prompt for confirmation before removing worktrees

#### Scenario: Purge --force skips confirmation but retains unmerged branches

- **GIVEN** one worktree branch git-paw created has unmerged commits
- **WHEN** `git paw purge --force` is run
- **THEN** the warning SHALL still be displayed
- **AND** purge SHALL proceed without waiting for interactive confirmation
- **AND** the worktree, tmux session, and session state SHALL be removed
- **AND** the branch SHALL still exist at the same tip afterwards
- **AND** the output SHALL name the retained branch with its tip SHA and a way to resume or discard it

#### Scenario: Confirmed purge also retains unmerged branches

- **GIVEN** one worktree branch git-paw created has unmerged commits
- **WHEN** `git paw purge` is run and the user answers "Y"
- **THEN** the worktree SHALL be removed
- **AND** the branch SHALL still exist at the same tip afterwards

#### Scenario: Purge cancelled by user preserves worktrees

- **GIVEN** one worktree branch has unmerged commits
- **WHEN** `git paw purge` is run and the user answers "N" to the confirmation
- **THEN** no worktrees SHALL be removed
- **AND** the system SHALL exit with a non-error message indicating purge was cancelled

#### Scenario: An undeterminable merge state is treated as unmerged

- **GIVEN** a worktree branch whose comparison against the default branch fails (the default branch cannot be resolved, or `git merge-base` errors)
- **WHEN** `git paw purge --force` is run
- **THEN** the branch SHALL be retained
- **AND** the output SHALL state that its merge state could not be determined

#### Scenario: Purge in a master-default repository without origin/HEAD compares against master

- **GIVEN** a repository whose default branch is `master`, with no `refs/remotes/origin/HEAD`, and a created worktree branch with one commit not in `master`
- **WHEN** `git paw purge --force` is run
- **THEN** the warning SHALL report the branch as having 1 unmerged commit
- **AND** the branch SHALL be retained

#### Scenario: Help text states unmerged branches are kept

- **WHEN** `git paw purge --help` is run
- **THEN** the output SHALL state that branches with unmerged commits are kept
- **AND** the `--force` description SHALL NOT imply that it deletes them
