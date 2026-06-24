# tmux-orchestration (delta)

## ADDED Requirements

### Requirement: Launch-readiness gate before boot-block injection

The system SHALL verify that a pane's CLI process has actually started and
reached an interactive ready state before injecting any boot block,
framing prompt, or `/opsx:apply` task prompt into that pane, rather than
relying solely on a fixed wall-clock sleep. The system SHALL poll the
pane (via `tmux capture-pane`) for a CLI-readiness marker — content that
distinguishes the launched CLI's interactive UI from a bare shell prompt
— up to a bounded timeout. The boot block SHALL NOT be injected while the
pane is still a bare shell, because the shell would interpret the
multi-line boot block line-by-line as failing commands (the v0.8.0 G1
dogfood failure).

If the readiness timeout elapses while the pane is still a bare shell,
the system SHALL relaunch the CLI command into that pane (re-clearing the
input line first) and poll again, up to a bounded number of relaunch
attempts. The readiness gate and its timeout/attempt budget SHALL apply
uniformly to the start-time launch path and the `git paw add` launch
path so an added agent receives the same protection.

The readiness gate SHALL be conservative: a CLI that the gate cannot
positively classify within the budget (e.g. an unrecognised
custom CLI whose UI the marker heuristic does not match) SHALL fall back
to injecting the boot block after the budget elapses rather than failing
the launch, so launch behaviour is never worse than the previous
fixed-sleep behaviour for an unrecognised CLI.

#### Scenario: Boot block withheld until the pane is CLI-ready

- **GIVEN** a freshly split agent pane whose CLI process has not yet
  reached its interactive prompt
- **WHEN** the launch flow reaches the boot-block injection step for that
  pane
- **THEN** the system SHALL poll the pane for a CLI-readiness marker and
  SHALL NOT inject the boot block while the pane still shows only a bare
  shell prompt

#### Scenario: Boot block injected once the CLI is ready

- **GIVEN** an agent pane whose CLI has reached its interactive ready
  state within the readiness timeout
- **WHEN** the readiness poll observes the CLI-readiness marker
- **THEN** the system SHALL inject the boot block into that pane exactly
  once

#### Scenario: CLI relaunched when the pane is still a bare shell

- **GIVEN** an agent pane whose CLI never started (the pane is still a
  bare shell when the readiness timeout elapses)
- **WHEN** the readiness gate exhausts its timeout for that pane
- **THEN** the system SHALL relaunch the CLI command into that pane and
  poll for readiness again, up to the relaunch-attempt budget, before
  falling back to injection

#### Scenario: Unrecognised CLI falls back to fixed-budget injection

- **GIVEN** an agent pane running a custom CLI whose interactive UI does
  not match any known readiness marker
- **WHEN** the readiness budget elapses without a positive readiness
  classification
- **THEN** the system SHALL inject the boot block after the budget rather
  than rejecting the launch, so behaviour is no worse than the prior
  fixed-sleep launch for an unrecognised CLI

## MODIFIED Requirements

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

- **Equal-width agent panes within a row**: every agent pane in the same agent row SHALL be rendered at equal width (within a one-column rounding tolerance), i.e. each pane occupies `100 / agents_in_that_row` percent of the row's width. Because agents are added to a row by successive `tmux split-window -h` — and each `-h` split halves the *current* pane — a row populated by raw splits renders unequal widths (e.g. a 3-agent row renders 50/25/25, not equal thirds: the v0.8.0 G3 dogfood failure). The system SHALL therefore rebalance each agent row to equal width after its panes are created, by applying `tmux select-layout even-horizontal` scoped to that row's agent panes OR by issuing `tmux resize-pane` to set each agent pane to `100 / agents_in_row` percent of the row's width. The rebalance SHALL NOT alter the top row's explicit supervisor/dashboard 50/50 horizontal proportions nor the per-row vertical height proportions in the table above.

- **Minimum usable pane width**: the layout SHALL keep agent panes wide enough to be usable up to the agent cap; at the maximum 5 columns per row the equal-width target is 20% of the window width per pane. The system SHALL NOT produce agent rows with more than 5 panes (`SUPERVISOR_AGENTS_PER_ROW`), which bounds the minimum equal-width target per pane.

- **Hard cap**: 25 agents per session. Above 25, the system SHALL reject the launch with a clear "split into multiple sessions" error before any tmux command runs.

The layout SHALL be built using `tmux split-window -h` and `-v` with explicit percentages, then enforced via `tmux resize-pane -y <pct>` for the height proportions and the per-row equal-width rebalance described above. `select-layout tiled` (or other whole-window auto-layouts) SHALL NOT be used for the supervisor-mode layout because they don't preserve the predictable pane-index ordering this layout relies on; `select-layout even-horizontal` MAY be used only when scoped so it rebalances a single agent row's widths without reordering pane indices or disturbing the top row.

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

#### Scenario: Three agents in a row render at equal width

- **GIVEN** a supervisor session with 3 agent branches (a single agent row)
- **WHEN** the tmux layout is built and applied to a live tmux window
- **THEN** the three agent panes SHALL each occupy approximately one third of the window width (within a one-column rounding tolerance)
- **AND** the row SHALL NOT render as 50/25/25

#### Scenario: Row rebalance leaves the top row 50/50

- **GIVEN** a supervisor session whose agent row is rebalanced to equal width
- **WHEN** the layout is applied to a live tmux window
- **THEN** pane 0 (supervisor) and pane 1 (dashboard) SHALL remain at approximately 50% of the window width each
- **AND** the per-row vertical height proportions SHALL match the layout table

#### Scenario: Full agent row stays at five equal-width columns

- **GIVEN** a supervisor session with 5 agents in a single row
- **WHEN** the layout is applied to a live tmux window
- **THEN** each of the 5 agent panes SHALL occupy approximately 20% of the window width (within a one-column rounding tolerance)
- **AND** no agent row SHALL contain more than 5 panes
