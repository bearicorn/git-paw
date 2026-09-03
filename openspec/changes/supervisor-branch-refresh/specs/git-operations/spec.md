## ADDED Requirements

### Requirement: Mid-session rebase of a live worker branch

The system SHALL provide a rebase entry point that rebases a named branch onto the repository's default branch while that branch is checked out in a live worktree, separate from `create_worktree`'s creation-time `rebase_onto_main` path. The entry point SHALL run the rebase inside the worktree where the branch is currently checked out. On a non-zero `git rebase` exit it SHALL invoke `git rebase --abort` and return an error, leaving the branch at its pre-rebase HEAD. When the branch is already at or ahead of the default branch, `git rebase` exits zero with no rewrite and the entry point SHALL treat that as success.

The creation-time contract of `create_worktree` (including its `rebase_onto_main` parameter, its ordering relative to the existence check, and its idempotent resume behaviour) SHALL be unchanged by this addition.

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
- **THEN** its behaviour SHALL be identical to the behaviour before the mid-session entry point was added
