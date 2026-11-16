## ADDED Requirements

### Requirement: The watcher never takes a worktree's index lock

Every git command the watcher runs against a worktree SHALL run with git's optional locks disabled, so a watcher poll SHALL NEVER hold the worktree's `index.lock` and SHALL never make a concurrent rebase, commit or merge in that worktree fail on it.

#### Scenario: A long rebase is not stopped by the watcher

- **GIVEN** a watched agent worktree polled at the normal interval
- **WHEN** a rebase replaying 30 commits runs in that worktree
- **THEN** the rebase SHALL complete without an `index.lock` error

#### Scenario: Polling leaves the index untouched

- **GIVEN** a watched worktree whose index stat data is stale
- **WHEN** the watcher polls it
- **THEN** the index file's modification time SHALL be unchanged

### Requirement: The watcher is silent while a history operation is in progress

While a rebase, `git am`, merge or cherry-pick is in progress in a watched worktree, the watcher SHALL NOT publish an `agent.status` for it. Its branch-tip observation SHALL continue. Once the operation ends, the next differing file set SHALL publish as usual.

#### Scenario: Conflict markers during a rebase are not published

- **GIVEN** a watched agent worktree whose rebase stopped on a conflict in `src/a.rs`
- **WHEN** the watcher polls while the rebase is in progress
- **THEN** no `agent.status` listing `src/a.rs` SHALL be published for that agent

#### Scenario: Publishing resumes after the operation

- **GIVEN** a watched worktree whose rebase has just completed
- **WHEN** the agent then edits `src/b.rs`
- **THEN** the watcher SHALL publish an `agent.status` whose `modified_files` contains `src/b.rs`
