## Context

The auto-approve poll loop runs in the dashboard subprocess (attended supervisor sessions;
disabled under `--unattended`, where the drive loop is the sole approver). Each tick it
fetches broker `/status`, picks stalled agents, and for each calls
`PaneResolver::pane_index_for` and `WorktreeResolver::worktree_root_for`. Production backs
both with maps frozen at dashboard start; tests back them with closures. The traits are the
right seam — only the production backing is wrong.

The drive loop already resolves panes by path every sweep (`drive::resolve_pane_agent`,
`TmuxPaneEnumerator`, `PaneInfo`, `AgentPane`). v0.18.0 removes the supervisor pane; this
fix keeps v0.17.0 safe until then with the least new code.

## Goals / Non-Goals

**Goals:**
- Every approval targets the pane whose live working directory is the agent's recorded
  worktree, and classifies against that worktree.
- Agents added mid-session are approved; removals never shift approvals onto a neighbour.

**Non-Goals:**
- Changing the drive loop (its roster is fixed at loop start; an agent added during an
  `--unattended` run resolves `Unknown`. Out of scope here; v0.18.0 reworks the loop).
- Refreshing the learnings aggregator or conflict watcher rosters.
- Changing classification rules, the send gate, or dedup.

## Decisions

### D1 — A per-tick `LiveRoster` implementing both resolver traits

`poll::LiveRoster { panes: HashMap<String, usize>, worktrees: HashMap<String, PathBuf> }`,
built by `LiveRoster::resolve(panes: &[PaneInfo], agents: &[AgentPane])`:
- for each pane, `drive::resolve_pane_agent(..)`; keep only `PaneRole::Coding(id)`;
- an id resolved by more than one pane is removed (ambiguous → skip);
- `worktrees` holds `agent_id → worktree_path` only for agents that resolved to a pane, so
  pane and boundary always come from the same roster entry.

It implements `PaneResolver` and `WorktreeResolver`, so `PollContext`, `tick_from_status`
and `drive_outcomes` are unchanged. The existing `let Some(pane_index) = .. else
{ continue }` already gives "skip, never guess" for an unresolved agent.

*Alternative rejected:* modifying the drive-loop requirement to cover both loops. It would
couple two loops' contracts in one requirement and the drive loop needs no change; an
ADDED requirement in `approval-auto`, which owns the poll loop, is narrower.

*Alternative rejected:* refreshing the map only when the session file's mtime changes. It
saves one `tmux list-panes` per tick (the period is ≥5s) but still misses a pane renumber
that happens without a session write (a pane dying), and adds state.

### D2 — The thread rebuilds the roster each tick

In `spawn_auto_approve_thread`, after fetching `/status` and before building `PollContext`:
1. `session::find_session_for_repo(&repo_root)` and
   `drive::TmuxPaneEnumerator.list_panes(&session_name)` (enumerator made `pub`).
2. `LiveRoster::for_session(session: Option<&Session>, session_name: &str, panes:
   &[PaneInfo]) -> Option<LiveRoster>` — `None` when the session is absent, unreadable, or
   its name differs (torn down or replaced): the tick is skipped, no action. Otherwise
   `AgentPane::roster_from_session(&session)` (new; `slugify_branch(&wt.branch)`,
   `wt.worktree_path`, `wt.branch` — the mapping `drive_unattended_loop` builds inline today,
   which switches to it) feeds `LiveRoster::resolve`.
3. The returned roster is passed as both `resolver` and `worktree_resolver`.

Keeping step 2 a pure function over already-read inputs is what makes the "unreadable
roster takes no action" scenario unit-testable; the thread body keeps only the two reads.

`AutoApproveWiring.pane_map` / `worktree_map` and their construction in `cmd_dashboard`
are deleted (orphaned by this change).

## Risks / Trade-offs

- [An agent `cd`s its pane into a subdirectory or elsewhere → no match → its prompts are no
  longer auto-approved] → intended: fail closed. The drive loop has the same behaviour and
  the skill already treats `pane_current_path` as authoritative.
- [One extra `tmux list-panes` and one session-file read per tick] → negligible at a ≥5s
  period.
- [`find_session_for_repo` semantics change in `bundled-helper-discovery`] → that change
  only widens matching for worktree paths; `repo_root` is the repository root, unaffected.

## Migration Plan

None — internal wiring only. Rollback is a plain revert.
