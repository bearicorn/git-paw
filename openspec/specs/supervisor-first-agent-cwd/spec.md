# supervisor-first-agent-cwd Specification

## Purpose
Ensures the first coding agent's pane runs its CLI in that agent's own worktree rather than the supervisor's repo root, by assigning the split-time `-c <cwd>` values to compensate for the pane-1/2 swap so each post-swap pane's cwd matches the command sent to it.

## Requirements
### Requirement: First agent pane launches in its own worktree

The supervisor session build SHALL ensure the first coding agent's pane runs
its CLI in that agent's worktree, never in the supervisor's repo-root working
directory. Because the build swaps panes 1 and 2 (to order dashboard before
the agent area) and sends each pane's CLI command after the swap by index,
the split-time `-c <cwd>` values SHALL be assigned to compensate for the
swap: the agent-area split takes the dashboard's cwd and the dashboard split
takes the first agent's worktree, so that post-swap each index's cwd matches
the command sent to it.

#### Scenario: First agent's CLI runs in its worktree

- **GIVEN** a supervisor session launched with at least one coding agent
- **WHEN** the layout is built and the first agent's CLI command is sent to
  its pane
- **THEN** that pane's working directory SHALL be the first agent's worktree
  (so its commits land on the agent's own branch), NOT the repo root

#### Scenario: Compensated split cwds

- **WHEN** the supervisor build's two top-region splits are inspected
- **THEN** the agent-area (`split-window -v`) SHALL carry `-c <dashboard
  cwd>` and the dashboard (`split-window -h`) SHALL carry `-c <first agent
  worktree>`, the assignment that, after the pane-1/2 swap, places the first
  agent's worktree under the agent's command

#### Scenario: Later agents unaffected

- **GIVEN** a supervisor session with two or more coding agents
- **THEN** the second and later agents (created by their own
  `split-window -c <worktree>` with no swap) SHALL each run in their own
  worktree, as before

