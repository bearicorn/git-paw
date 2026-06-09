# session-recovery-integrity Specification

## Purpose
TBD - created by archiving change session-recovery-pane-integrity-v0-6-x. Update Purpose after archive.
## Requirements
### Requirement: Recovery rebuilds exactly the session's panes

Recovering a stopped session SHALL rebuild exactly the panes the session
defines — in supervisor mode, the supervisor pane plus the dashboard pane
plus one pane per recorded worktree (`N + 2`). The recovery SHALL NOT create
more panes than the session defines.

#### Scenario: Recovery of an N-worktree supervisor session creates N+2 panes

- **GIVEN** a stopped supervisor-mode session with 3 recorded worktrees
- **WHEN** `git paw start` recovers it
- **THEN** the rebuilt tmux session SHALL have exactly 5 panes (supervisor +
  dashboard + 3 agents) — not a repeatedly-split column that overflows the
  window

#### Scenario: Recovery that cannot fit fails cleanly

- **GIVEN** a recovery whose panes cannot be tiled in the available canvas
- **WHEN** the rebuild fails
- **THEN** it SHALL NOT leave a half-tiled session and SHALL NOT mutate the
  persisted session state

### Requirement: A failed start preserves persisted session state

The system SHALL leave the persisted per-repo session JSON unchanged when a
`git paw start` invocation errors before completing its launch. The session
state file SHALL be rewritten only after a launch completes successfully, so
an aborted start cannot destroy a recoverable session (e.g. rewrite it to an
empty worktree list).

#### Scenario: Aborted start does not corrupt the session JSON

- **GIVEN** an existing saved session with N worktrees
- **WHEN** a subsequent `git paw start` errors mid-launch
- **THEN** the saved session JSON SHALL still list those N worktrees
  (unchanged), so a later `git paw start` can still recover it

### Requirement: Headless canvas fits multi-agent supervisor layouts

The system SHALL size the detached/headless `new-session` canvas and the
global `default-size` large enough to tile a supervisor session with the
supported number of agents without tmux returning `no space for new pane`.
An attached client still resizes the session to the real terminal on attach.

#### Scenario: Headless supervisor session tiles without overflow

- **GIVEN** a detached (no attached client) supervisor launch with several
  agents
- **WHEN** the session is built at the headless fallback size
- **THEN** all panes SHALL tile successfully (no `no space for new pane`
  error)

