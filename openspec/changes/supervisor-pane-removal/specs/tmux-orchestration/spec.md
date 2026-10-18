## MODIFIED Requirements

### Requirement: Supervisor-mode pane layout

When the tmux session is built for a supervised session (per the `supervisor-launch` capability), the system SHALL produce a layout with these structural properties:

- **Top row**: pane 0 (dashboard) spanning the full window width.
- **Agent grid below**: dynamically sized by agent count, with up to 5 columns per row. The agent grid is a sequence of horizontal rows; each row holds up to 5 agent panes side-by-side.
- **Pane indices**: pane 0 = dashboard; panes 1..N = coding agents in row-major order (left-to-right, top-to-bottom). No pane runs an orchestrator CLI.
- **Vertical proportions**: the dashboard row SHALL occupy 50% of the session height regardless of agent count, and the agent rows SHALL share the remaining 50% evenly — each agent row occupying `50 / agent_rows` percent. Adding agents SHALL shrink the agent rows, never the dashboard row.

  | Agent rows | Dashboard row height | Each agent row height |
  |---|---|---|
  | 1 (1-5 agents) | 50% | 50% |
  | 2 (6-10 agents) | 50% | 25% each |
  | 3 (11-15 agents) | 50% | 16.7% each |
  | 4 (16-20 agents) | 50% | 12.5% each |
  | 5 (21-25 agents) | 50% | 10% each |

- **Equal-width agent panes within a row**: every agent pane in the same agent row SHALL be rendered at equal width (within a one-column rounding tolerance). Because agents are added to a row by successive `tmux split-window -h` — and each `-h` split halves the *current* pane — a row populated by raw splits renders unequal widths, so the system SHALL rebalance each agent row to equal width after its panes are created, by applying `tmux select-layout even-horizontal` scoped to that row's agent panes OR by issuing `tmux resize-pane` to set each agent pane to `100 / agents_in_row` percent of the row's width. The rebalance SHALL NOT alter the dashboard row's full width nor the vertical height proportions described above.

- **Minimum usable pane width**: the layout SHALL keep agent panes wide enough to be usable up to the agent cap; at the maximum 5 columns per row the equal-width target is 20% of the window width per pane. The system SHALL NOT produce agent rows with more than 5 panes (`SUPERVISOR_AGENTS_PER_ROW`).

- **Hard cap**: 25 agents per session. Above 25, the system SHALL reject the launch with a clear "split into multiple sessions" error before any tmux command runs.

The layout SHALL be built using `tmux split-window -h` and `-v` with explicit percentages, then enforced via `tmux resize-pane -y <pct>` for the height proportions and the per-row equal-width rebalance described above. `select-layout tiled` (or other whole-window auto-layouts) SHALL NOT be used for the supervised layout because they do not preserve the predictable pane-index ordering this layout relies on; `select-layout even-horizontal` MAY be used only when scoped so it rebalances a single agent row's widths without reordering pane indices or disturbing the dashboard row. No `swap-pane` SHALL be emitted.

#### Scenario: 5-agent supervised layout has 1 agent row

- **GIVEN** a supervised session with 5 agent branches
- **WHEN** the tmux layout is built
- **THEN** pane 0 SHALL be the dashboard spanning the full window width
- **AND** panes 1-5 SHALL be agents arranged in a single row below the dashboard
- **AND** the dashboard row's height SHALL be 50% and the agent row's height SHALL be 50%

#### Scenario: 10-agent supervised layout has 2 agent rows

- **GIVEN** a supervised session with 10 agent branches
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 3 (1 dashboard + 2 agent rows)
- **AND** the dashboard row's height SHALL be 50%
- **AND** each agent row's height SHALL be 25%
- **AND** the first agent row SHALL contain panes 1-5, the second agent row SHALL contain panes 6-10

#### Scenario: 20-agent supervised layout has 4 agent rows

- **GIVEN** a supervised session with 20 agent branches
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 5 (1 dashboard + 4 agent rows)
- **AND** the dashboard row's height SHALL be 50%
- **AND** each of the 4 agent rows' height SHALL be 12.5%

#### Scenario: 26-agent supervised session is rejected

- **GIVEN** 26 agent branches resolved
- **WHEN** the supervised launch flow runs
- **THEN** the launch SHALL be rejected with a `PawError` stating the requested count (26), the maximum (25), and a hint suggesting `--branches <subset>`
- **AND** no tmux session SHALL be created

#### Scenario: Pane indices match row-major order

- **GIVEN** a supervised session with 7 agents
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

- **GIVEN** a supervised session with 5 agents in a single row
- **WHEN** the layout is applied to a live tmux window
- **THEN** each of the 5 agent panes SHALL occupy approximately 20% of the window width
- **AND** no agent row SHALL contain more than 5 panes
