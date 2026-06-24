# remove-branch (delta)

## MODIFIED Requirements

### Requirement: Pane closure with grid re-tiling

When removing an agent, the system SHALL kill the agent's tmux
pane and re-apply the agent-grid layout for the new (smaller)
agent count so the grid re-flows without a hole. Remaining panes'
relative order SHALL be preserved.

The system SHALL resolve the target pane by mapping the removed
branch's worktree to a live pane via `pane_current_path` and SHALL kill
that pane by its tmux pane id, regardless of the process running in it
— a bare shell (a failed/never-started CLI), a CLI, or any other
process. Killing by resolved pane id (not by a position computed from
the session JSON) ensures a failed agent whose pane never launched a CLI
is still closed rather than orphaned, and that the kill targets the
removed agent's pane and never a different agent's pane (the v0.8.0 G2
dogfood failure killed/dropped a different agent's pane because the index
was computed from JSON position while a stale orphan pane shifted the
grid). The re-tile SHALL preserve every OTHER agent's pane: after the
removal the live tmux window SHALL contain exactly one pane per remaining
session-JSON agent plus the supervisor and dashboard panes, and each
agent row SHALL be rebalanced to equal width per the `tmux-orchestration`
"Supervisor-mode pane layout" requirement.

#### Scenario: Grid re-flows after a removal

- **GIVEN** an active session with 5 agent panes (single row)
- **WHEN** the user runs `git paw remove feat/middle`
- **THEN** the agent grid SHALL be laid out as the 4-pane layout,
  matching what a 4-agent `start` would produce, and the order of
  the remaining 4 agents SHALL be preserved

#### Scenario: Branch→pane mapping is re-derived after removal

- **GIVEN** an active session whose branch→pane mapping was
  established via `pane_current_path`
- **WHEN** an agent in the middle of the grid is removed
- **THEN** subsequent supervisor sweeps SHALL re-derive the
  branch→pane mapping via `pane_current_path` and SHALL continue
  to target the correct panes for the remaining agents

#### Scenario: Removing a failed shell-occupied pane still closes it

- **GIVEN** an active session whose agent `feat/x` pane is a bare shell
  (its CLI never started, the v0.8.0 G1 condition)
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the system SHALL resolve `feat/x`'s pane via
  `pane_current_path` and kill it by pane id, leaving no orphan pane,
  even though the pane is running a shell rather than the expected CLI

#### Scenario: Removal does not drop a different agent's pane

- **GIVEN** an active session with agents `feat/a`, `feat/b`, `feat/c`
  each mapped to a live pane via `pane_current_path`
- **WHEN** the user runs `git paw remove feat/b`
- **THEN** only `feat/b`'s pane SHALL be killed
- **AND** `feat/a` and `feat/c` SHALL each still have exactly one live
  pane after the re-tile (no collateral pane loss, no orphan)
