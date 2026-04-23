## ADDED Requirements

### Requirement: Watch worktree git state for changes

The broker process SHALL poll each worktree at a fixed interval using `git status --porcelain` and auto-publish `agent.status` when the set of reported paths differs from the previous tick. The `modified_files` field SHALL contain the paths currently reported by git status.

The poll interval SHALL be 2 seconds. The watcher SHALL NOT publish when the snapshot is unchanged from the previous tick.

#### Scenario: File edit triggers status publish

- **GIVEN** a running broker watching worktree `/path/to/git-paw-feat-x`
- **WHEN** a file at `src/lib.rs` is modified in that worktree
- **THEN** within 3 seconds the broker publishes `agent.status` for `feat-x` with `modified_files` containing `src/lib.rs`

#### Scenario: Multiple rapid edits are collapsed into one publish

- **GIVEN** a running broker watching a worktree
- **WHEN** 5 files are modified within a single poll interval
- **THEN** a single `agent.status` is published with all 5 files in `modified_files`

#### Scenario: Build artifacts are excluded via gitignore

- **GIVEN** a running broker watching a worktree whose `.gitignore` lists `target/` and `node_modules/`
- **WHEN** files change in `target/` or `node_modules/`
- **THEN** no `agent.status` is published for those changes

#### Scenario: Unchanged state does not re-publish

- **GIVEN** a running broker that has just published an `agent.status` for a worktree
- **WHEN** the next poll tick runs and `git status --porcelain` output is byte-identical to the previous tick
- **THEN** no new `agent.status` is published

#### Scenario: Watcher stops when broker stops

- **GIVEN** a running broker with active watchers
- **WHEN** the `BrokerHandle` is dropped
- **THEN** all watcher tasks stop within one poll interval

### Requirement: Worktree-to-agent mapping

The broker SHALL accept a list of `WatchTarget { agent_id, worktree_path }` at startup. Each watcher task SHALL publish status for the `agent_id` of its assigned `WatchTarget`.

#### Scenario: Events map to correct agent

- **GIVEN** two watch targets: `feat-a` at `/wt-a/` and `feat-b` at `/wt-b/`
- **WHEN** a file changes in `/wt-a/src/lib.rs`
- **THEN** the status is published for agent `feat-a`, not `feat-b`
