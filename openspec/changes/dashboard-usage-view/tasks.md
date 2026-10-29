## 0. Preconditions

- [ ] 0.1 Confirm that every MODIFIED header in this change exists in `openspec/specs/`, and rebase this change's text onto the archived wording of each one. The headers come from:
  - v0.18 `broker-host-separation`, `orchestrator-read-surface`, `pane-less-session-parity` and `supervisor-pane-removal`;
  - v0.21 `configurable-agent-grid` (*Supervisor-mode pane layout*, *Supervisor auto-start flow*) and `terminal-output-robustness` (*Dashboard renders a notice below its minimum size*).
- [ ] 0.2 Confirm these have landed: v0.19 `worker-usage-budget` (usage facet, `status --json` usage, caps); v0.21 `supervisor-vocabulary-split` (config version 2, `[drive]` caps); v0.21 `session-state-locking` (`session::store`).

## 1. Seam (behaviour-preserving)

- [ ] 1.1 Split the view in `src/dashboard/view.rs`: add a `DashboardView` value and a pure `view_model(&Model) -> DashboardView` that still holds today's roster content, and make `render` only lay out a `DashboardView`. Move the clock read into `Msg::Tick { now }`. The full suite passes unchanged, which makes this a checkpoint commit.
- [ ] 1.2 Add `Session::has_dashboard()`. It returns true when `dashboard_pane` is recorded, or when there is no layout record and a broker was recorded. Rewrite `Session::agent_pane_offset` as `dashboard_pane + 1` when the session has a dashboard, else `0`, and route restart, recovery, `add`/`remove`, status, liveness, the drive loop's sweep and the discovery roster `sweep.sh approve` reads through it. Every existing session shape keeps its current offset; prove it with the existing tests plus a table over the pre-v0.4, pre-v0.18, pre-relocation and v0.18 shapes.

## 2. Session records

- [ ] 2.1 Add an additive `Session` list of orchestrator invocations `{ cli, cli_session_id: Option }` (`serde(default, skip_serializing_if = "Vec::is_empty")`). Map it through the session wire DTO if `wire-format-dtos` has landed.
- [ ] 2.2 In `src/supervisor/drive.rs`, append each headless invocation at spawn through `session::store`, without holding the lock across the spawn.
- [ ] 2.3 Tests: "An invocation is recorded at spawn" (fake runner), "Earlier session files load unchanged", "An invocation without a session id counts as unknown".

## 3. Config and flag

- [ ] 3.1 `src/config/dashboard.rs`: add `enabled: Option<bool>` (absent = false in version 2). Keep `show_message_log` accepted and inert. Add the commented `# enabled = true` example to the init default config.
- [ ] 3.2 Extend the v1→v2 migration step with the implication rule (`[broker] enabled = true`, or a `[dashboard]` table present, and no explicit `enabled`). Keep it pure, and keep the vocabulary split's row-by-row equality tests green.
- [ ] 3.3 When git-paw stamps or rewrites a version-1 file whose migration implied the dashboard, write `[dashboard] enabled = true`. This covers `init`'s `version = 2` backfill and `add-cli` / `remove-cli`.
- [ ] 3.4 `src/cli.rs`: add `start --dashboard` (`dashboard: bool`) with help text that names the usage-and-events view and the broker requirement.
- [ ] 3.5 Tests:
  - `core-configuration`: every scenario of both requirements, including stamping through `init`.
  - `cli-parsing`: the three `--dashboard` scenarios.

## 4. Launch, recovery and restart

- [ ] 4.1 Resolve "launch has a dashboard" once per launch in `commands/start.rs` (bare and spec) and `commands/supervisor.rs`: `--dashboard`, else config, else false, and only with the broker. `--dashboard` without a broker fails before any worktree is created. Create the dashboard pane and record `dashboard_pane` only when the launch has one.
- [ ] 4.2 Key the grid builders' "has top row" input on the launch's dashboard decision, not on `broker.enabled`. With no top row, agents start at pane 0 and the rows use `100 / agent_rows`. The builders are `TmuxSessionBuilder` / `build_supervisor_session`, routed through the terminal port after v0.18.
- [ ] 4.3 Recovery (`commands/recover.rs`) and `restart_from_pause` (`commands/start.rs`) recreate the dashboard pane only when `has_dashboard()`, and never from the current flag or config.
- [ ] 4.4 Integration tests, real tmux on the private test socket:
  - every `broker-runtime` scenario of *Start flow conditionally creates dashboard pane*;
  - *Broker host lifecycle* ("Pane indices are unaffected", "A session without a dashboard still runs the host");
  - *Start flow restarts a paused session* ("A paused session without a dashboard does not gain one");
  - the `tmux-orchestration` scenarios (dynamic pane count, the layout without a dashboard, the pane-less grid);
  - the `supervisor-launch` *Supervisor auto-start flow* scenarios;
  - the `session-state` recovery scenarios;
  - `broker-dashboard` *Dashboard pane layout*.

## 5. Usage query and status

- [ ] 5.1 Add a pure `usage` aggregation module: per-tier subtotals, known totals with unknown counts, orchestrator totals, and cap progress (percent, exceeded at ≥ 100%, cost cap not evaluable without a cost source). Make the v0.19 `status --json` code and the drive-loop exit summary use it.
- [ ] 5.2 `git paw status --json`: add `orchestrator_usage` (`null` when there are no invocations). `usage_total` stays workers only. Tests: "Recorded invocations are summed", "No invocations yields null".
- [ ] 5.3 `git paw status`: when the broker answers, join `/status` with the live pane map (`coordination::inventory`, bounded timeout). Append the phase or status, the age (`format_age`) and the pane to each worktree line, and add a `worktrees[].agent` object to the JSON. Tests: "A live broker adds the agent state", "JSON carries the agent object", "An unreachable broker leaves the lines unchanged".

## 6. Dashboard content

- [ ] 6.1 `Model` / `Msg`:
  - add `Msg::Usage { snapshot, read_at }` and `Msg::Status { reachable, wave }`; `Msg::Snapshot` and the roster fields go;
  - add the usage reader thread (`UsageReader`, 10 s cadence, channel drained on each wake-up);
  - change `run_dashboard(client, usage, shutdown)`.
- [ ] 6.2 Usage panel in `view_model` / `render`: worker rows (agent, family, tier, in, out, cost or `—`, `unknown` + reason), tier subtotals, the orchestrator row, the known-usage total with the unknown count, cap lines with a text bar and an exceeded marker (text attribute), the "excludes your own session" note, figure age, and a loading state.
- [ ] 6.3 Delete the roster: `AgentRow`, `format_agent_rows`, `arrange_with_supervisor_pinned`, `status_symbol`, `format_status_line`, the `SUPERVISOR_AGENT_ID` pinning and their tests. Keep `format_age`. Remove their re-exports from `src/dashboard.rs`.
- [ ] 6.4 Focus: `Focus { Usage, Log }`, starting on `Log`. `Tab` toggles it. Usage navigation (`Up/k`, `Down/j`, `PageUp/PageDown`, `Home/End`) uses `TableState`, with the orchestrator and total rows pinned below the worker rows. `l` hiding the log moves focus to usage. Replace `tab_key_ignored_no_buffer` with focus tests, because the *No prompt-inbox panel* scenario "Tab key is not handled" is removed.
- [ ] 6.5 `broker_log::collapse_status_runs` becomes per agent: an agent's run survives other agents' messages, and the merged row sits at its latest heartbeat. Share the heartbeat key with v0.22's `src/broker/summary.rs` if that change has landed.
- [ ] 6.6 Layout: the usage panel replaces the agent table in `build_layout_constraints` (positive minimum, absorbs the spare rows). The minimum-size computation uses the usage panel's widths. The degraded state keeps refreshing usage.
- [ ] 6.7 Tests:
  - unit tests on `view_model` / `update` for every `broker-dashboard` ADDED scenario (usage, pure view state, focus) and the per-agent collapse scenarios;
  - `TestBackend` tests for *Dashboard title*, *No prompt-inbox panel*, *Broker log panel exists in the dashboard*, *Panel toggle hotkey*, *Quit keybind*, *Dashboard renders a notice below its minimum size* and *Dashboard degrades when the broker is unreachable*;
  - stub-broker integration tests for *Periodic state polling* ("The log reads only new entries" asserts `since=12`; "A slow usage read does not block input" uses a blocking fake reader) and *Dashboard entry point*;
  - `NO_COLOR` assertions for the selection and exceeded marks.

## 7. Docs

- [ ] 7.1 `docs/src/user-guide/dashboard.md`. Rewrite it to say:
  - the dashboard is opt-in (`[dashboard] enabled`, `--dashboard`, needs the broker; off by default; version-1 configs keep it);
  - what the usage panel shows, including unknown versus zero, cost only when the CLI reports it, caps enforced only by the drive loop, and the operator's own session excluded;
  - the broker log, with per-agent collapse;
  - the keys (`Tab`, navigation, `l`, `q`);
  - "for agent state, run `git paw status`".
  
  The chapter scenarios ("Dashboard chapter …") get a content test.
- [ ] 7.2 `docs/src/configuration/README.md`: `[dashboard] enabled`, the corrected section intro (no "pane 1 beside the supervisor"), and the version-1 implication note next to the migration table. `docs/src/cli-reference.md`: `start --dashboard`, the `status` agent columns, `worktrees[].agent` and `orchestrator_usage`.
- [ ] 7.3 `docs/src/architecture.md` and `docs/src/user-guide/session-lifecycle.md`: the layout with and without a dashboard, and recovery and pause keeping the recorded dashboard. `docs/src/user-guide/tiered-model-workflow.md`: per-tier usage in the dashboard.
- [ ] 7.4 README: update the dashboard feature line (or the landing-page capability bullet if `readme-landing-page` landed first) to "opt-in usage and event view". `--help` comes from task 3.4.
- [ ] 7.5 At archive, rewrite `## Purpose` in `openspec/specs/broker-dashboard/spec.md` to describe the opt-in usage-and-events view (it still describes the agent-status table), and refresh its entry on the specifications page.

## 8. Traceability and gates

- [ ] 8.1 Map every scenario in the seven delta specs to a named test from groups 2–7, and list the map in the PR.
- [ ] 8.2 Grep `src/` and `tests/` for `format_agent_rows`, `status_symbol`, `format_status_line` and `AgentRow`, and confirm that no references remain outside removed code.
- [ ] 8.3 Run `cargo fmt`, then `just lint`.
- [ ] 8.4 Run `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast` and check the real exit code.
- [ ] 8.5 Run `just deny`.
- [ ] 8.6 Run `mdbook build docs/`.
- [ ] 8.7 Run `openspec validate dashboard-usage-view --strict`.
