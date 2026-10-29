## Why

A supervised session's agent grid is capped at 25 agents in rows of 5 by two constants
(`supervisor/layout.rs`: `SUPERVISOR_MAX_AGENTS = 25`, `SUPERVISOR_AGENTS_PER_ROW = 5`) whose doc
comments, error message ("Configurable max_agents is planned for v1.0.0") and spec text all promise a
configurable layout "later". Operators on wide or very tall terminals cannot use the space; operators on
small screens cannot ask for fewer columns. The v1.0 freeze is the wrong moment to add config keys, so
they land here.

After v0.18 (`supervisor-pane-removal`) the supervised layout is: dashboard as a full-width top row
(pane 0), coding agents below in row-major rows — the 50/50 supervisor/dashboard split is gone, the row
geometry stays. This change makes that geometry's two numbers configurable; it says nothing about
panes for an orchestrator, which no longer exist.

## What Changes

- `[layout] max_agents` (default 25) and `[layout] agents_per_row` (default 5) set the cap and the
  column count of the supervised agent grid; `git paw start --max-agents <N>` overrides the cap for one
  launch.
- Values must be ≥ 1 (a `0` is a config / usage error). The launch-time cap check and its error name
  the effective cap and how to raise it.
- The effective values are recorded in the session state at launch, so `git paw add` / `remove`
  re-tile and check the cap with the values the session was built with, and recovery rebuilds the same
  grid — recovery never refuses a session because the cap changed since.
- Bare sessions use the same grid since v0.18.0's `pane-less-session-parity`, so the cap and column count apply to them too.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `tmux-orchestration`: **Supervisor-mode pane layout** (cap and columns come from the effective grid
  settings); ADDED **Agent grid dimensions are configurable**.
- `supervisor-launch`: **Supervisor auto-start flow** (steps 3–4 use the effective settings).
- `git-add-remove-branch`: **Pane spawn with grid re-tiling** (the add-time cap is the session's).

## Impact

- **Code:** `src/config/layout.rs` (`LayoutConfig { border_affordances, max_agents, agents_per_row }`,
  validation at load), `src/supervisor/layout.rs` (`supervisor_layout(count, &GridSettings)`; constants
  become defaults), `src/tmux/command.rs` (`build_supervisor_session`, `build_add_agent_commands`,
  `build_remove_retile_commands`, `push_supervisor_resize_pass` take the column count),
  `src/tmux/layout.rs` (`agent_row_widths`, `rebalance_agent_rows`), `src/commands/{supervisor,add,remove,recover}.rs`,
  `src/cli.rs` (`start --max-agents`, `add` help text drops "25"), `src/session.rs` (additive
  `grid: Option<GridSettings>` with `#[serde(default, skip_serializing_if)]`).
- **Enum-variant ripple:** none.
- **Docs:** `--help` (`start --max-agents`, `add`), configuration reference `[layout]`, init default
  config (commented keys), `docs/src/architecture.md` layout section, `docs/src/user-guide/supervisor.md`
  layout notes, README if it mentions the 25-agent ceiling.
- **Cross-version dependencies:** written against v0.18 `supervisor-pane-removal`'s versions of
  **Supervisor-mode pane layout** and **Supervisor auto-start flow** (both MODIFIED there too — this
  change must rebase over them). v0.18 `terminal-backend-seam` keeps layout an opaque "apply this
  geometry" operation parameterised by the `SupervisorLayout` row math — the new settings are two more
  parameters of that geometry, not a layout language. The `session-state` additive field overlaps this
  release's **wire DTO split / config schema versioning** (other half): whichever lands second maps the
  field through the session DTO and adds `[layout]` keys to the versioned config schema.

## Rebase note (2026-10-19)

This change uses the pre-v0.21 supervisor vocabulary. If it archives after
`supervisor-vocabulary-split`, the user-facing text it adds (flags, config keys, help, docs, output)
SHALL use the new names (`--coordinated` / `--bare`, `[orchestrator]` / `[workers]` / `[drive]`,
`default_mode`, root identity `orchestrator`). Its deltas are not rewritten: they archive against
today's names, which that change's reading rule maps and v1.0.0's `remove-supervisor-vocabulary`
sweep renames (including the headers *Supervisor-mode pane layout* and *Supervisor auto-start flow*
and the `supervisor-launch` capability). Old terms used here:

- `git paw start --supervisor --max-agents 12` (tmux-orchestration scenario) → `--coordinated`
- `[supervisor].cli` → `[orchestrator] cli`; `agent_approval` → `[workers] approval`;
  `[supervisor] enabled = true` → `default_mode = "coordinated"`; "supervisor config" → the
  coordination tables (supervisor-launch copy)
- "supervised session" / "supervised layout" / "supervised agent grid" → coordinated session;
  "non-supervised" → bare (proposal, both specs, `--max-agents` help text)
- the roster's `supervisor` row → `orchestrator`; session `mode = Supervisor` → coordinated
- the launch line "Supervisor session '…' launched" → the coordinated wording
  `supervisor-vocabulary-split` prints
- `docs/src/user-guide/supervisor.md` → same path, retitled "Coordinated mode" (renamed at 1.0.0)
- Kept (not user-visible): `src/supervisor/layout.rs`, `supervisor_layout`, `SUPERVISOR_*`
  constants, `cmd_supervisor`, `build_supervisor_session`.
