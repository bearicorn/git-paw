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

### Requirement: A merge is checked against the last verified tip

`agent.advanced-main` SHALL accept an optional `merged_sha`, the merged branch's tip. When it is present and differs from the `sha` of the last `agent.verified` recorded for that branch's agent, or no verified SHA is recorded, the broker SHALL put one `agent.question` tagged `[merge-check]`, naming the branch and both SHAs, into the orchestrator inbox.

#### Scenario: Merging a newer tip than the verified one is flagged

- **GIVEN** `feat-x` last verified at `sha = aaa111`
- **WHEN** an `agent.advanced-main` arrives with `merged_branch = "feat/x"` and `merged_sha = bbb222`
- **THEN** the orchestrator inbox SHALL receive one `[merge-check]` question naming `feat/x`, `aaa111` and `bbb222`

#### Scenario: Merging the verified tip raises nothing

- **GIVEN** `feat-x` last verified at `sha = aaa111`
- **WHEN** an `agent.advanced-main` arrives with `merged_branch = "feat/x"` and `merged_sha = aaa111`
- **THEN** no `[merge-check]` question SHALL be emitted

#### Scenario: An advanced-main without merged_sha is unchanged

- **GIVEN** any verified state
- **WHEN** an `agent.advanced-main` without `merged_sha` arrives
- **THEN** no `[merge-check]` question SHALL be emitted and routing SHALL be as before
