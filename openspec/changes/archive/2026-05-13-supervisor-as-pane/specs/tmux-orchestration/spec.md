## ADDED Requirements

### Requirement: Supervisor-mode pane layout

When the tmux session is built for supervisor mode (per the `supervisor-launch` capability), the system SHALL produce a layout with these structural properties:

- **Top row**: split horizontally 50/50 between pane 0 (supervisor agent) and pane 1 (dashboard).
- **Agent grid below**: dynamically sized by agent count, with up to 5 columns per row in v0.5.0. The agent grid is a sequence of horizontal rows; each row holds up to 5 agent panes side-by-side.
- **Pane indices**: pane 0 = supervisor; pane 1 = dashboard; panes 2..N+1 = coding agents in row-major order (left-to-right, top-to-bottom).
- **Vertical proportions** by total-row count (top row + agent rows):

  | Total rows | Top row height | Each agent row height |
  |---|---|---|
  | 2 (1-5 agents) | 60% | 40% |
  | 3 (6-10 agents) | 40% | 30% each |
  | 4 (11-15 agents) | 28% | 24% each |
  | 5 (16-20 agents) | 28% | 18% each |
  | 6 (21-25 agents) | 28% | 14.4% each |

- **Hard cap**: 25 agents per session. Above 25, the system SHALL reject the launch with a clear "split into multiple sessions" error before any tmux command runs.

The layout SHALL be built using `tmux split-window -h` and `-v` with explicit percentages, then enforced via `tmux resize-pane -y <pct>` for the height proportions. `select-layout tiled` (or other auto-layouts) SHALL NOT be used for the supervisor-mode layout because they don't preserve the predictable pane-index ordering this layout relies on.

#### Scenario: 5-agent supervisor layout has 1 agent row

- **GIVEN** a supervisor session with 5 agent branches
- **WHEN** the tmux layout is built
- **THEN** pane 0 SHALL be the supervisor at 50% of the top row's width
- **AND** pane 1 SHALL be the dashboard at 50% of the top row's width
- **AND** panes 2-6 SHALL be agents arranged in a single row below the top row
- **AND** the top row's height SHALL be 60% and the agent row's height SHALL be 40%

#### Scenario: 10-agent supervisor layout has 2 agent rows

- **GIVEN** a supervisor session with 10 agent branches
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 3 (1 top + 2 agent rows)
- **AND** the top row's height SHALL be 40%
- **AND** each agent row's height SHALL be 30%
- **AND** the first agent row SHALL contain panes 2-6, the second agent row SHALL contain panes 7-11

#### Scenario: 20-agent supervisor layout has 4 agent rows

- **GIVEN** a supervisor session with 20 agent branches
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 5 (1 top + 4 agent rows)
- **AND** the top row's height SHALL be 28%
- **AND** each of the 4 agent rows' height SHALL be 18%

#### Scenario: 26-agent supervisor session is rejected

- **GIVEN** 26 agent branches resolved (via specs, --branches, or a combination)
- **WHEN** the supervisor launch flow runs
- **THEN** the launch SHALL be rejected with a `PawError`
- **AND** the error message SHALL state the requested count (26), the maximum (25), and a hint suggesting `--branches <subset>` for splitting into multiple sessions
- **AND** no tmux session SHALL be created

#### Scenario: Pane indices match row-major order

- **GIVEN** a supervisor session with 7 agents
- **WHEN** the tmux layout is built
- **THEN** pane 2 SHALL be the first agent (top-left of the agent grid)
- **AND** pane 6 SHALL be the fifth agent (top-right of the first agent row, since `agents_per_row = 5`)
- **AND** pane 7 SHALL be the sixth agent (start of the second agent row)
