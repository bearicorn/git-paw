## ADDED Requirements

### Requirement: Session-state reconciliation is a pure decision over observed state

The system SHALL provide a reconciliation decision that takes a session record, the session's per-repo discovery record, and a set of observations, and returns the list of drifts between them, performing no I/O itself. Observations SHALL be gathered by the caller through ports: the terminal backend (session liveness, and the live surfaces with the worktree each one is in), version control (registered worktrees and whether each directory exists, and what a worktree prune would remove), the broker client (whether `/status` answers and for which repository), and the filesystem (the discovery file, untracked directories under `.git-paw/worktrees/`, and leftover session temp files). The drift kinds SHALL be: session gone while recorded active or paused; recorded worktree missing; stale git-paw-owned worktree registration; recorded agent without a live surface; terminal session up but every agent CLI exited; untracked worktree directory; broker unreachable; broker serving another repository; discovery file out of sync; leftover temp file; and session file from a newer schema. An observation that could not be made SHALL produce no drift. This reconciliation SHALL NOT replace or extend the single liveness probe that `status`, `start`, and `purge --stale` use for receipt staleness.

#### Scenario: A dead terminal session behind an active record is drift

- **GIVEN** a record with status `active` and an observation that the terminal session is gone
- **WHEN** reconciliation runs
- **THEN** it SHALL return a session-gone drift

#### Scenario: An indeterminate terminal observation yields no drift

- **GIVEN** a record with status `active` and a terminal observation that could not be made
- **WHEN** reconciliation runs
- **THEN** it SHALL return no session-gone drift

#### Scenario: A missing worktree and a missing surface are distinguished

- **GIVEN** a live session whose record lists `feat/a` with a worktree directory that no longer exists, and `feat/b` whose directory exists but which no live surface is in
- **WHEN** reconciliation runs
- **THEN** it SHALL return a worktree-missing drift for `feat/a` and a surface-missing drift for `feat/b`

#### Scenario: A discovery file that disagrees with the record is drift

- **GIVEN** a live session whose record lists three worktrees and whose discovery file lists two agents
- **WHEN** reconciliation runs
- **THEN** it SHALL return a discovery-out-of-sync drift

#### Scenario: A consistent session yields no drift

- **GIVEN** a live session whose worktrees all exist, each with a live surface, a matching discovery file, an answering broker for this repository, and no leftover files
- **WHEN** reconciliation runs
- **THEN** it SHALL return no drift

### Requirement: A session marked stopped by repair is recovered by start

When reconciliation repair marks a session `stopped` because its terminal session is gone, a subsequent `git paw start` SHALL take the recovery path for that session — rebuilding its surfaces from the record and relaunching each worker with its recorded CLI session id where available — and SHALL NOT purge the session's worktrees or branches.

#### Scenario: Crash recovery through repair then start

- **GIVEN** a session recorded `active` whose terminal session was killed externally, with all worktrees intact
- **WHEN** the user runs `git paw doctor --fix` and then `git paw start`
- **THEN** the session SHALL be rebuilt with one surface per recorded worktree
- **AND** every recorded worktree and branch SHALL still exist
