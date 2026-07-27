# add-remove-branch Specification

## Purpose
Provides `git paw add <branch-name>` to attach a new worktree and agent pane to an already-running session (re-tiling the agent grid, registering the branch in session JSON, and injecting the same boot block a start-time agent receives) and `git paw remove <branch>` to detach a single agent from an active session (closing its tmux pane resolved by worktree path, re-tiling the grid, deregistering the session entry, and reusing purge logic to delete the worktree, with an uncommitted-work safety check bypassable via `--force` and skippable via `--keep-worktree` that ignores git-paw's own injected files). Together they let a supervisor grow or shrink a live session incrementally without restarting it or dropping existing agents' panes.

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

The re-tile SHALL preserve every OTHER agent's pane: adding an agent
SHALL NOT close, drop, or orphan any existing agent's pane, and after
the re-tile the live tmux window SHALL contain exactly one pane per
session-JSON agent plus the supervisor and dashboard panes (the v0.8.0
G2 dogfood failure dropped a different agent's pane during the re-tile).
After re-tiling, each agent row SHALL be rebalanced to equal width per
the `tmux-orchestration` "Supervisor-mode pane layout" requirement, so
an incrementally-added grid matches a start-time grid of the same agent
count in both pane count and pane widths.

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

#### Scenario: No existing agent pane is dropped by the re-tile

- **GIVEN** an active session with 3 agent panes mapped to their
  worktrees via `pane_current_path`
- **WHEN** the user runs `git paw add feat/fourth`
- **THEN** the live tmux window SHALL contain a pane for each of the
  original 3 agents plus the new agent (4 agent panes total), with no
  original agent left without a pane

#### Scenario: Added grid matches a start-time grid width-for-width

- **GIVEN** an active session with 2 agent panes
- **WHEN** the user runs `git paw add feat/third` and the re-tile is
  applied to a live tmux window
- **THEN** the 3 agent panes SHALL each render at approximately one
  third of the window width (within a one-column rounding tolerance),
  matching what a 3-agent `start` would produce

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
with the rest of the session. On the next `git paw start` (which
resumes a paused session — there is no separate `resume`
subcommand), the added agent SHALL begin working alongside the
others.

#### Scenario: Add while paused starts the new pane paused

- **GIVEN** a paused session
- **WHEN** the user runs `git paw add feat/x`
- **THEN** the new pane SHALL be in the paused state (not actively
  working) until the next `git paw start`

#### Scenario: Resuming starts the added agent

- **GIVEN** a paused session to which `feat/x` was added
- **WHEN** the user runs `git paw start` to resume the session
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

### Requirement: Session-JSON to tmux reconciliation

The system SHALL provide a reconciliation that detects divergence
between the session JSON (`.git-paw/sessions/paw-<project>.json`) and
the live tmux panes — specifically a session-JSON agent that has no live
tmux pane (the v0.8.0 G2 desync, where an agent remained in the JSON and
broker roster while its pane had been dropped). On the `add` path, after
the new pane is spliced and the grid re-tiled, the system SHALL verify
that every session-JSON agent maps to a live pane via `pane_current_path`
and SHALL surface any agent that does not, so the desync is visible and
recoverable rather than silent.

#### Scenario: Reconciliation reports an agent with no live pane

- **GIVEN** a session JSON listing an agent whose tmux pane is missing
  (its worktree path appears in no live pane's `pane_current_path`)
- **WHEN** the reconciliation runs
- **THEN** it SHALL report that agent as having no live pane

#### Scenario: Reconciliation passes when JSON and tmux agree

- **GIVEN** a session whose JSON agents each map to a live pane via
  `pane_current_path`
- **WHEN** the reconciliation runs after `git paw add`
- **THEN** it SHALL report no divergence

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

The uncommitted-work check SHALL ignore git-paw's own managed/injected
files when deciding whether to refuse and when listing changed files.
A path is git-paw-managed when it is the injected sidecar
`.git-paw/AGENTS.local.md`, or when it is the tracked `AGENTS.md` whose
only uncommitted change is the presence of git-paw's managed
`<!-- git-paw:start -->` block (i.e. the file is otherwise unmodified
relative to HEAD). These files are git-paw injection produced by
`start`/`setup_worktree_agents_md`, not the user's uncommitted work, so
they SHALL NOT, on their own, cause `remove` to refuse, and they SHALL
NOT appear in the refusal message. A worktree whose ONLY uncommitted
entries are git-paw-managed files SHALL be treated as clean: the pane
SHALL close, the worktree SHALL be removed, and the session entry SHALL
be dropped without requiring `--force`. Any uncommitted change to a file
that is NOT git-paw-managed — including a user edit to `AGENTS.md`
outside the managed block — SHALL still cause `remove` to refuse without
`--force`, and SHALL be listed in the refusal message.

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

#### Scenario: Clean just-started worktree with only git-paw-injected files is removed

- **GIVEN** an agent `feat/x` whose worktree was just provisioned by
  `start`, so its only uncommitted entry is the git-paw-injected sidecar
  `.git-paw/AGENTS.local.md` (and/or the managed `<!-- git-paw:start -->`
  block) with no user edits
- **WHEN** the user runs `git paw remove feat/x` without `--force`
- **THEN** the command SHALL succeed, the pane SHALL be closed, the
  worktree SHALL be removed, and the branch entry SHALL be dropped from
  the session JSON
- **AND** the command SHALL NOT report `.git-paw/AGENTS.local.md` or the
  managed block as uncommitted changes

#### Scenario: Genuine user edit still refuses, and managed files are not listed

- **GIVEN** an agent `feat/x` whose worktree contains BOTH a
  user-authored uncommitted change in `src/foo.rs` AND the git-paw-injected
  sidecar `.git-paw/AGENTS.local.md`
- **WHEN** the user runs `git paw remove feat/x` without `--force`
- **THEN** the command SHALL exit non-zero and refuse the removal
- **AND** the refusal message SHALL list `src/foo.rs`
- **AND** the refusal message SHALL NOT list `.git-paw/AGENTS.local.md`

### Requirement: Pane closure with grid re-tiling

When removing an agent, the system SHALL kill the agent's tmux
pane and re-apply the agent-grid layout for the new (smaller)
agent count so the grid re-flows without a hole. Remaining panes'
relative order SHALL be preserved.

The system SHALL resolve the target pane by mapping the removed
branch's worktree to a live pane via `pane_current_path` and SHALL kill
that pane by its tmux pane id, regardless of the process running in it
— a bare shell (a failed/never-started CLI), a CLI, or any other
process. Killing by resolved pane id (not by a position computed from
the session JSON) ensures a failed agent whose pane never launched a CLI
is still closed rather than orphaned, and that the kill targets the
removed agent's pane and never a different agent's pane (the v0.8.0 G2
dogfood failure killed/dropped a different agent's pane because the index
was computed from JSON position while a stale orphan pane shifted the
grid). The re-tile SHALL preserve every OTHER agent's pane: after the
removal the live tmux window SHALL contain exactly one pane per remaining
session-JSON agent plus the supervisor and dashboard panes, and each
agent row SHALL be rebalanced to equal width per the `tmux-orchestration`
"Supervisor-mode pane layout" requirement.

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

#### Scenario: Removing a failed shell-occupied pane still closes it

- **GIVEN** an active session whose agent `feat/x` pane is a bare shell
  (its CLI never started, the v0.8.0 G1 condition)
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the system SHALL resolve `feat/x`'s pane via
  `pane_current_path` and kill it by pane id, leaving no orphan pane,
  even though the pane is running a shell rather than the expected CLI

#### Scenario: Removal does not drop a different agent's pane

- **GIVEN** an active session with agents `feat/a`, `feat/b`, `feat/c`
  each mapped to a live pane via `pane_current_path`
- **WHEN** the user runs `git paw remove feat/b`
- **THEN** only `feat/b`'s pane SHALL be killed
- **AND** `feat/a` and `feat/c` SHALL each still have exactly one live
  pane after the re-tile (no collateral pane loss, no orphan)

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

### Requirement: Robust uncommitted-work detection

The uncommitted-work check SHALL parse `git status` output in NUL-delimited porcelain form (`--porcelain -z`) rather than splitting on newlines, so that a status entry whose path or content contains whitespace or a newline — including git-paw's own multi-line injected coordination block — can never be misparsed into a phantom changed-file entry. Rename and copy entries, which carry a second NUL-delimited path, SHALL be parsed correctly. The path classification that identifies git-paw-managed files SHALL treat the entire `.git-paw/` subtree as git-paw-managed, in addition to the injected sidecar and the managed `AGENTS.md` block.

#### Scenario: A path containing a newline is not misparsed

- **WHEN** the uncommitted-work check reads `git status` output containing an entry whose path or content includes a newline
- **THEN** that entry is parsed as a single record and is never split into a phantom changed-file entry (such as a `**WARNING:` fragment)

#### Scenario: Clean just-started worktree is not flagged by parse bleed

- **GIVEN** a just-started, otherwise-clean agent worktree carrying only git-paw's injected files
- **WHEN** `git paw remove` runs its uncommitted-work check under load (for example, concurrent test execution)
- **THEN** the check reports no uncommitted user changes and surfaces no git-paw-injected content as a changed path, so removal proceeds without requiring `--force`
