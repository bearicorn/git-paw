## Why

After `supervisor-pane-removal` every session is pane-less: a dashboard (when the broker is on)
plus coding agents. Two launch-path gaps that were narrow while a supervised session was the
"real" mode now cover most sessions:

- **GP-49 — hot-attach refuses bare sessions.** `commands/add.rs` and `commands/remove.rs` return
  `bare_mode_unsupported` for `SessionMode::Bare`, whose `main-horizontal` / `tiled` layout
  (`TmuxSessionBuilder::build`) is not re-tiled incrementally. The released
  `git-add-remove-branch` requirement *Bare-session add and remove refusal states the limitation
  plainly* specifies that refusal ("add/remove require a supervisor-mode session"). An external
  orchestrator therefore cannot open a new wave on a bare session without tearing it down — the
  step it most wants to avoid. This change replaces that requirement.
- **DS-5 — one injecting path is still ungated.** v0.17.0 gates bare `start`
  (`commands/start.rs:406`), spec-driven `start` (`:823`), supervised `start` and `add` through
  `gate_pane_or_fail_on_dialog`, with dialog-stuck tests for the first two
  (`tests/start_spec_launch_parity_integration.rs:903`, `:1000`). The pending-prompt submit in
  `restart_from_pause` (`commands/start.rs` around `:973`) still submits a held boot prompt without
  the gate, so a pane stuck on a first-run dialog can receive it on top of the dialog — the
  GP-22/23/24 class.

Archive position 9 (the pane-less companion of the pane removal).

## What Changes

- **One layout for every pane-less session.** Bare `start`, spec-driven `start` and supervised
  `start` (and recovery of each) build the same agent grid: the dashboard as the full-width top
  row when the broker is enabled, coding agents row-major below, ≤5 per row, equal-width rows;
  with no dashboard the agent rows share the full height. The supervised grid geometry is carried
  over rather than inventing a new one.
- **The session records its layout.** An additive `layout` field (`"grid"`) is written by every
  new session; its absence on a `SessionMode::Bare` session means the earlier tiled layout.
- **Hot-attach on every grid session.** `add` and `remove` work on bare and supervised grid
  sessions alike, using the session's recorded first-agent index. A live session started by an
  earlier binary with the tiled layout is refused with an accurate, version-free message telling
  the user to stop and start it again (recovery rebuilds it as a grid). Supervised sessions from an
  earlier binary keep working (their grid geometry is unchanged; only the offset differs). The
  released bare-session refusal requirement is REMOVED.
- **Readiness gate on the last ungated path.** The pending-prompt submit on restart from pause
  passes the launch-readiness gate first; a pane blocked on an unanswered first-run dialog keeps
  its held prompt for the next restart and is reported on stderr while the rest of the session
  restarts. The gate requirement is re-worded to name every injecting path (bare, spec-driven and
  supervised `start`, `add`, restart from pause); the first four are already gated in v0.17.0.
- The orchestrator and the drive loop discover added or removed agents passively (broker
  `/status` and the discovery roster), with no pane to signal. The drive loop re-reads the agent
  roster every sweep: `drive_loop` fixes `agents` at start today, so after the pane removal's
  coding-only gate an agent added mid-run would never be driven.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `git-add-remove-branch`: add/remove on every grid session; earlier tiled sessions refused
  explicitly; the bare-session refusal requirement removed; re-tiling requirements re-indexed (no
  supervisor pane); passive discovery by the external orchestrator and the drive loop.
- `tmux-orchestration`: every pane-less launch builds the agent grid; the readiness gate names
  every injecting launch path.
- `broker-dashboard`: the dashboard pane layout places agents in the grid, not a tiled area.
- `session-state`: the session records its pane layout additively.
- `supervisor-unattended-operation`: the drive loop follows the live agent roster.

## Impact

- **Predecessors (implementation order):** archive position 9 of 20, after
  `supervisor-role-typing` (1) and `supervisor-pane-removal` (6) (the hard dependencies: layout
  offsets from `SessionLayout`, no supervisor pane), `unattended-heartbeat-progress` (7) and
  `agent-hook-and-tmux-guards` (8), which share no requirement header with this change. v0.17:
  `start-spec-launch-parity` (already gates the bare and spec-driven submits and pins them with
  dialog-stuck tests; this change only re-words the gate requirement to name every path),
  `bundled-helper-discovery` (`provision_worktree` helper provisioning on every path — unchanged
  here), `classifier-retirement` (the drive loop escalates every live permission prompt and answers
  none; an agent added mid-run is driven under that rule).
- **Successors:** `drive-loop-reattach` (10) and `launch-prompt-entry-point` (11) build on the same
  launch paths; `terminal-backend-seam` (12) moves the grid build and the gate behind the terminal
  port.
- **Code:** `src/commands/{start,add,remove,recover,helpers}.rs` (`bare_mode_unsupported`
  replaced by a legacy-layout refusal; the gate call on `restart_from_pause`'s pending-prompt
  submit), `src/tmux/command.rs` (`TmuxSessionBuilder` bare build uses the grid builder;
  `build_add_agent_commands` / `build_remove_retile_commands` take the first-agent index from the
  layout record; a no-dashboard grid variant), `src/tmux/layout.rs` (`rebalance_agent_rows`
  offset), `src/session.rs` (`layout` field), `src/main.rs` (`gate_pane_or_fail_on_dialog` reused).
- **Persisted state:** additive `Session.layout: Option<…>` (`serde(default, skip_serializing_if)`);
  sessions saved earlier load unchanged.
- **CLI:** no new flag; the add/remove refusal on bare sessions is replaced by a legacy-tiled
  refusal (no version number, as today).
- **Tests:** `tests/add_remove_e2e.rs` (bare add/remove, legacy tiled refusal); the
  `bare_mode_unsupported` unit tests in `commands/helpers.rs` that pin the released bare-session
  refusal are replaced by legacy-tiled-refusal tests; start e2e for the bare/spec grid geometry,
  `pause_e2e.rs` (pending prompt gated, held prompt kept for a dialog-stuck pane), the existing
  `start_spec_launch_parity_integration.rs` dialog-stuck tests mapped to the re-worded gate
  scenarios, layout unit tests (no-dashboard grid).
- **Docs:** `docs/src/user-guide/session-lifecycle.md` (add/remove on every session),
  `docs/src/architecture.md` (one layout), `user-guide/dashboard.md`, `cli-reference.md`
  (`add` / `remove` no longer "supervisor-mode only"), README CLI table.
- **Enum-variant ripple:** none.
- **Security / safety:** none new; the gate refuses to inject on top of an unanswered dialog on
  more paths, which only narrows what reaches a pane.
- **Dependencies:** none.

## Rebase note (2026-10-19)

v0.17.0 dropped `approval-pane-map-refresh` (superseded by `classifier-retirement`, which deletes the dashboard auto-approve poll loop it targeted), so no archived roster-refresh requirement exists to align with. The drive loop's per-sweep roster re-read in this change stands on its own. Under `classifier-retirement` the loop escalates every live permission prompt and answers none, so an agent added mid-run has its prompts escalated, never approved; the scenarios here say so.

## Rebase note (2026-10-26): audit against the v0.18.0 integration branch

Re-checked every code premise against the branch this change is built on, which already carries
`supervisor-pane-removal`, `unattended-heartbeat-progress` and `agent-hook-and-tmux-guards`. Four
premises had moved:

- **The incremental builders are already parameterised.** `build_add_agent_commands`,
  `build_remove_retile_commands` and `rebalance_agent_rows` take `first_agent` from `SessionLayout`
  as of `supervisor-pane-removal`, and `SessionLayout::first_agent_index` already derives it from
  the recorded `dashboard_pane` (so a pre-relocation supervised session resolves without new code).
  §1.1 shrinks to the "has top row" geometry input and the no-dashboard variant.
- **The bare launch paths share one builder.** `cmd_start`, `launch_spec_session` and
  `recover_bare_session` all build through `TmuxSessionBuilder`, so §1.2 and §1.3 are one change —
  making `TmuxSessionBuilder::build` emit the grid — rather than three call-site rewrites.
- **`drive_loop` moved** to `src/supervisor/drive.rs` and takes its roster as a borrowed
  `&[AgentPane]` fixed for the run, with a `coding_ids` list derived once at loop start; the
  per-sweep re-read needs an injected `DriveDeps` port, because `drive_loop`'s unit-test harness
  runs against an empty tempdir where a direct session read would always fail (and so would
  silently exercise only the fallback branch).
- **Every line number in §5 was stale.** `restart_from_pause`'s held-prompt submit is at
  `start.rs:1005-1014`, not `~:973`; the already-gated bare and spec-driven submits are at `:430`
  and `:861`, not `:406` and `:823`. Two substantive consequences, now in §5.1: the submit loop
  `.take()`s the pending prompt before saving the session, so the gate must run *before* the take
  for a stuck pane to keep its prompt; and it addresses panes by `offset + idx` while the
  discovery-file rewrite immediately below deliberately re-resolves live indices, because a resume
  re-splits the dashboard wherever tmux puts it.

The change's shape, decisions and spec deltas are unaffected.

## Rebase note (2026-10-25): audit against released v0.17.0

- DS-5 shrank to one path: v0.17.0 already gates bare `start` (`commands/start.rs:406`) and spec-driven `start` (`:823`) and tests both (`tests/start_spec_launch_parity_integration.rs:903`, `:1000`). Only `restart_from_pause`'s pending-prompt submit (around `start.rs:973`) is ungated; §5 tasks, the Why, the design and Impact now say so. The MODIFIED gate requirement still names every path, and its bare / spec-driven scenarios map to the existing tests.
- Added the missing `## REMOVED Requirements` entry for released `git-add-remove-branch` *Bare-session add and remove refusal states the limitation plainly* (`:581`), which the ADDED *Hot-attach works on every grid session* contradicts; without it the archive would keep both.
- Dropped the stale claims that the refusal still cites "v0.6.0" (the released text already has no version) and that the refusal is unspecified.
- Predecessors updated to the v0.18 archive order (position 9, after `supervisor-pane-removal`, `unattended-heartbeat-progress` and `agent-hook-and-tmux-guards`).
