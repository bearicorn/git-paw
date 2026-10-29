## Context

`supervisor_layout(agent_count)` (`src/supervisor/layout.rs`) rejects counts above
`SUPERVISOR_MAX_AGENTS` with `PawError::ConfigError` and computes
`agent_rows = ceil(count / SUPERVISOR_AGENTS_PER_ROW)`; `layout_for` is the alias used by `add`,
`remove` and `recover`. The column constant is also read by `tmux/command.rs` (row splits, resize
passes) and `tmux/layout.rs` (equal-width rebalance). `[layout]` already exists with one field,
`border_affordances`; the overlay merges `[layout]` as a whole table. Today bare sessions use
`select-layout tiled`; from v0.18.0 (`pane-less-session-parity`) every session, bare included, uses the
agent grid, so these settings apply to all of them. After v0.18 the dashboard row stays 50% and the
agent rows share the other 50% (from v0.21's `dashboard-usage-view` the dashboard is opt-in, and with
it off the agent rows fill the window); the headless canvas is `480x140`.

## Goals / Non-Goals

**Goals:** two config keys + one launch flag; a session keeps the grid it was built with.

**Non-Goals:** changing the dashboard's 50% share (`dashboard-usage-view` keeps it when the dashboard
is on); a general layout language; per-row heights.

## Decisions

- **D1 — `GridSettings { max_agents, agents_per_row }` resolved once per launch.** Precedence:
  `--max-agents` > `[layout] max_agents` > 25; `[layout] agents_per_row` > 5. Passed into
  `supervisor_layout` and the tmux builders; the constants become `DEFAULT_*`. A pure function, unit
  tested with the existing layout table extended to non-default values.
- **D2 — Persist the effective settings on the session (additive).** `add`/`remove` re-tile against
  the existing grid; recomputing with a since-edited config would reshuffle rows mid-session. Absent
  field (older sessions) → defaults, which is exactly what built them. Alternative — a `--max-agents`
  flag on `add` too — rejected: the operator would have to remember the launch value.
- **D3 — Recovery ignores the cap.** It rebuilds what was running; refusing to recover a 30-agent
  session because the config now says 25 would strand work.
- **D4 — Validation: ≥ 1, no upper bound.** `0` is a `ConfigError` at load / a clap range error.
  No arbitrary ceiling is invented; if the terminal cannot fit the grid, tmux refuses the split and
  the launch fails with an error naming the effective `max_agents` / `agents_per_row` (the existing
  failed-launch path tears the half-built session down).
- **D5 — `[layout]` stays a whole-table overlay.** Matches the existing merge rule; documented that a
  repo `[layout]` replaces the global one (unchanged behaviour for `border_affordances`).

## Risks / Trade-offs

- [Very wide rows produce unusably narrow panes] → operator's explicit choice; the docs give the
  canvas arithmetic (`window width / agents_per_row`).
- [Struct-literal ripple for `Session`/`LayoutConfig` on concurrent branches] → `..Default::default()`
  in test literals (concept-decoupling §9).
