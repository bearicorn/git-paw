## REMOVED Requirements

### Requirement: Mid-session rebase of a live worker branch

**Reason**: Its last scenario, "Creation-time rebase contract is unaffected", is phrased relative to the change that introduced the entry point, and the body sentence it pins ("unaffected by this entry point") says the same. OpenSpec rejects a scenario rename inside a MODIFIED block, so the requirement returns under a new name, with that scenario retitled to state the behaviour it asserts.

**Migration**: Replaced by "Mid-session rebase runs in the worktree holding the branch" in this change. All four scenarios keep their bodies and tests.

## ADDED Requirements

### Requirement: Mid-session rebase runs in the worktree holding the branch

A rebase entry point, separate from `create_worktree`'s creation-time rebase, SHALL rebase a named branch onto the default branch inside the live worktree where it is checked out. On a non-zero `git rebase` exit it SHALL run `git rebase --abort` and return an error, leaving the branch at its pre-rebase HEAD. A branch already at or ahead of the default branch SHALL be a successful no-op.

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

#### Scenario: create_worktree keeps the creation-time rebase rules

- **WHEN** `create_worktree()` is called with `rebase_onto_main = true` for an existing branch
- **THEN** it SHALL rebase in the branch's own worktree, skip a dirty worktree or a stopped rebase, and refuse a branch the main checkout holds, as the creation-time rebase requirements state

Test: `src/git.rs::create_worktree_rebases_a_branch_without_a_worktree_in_its_new_worktree`, `src/git.rs::create_worktree_skips_the_rebase_over_a_dirty_worktree`, `src/git.rs::create_worktree_never_aborts_a_rebase_already_in_progress`, `src/git.rs::create_worktree_refuses_a_branch_checked_out_in_the_main_checkout`
