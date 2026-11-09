## MODIFIED Requirements

### Requirement: add never rebases over uncommitted work

`git paw add` SHALL follow `git-operations` "Creation-time rebase never runs over uncommitted work": when the worktree holding the branch has uncommitted changes, `add` SHALL NOT rebase the branch and SHALL NOT stash, commit, reset, check out over, or otherwise modify or discard any of those changes.

#### Scenario: A CLI swap on a dirty kept worktree completes

- **GIVEN** an active session whose agent `feat/x` has a modified tracked file and an untracked file in its worktree
- **AND** the default branch has gained commits since `feat/x` was based on it
- **AND** `git paw remove feat/x --keep-worktree` has run
- **WHEN** the user runs `git paw add feat/x --cli my-agent` without `--no-rebase`
- **THEN** the command SHALL exit zero and `feat/x` SHALL be attached with `my-agent` in the same worktree
- **AND** `feat/x`'s HEAD SHALL equal its HEAD before the command
- **AND** the modified and untracked files SHALL be byte-identical to their state before the command
- **AND** `git stash list` SHALL be unchanged
- **AND** stderr SHALL contain a notice naming `feat/x` and `--no-rebase`

#### Scenario: A modified tracked file alone blocks the rebase

- **GIVEN** a kept worktree for `feat/x` whose only change is one modified tracked file
- **AND** the default branch has gained commits since `feat/x` was based on it
- **WHEN** the user runs `git paw add feat/x`
- **THEN** the command SHALL exit zero without rebasing `feat/x`
- **AND** the modified file SHALL be byte-identical to its state before the command
- **AND** stderr SHALL contain the skip notice naming `feat/x` and `--no-rebase`

### Requirement: add judges uncommitted work as remove does

`add` SHALL judge uncommitted work by `git-operations` "Creation-time rebase judges uncommitted work as remove does": a worktree is dirty when at least one entry, modified, staged, renamed, deleted or untracked, is not git-paw-managed, and a worktree whose only entries are git-paw-managed SHALL be treated as clean.

#### Scenario: git-paw-managed files alone do not block the rebase

- **GIVEN** a kept worktree whose only uncommitted entries are git-paw-managed (for example the injected `.git-paw/AGENTS.local.md` sidecar)
- **AND** the default branch has gained commits
- **WHEN** the user runs `git paw add feat/x`
- **THEN** `feat/x` SHALL be rebased onto the default branch
- **AND** no skip notice SHALL be printed

### Requirement: add treats an unreadable worktree as dirty

When the uncommitted-work check fails for the worktree holding the branch, `add` SHALL treat that worktree as dirty rather than abort, as `git-operations` "Creation-time rebase judges uncommitted work as remove does" requires, and its skip notice SHALL say the check failed.

#### Scenario: A worktree whose status cannot be read is not rebased

- **GIVEN** a kept worktree for `feat/x` that is still registered with git but whose directory was deleted
- **WHEN** the user runs `git paw add feat/x`
- **THEN** `add` SHALL NOT abort on the failed status check and SHALL NOT attempt the rebase
- **AND** `feat/x`'s tip SHALL be unchanged
- **AND** stderr SHALL contain a skip notice naming `feat/x`, the failed check and `--no-rebase`
