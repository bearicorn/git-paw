# remove-branch Specification

## Purpose
TBD - created by archiving change git-paw-add. Update Purpose after archive.
## Requirements
### Requirement: git paw remove subcommand

The system SHALL provide a `git paw remove <branch-name>`
subcommand that detaches a single agent from an active session.
The subcommand SHALL accept `--keep-worktree` (skip worktree
deletion; only detach pane + session entry) and `--force` (bypass
the uncommitted-work safety check). The subcommand SHALL fail with
an actionable error when no session is active for the repository.

#### Scenario: Remove a clean branch from a running session

- **GIVEN** an active session with agent `feat/x` whose worktree
  has no uncommitted changes
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the agent's pane SHALL be closed, the worktree SHALL be
  removed, and the branch entry SHALL be dropped from the session
  JSON

#### Scenario: Remove a branch not in the session

- **GIVEN** an active session whose agent list does NOT include
  `feat/ghost`
- **WHEN** the user runs `git paw remove feat/ghost`
- **THEN** the command SHALL exit non-zero with the list of live
  agents and SHALL NOT touch any pane, worktree, or session state

#### Scenario: Remove when no session is active

- **GIVEN** no active session for the repository
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the command SHALL exit non-zero with a message
  explaining there is no active session

### Requirement: Uncommitted-work safety

`remove` SHALL refuse to delete a worktree containing uncommitted
changes unless `--force` is passed. The refusal message SHALL
identify the changed files so the user knows what would be lost.
`--keep-worktree` SHALL bypass this check (since nothing is
deleted from disk).

#### Scenario: Refusal on dirty worktree

- **GIVEN** an agent `feat/x` whose worktree has uncommitted
  changes in `src/foo.rs`
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the command SHALL exit non-zero, list `src/foo.rs` as
  uncommitted, and instruct the user to commit or pass `--force`,
  leaving the pane and worktree intact

#### Scenario: --force bypasses the safety check

- **GIVEN** the same dirty worktree
- **WHEN** the user runs `git paw remove feat/x --force`
- **THEN** the worktree SHALL be removed despite the uncommitted
  changes

#### Scenario: --keep-worktree skips the safety check

- **GIVEN** the same dirty worktree
- **WHEN** the user runs `git paw remove feat/x --keep-worktree`
- **THEN** the pane SHALL be closed and the session entry SHALL be
  dropped, but the worktree (including uncommitted changes) SHALL
  remain on disk

### Requirement: Pane closure with grid re-tiling

When removing an agent, the system SHALL kill the agent's tmux
pane and re-apply the agent-grid layout for the new (smaller)
agent count so the grid re-flows without a hole. Remaining panes'
relative order SHALL be preserved.

#### Scenario: Grid re-flows after a removal

- **GIVEN** an active session with 5 agent panes (single row)
- **WHEN** the user runs `git paw remove feat/middle`
- **THEN** the agent grid SHALL be laid out as the 4-pane layout,
  matching what a 4-agent `start` would produce, and the order of
  the remaining 4 agents SHALL be preserved

#### Scenario: Branch→pane mapping is re-derived after removal

- **GIVEN** an active session whose branch→pane mapping was
  established via `pane_current_path`
- **WHEN** an agent in the middle of the grid is removed
- **THEN** subsequent supervisor sweeps SHALL re-derive the
  branch→pane mapping via `pane_current_path` and SHALL continue
  to target the correct panes for the remaining agents

### Requirement: Worktree removal reuses purge logic

`remove` SHALL delegate to the same per-worktree removal logic
`git paw purge` uses (worktree-remove + branch cleanup), unless
`--keep-worktree` is passed.

#### Scenario: Worktree is removed and branch cleaned

- **GIVEN** a clean agent `feat/x`
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the worktree directory SHALL be removed and the branch
  cleanup SHALL match what `git paw purge` would have done for the
  same worktree

#### Scenario: --keep-worktree leaves the worktree and branch in place

- **GIVEN** a clean agent `feat/x`
- **WHEN** the user runs `git paw remove feat/x --keep-worktree`
- **THEN** the worktree directory SHALL remain on disk and the
  branch SHALL remain registered as a normal git worktree (callable
  by `git worktree list`)

### Requirement: Session deregistration

The system SHALL remove the target branch/pane entry from the
session JSON so subsequent `status`, `stop`, `purge`, and `pause`
operations no longer reference the removed agent.

#### Scenario: status no longer lists the removed agent

- **GIVEN** an active session with N agents including `feat/x`
- **WHEN** the user runs `git paw remove feat/x` then
  `git paw status`
- **THEN** the status output SHALL list N−1 agents and SHALL NOT
  include `feat/x`

#### Scenario: purge after remove ignores the removed worktree

- **GIVEN** a session from which `feat/x` was removed
- **WHEN** the user runs `git paw purge`
- **THEN** `purge` SHALL operate on the remaining worktrees and
  SHALL NOT attempt to delete `feat/x` again

### Requirement: Supervisor discovers removal passively

When a supervisor pane is part of the session, the system SHALL
NOT directly signal the supervisor on remove. The supervisor SHALL
notice the agent's absence on its next broker `/status` poll (the
agent's heartbeat stops) and remove it from its coordination
scope.

#### Scenario: Supervisor stops scoping the removed agent within one sweep

- **GIVEN** an active supervisor session containing `feat/x`
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the supervisor SHALL drop `feat/x` from its
  coordination scope by its next sweep, without the `remove`
  command restarting or signalling the supervisor

