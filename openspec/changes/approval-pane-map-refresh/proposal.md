## Why

**GP-45 (safety).** The dashboard's auto-approve poll loop maps agents to panes once, at
dashboard start, by position: `cmd_dashboard` (`src/main.rs`) builds `pane_map` as
`idx + SUPERVISOR_PANE_OFFSET` over the saved session's worktrees, and
`spawn_auto_approve_thread` (`src/commands/supervisor.rs`) moves it — and the parallel
`worktree_map` — into the thread closure, where neither is ever refreshed.

- After `git paw add`, the new agent has no entry: it is never auto-approved, and stalls on
  every prompt until a human notices.
- After `git paw remove` of a middle agent, tmux renumbers the panes. Every later agent's
  stale index now points at its neighbour's pane: the loop captures the neighbour's prompt,
  classifies it against the *wrong agent's* worktree boundary (the worktree-confined
  `git add`/`commit`, file-op, and dev-test rules), and dispatches the approval keystrokes
  into the neighbour's pane. A command can be judged "inside the worktree" relative to a
  worktree it is not running in.

The drive loop already avoids this: *Pane-to-agent resolution via pane_current_path*
(`supervisor-unattended-operation`) mandates path-based resolution, and
`drive::resolve_pane_agent` implements it. The poll loop never adopted it.

## What Changes

- On every tick the poll loop re-reads the session's worktree roster and the live pane list
  (`tmux list-panes` with `pane_current_path`) and resolves each agent's pane by matching
  its `worktree_path`, reusing `drive::resolve_pane_agent`. The worktree boundary used for
  classification comes from the same tick's roster.
- An agent whose worktree matches no live pane — or more than one — is skipped for that
  tick: no capture, no keystrokes, no forward. The loop never guesses.
- When the session roster cannot be read, the tick takes no action.
- No CLI, config, or wire-format change. The drive loop is unchanged.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `approval-auto`: ADDED *Poll-loop pane resolution follows the live layout* — per-tick,
  path-based pane and worktree resolution for the auto-approve poll loop.

## Impact

- `src/supervisor/poll.rs` — new `LiveRoster` (agent → pane index and agent → worktree root
  for one tick), built by `LiveRoster::resolve(&[PaneInfo], &[AgentPane])` via
  `drive::resolve_pane_agent`; implements `PaneResolver` and `WorktreeResolver`. Tick tests.
- `src/supervisor/drive.rs` — make the production `TmuxPaneEnumerator` `pub` (reused, not
  duplicated); add `AgentPane::roster_from_session(&Session) -> Vec<AgentPane>`.
- `src/commands/supervisor.rs` — `AutoApproveWiring` drops `pane_map` / `worktree_map`;
  `spawn_auto_approve_thread` rebuilds a `LiveRoster` each tick from
  `session::find_session_for_repo(&repo_root)` (matching `session_name`) and the live pane
  list. `drive_unattended_loop` uses `roster_from_session` instead of its inline mapping.
- `src/main.rs` — `cmd_dashboard` no longer builds `pane_map` / `worktree_map`.
- **Safety**: strictly narrows what is approved — an approval now requires the pane's live
  working directory to equal the agent's recorded worktree. Failure mode of a bug here is a
  missed approval, not a misdirected one.
- **Delta coordination** (grepped every pending change): `live-prompt-tail-window` modifies
  *Live-prompt gate* and `broad-grant-scope-safety` modifies *Broad grant restricted to
  allowlisted non-arbitrary-code verbs* and *Option-index selection for Yes/No prompts* in
  `approval-auto`; this change ADDs a new requirement, so no collision. No pending change
  touches `supervisor-unattended-operation`'s *Pane-to-agent resolution via
  pane_current_path*; it is left unmodified (the drive loop already conforms).
- **Sequencing**: independent of `cli-unavailable-detection-gaps` (which edits
  `sweep_pane`); this change only touches `drive.rs` visibility and adds a constructor.
