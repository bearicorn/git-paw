# add-branch Specification

## Purpose
TBD - created by archiving change git-paw-add. Update Purpose after archive.
## Requirements
### Requirement: git paw add subcommand

The system SHALL provide a `git paw add <branch-name>` subcommand
that attaches a new worktree and tmux pane to an already-running
session. The subcommand SHALL accept `--cli <id>` to choose the
agent CLI (defaulting to the session's default CLI) and
`--from-spec <change>` to resolve the branch name and CLI from a
spec. The subcommand SHALL fail with an actionable error when no
session is active for the repository.

#### Scenario: Add a branch to a running session

- **GIVEN** an active session with N agent panes
- **WHEN** the user runs `git paw add feat/new-thing`
- **THEN** the system SHALL create a worktree for `feat/new-thing`,
  spawn a new agent pane running the default CLI, and register the
  branch in the session JSON, leaving the existing N panes intact

#### Scenario: Add with an explicit CLI

- **GIVEN** an active session
- **WHEN** the user runs `git paw add feat/x --cli codex`
- **THEN** the new pane SHALL launch the `codex` CLI in the new
  worktree

#### Scenario: Add when no session is active

- **GIVEN** no active session for the repository
- **WHEN** the user runs `git paw add feat/x`
- **THEN** the command SHALL exit non-zero with a message
  explaining there is no active session and suggesting
  `git paw start`

#### Scenario: Add an unknown --cli value

- **WHEN** the user runs `git paw add feat/x --cli nonesuch`
  where `nonesuch` is not a detected CLI
- **THEN** the command SHALL exit non-zero with a message listing
  the detected CLI ids, and SHALL NOT create a worktree or pane

### Requirement: Worktree creation reuses start conventions

The `add` subcommand SHALL create the worktree using the same
naming convention, base-branch resolution, and idempotent-create
behaviour as `git paw start`. Adding a branch whose worktree
already exists SHALL reuse the existing worktree rather than error.

#### Scenario: Worktree naming matches start

- **WHEN** the user runs `git paw add feat/x` in project `myproj`
- **THEN** the created worktree SHALL follow the same path
  convention a `git paw start` launch of `feat/x` would produce

#### Scenario: Idempotent worktree create on re-add

- **GIVEN** a worktree for `feat/x` already exists on disk
- **WHEN** the user runs `git paw add feat/x`
- **THEN** the command SHALL reuse the existing worktree without
  error

### Requirement: Pane spawn with grid re-tiling

When adding a pane, the system SHALL recompute the agent-grid
layout for the new total agent count and re-apply it so all panes
match the layout `git paw start` would have produced for that
count. Existing agent panes SHALL retain their pane indices so that
in-flight `send-keys` targeting continues to address the correct
panes.

#### Scenario: Grid re-tiles for the new agent count

- **GIVEN** an active session with 4 agent panes (single row)
- **WHEN** the user runs `git paw add feat/fifth`
- **THEN** the agent grid SHALL be laid out as the 5-pane layout
  (single row of 5), matching what a 5-agent `start` would produce

#### Scenario: Existing pane indices are preserved

- **GIVEN** an active session whose agents occupy pane indices
  2 through 6
- **WHEN** the user runs `git paw add feat/new`
- **THEN** the existing panes SHALL retain indices 2 through 6 and
  the new pane SHALL receive the next index, verified by
  `pane_current_path` mapping

#### Scenario: Adding past the agent cap is rejected

- **GIVEN** an active session already at the 25-agent cap
- **WHEN** the user runs `git paw add feat/twenty-six`
- **THEN** the command SHALL exit non-zero with the same
  "split into multiple sessions" message `start` uses, and SHALL
  NOT create a worktree or pane

### Requirement: Session registration

The system SHALL append the new branch/pane to the session JSON
(`.git-paw/sessions/paw-<project>.json`) so that subsequent
`status`, `stop`, `purge`, and `pause` operations include the
added agent.

#### Scenario: status reflects the added agent

- **GIVEN** an active session with N agents
- **WHEN** the user runs `git paw add feat/x` then `git paw status`
- **THEN** the status output SHALL list N+1 agents including
  `feat/x`

#### Scenario: purge removes the added worktree

- **GIVEN** a session to which `feat/x` was added
- **WHEN** the user runs `git paw purge`
- **THEN** the `feat/x` worktree SHALL be removed alongside the
  originally-started worktrees

### Requirement: Boot injection parity

The added agent SHALL receive the same boot injection a start-time
agent receives: the AGENTS.md boot block, the broker boot block
(when broker is enabled), the initial spec/task prompt, and the
paste-buffer double-Enter submit. The added agent SHALL begin
working from its boot prompt without further user action (when the
session is not paused).

#### Scenario: Added agent receives the full boot block

- **GIVEN** an active session with broker enabled
- **WHEN** the user runs `git paw add feat/x --from-spec my-change`
- **THEN** the new pane SHALL contain the injected broker boot
  block and the full spec/task prompt, submitted (not left in the
  paste buffer)

#### Scenario: Added agent auto-registers with the broker

- **GIVEN** an active session with broker enabled
- **WHEN** an agent is added
- **THEN** the broker `/status` endpoint SHALL list the new agent
  after it publishes its first heartbeat, with no broker restart

### Requirement: Supervisor discovers the added agent passively

When a supervisor pane is part of the session, the system SHALL
NOT directly signal the supervisor on add. The supervisor SHALL
discover the new agent through its normal broker `/status` poll /
sweep cycle.

#### Scenario: Supervisor picks up the new agent on its next sweep

- **GIVEN** an active supervisor session
- **WHEN** an agent is added
- **THEN** the supervisor SHALL include the new agent in its
  coordination scope by its next sweep, without the `add` command
  restarting or re-prompting the supervisor

### Requirement: Paused-session interplay

When the session is in the paused state, an added pane SHALL also
start paused (boot block injected but the agent held), consistent
with the rest of the session. On `resume`, the added agent SHALL
begin working alongside the others.

#### Scenario: Add while paused starts the new pane paused

- **GIVEN** a paused session
- **WHEN** the user runs `git paw add feat/x`
- **THEN** the new pane SHALL be in the paused state (not actively
  working) until the next `git paw resume`

#### Scenario: Resume starts the added agent

- **GIVEN** a paused session to which `feat/x` was added
- **WHEN** the user runs `git paw resume`
- **THEN** the `feat/x` agent SHALL submit its boot prompt and
  begin working alongside the resumed agents

### Requirement: --from-spec resolution

The `--from-spec <change>` flag SHALL resolve a single spec across
all three backends (OpenSpec change, Markdown spec file, Spec Kit
feature) using the same resolution logic as `--specs NAME`, and
SHALL derive the branch name and CLI from it. An unknown spec name
SHALL error with the discovered-set candidate list.

#### Scenario: Add a branch from an OpenSpec change

- **WHEN** the user runs `git paw add --from-spec add-export`
  where `add-export` is a discovered OpenSpec change
- **THEN** the system SHALL derive the branch name and CLI from
  that change and attach the agent accordingly

#### Scenario: Unknown spec name errors with candidates

- **WHEN** the user runs `git paw add --from-spec no-such-change`
- **THEN** the command SHALL exit non-zero listing the discovered
  spec names, and SHALL NOT create a worktree or pane

