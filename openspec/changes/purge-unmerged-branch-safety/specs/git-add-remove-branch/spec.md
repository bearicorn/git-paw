## ADDED Requirements

### Requirement: Per-worktree teardown retains a branch carrying unmerged commits

The shared per-worktree teardown that `git paw purge` and `git paw remove` delegate to SHALL delete a worktree's branch only when git-paw created that branch (`branch_created`) AND the branch tip is an ancestor of the default branch resolved through the single default-branch resolver OR of the branch recorded as that worktree's base. A branch carrying commits in neither, or whose merge state cannot be determined, SHALL be retained after its worktree is removed.

Every ref named in a merge comparison or a tip lookup SHALL be qualified as `refs/heads/<branch>`. An unqualified name is resolved by git's `rev-parse` precedence, which tries `refs/tags/<name>` before `refs/heads/<name>`, so a tag sharing a branch's name would answer the merge question — and print the SHA — for the tag's history rather than the branch's.

Because `git worktree add -b <branch> <path>` takes no start point, a branch git-paw creates starts at the main checkout's `HEAD`. The system SHALL record that start point on the worktree entry as `base_branch` when it creates the branch, SHALL omit the field when there is none to record, and SHALL load a session file written before the field existed with no base (comparing against the default branch alone). When a base is recorded, the count of unmerged commits SHALL be taken against it rather than against the default branch.

Every deletion SHALL print the branch and the tip SHA it was at (`Deleted branch <name> (was <sha>)`). Every retention SHALL print the branch, its tip SHA, the count of unmerged commits (or that the comparison could not be made), and a recovery hint — `git worktree add '<path>' <branch>` to resume the work, with the path quoted, and a deliberate-discard command that is `git branch -d <branch>` when git would accept the safe delete and `git branch -D <branch>` otherwise.

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

#### Scenario: A same-named tag never stands in for the branch

- **GIVEN** a created branch `feat/x` with commits not in the default branch
- **AND** a tag also named `feat/x` pointing at fully merged history
- **WHEN** its worktree is torn down
- **THEN** the branch SHALL be retained
- **AND** the tip SHA printed SHALL be the branch's, not the tag's

#### Scenario: A branch merged into the branch it was created from is deleted

- **GIVEN** a session launched from an integration branch that is ahead of the default branch
- **AND** a created branch whose `base_branch` is that integration branch and whose commits have been merged into it
- **WHEN** its worktree is torn down
- **THEN** the branch SHALL be deleted
- **AND** the output SHALL print `Deleted branch <name> (was <sha>)`

#### Scenario: An unmerged branch is counted against the branch it was created from

- **GIVEN** an integration branch two commits ahead of the default branch
- **AND** a created branch whose `base_branch` is that integration branch and which adds one commit of its own
- **WHEN** its worktree is torn down
- **THEN** the branch SHALL be retained
- **AND** the output SHALL report 1 unmerged commit against the integration branch, not 3 against the default branch

#### Scenario: A session file written before the base branch was recorded loads with no base

- **GIVEN** a session file whose worktree entries carry no `base_branch` field
- **WHEN** the session is loaded
- **THEN** every entry SHALL report no base
- **AND** teardown SHALL compare against the default branch alone

#### Scenario: The retention hint quotes the path and offers the safe delete when it would work

- **GIVEN** a retained branch whose worktree path is printed in the resume hint
- **WHEN** the hint is emitted
- **THEN** the worktree path SHALL be quoted in the `git worktree add` command
- **AND** the discard command SHALL be `git branch -d <branch>` when the branch is contained in the checked-out branch, and `git branch -D <branch>` otherwise
