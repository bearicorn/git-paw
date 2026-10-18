## Context

git-paw shells out to tmux for every terminal-surface operation, and tmux's model has become the
domain model: agents are resolved by `pane_current_path`, output is captured by `pipe-pane`, layouts
are `split-window` sequences, and `pane_index` is written into the per-repo discovery JSON that
`sweep.sh` reads. The only existing seam, `CommandRunner`, abstracts *how* a command runs, not *what
the terminal is*. `docs/src/architecture-direction.md` names the terminal port as the main open item
and fixes its rules: domain vocabulary, opaque surface ids, compile-time in-tree adapters, zero new
dependencies, persisted state additive.

By this step `supervisor-role-typing` has put `SurfaceRole` and `SessionLayout` in core, and
`supervisor-pane-removal` + `pane-less-session-parity` have left one pane-less grid layout with no
supervisor surface — so the port needs no supervisor-pane cases.

v0.17.0 `classifier-retirement` has deleted the attended auto-approver (`poll.rs`, `approve.rs`,
`approval_send_gate`, the dashboard's positional `pane_map`), moved `KeyDispatcher` /
`TmuxKeyDispatcher` to `src/supervisor/keys.rs`, and left the drive loop sending only nudge,
stale-input and buffered-input keystrokes. GP-45 (that approver's positional pane map) is moot, so
this change has no behavior change to make.

## Goals / Non-Goals

**Goals:**
- One port, `TerminalBackend`, that every orchestration-path terminal operation goes through: the
  drive loop, the launch paths (bare, spec-driven and supervised `start`, recovery, restart from
  pause), `add`, `remove`, the readiness gate, prompt submission and the agent inventory.
- Domain vocabulary only; surfaces addressed by an opaque `SurfaceId`.
- `TmuxBackend` as the only production adapter, argv-identical to today.
- One owner-resolution function in core; one fake backend for logic tests.
- Standing guards; persisted schema byte-identical with the back-compat rule written down.

**Non-Goals:**
- Any behavior change, a second backend, runtime plugins or a stable ABI.
- Moving the lifecycle commands (`pause`, `stop`, `attach`, `status`, `doctor`) behind the port;
  they keep calling `src/tmux/` helpers, which the confinement guard allows.
- Changing how `inventory::match_pane` or `sweep.sh` match owners (see D4).
- Rewriting `src/tmux/`; it stays the command-building substrate.
- The service-surface operation for the broker host (`broker-host-separation`).
- Deciding the tmux / PTY / headless question (a decision-only spike this cycle).

## Decisions

### D1 — Port shape: what git-paw needs, named in git-paw's terms

`src/terminal/mod.rs` (illustrative):

- `SurfaceId` — opaque newtype, constructed only by an adapter.
- `SurfaceInfo { id: SurfaceId, owner: Option<PathBuf> }` — the answer to "which worktree owns
  this surface?" (tmux answers with `pane_current_path`; a PTY backend knows by construction; a
  headless backend never asks).
- `SessionPlan { name, surfaces: Vec<SurfaceSpec>, env, geometry }`,
  `SurfaceSpec { label, owner, launch_command }`.
- `Key` — named keys (`Enter`, `CtrlU`, digits) kept distinct from text so the split-send +
  settle-delay discipline survives any backend.
- `trait TerminalBackend { open_session, session_alive, close_session, set_session_env,
  spawn_surface, kill, list_surfaces, label_surface, send_text, send_key, capture_tail,
  stream_to, liveness, attach, detach_clients, layout }` — `layout` takes the existing grid
  geometry and is a no-op for a backend that does not tile.

*Review question per method:* how would a PTY backend and a headless backend answer it? If only tmux
can, the method is mis-shaped. *Alternative rejected:* a tidier `src/tmux/` module with tmux-shaped
functions — it removes duplication but keeps every future backend tmux-shaped.

### D2 — Layer the adapter over `src/tmux/` and `CommandRunner`

`TmuxBackend` composes `TmuxSessionBuilder`, `TmuxCommand`, the add/remove grid builders,
`list_panes_with_paths`, `list_panes_with_indices`, `resolve_pane_id_for_worktree`,
`resolve_pane_index_for_worktree`, `reconcile_agents_to_panes`, `session_liveness_for` (its
`@paw_role` filter unchanged), `rebalance_agent_rows`, and absorbs the
ad-hoc spawns (`submit_prompt_to_pane`, `launch-prompt-entry-point`'s idle-wait captures — now
`capture_tail` reads judged by the pure idle predicate — and its entry-point submit — `send_text`
then `send_key`), `permission_prompt::capture_pane`, `keys.rs`'s
`TmuxKeyDispatcher`, `TmuxPaneEnumerator`, the inventory's `list-panes` / title reads,
`restart_from_pause`'s split).
Argv is unchanged; `CommandRunner` stays underneath for process execution and argv-recording tests.
Readiness polling (`tmux/readiness.rs`) captures through the adapter, removing its import of
`supervisor::permission_prompt`.

### D3 — Adapt the existing seams, do not rename them

The seams that survive v0.17.0 — `PaneEnumerator` and `PaneCapture` (`drive.rs`) and
`KeyDispatcher` (`keys.rs`) — get blanket implementations over `&dyn TerminalBackend`; their tmux
bodies are deleted. (`PaneResolver`, `PaneInspector` and `PaneCapturer` served the deleted
approver and `approval_send_gate`; if any survives v0.17.0 it is adapted the same way.)
`PaneInfo.pane_current_path` becomes the port's owner. Renaming the seam traits is churn with no
behavior value; `drive-loop-functional-core` revisits them when it restructures the loop.

### D4 — One owner resolution for surfaces in core; two matchers deliberately unchanged

Where the owner matching lives in v0.17.0:

| Site | Rule | After this change |
|---|---|---|
| `drive.rs::resolve_pane_agent` (`:221`) | exact canonical path | core `resolve_agent` |
| `tmux::resolve_pane_id_for_worktree` (`remove`) | exact canonical path | core `resolve_agent` over `list_surfaces` |
| `tmux::resolve_pane_index_for_worktree` (`tmux/session.rs:571`, `restart_from_pause`) | exact canonical path | core `resolve_agent` over `list_surfaces` |
| `tmux::agents_without_live_pane` (`add` reconciliation) | exact canonical path | core `resolve_agent` |
| `inventory::match_pane` | basename suffix (`-<agent_id>`) | unchanged; owners read through the port |
| `sweep.sh` `resolve_agent_for_path` | longest realpath prefix | unchanged; documented difference |
| `@paw_role` liveness filter (`tmux/session.rs:188`), `sweep.sh` `discover_coding_panes` | surface label / title | unchanged (classification, not owner matching) |

`resolve_agent(owner, roster) -> Option<AgentId>` keeps exact canonical-path equality (design call
1): a surface's owner is its worktree root at launch, so equality is the rule every Rust site
already applies, and a path below a worktree root is not a match. The dashboard is not found by
its owner (it shares the repo root with any stray surface there) but by the layout record: the
surface at `SessionLayout`'s dashboard position, converted to a `SurfaceId` inside the tmux
backend; every other unmatched surface is `SurfaceRole::Unknown`. This is the rule
`supervisor-role-typing` and `supervisor-pane-removal` already use.

`inventory::match_pane` keeps its basename-suffix match (design call 2). Its input is a broker
`/status` row, which carries no worktree path, so moving it to the core full-path match would
need the roster as a new input and would change which pane a row resolves to in edge cases — a
behavior change this refactor does not make. `sweep.sh`'s `resolve_agent_for_path` keeps its
longest-realpath-prefix match: it maps arbitrary paths (a pane that `cd`'d into a subdirectory, a
file path) to an agent, not surfaces, so the prefix rule is right for it. The difference is
documented in a comment on that helper, and the parity test covers the surface case only: an
owner equal to a worktree root names the same agent in `sweep.sh` and in Rust.

### D5 — GP-45 is moot

An earlier draft closed GP-45 by having the attended approver resolve surfaces per poll. v0.17.0
deleted that approver, so there is nothing to fix; the drive loop already resolves surfaces by
owner each sweep, and D4 makes that the single rule for surfaces.

### D6 — Persistence: state the obligation, pay nothing

`pane_index` (`RepoAgentEntry`) and `dashboard_pane` (`Session`) keep name, type and bytes. The
`SurfaceId ⇄ pane index` conversion is private to `src/terminal/tmux.rs`. A future backend persists
its own handle in an additive `Option<…>` field (`serde(default, skip_serializing_if)`), never by
repurposing these integers. A round-trip test locks the bytes.

### D7 — Guards are source audits

(a) No `tmux` process construction outside `src/tmux/**` and `src/terminal/tmux.rs`, except the
environment probes in `doctor.rs` and `selftest.rs`, which test the tmux binary itself and are
excepted by name. (b) The port's public identifiers carry no tmux vocabulary. Same pattern as
`no_stdout_macros_under_src_mcp`.

## Risks / Trade-offs

- [The port ossifies around tmux] → the vocabulary guard plus the per-method review question.
- [Argv drift during migration (a lost `-c`, a lost settle delay)] → argv fixtures for fixed plans
  at every wave; e2e suites run unchanged.
- [`drive.rs` test churn] → adapting the existing seams keeps most tests compiling unchanged.
- [Over-abstracting layout] → `layout` takes the existing geometry; no layout language.
- [Frozen surfaces] → no change to `broker/messages.rs`, config serde, broker locks or the SIGHUP path.

## Migration Plan

Four test-gated waves (`code-architecture` "Structural refactors land in test-gated waves"):

1. **Port.** `src/terminal/{mod,tmux,fake}.rs`, argv-fixture tests; no call site changes.
2. **Drive loop.** Seams over the port; owner resolution in core; fake-backend logic tests.
3. **Commands + coordination.** Launch, add/remove, recover, restart, inventory, readiness capture.
4. **Guards + docs.** Source audits, round-trip test, architecture module list.

Rollback: each wave is a plain commit that compiles and passes `just check`.

## Open Questions

- Whether `SessionLayout` should own the `SurfaceId` of the dashboard rather than its index. Not
  needed for tmux; revisit with a second backend.
