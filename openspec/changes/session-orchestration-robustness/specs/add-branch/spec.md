# add-branch (delta)

## MODIFIED Requirements

### Requirement: Pane spawn with grid re-tiling

When adding a pane, the system SHALL recompute the agent-grid
layout for the new total agent count and re-apply it so all panes
match the layout `git paw start` would have produced for that
count. Existing agent panes SHALL retain their pane indices so that
in-flight `send-keys` targeting continues to address the correct
panes.

The re-tile SHALL preserve every OTHER agent's pane: adding an agent
SHALL NOT close, drop, or orphan any existing agent's pane, and after
the re-tile the live tmux window SHALL contain exactly one pane per
session-JSON agent plus the supervisor and dashboard panes (the v0.8.0
G2 dogfood failure dropped a different agent's pane during the re-tile).
After re-tiling, each agent row SHALL be rebalanced to equal width per
the `tmux-orchestration` "Supervisor-mode pane layout" requirement, so
an incrementally-added grid matches a start-time grid of the same agent
count in both pane count and pane widths.

#### Scenario: Grid re-tiles for the new agent count

- **GIVEN** an active session with 4 agent panes (single row)
- **WHEN** the user runs `git paw add feat/fifth`
- **THEN** the agent grid SHALL be laid out as the 5-pane layout
  (single row of 5), matching what a 5-agent `start` would produce

#### Scenario: Existing pane indices are preserved

- **GIVEN** an active session whose agents occupy pane indices
  2 through 6
- **WHEN** the user runs `git paw add feat/new`
- **THEN** the existing panes SHALL retain indices 2 through 6 and
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

- **GIVEN** an active session already at the 25-agent cap
- **WHEN** the user runs `git paw add feat/twenty-six`
- **THEN** the command SHALL exit non-zero with the same
  "split into multiple sessions" message `start` uses, and SHALL
  NOT create a worktree or pane

## ADDED Requirements

### Requirement: Session-JSON to tmux reconciliation

The system SHALL provide a reconciliation that detects divergence
between the session JSON (`.git-paw/sessions/paw-<project>.json`) and
the live tmux panes — specifically a session-JSON agent that has no live
tmux pane (the v0.8.0 G2 desync, where an agent remained in the JSON and
broker roster while its pane had been dropped). On the `add` path, after
the new pane is spliced and the grid re-tiled, the system SHALL verify
that every session-JSON agent maps to a live pane via `pane_current_path`
and SHALL surface any agent that does not, so the desync is visible and
recoverable rather than silent.

#### Scenario: Reconciliation reports an agent with no live pane

- **GIVEN** a session JSON listing an agent whose tmux pane is missing
  (its worktree path appears in no live pane's `pane_current_path`)
- **WHEN** the reconciliation runs
- **THEN** it SHALL report that agent as having no live pane

#### Scenario: Reconciliation passes when JSON and tmux agree

- **GIVEN** a session whose JSON agents each map to a live pane via
  `pane_current_path`
- **WHEN** the reconciliation runs after `git paw add`
- **THEN** it SHALL report no divergence
