## ADDED Requirements

### Requirement: A stopped rebase is found at any registered worktree path

When no worktree lists the branch as checked out, `create_worktree` SHALL look through every worktree git registers for one whose stopped rebase is rebasing that branch, whatever its path and whatever the current `worktree_placement`. A worktree found this way SHALL be reused at its own path and judged like a held worktree: not rebased, not aborted, and reported with the rebase-in-progress skip reason.

#### Scenario: A stopped rebase under the previous placement is reused

- **GIVEN** a worktree for `feat/x` created under `worktree_placement = "sibling"` with a rebase stopped part-way in it
- **AND** the config now sets `worktree_placement = "child"`
- **WHEN** `create_worktree` is called for `feat/x` with `rebase_onto_main = true`
- **THEN** it SHALL return the sibling worktree's path, with the skip reason saying a rebase is already in progress
- **AND** no worktree SHALL be added at the child path, and the stopped rebase SHALL still be in progress

#### Scenario: A detached worktree of another branch is not taken

- **GIVEN** a detached worktree whose stopped rebase is rebasing `feat/y`
- **WHEN** `create_worktree` is called for `feat/x`
- **THEN** that worktree SHALL NOT be returned for `feat/x`
