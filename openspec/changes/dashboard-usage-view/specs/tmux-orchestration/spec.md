## MODIFIED Requirements

### Requirement: Dynamic pane count matches input

The number of panes in the session's window 0 SHALL match the number of `PaneSpec` entries added via the builder. When the launch has a dashboard (per `broker-runtime`'s "Start flow conditionally creates dashboard pane"), the builder SHALL receive an additional `PaneSpec` for the dashboard in pane 0, increasing the total pane count by one. A launch with the broker but without a dashboard SHALL add no such pane. A detached service window (the broker host's `broker` window) is not part of this count.

#### Scenario: Two agent panes plus dashboard created
- **GIVEN** a launch with a dashboard and 2 agent pane specs
- **WHEN** the session is built
- **THEN** exactly 3 panes SHALL exist in window 0: pane 0 (dashboard) + panes 1-2 (agents)

#### Scenario: Two panes with the broker but no dashboard
- **GIVEN** the broker enabled, no dashboard enabled, and 2 agent pane specs
- **WHEN** the session is built
- **THEN** exactly 2 panes SHALL exist in window 0: panes 0-1 (agents)

#### Scenario: Two panes without broker
- **GIVEN** broker is disabled and 2 pane specs are added
- **WHEN** the session is built
- **THEN** exactly 2 panes SHALL exist in window 0 (same as v0.2.0)

### Requirement: Supervisor-mode pane layout

When the tmux session is built for a coordinated session (per the `supervisor-launch` capability), the system SHALL produce a layout with the structural properties below. `agents_per_row` and `max_agents` are the session's effective grid settings (see "Agent grid dimensions are configurable"; defaults 5 and 25).

- **Top row**: when the session has a dashboard, pane 0 (dashboard) spans the full window width. When it has none, there is no top row.
- **Agent grid**: dynamically sized by agent count, with up to `agents_per_row` columns per row. The agent grid is a sequence of horizontal rows; each row holds up to `agents_per_row` agent panes side-by-side.
- **Pane indices**: with a dashboard, pane 0 = dashboard and panes 1..N = coding agents in row-major order (left-to-right, top-to-bottom). Without one, panes 0..N-1 = coding agents in the same order. No pane runs an orchestrator CLI.
- **Vertical proportions**: with a dashboard, the dashboard row SHALL occupy 50% of the session height regardless of agent count. The agent rows SHALL share the remaining 50% evenly, each occupying `50 / agent_rows` percent, where `agent_rows = ceil(N / agents_per_row)`. Adding agents SHALL shrink the agent rows, never the dashboard row. Without a dashboard, the agent rows SHALL share the full height evenly, each occupying `100 / agent_rows` percent. With the defaults and a dashboard:

  | Agent rows | Dashboard row height | Each agent row height |
  |---|---|---|
  | 1 (1-5 agents) | 50% | 50% |
  | 2 (6-10 agents) | 50% | 25% each |
  | 3 (11-15 agents) | 50% | 16.7% each |
  | 4 (16-20 agents) | 50% | 12.5% each |
  | 5 (21-25 agents) | 50% | 10% each |

- **Equal-width agent panes within a row**: every agent pane in the same agent row SHALL be rendered at equal width (within a one-column rounding tolerance). Agents are added to a row by successive `tmux split-window -h`, and each `-h` split halves the *current* pane, so a row built from raw splits renders unequal widths. The system SHALL therefore rebalance each agent row to equal width after its panes are created, either by applying `tmux select-layout even-horizontal` scoped to that row's agent panes, or by issuing `tmux resize-pane` to set each agent pane to `100 / agents_in_row` percent of the row's width. The rebalance SHALL NOT alter the dashboard row's full width nor the vertical height proportions described above.

- **Row width bound**: the system SHALL NOT produce agent rows with more than `agents_per_row` panes; the equal-width target per pane is `100 / agents_per_row` percent of the window width at a full row.

- **Agent cap**: at most `max_agents` agents per session. Above it, the system SHALL reject the launch before any tmux command runs with an error stating the requested count, the effective maximum, and how to proceed (`--branches <subset>` to split, or `--max-agents` / `[layout] max_agents` to raise the cap).

The layout SHALL be built using `tmux split-window -h` and `-v` with explicit percentages, then enforced via `tmux resize-pane -y <pct>` for the height proportions and the per-row equal-width rebalance described above. `select-layout tiled`, or any other whole-window auto-layout, SHALL NOT be used for the coordinated layout, because those layouts do not preserve the predictable pane-index ordering this layout relies on. `select-layout even-horizontal` MAY be used only when scoped so it rebalances a single agent row's widths without reordering pane indices or disturbing the dashboard row. No `swap-pane` SHALL be emitted.

#### Scenario: 5-agent coordinated layout has 1 agent row

- **GIVEN** a coordinated session with a dashboard, 5 agent branches and default grid settings
- **WHEN** the tmux layout is built
- **THEN** pane 0 SHALL be the dashboard spanning the full window width
- **AND** panes 1-5 SHALL be agents arranged in a single row below the dashboard
- **AND** the dashboard row's height SHALL be 50% and the agent row's height SHALL be 50%

#### Scenario: 10-agent coordinated layout has 2 agent rows

- **GIVEN** a coordinated session with a dashboard, 10 agent branches and default grid settings
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 3 (1 dashboard + 2 agent rows)
- **AND** the dashboard row's height SHALL be 50%
- **AND** each agent row's height SHALL be 25%
- **AND** the first agent row SHALL contain panes 1-5, the second agent row SHALL contain panes 6-10

#### Scenario: 20-agent coordinated layout has 4 agent rows

- **GIVEN** a coordinated session with a dashboard, 20 agent branches and default grid settings
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 5 (1 dashboard + 4 agent rows)
- **AND** the dashboard row's height SHALL be 50%
- **AND** each of the 4 agent rows' height SHALL be 12.5%

#### Scenario: Without a dashboard the agent rows fill the window

- **GIVEN** a coordinated session without a dashboard, 7 agent branches and default grid settings
- **WHEN** the tmux layout is built
- **THEN** panes 0-4 SHALL form the first agent row and panes 5-6 the second
- **AND** each agent row's height SHALL be 50% of the window, with no dashboard row

#### Scenario: Agents beyond the cap are rejected

- **GIVEN** default grid settings and 26 agent branches resolved
- **WHEN** the coordinated launch flow runs
- **THEN** the launch SHALL be rejected with a `PawError` stating the requested count (26), the maximum (25), and hints naming `--branches <subset>` and `--max-agents` / `[layout] max_agents`
- **AND** no tmux session SHALL be created

#### Scenario: Configured columns change the row split

- **GIVEN** `[layout] agents_per_row = 3`, a dashboard, and 7 agent branches
- **WHEN** the tmux layout is built
- **THEN** there SHALL be 3 agent rows holding panes 1-3, 4-6 and 7
- **AND** each agent row's height SHALL be `50 / 3` percent

#### Scenario: Pane indices match row-major order

- **GIVEN** a coordinated session with a dashboard, 7 agents and default grid settings
- **WHEN** the tmux layout is built
- **THEN** pane 1 SHALL be the first agent (top-left of the agent grid)
- **AND** pane 5 SHALL be the fifth agent (top-right of the first agent row)
- **AND** pane 6 SHALL be the sixth agent (start of the second agent row)

#### Scenario: Three agents in a row render at equal width

- **GIVEN** a coordinated session with 3 agent branches (a single agent row)
- **WHEN** the tmux layout is built and applied to a live tmux window
- **THEN** the three agent panes SHALL each occupy approximately one third of the window width (within a one-column rounding tolerance)

#### Scenario: Row rebalance leaves the dashboard row full width

- **GIVEN** a coordinated session with a dashboard whose agent row is rebalanced to equal width
- **WHEN** the layout is applied to a live tmux window
- **THEN** pane 0 (dashboard) SHALL remain at the full window width
- **AND** the vertical height proportions SHALL be unchanged: the dashboard row at 50% and each agent row at `50 / agent_rows` percent

#### Scenario: Full agent row stays at equal-width columns

- **GIVEN** a coordinated session with `agents_per_row` agents in a single row
- **WHEN** the layout is applied to a live tmux window
- **THEN** each agent pane SHALL occupy approximately `100 / agents_per_row` percent of the window width
- **AND** no agent row SHALL contain more than `agents_per_row` panes

### Requirement: Every pane-less session uses the agent grid

Every launch path (bare `start`, spec-driven `start` and coordinated `start`) and the recovery of a stopped session SHALL build the window with the agent-grid geometry defined by "Supervisor-mode pane layout": the dashboard as the full-width top row when the session has a dashboard, coding agents row-major below at no more than `agents_per_row` per row, each row rebalanced to equal width, and the agent cap. When the session has no dashboard, because the dashboard is not enabled or because the broker does not run, there SHALL be no top row. In that case the coding agents SHALL start at pane 0 and the agent rows SHALL share the full window height evenly. The whole-window `select-layout tiled` and `main-horizontal` layouts SHALL NOT be used to arrange coding agents. Every coding agent's pane index SHALL be its row-major position — the session's first agent index plus the agent's position — at any agent count: each split that creates an agent pane, including the one that opens a new row and the one `git paw add` performs, SHALL target the session's last pane, so the new pane takes the next index and no existing pane is renumbered.

#### Scenario: Bare start with a dashboard builds the grid

- **GIVEN** `[broker] enabled = true`, `[dashboard] enabled = true` and a bare launch
- **WHEN** `git paw start` launches 7 branches
- **THEN** pane 0 SHALL be the dashboard spanning the full width
- **AND** panes 1-5 SHALL form the first agent row and panes 6-7 the second, each row at equal widths

#### Scenario: Spec-driven start with a dashboard builds the grid

- **GIVEN** a spec-driven bare `git paw start --specs a b c` with the broker and the dashboard enabled
- **WHEN** the session is built
- **THEN** pane 0 SHALL be the dashboard and panes 1-3 SHALL be a single equal-width agent row

#### Scenario: A broker session without a dashboard uses the full height

- **GIVEN** `[broker] enabled = true` and no dashboard enabled
- **WHEN** `git paw start` launches 6 branches
- **THEN** there SHALL be no dashboard pane
- **AND** panes 0-4 SHALL form the first agent row and pane 5 the second, the two rows sharing the full height evenly

#### Scenario: Every agent's pane index holds that agent

- **GIVEN** a bare session started with 7 branches, with a dashboard and again without one
- **WHEN** the window is built, and again when `git paw add` attaches a sixth agent to a 5-agent session
- **THEN** pane `first_agent + i` SHALL run in the worktree of the i-th agent and run that agent's CLI, for every agent
- **AND** no launch command or prompt SHALL be typed into a pane other than the one its agent occupies

#### Scenario: Broker-less start uses the full height

- **GIVEN** `[broker] enabled = false`
- **WHEN** `git paw start` launches 6 branches
- **THEN** there SHALL be no dashboard pane
- **AND** panes 0-4 SHALL form the first agent row and pane 5 the second, the two rows sharing the full height evenly

#### Scenario: Recovery rebuilds a grid

- **GIVEN** a stopped bare session saved with the earlier tiled layout and 3 recorded worktrees with the broker enabled
- **WHEN** `git paw start` recovers it
- **THEN** the rebuilt window SHALL be the grid (dashboard row + one agent row of 3), because a session saved before the layout record has a dashboard when it recorded a broker
- **AND** the re-saved session SHALL record the grid layout and `dashboard_pane = Some(0)`
