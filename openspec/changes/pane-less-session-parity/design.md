## Context

`supervisor-pane-removal` leaves two pane-less shapes: supervised sessions (dashboard row + the
row-major agent grid built by `tmux::build_supervisor_session`, re-tiled incrementally by
`build_add_agent_commands` / `build_remove_retile_commands` / `rebalance_agent_rows`) and bare
sessions (`TmuxSessionBuilder::build`: `select-layout tiled` before each split, then
`main-horizontal` with a dashboard or `tiled` without). Both now put the dashboard at pane 0 and
agents from 1, so the only thing that stops `add`/`remove` on a bare session is the tiling, which
the incremental builders cannot extend (`bare_mode_unsupported` in `commands/helpers.rs`, specified
by the released *Bare-session add and remove refusal states the limitation plainly*).

The readiness gate (`tmux::gate_pane_for_injection`, wrapped by `main.rs`'s
`gate_pane_or_fail_on_dialog`) runs in `cmd_supervisor`, `cmd_add` (non-paused), `cmd_start`
before its boot-block pre-fill (`commands/start.rs:406`) and `launch_spec_session` before its
submit (`:823`); v0.17.0 `start-spec-launch-parity` added the last two with dialog-stuck tests
(`tests/start_spec_launch_parity_integration.rs:903`, `:1000`). The one injecting path left
ungated is `restart_from_pause`, which submits held boot prompts through `submit_prompt_to_pane`
(around `start.rs:973`).

## Goals / Non-Goals

**Goals:**
- One grid geometry for every pane-less launch and recovery.
- `add`/`remove` on every grid session; an explicit, accurate refusal for an earlier tiled session.
- The readiness gate before every prompt injection (only restart-from-pause's held-prompt submit
  is new).

**Non-Goals:**
- Changing the dashboard's share of the screen (the dashboard-survival question).
- Re-tiling a live tiled session in place (see D3).
- The terminal port (`terminal-backend-seam`) or the broker host split.

## Decisions

### D1 — Carry the supervised grid to every session

The grid is the layout that already supports incremental re-tiling and predictable row-major
indices; the bare `main-horizontal` / `tiled` layouts support neither. *Alternative rejected:* retire
hot-attach explicitly — it removes the step the external orchestrator most needs between waves.
*Alternative rejected:* teach the tiled layout to re-tile incrementally — `select-layout tiled`
reorders geometry unpredictably, which is why the supervised layout avoided it.

Visible consequence: a bare session's dashboard row becomes the grid's 50% top row instead of
tmux's `main-horizontal` main-pane height. Recorded as a decision for confirmation.

### D2 — No-dashboard grid

With the broker disabled there is no top row; agent rows share the full height (`100 / agent_rows`)
and agents start at pane 0. The geometry function takes "has top row" as an input.

### D3 — Record the layout; refuse an earlier tiled session

`Session.layout` (additive, `Some(Grid)` for every new session) distinguishes a grid session from a
bare session saved earlier (`None` + `SessionMode::Bare` = tiled). `add`/`remove` on such a live
session fail before mutating anything, with a message that names the tiled layout and says to stop
the session and `git paw start` again; recovery of a stopped tiled session rebuilds it as a grid and
records `layout`. A supervised session saved earlier (`None` + `SessionMode::Supervisor`) is a grid
already and stays hot-attachable with its recorded offset. *Alternative rejected:* re-tile a live
tiled session into a grid on the first `add` — it would move every running pane under the user.

### D4 — Offsets come from the layout record

The incremental builders take the first-agent index from `SessionLayout` instead of the supervised
constant, so the same code serves a grid with or without a dashboard and a pre-relocation supervised
session (agents from 2).

### D5 — Gate the last ungated injection; keep each path's submit semantics

The gate decides when a pane may receive text; it does not decide whether to submit. Bare `start`
keeps pre-filling without `Enter` (the human supplies the task); spec-driven and supervised paths
submit; the pending-prompt path on restart submits. Those launch paths already fail on a
`DialogStuck` outcome with the existing actionable error; this change adds the gate to
restart-from-pause only, where it runs per pending pane and a stuck pane keeps its pending prompt
for the next restart rather than losing it (the rest of the session still restarts). The gate
requirement is re-worded to list every path so the coverage is stated, not implied.

## Risks / Trade-offs

- [Bare users notice the new geometry] → documented in the session-lifecycle and dashboard chapters
  and the changelog; the agent grid is what supervised users already have.
- [A gated restart is slower] → the gate polls with the same bounded budget and falls back to
  injecting for unrecognised CLIs, so it is never worse than the previous fixed sleep; it runs only
  for panes holding a pending prompt.
- [Legacy tiled sessions] → explicit refusal, one restart away from working.

## Migration Plan

1. Geometry: "has top row" input; no-dashboard grid; builders parameterised by first-agent index.
2. Bare and spec-driven builds use the grid; recovery rebuilds bare sessions as grids; write
   `layout`.
3. `add`/`remove`: replace `bare_mode_unsupported` with the legacy-tiled refusal (the released
   bare-session refusal requirement is REMOVED).
4. Gate restart-from-pause's pending-prompt submit.
5. Tests, docs.

Rollback: revert; the additive field is ignored by older binaries, which would treat a grid bare
session as tiled (they refuse add/remove on it, as before).

## Open Questions

- Should the bare dashboard row keep a smaller share than 50%? Recommendation: no change here; the
  share belongs to the dashboard-survival decision downstream of `broker-host-separation`.
