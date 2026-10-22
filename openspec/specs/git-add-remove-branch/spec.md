# git-add-remove-branch Specification

## Purpose
Provides `git paw add <branch-name>` to attach a new worktree and agent pane to an already-running session (re-tiling the agent grid, registering the branch in session JSON, and injecting the same boot block a start-time agent receives) and `git paw remove <branch>` to detach a single agent from an active session (closing its tmux pane resolved by worktree path, re-tiling the grid, deregistering the session entry, and reusing purge logic to delete the worktree, with an uncommitted-work safety check bypassable via `--force` and skippable via `--keep-worktree` that ignores git-paw's own injected files). Together they let a supervisor grow or shrink a live session incrementally without restarting it or dropping existing agents' panes. It also makes agent worktree placement configurable via `worktree_placement`: `"child"` nests worktrees inside the repo at `.git-paw/worktrees/<branch-slug>/` while `"sibling"` (the default when unset, preserving the v0.7.0 layout) places them beside the repo, with the child-layout slug derived from the branch name alone.
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

When adding a pane, the system SHALL recompute the agent-grid layout for the new total agent count and re-apply it so all panes match the layout `git paw start` would have produced for that count. Existing agent panes SHALL retain their pane indices so that in-flight `send-keys` targeting continues to address the correct panes.

The re-tile SHALL preserve every OTHER agent's pane: adding an agent SHALL NOT close, drop, or orphan any existing agent's pane, and after the re-tile the live tmux window SHALL contain exactly one pane per session-JSON agent plus the dashboard pane when the session has one (the v0.8.0 G2 dogfood failure dropped a different agent's pane during the re-tile). After re-tiling, each agent row SHALL be rebalanced to equal width per the `tmux-orchestration` "Supervisor-mode pane layout" requirement, so an incrementally-added grid matches a start-time grid of the same agent count in both pane count and pane widths. The first coding-agent index SHALL come from the session's recorded layout, not from a mode constant.

#### Scenario: Grid re-tiles for the new agent count

- **GIVEN** an active session with 4 agent panes (single row)
- **WHEN** the user runs `git paw add feat/fifth`
- **THEN** the agent grid SHALL be laid out as the 5-pane layout (single row of 5), matching what a 5-agent `start` would produce

#### Scenario: Existing pane indices are preserved

- **GIVEN** an active session whose agents occupy pane indices 1 through 5
- **WHEN** the user runs `git paw add feat/new`
- **THEN** the existing panes SHALL retain indices 1 through 5 and the new pane SHALL receive index 6, verified by `pane_current_path` mapping

#### Scenario: No existing agent pane is dropped by the re-tile

- **GIVEN** an active session with 3 agent panes mapped to their worktrees via `pane_current_path`
- **WHEN** the user runs `git paw add feat/fourth`
- **THEN** the live tmux window SHALL contain a pane for each of the original 3 agents plus the new agent (4 agent panes total), with no original agent left without a pane

#### Scenario: Added grid matches a start-time grid width-for-width

- **GIVEN** an active session with 2 agent panes
- **WHEN** the user runs `git paw add feat/third` and the re-tile is applied to a live tmux window
- **THEN** the 3 agent panes SHALL each render at approximately one third of the window width (within a one-column rounding tolerance), matching what a 3-agent `start` would produce

#### Scenario: Adding past the agent cap is rejected

- **GIVEN** an active session already at the 25-agent cap
- **WHEN** the user runs `git paw add feat/twenty-six`
- **THEN** the command SHALL exit non-zero with the same "split into multiple sessions" message `start` uses, and SHALL NOT create a worktree or pane

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

The system SHALL NOT directly signal the orchestrator or the drive loop on add. The external orchestrator SHALL discover the new agent through the broker `/status` roster and the per-repo discovery file, and an in-process drive loop SHALL pick it up on its next sweep from the same sources, without the `add` command restarting or prompting either.

#### Scenario: The new agent appears in the roster sources

- **GIVEN** an active session
- **WHEN** an agent is added
- **THEN** the per-repo discovery file SHALL list the new agent with its pane index
- **AND** the broker watcher SHALL be registered for the new worktree so the agent appears in `/status`
- **AND** no keystroke SHALL be sent to any surface other than the new agent's pane

#### Scenario: A running drive loop sweeps the new agent

- **GIVEN** an `--unattended` session whose drive loop is running
- **WHEN** an agent is added and later shows a live permission prompt
- **THEN** the drive loop SHALL escalate it as a `permission-prompt` naming the added agent on a subsequent sweep
- **AND** SHALL send no keystroke answering it

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

When removing an agent, the system SHALL kill the agent's tmux pane and re-apply the agent-grid layout for the new (smaller) agent count so the grid re-flows without a hole. Remaining panes' relative order SHALL be preserved.

The system SHALL resolve the target pane by mapping the removed branch's worktree to a live pane via `pane_current_path` and SHALL kill that pane by its tmux pane id, regardless of the process running in it — a bare shell (a failed/never-started CLI), a CLI, or any other process. Killing by resolved pane id (not by a position computed from the session JSON) ensures a failed agent whose pane never launched a CLI is still closed rather than orphaned, and that the kill targets the removed agent's pane and never a different agent's pane (the v0.8.0 G2 dogfood failure killed/dropped a different agent's pane because the index was computed from JSON position while a stale orphan pane shifted the grid). The re-tile SHALL preserve every OTHER agent's pane: after the removal the live tmux window SHALL contain exactly one pane per remaining session-JSON agent plus the dashboard pane when the session has one, and each agent row SHALL be rebalanced to equal width per the `tmux-orchestration` "Supervisor-mode pane layout" requirement.

#### Scenario: Grid re-flows after a removal

- **GIVEN** an active session with 5 agent panes (single row)
- **WHEN** the user runs `git paw remove feat/middle`
- **THEN** the agent grid SHALL be laid out as the 4-pane layout, matching what a 4-agent `start` would produce, and the order of the remaining 4 agents SHALL be preserved

#### Scenario: Branch→pane mapping is re-derived after removal

- **GIVEN** an active session whose branch→pane mapping was established via `pane_current_path`
- **WHEN** an agent in the middle of the grid is removed
- **THEN** subsequent sweeps by the drive loop or the orchestrator's helpers SHALL re-derive the branch→pane mapping via `pane_current_path` and SHALL continue to target the correct panes for the remaining agents

#### Scenario: Removing a failed shell-occupied pane still closes it

- **GIVEN** an active session whose agent `feat/x` pane is a bare shell (its CLI never started, the v0.8.0 G1 condition)
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the system SHALL resolve `feat/x`'s pane via `pane_current_path` and kill it by pane id, leaving no orphan pane, even though the pane is running a shell rather than the expected CLI

#### Scenario: Removal does not drop a different agent's pane

- **GIVEN** an active session with agents `feat/a`, `feat/b`, `feat/c` each mapped to a live pane via `pane_current_path`
- **WHEN** the user runs `git paw remove feat/b`
- **THEN** only `feat/b`'s pane SHALL be killed
- **AND** `feat/a` and `feat/c` SHALL each still have exactly one live pane after the re-tile (no collateral pane loss, no orphan)

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

The system SHALL NOT directly signal the orchestrator or the drive loop on remove. The removed agent SHALL disappear from the per-repo discovery file immediately, and the external orchestrator and any running drive loop SHALL stop acting on it from their next read of the roster sources.

#### Scenario: The removed agent leaves the roster sources

- **GIVEN** an active session containing `feat/x`
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the per-repo discovery file SHALL no longer list `feat/x`
- **AND** no keystroke SHALL be sent to any surface as part of the removal

#### Scenario: A running drive loop stops sweeping the removed agent

- **GIVEN** an `--unattended` session whose drive loop is running and contains `feat/x`
- **WHEN** `feat/x` is removed
- **THEN** the drive loop SHALL send no further keystrokes attributed to `feat/x`

### Requirement: Robust uncommitted-work detection

The uncommitted-work check SHALL parse `git status` output in NUL-delimited porcelain form (`--porcelain -z`) rather than splitting on newlines, so that a status entry whose path or content contains whitespace or a newline — including git-paw's own multi-line injected coordination block — can never be misparsed into a phantom changed-file entry. Rename and copy entries, which carry a second NUL-delimited path, SHALL be parsed correctly. The path classification that identifies git-paw-managed files SHALL treat the entire `.git-paw/` subtree as git-paw-managed, in addition to the injected sidecar and the managed `AGENTS.md` block.

#### Scenario: A path containing a newline is not misparsed

- **WHEN** the uncommitted-work check reads `git status` output containing an entry whose path or content includes a newline
- **THEN** that entry is parsed as a single record and is never split into a phantom changed-file entry (such as a `**WARNING:` fragment)

#### Scenario: Clean just-started worktree is not flagged by parse bleed

- **GIVEN** a just-started, otherwise-clean agent worktree carrying only git-paw's injected files
- **WHEN** `git paw remove` runs its uncommitted-work check under load (for example, concurrent test execution)
- **THEN** the check reports no uncommitted user changes and surfaces no git-paw-injected content as a changed path, so removal proceeds without requiring `--force`

### Requirement: Worktree placement is configurable as child or sibling

The system SHALL support a `worktree_placement` setting with two values:
`"child"` and `"sibling"`. When `worktree_placement` is `"child"`, agent
worktrees SHALL be created inside the repository at
`<repo_root>/.git-paw/worktrees/<branch-slug>/`. When `worktree_placement`
is `"sibling"`, agent worktrees SHALL be created in the repository's
parent directory at `<repo_parent>/<project>-<branch-slug>/`, matching the
v0.7.0 layout.

#### Scenario: Child placement creates worktree inside the repo

- **GIVEN** a repository whose effective config has `worktree_placement = "child"`
- **WHEN** a worktree is created for branch `feat/auth-flow`
- **THEN** the worktree SHALL be created at `<repo_root>/.git-paw/worktrees/feat-auth-flow/`

#### Scenario: Sibling placement creates worktree beside the repo

- **GIVEN** a repository whose effective config has `worktree_placement = "sibling"`
- **WHEN** a worktree is created for branch `feat/auth-flow`
- **THEN** the worktree SHALL be created at `<repo_parent>/<project>-feat-auth-flow/`

### Requirement: Child placement derives the branch slug from the branch name

The system SHALL derive the child-layout branch slug from the branch name
alone, replacing `/` with `-` and stripping characters outside the safe
set of letters, digits, dot, dash, and underscore. The slug SHALL NOT
include the project name, because the directory already resides under that
project's `.git-paw/worktrees/`.

#### Scenario: Branch with a slash maps to a dashed slug

- **GIVEN** `worktree_placement = "child"`
- **WHEN** a worktree is created for branch `feat/auth-flow`
- **THEN** the slug SHALL be `feat-auth-flow`
- **AND** the worktree path SHALL end with `.git-paw/worktrees/feat-auth-flow`

#### Scenario: Branch with unsafe characters has them stripped

- **GIVEN** `worktree_placement = "child"`
- **WHEN** a worktree is created for branch `fix/issue#42`
- **THEN** the slug SHALL be `fix-issue42`
- **AND** the worktree path SHALL end with `.git-paw/worktrees/fix-issue42`

### Requirement: Absent placement config defaults to sibling

The system SHALL behave as if placement were sibling when the effective
config does not specify `worktree_placement`, producing the exact v0.7.0
sibling layout. Pre-existing configs and sessions created before this
field SHALL be unaffected.

#### Scenario: Missing field uses sibling layout

- **GIVEN** a repository whose effective config has no `worktree_placement` field
- **WHEN** a worktree is created for branch `feat/auth-flow`
- **THEN** the worktree SHALL be created at `<repo_parent>/<project>-feat-auth-flow/` (sibling layout)

### Requirement: Sessions record concrete worktree paths for both layouts

The session JSON SHALL record the concrete absolute worktree path produced
at creation time for each worktree, regardless of placement. Resume,
status, and purge SHALL operate on the recorded path and SHALL NOT
re-derive the path from `worktree_placement`, so a session created under
one placement remains resumable and purgeable even if the config later
changes.

#### Scenario: Child-layout session round-trips through resume and purge

- **GIVEN** a session created with `worktree_placement = "child"` whose worktree path is `<repo_root>/.git-paw/worktrees/feat-x/`
- **WHEN** the session is saved, reloaded, and then purged
- **THEN** the reloaded session SHALL report the child path
- **AND** purge SHALL remove the worktree at that recorded child path

#### Scenario: Sibling-layout session round-trips through resume and purge

- **GIVEN** a session created with `worktree_placement = "sibling"` whose worktree path is `<repo_parent>/<project>-feat-x/`
- **WHEN** the session is saved, reloaded, and then purged
- **THEN** the reloaded session SHALL report the sibling path
- **AND** purge SHALL remove the worktree at that recorded sibling path

#### Scenario: Config flip does not orphan an existing session's worktree

- **GIVEN** a session was created under `worktree_placement = "sibling"` with a recorded sibling path
- **AND** the config is later changed to `worktree_placement = "child"`
- **WHEN** the session is purged
- **THEN** purge SHALL remove the worktree at the recorded sibling path, not at a re-derived child path

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

### Requirement: Hot-attach works on every grid session

The `add` and `remove` subcommands SHALL operate on every live session whose recorded pane layout is the agent grid — bare, spec-driven and supervised alike — and on supervised sessions saved by earlier binaries, using the session's recorded first coding-agent index. They SHALL NOT refuse a session because of its `SessionMode`. A live session saved with the earlier tiled layout (a bare session with no recorded grid layout) SHALL be refused before any worktree, pane or session state is touched, with an error that names the tiled layout as the reason and tells the user to stop the session and run `git paw start` to relaunch it in the grid layout; the message SHALL NOT cite a release version.

#### Scenario: Add to a bare grid session

- **GIVEN** a live bare session (no supervisor mode) started with two branches and the broker enabled
- **WHEN** the user runs `git paw add feat/third`
- **THEN** a third agent pane SHALL be created at pane 3 running the session's CLI in the new worktree
- **AND** the agent grid SHALL match what a 3-agent `start` would produce

#### Scenario: Remove from a bare grid session

- **GIVEN** a live bare grid session with agents `feat/a`, `feat/b`, `feat/c`
- **WHEN** the user runs `git paw remove feat/b`
- **THEN** only `feat/b`'s pane SHALL be closed and the remaining grid SHALL re-flow to the 2-agent layout

#### Scenario: Add to a broker-less grid session

- **GIVEN** a live session with the broker disabled whose agents start at pane 0
- **WHEN** the user runs `git paw add feat/new`
- **THEN** the new agent SHALL receive the next pane index after the existing agents

#### Scenario: An earlier tiled session is refused explicitly

- **GIVEN** a live bare session saved without a recorded grid layout
- **WHEN** the user runs `git paw add feat/x` or `git paw remove feat/y`
- **THEN** the command SHALL exit non-zero with an error naming the tiled layout and telling the user to stop and start the session again
- **AND** the error SHALL NOT contain a version string such as `v0.6.0`
- **AND** no worktree, pane or session entry SHALL be created or removed

#### Scenario: A supervised session saved earlier stays hot-attachable

- **GIVEN** a live supervised session saved by an earlier binary with `dashboard_pane = Some(1)` and agents from pane 2
- **WHEN** the user runs `git paw add feat/new`
- **THEN** the new agent SHALL be added after the existing agents and the existing agents' panes SHALL keep their indices

