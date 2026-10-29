## MODIFIED Requirements

### Requirement: Pane spawn with grid re-tiling

When adding a pane, the system SHALL recompute the agent-grid
layout for the new total agent count — using the grid settings
recorded for the session at launch (defaults when the session
records none) — and re-apply it so all panes match the layout
`git paw start` would have produced for that count with those
settings. Existing agent panes SHALL retain their pane indices so that
in-flight `send-keys` targeting continues to address the correct
panes.

The re-tile SHALL preserve every OTHER agent's pane: adding an agent
SHALL NOT close, drop, or orphan any existing agent's pane, and after
the re-tile the live tmux window SHALL contain exactly one pane per
session-JSON agent plus the dashboard pane when the session has one (the v0.8.0
G2 dogfood failure dropped a different agent's pane during the re-tile).
After re-tiling, each agent row SHALL be rebalanced to equal width per
the `tmux-orchestration` "Supervisor-mode pane layout" requirement, so
an incrementally-added grid matches a start-time grid of the same agent
count in both pane count and pane widths. The first coding-agent index SHALL come from the session's recorded layout, not from a mode constant.

#### Scenario: Grid re-tiles for the new agent count

- **GIVEN** an active session with default grid settings and 4 agent panes (single row)
- **WHEN** the user runs `git paw add feat/fifth`
- **THEN** the agent grid SHALL be laid out as the 5-pane layout
  (single row of 5), matching what a 5-agent `start` would produce

#### Scenario: Existing pane indices are preserved

- **GIVEN** an active session whose agents occupy consecutive pane indices
- **WHEN** the user runs `git paw add feat/new`
- **THEN** the existing panes SHALL retain their indices and
  the new pane SHALL receive the next index, verified by
  `pane_current_path` mapping

#### Scenario: No existing agent pane is dropped by the re-tile

- **GIVEN** an active session with 3 agent panes mapped to their
  worktrees via `pane_current_path`
- **WHEN** the user runs `git paw add feat/fourth`
- **THEN** the live tmux window SHALL contain a pane for each of the
  original 3 agents plus the new agent (4 agent panes total), with no
  original agent left without a pane

#### Scenario: Added grid matches a start-time grid width-for-width

- **GIVEN** an active session with 2 agent panes
- **WHEN** the user runs `git paw add feat/third` and the re-tile is
  applied to a live tmux window
- **THEN** the 3 agent panes SHALL each render at approximately one
  third of the window width (within a one-column rounding tolerance),
  matching what a 3-agent `start` would produce

#### Scenario: Adding past the agent cap is rejected

- **GIVEN** an active session already holding as many agents as its recorded `max_agents` (25 when none is recorded)
- **WHEN** the user runs `git paw add` for one more branch
- **THEN** the command SHALL exit non-zero with the same
  cap message `start` uses, and SHALL
  NOT create a worktree or pane

#### Scenario: Re-tile uses the recorded column count

- **GIVEN** a session launched with `[layout] agents_per_row = 3` holding 3 agents, and a config since edited to `agents_per_row = 5`
- **WHEN** the user runs `git paw add feat/fourth`
- **THEN** the fourth agent SHALL start a second agent row
