## ADDED Requirements

### Requirement: Per-worktree teardown retains a branch carrying unmerged commits

The shared per-worktree teardown that `git paw purge` and `git paw remove` delegate to SHALL delete a worktree's branch only when git-paw created that branch (`branch_created`) AND the branch tip is an ancestor of the default branch resolved through the single default-branch resolver. A branch carrying commits not in the default branch, or whose merge state cannot be determined, SHALL be retained after its worktree is removed. Every deletion SHALL print the branch and the tip SHA it was at (`Deleted branch <name> (was <sha>)`). Every retention SHALL print the branch, its tip SHA, the count of unmerged commits (or that the comparison could not be made), and a recovery hint — `git worktree add <path> <branch>` to resume the work, `git branch -D <branch>` to discard it deliberately.

#### Scenario: remove keeps a created branch with unmerged commits

- **GIVEN** an agent `feat/x` whose branch git-paw created and which has commits not in the default branch
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the worktree directory SHALL be removed
- **AND** the branch `feat/x` SHALL still exist at the same tip
- **AND** the output SHALL name the retained branch with its tip SHA, its unmerged-commit count, and the recovery hint

#### Scenario: remove deletes a created branch that is fully merged

- **GIVEN** an agent `feat/x` whose branch git-paw created and whose tip is an ancestor of the default branch
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the worktree directory SHALL be removed
- **AND** the branch SHALL be deleted
- **AND** the output SHALL print `Deleted branch feat/x (was <sha>)`

#### Scenario: A pre-existing branch is never deleted

- **GIVEN** an agent whose branch existed before the session (`branch_created = false`)
- **WHEN** its worktree is torn down by `purge` or `remove`
- **THEN** the branch SHALL remain, whether or not it is merged

#### Scenario: A failed merge comparison retains the branch

- **GIVEN** a created branch whose comparison against the default branch cannot be made
- **WHEN** its worktree is torn down
- **THEN** the branch SHALL be retained
- **AND** the output SHALL state that the merge state could not be determined
