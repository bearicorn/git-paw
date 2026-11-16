## MODIFIED Requirements

### Requirement: Supervisor-mode pane layout

When the tmux session is built for a supervised session (per the `supervisor-launch` capability), the system SHALL produce a layout with these structural properties, where `agents_per_row` and `max_agents` are the session's effective grid settings (see "Agent grid dimensions are configurable"; defaults 5 and 25):

- **Top row**: pane 0 (dashboard) spanning the full window width.
- **Agent grid below**: dynamically sized by agent count, with up to `agents_per_row` columns per row. The agent grid is a sequence of horizontal rows; each row holds up to `agents_per_row` agent panes side-by-side.
- **Pane indices**: pane 0 = dashboard; panes 1..N = coding agents in row-major order (left-to-right, top-to-bottom). No pane runs an orchestrator CLI.
- **Vertical proportions**: the dashboard row SHALL occupy 50% of the session height regardless of agent count, and the agent rows SHALL share the remaining 50% evenly — each agent row occupying `50 / agent_rows` percent, where `agent_rows = ceil(N / agents_per_row)`. Adding agents SHALL shrink the agent rows, never the dashboard row. With the defaults:

  | Agent rows | Dashboard row height | Each agent row height |
  |---|---|---|
  | 1 (1-5 agents) | 50% | 50% |
  | 2 (6-10 agents) | 50% | 25% each |
  | 3 (11-15 agents) | 50% | 16.7% each |
  | 4 (16-20 agents) | 50% | 12.5% each |
  | 5 (21-25 agents) | 50% | 10% each |

- **Equal-width agent panes within a row**: every agent pane in the same agent row SHALL be rendered at equal width (within a one-column rounding tolerance). Because agents are added to a row by successive `tmux split-window -h` — and each `-h` split halves the *current* pane — a row populated by raw splits renders unequal widths, so the system SHALL rebalance each agent row to equal width after its panes are created, by applying `tmux select-layout even-horizontal` scoped to that row's agent panes OR by issuing `tmux resize-pane` to set each agent pane to `100 / agents_in_row` percent of the row's width. The rebalance SHALL NOT alter the dashboard row's full width nor the vertical height proportions described above.

- **Row width bound**: the system SHALL NOT produce agent rows with more than `agents_per_row` panes; the equal-width target per pane is `100 / agents_per_row` percent of the window width at a full row.

- **Agent cap**: at most `max_agents` agents per session. Above it, the system SHALL reject the launch before any tmux command runs with an error stating the requested count, the effective maximum, and how to proceed (`--branches <subset>` to split, or `--max-agents` / `[layout] max_agents` to raise the cap).

The layout SHALL be built using `tmux split-window -h` and `-v` with explicit percentages, then enforced via `tmux resize-pane -y <pct>` for the height proportions and the per-row equal-width rebalance described above. `select-layout tiled` (or other whole-window auto-layouts) SHALL NOT be used for the supervised layout because they do not preserve the predictable pane-index ordering this layout relies on; `select-layout even-horizontal` MAY be used only when scoped so it rebalances a single agent row's widths without reordering pane indices or disturbing the dashboard row. No `swap-pane` SHALL be emitted.

#### Scenario: 5-agent supervised layout has 1 agent row

- **GIVEN** a supervised session with 5 agent branches and default grid settings
- **WHEN** the tmux layout is built
- **THEN** pane 0 SHALL be the dashboard spanning the full window width
- **AND** panes 1-5 SHALL be agents arranged in a single row below the dashboard
- **AND** the dashboard row's height SHALL be 50% and the agent row's height SHALL be 50%

#### Scenario: 10-agent supervised layout has 2 agent rows

- **GIVEN** a supervised session with 10 agent branches and default grid settings
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 3 (1 dashboard + 2 agent rows)
- **AND** the dashboard row's height SHALL be 50%
- **AND** each agent row's height SHALL be 25%
- **AND** the first agent row SHALL contain panes 1-5, the second agent row SHALL contain panes 6-10

#### Scenario: 20-agent supervised layout has 4 agent rows

- **GIVEN** a supervised session with 20 agent branches and default grid settings
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 5 (1 dashboard + 4 agent rows)
- **AND** the dashboard row's height SHALL be 50%
- **AND** each of the 4 agent rows' height SHALL be 12.5%

#### Scenario: 26-agent supervised session is rejected

- **GIVEN** default grid settings (no `--max-agents`, no `[layout] max_agents`) and 26 agent branches resolved
- **WHEN** the supervised launch flow runs
- **THEN** the launch SHALL be rejected with a `PawError` stating the requested count (26), the maximum (25), and hints naming `--branches <subset>` and `--max-agents` / `[layout] max_agents`
- **AND** no tmux session SHALL be created

#### Scenario: Configured columns change the row split

- **GIVEN** `[layout] agents_per_row = 3` and 7 agent branches
- **WHEN** the tmux layout is built
- **THEN** there SHALL be 3 agent rows holding panes 1-3, 4-6 and 7
- **AND** each agent row's height SHALL be `50 / 3` percent

#### Scenario: Pane indices match row-major order

- **GIVEN** a supervised session with 7 agents and default grid settings
- **WHEN** the tmux layout is built
- **THEN** pane 1 SHALL be the first agent (top-left of the agent grid)
- **AND** pane 5 SHALL be the fifth agent (top-right of the first agent row)
- **AND** pane 6 SHALL be the sixth agent (start of the second agent row)

#### Scenario: Three agents in a row render at equal width

- **GIVEN** a supervised session with 3 agent branches (a single agent row)
- **WHEN** the tmux layout is built and applied to a live tmux window
- **THEN** the three agent panes SHALL each occupy approximately one third of the window width (within a one-column rounding tolerance)

#### Scenario: Row rebalance leaves the dashboard row full width

- **GIVEN** a supervised session whose agent row is rebalanced to equal width
- **WHEN** the layout is applied to a live tmux window
- **THEN** pane 0 (dashboard) SHALL remain at the full window width
- **AND** the vertical height proportions SHALL be unchanged: the dashboard row at 50% and each agent row at `50 / agent_rows` percent

#### Scenario: Full agent row stays at five equal-width columns

- **GIVEN** a supervised session with default grid settings and 5 agents in a single row
- **WHEN** the layout is applied to a live tmux window
- **THEN** each of the 5 agent panes SHALL occupy approximately 20% of the window width
- **AND** no agent row SHALL contain more than 5 panes
- **AND** with `[layout] agents_per_row = 3` and 3 agents, each agent pane SHALL instead occupy approximately one third of the window width and no agent row SHALL contain more than 3 panes

### Requirement: Every pane-less session uses the agent grid

Every launch path — bare `start`, spec-driven `start`, and supervised `start` — and the recovery of a stopped session SHALL build the window with the agent-grid geometry defined by "Supervisor-mode pane layout": the dashboard as the full-width top row when the broker is enabled, coding agents row-major below at no more than the session's effective `agents_per_row` per row, each row rebalanced to equal width, and the session's effective `max_agents` cap (see "Agent grid dimensions are configurable"; defaults 5 and 25). When the session has no dashboard (broker disabled), there SHALL be no top row: coding agents SHALL start at pane 0 and the agent rows SHALL share the full window height evenly. The whole-window `select-layout tiled` and `main-horizontal` layouts SHALL NOT be used to arrange coding agents. Every coding agent's pane index SHALL be its row-major position — the session's first agent index plus the agent's position — at any agent count: each split that creates an agent pane, including the one that opens a new row and the one `git paw add` performs, SHALL target the session's last pane, so the new pane takes the next index and no existing pane is renumbered.

#### Scenario: Bare start builds the grid

- **GIVEN** `[broker] enabled = true`, no supervisor mode and default grid settings
- **WHEN** `git paw start` launches 7 branches
- **THEN** pane 0 SHALL be the dashboard spanning the full width
- **AND** panes 1-5 SHALL form the first agent row and panes 6-7 the second, each row at equal widths

#### Scenario: Spec-driven start builds the grid

- **GIVEN** a spec-driven `git paw start --specs a b c` without supervisor mode and the broker enabled
- **WHEN** the session is built
- **THEN** pane 0 SHALL be the dashboard and panes 1-3 SHALL be a single equal-width agent row

#### Scenario: Broker-less start uses the full height

- **GIVEN** `[broker] enabled = false` and default grid settings
- **WHEN** `git paw start` launches 6 branches
- **THEN** there SHALL be no dashboard pane
- **AND** panes 0-4 SHALL form the first agent row and pane 5 the second, the two rows sharing the full height evenly

#### Scenario: Every agent's pane index holds that agent

- **GIVEN** a bare session started with 7 branches, with the broker enabled and again with it disabled
- **WHEN** the window is built, and again when `git paw add` attaches a sixth agent to a 5-agent session
- **THEN** pane `first_agent + i` SHALL run in the worktree of the i-th agent and run that agent's CLI, for every agent
- **AND** no launch command or prompt SHALL be typed into a pane other than the one its agent occupies

#### Scenario: Recovery rebuilds a grid

- **GIVEN** a stopped bare session saved with the earlier tiled layout and 3 recorded worktrees with the broker enabled
- **WHEN** `git paw start` recovers it
- **THEN** the rebuilt window SHALL be the grid (dashboard row + one agent row of 3)
- **AND** the re-saved session SHALL record the grid layout

#### Scenario: A bare session uses the configured column count

- **GIVEN** `[layout] agents_per_row = 3`, the broker enabled and no supervisor mode
- **WHEN** `git paw start` launches 5 branches
- **THEN** panes 1-3 SHALL form the first agent row and panes 4-5 the second, each row at equal widths

## ADDED Requirements

### Requirement: Agent grid dimensions are configurable

The supervised agent grid SHALL take its cap and column count from effective grid settings resolved once per launch: `max_agents` from `git paw start --max-agents <N>`, else `[layout] max_agents`, else 25; `agents_per_row` from `[layout] agents_per_row`, else 5. Both values SHALL be integers ≥ 1; a configured `0` SHALL fail config loading with an error naming the key, and `--max-agents 0` SHALL be a usage error. The effective settings SHALL be recorded in the session state at launch as an additive field that older session files omit; `git paw add`, `git paw remove` and recovery SHALL use the recorded settings (defaults when absent) rather than the current config. Recovery SHALL NOT reject a session because its agent count exceeds the current cap. When the terminal cannot fit the requested grid, the launch SHALL fail with an error naming the effective `max_agents` and `agents_per_row`. Since v0.18.0's `pane-less-session-parity` every session, coordinated or bare, uses the agent grid, so these settings SHALL apply to bare sessions too. Configs without the new keys SHALL load unchanged.

#### Scenario: Config raises the cap

- **GIVEN** `[layout] max_agents = 30`
- **WHEN** a supervised session is started with 28 agent branches
- **THEN** the launch SHALL succeed with 28 agent panes

#### Scenario: Flag overrides config for one launch

- **GIVEN** `[layout] max_agents = 10`
- **WHEN** the user runs `git paw start --supervisor --max-agents 12` with 12 agent branches
- **THEN** the launch SHALL succeed

#### Scenario: Zero is rejected

- **GIVEN** `[layout] agents_per_row = 0`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `agents_per_row`

#### Scenario: add uses the session's recorded settings

- **GIVEN** a session launched with `--max-agents 12` and now holding 12 agents, and a config that says `max_agents = 25`
- **WHEN** the user runs `git paw add feat/thirteenth`
- **THEN** the command SHALL be rejected with the cap error naming 12

#### Scenario: Recovery ignores a lowered cap

- **GIVEN** a stopped session of 20 agents and a config edited to `max_agents = 10`
- **WHEN** the session is recovered
- **THEN** all 20 agent panes SHALL be rebuilt

#### Scenario: Older session files use the defaults

- **GIVEN** a session file written before this change (no grid field)
- **WHEN** `git paw add` re-tiles that session
- **THEN** it SHALL use 5 columns per row and a cap of 25

#### Scenario: Bare sessions use the same grid settings

- **GIVEN** `[layout] max_agents = 2` and a bare (non-coordinated) launch of 3 branches
- **WHEN** the session is built
- **THEN** the launch SHALL fail with an error naming the effective `max_agents`, as a coordinated launch would
