## Why

The v0.18 open question "does the dashboard survive?" is answered (user decision, 2026-10-19). With
the orchestrator outside the session, the dashboard's agent roster repeats what `git paw status`
is for. The live view worth keeping is **usage**: tokens and cost per worker, per tier and for the
drive loop's headless orchestrator one-shots, measured against the v0.19 budget caps, next to the
live broker event log. That view is useful only to some operators, so the dashboard becomes
opt-in, and a session without it gives the whole window to the agent grid.

This replaces the gated `dashboard-tabbed-roster` draft. Two parts of it are kept: scrollable
panels with `Tab` focus, and per-agent heartbeat collapse in the log. Tabs are dropped because
once the roster is gone only two panels are left.

## What Changes

- **Opt-in dashboard.** A new session gets a dashboard pane only with `git paw start --dashboard`
  or `[dashboard] enabled = true`, and only when the session runs the broker. `--dashboard` with
  no broker is an error. Without a dashboard there is no top row: the agent grid starts at pane 0
  and fills the window.
- **Back-compat.**
  - A version-1 config that sets `[broker] enabled = true` or has a `[dashboard]` table, which
    means it produced a dashboard before this change, migrates with `[dashboard] enabled = true`.
    When git-paw stamps or rewrites such a file as version 2, it writes that key explicitly.
  - A live, paused or stopped session keeps the dashboard pane it was started with. Restart and
    recovery follow the session record, not the current config.
- **New content.**
  - A **usage panel**: one row per worker with CLI, tier, tokens in/out, cost when the CLI
    reports one, or `unknown` with the reason; a subtotal per tier; an orchestrator-invocations
    row; and a known-usage total with progress against `budget_max_tokens` / `budget_max_cost`.
    The figures come from the same query as `status --json`. There is no price table.
  - The **broker log**, read through the broker's event cursor. Heartbeats collapse per agent.
- **BREAKING (view):** the agent roster (status table, symbols, counters, pinned root row) is
  removed from the dashboard.
- **Roster moves to `git paw status`.** Today `git paw status` lists only branch, CLI and path.
  Each worktree line now also shows the agent's broker status (or phase), its age and its pane.
  `--json` gains an `agent` object for each worktree.
- **Pure view model:** a pure function from the dashboard model (usage snapshot, caps, events,
  filter, focus) to the rendered view state, testable without a terminal.
- **Orchestrator invocations are recorded.** The drive loop records each headless one-shot's CLI
  and session id in the session file, so the dashboard and `status --json`
  (`orchestrator_usage`) can attribute them.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `broker-dashboard`:
  - ADDED: usage panel, pure view state, `Tab` focus and usage scrolling.
  - MODIFIED: entry point, polling, quit keybind, title, pane layout, no-inbox panel, log panel,
    toggle hotkey, chapter, heartbeat dedup (per agent), degraded state, minimum size.
  - REMOVED: agent status table, agent row formatting, status symbols, committed counter, legacy
    message panel.
- `broker-runtime`: MODIFIED start-flow dashboard pane (opt-in), the thin-client subcommand, the
  host lifecycle, and restart from pause.
- `session-state`:
  - MODIFIED: dashboard pane record ("has a dashboard"), recovery data, recovery pane count.
  - ADDED: recorded orchestrator invocations, `orchestrator_usage`, agent state in `status`.
- `tmux-orchestration`: MODIFIED dynamic pane count, *Supervisor-mode pane layout*, and *Every
  pane-less session uses the agent grid* (no top row without a dashboard).
- `supervisor-launch`: MODIFIED *Supervisor auto-start flow* (dashboard pane conditional).
- `cli-parsing`: ADDED `start --dashboard`.
- `core-configuration`: ADDED `[dashboard] enabled` and the version-1 implication.

## Impact

- **Code:**
  - `src/dashboard.rs` (`Model`, `Msg`, `update`; a usage reader thread; focus)
  - `src/dashboard/view.rs` (the roster table is replaced by the usage panel and the view-model
    function; `AgentRow`, `format_agent_rows`, `status_symbol`, `format_status_line` and
    `arrange_with_supervisor_pinned` are deleted)
  - `src/dashboard/broker_log.rs` (`collapse_status_runs` per agent)
  - `src/config/dashboard.rs` (`enabled`), the config v1→v2 migration step, `src/init.rs`
    (stamp)
  - `src/cli.rs` (`--dashboard`)
  - `src/commands/{start,supervisor,recover,status}.rs`, `src/session.rs` (`agent_pane_offset`,
    `dashboard_pane` rule, invocation list)
  - `src/supervisor/drive.rs` (record invocations), and the tmux / terminal-port builders (grid
    without a top row)
- **Enum-variant ripple:** none. `BrokerMessage` and `SpecBackendKind` are untouched. `Msg` is
  internal to the dashboard.
- **Docs:** `docs/src/user-guide/dashboard.md` (rewritten), the configuration reference
  (`[dashboard] enabled`, the migration note), `cli-reference.md` (`start --dashboard`,
  `status`), `architecture.md` and `session-lifecycle.md` (layout without a dashboard), and the
  README feature line.
- **Cross-version dependencies:**
  - v0.17 `classifier-retirement`: *Start flow conditionally creates dashboard pane* keeps its
    clause that the dashboard sends no keystrokes to any other pane; the dashboard auto-approve
    thread no longer exists, so the thin client lists none; the pane-offset consumers are the
    drive loop's sweep and `sweep.sh approve`, not auto-approvers.
  - v0.18 `broker-host-separation`: thin client, host lifecycle, restart. Five of its
    requirements are modified again here.
  - v0.18 `orchestrator-read-surface`: event cursor and wave indicator.
  - v0.18 `pane-less-session-parity`: *Dashboard pane layout*, *Every pane-less session uses the
    agent grid*, `Session.layout`.
  - v0.18 `supervisor-pane-removal`: *Start flow conditionally creates dashboard pane*, *No
    prompt-inbox panel*, the chapter, *Dashboard pane index persisted*, *Recovery rebuilds
    exactly*.
  - v0.18 `orchestrator-escalation-handoff`: one-shots.
  - v0.19 `worker-usage-budget`: usage facet, `status --json` usage, caps.
  - v0.21 `supervisor-vocabulary-split`: version 2 and its migration step, `[drive]` caps,
    `--coordinated`.
  - v0.21 `configurable-agent-grid`: *Supervisor-mode pane layout* and *Supervisor auto-start
    flow* are written on its text.
  - v0.21 `terminal-output-robustness`: the minimum-size requirement is written on its text, and
    its `NO_COLOR` rule applies.
  - v0.21 `session-state-locking`: the invocation write.
  - v0.21 `uniform-json-output`: additive status fields.
  - Every MODIFIED header listed above must exist in `openspec/specs/` before this change
    archives, so this change lands after all of them.
- **Downstream:** v1.0 `layer-isolation` was edited to treat the dashboard as an optional,
  off-by-default view of the Broker layer. v0.22 `replay-semantic-timeline` uses the same
  per-agent collapse rule. v0.24 `upgrade-compat-matrix` should add a v0.21 session that has a
  broker but no dashboard.
- **Dependencies:** none.
