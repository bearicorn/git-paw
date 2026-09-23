## ADDED Requirements

### Requirement: Poll-loop pane resolution follows the live layout

The auto-approve poll loop SHALL resolve each stalled agent's pane on every tick by matching the agent's `worktree_path`, read from the session's current worktree roster, against the `pane_current_path` of the session's live panes — the same path-based resolution the drive loop uses. It SHALL NOT resolve a pane from the agent's position in the session's worktree list or from a mapping computed once at dashboard start, because `git paw add` and `git paw remove` change the roster and tmux renumbers panes when one is removed.

The worktree root used to evaluate the worktree-confined classifier rules for an agent SHALL come from the same tick's roster entry as its pane. An agent whose `worktree_path` matches no live pane, or matches more than one, SHALL be skipped for that tick: no pane capture, no keystrokes, and no forwarded question. When the session's roster cannot be read on a tick, that tick SHALL take no approval action. Pane 0 (supervisor) and pane 1 (dashboard) SHALL NOT resolve to a coding agent.

#### Scenario: An agent added mid-session is resolved and approved

- **GIVEN** a running supervisor session with agents `feat-a` and `feat-b`, and an auto-approve poll loop already running
- **WHEN** `git paw add feat-c` attaches a new agent pane whose `pane_current_path` is `feat-c`'s worktree, and `feat-c` stalls on a live, safe-classified prompt
- **THEN** the next tick SHALL resolve `feat-c` to that pane
- **AND** SHALL dispatch the approval keystrokes to that pane

#### Scenario: After a middle agent is removed, later agents resolve to their own panes

- **GIVEN** agents `feat-a`, `feat-b`, `feat-c` in panes 2, 3, 4
- **WHEN** `feat-b` is removed, tmux renumbers `feat-c`'s pane to 3, and `feat-c` stalls on a live prompt
- **THEN** the next tick SHALL resolve `feat-c` to pane 3
- **AND** SHALL classify the prompt against `feat-c`'s worktree root
- **AND** SHALL NOT capture or send keystrokes to any pane on `feat-c`'s behalf other than pane 3

#### Scenario: A worktree-confined command is never judged against a neighbour's worktree

- **GIVEN** a roster from which a middle agent was just removed, and a stalled agent `feat-c` whose live prompt is a file edit targeting a path inside `feat-c`'s worktree and outside every other agent's worktree
- **WHEN** the poll loop ticks
- **THEN** the worktree-confined rule SHALL be evaluated with `feat-c`'s worktree root, so the edit classifies as worktree-confined
- **AND** the keystrokes SHALL go to the pane whose `pane_current_path` is that worktree

#### Scenario: An unresolvable agent is skipped

- **GIVEN** a stalled agent whose `worktree_path` matches no live pane's `pane_current_path`
- **WHEN** the poll loop ticks
- **THEN** it SHALL NOT capture any pane for that agent, SHALL NOT send keystrokes, and SHALL NOT forward a question for it

#### Scenario: An ambiguous match is skipped

- **GIVEN** two live panes whose `pane_current_path` both equal one agent's `worktree_path`
- **WHEN** the poll loop ticks with that agent stalled
- **THEN** it SHALL take no approval action for that agent on that tick

#### Scenario: An unreadable roster takes no action

- **GIVEN** a tick on which the session's worktree roster cannot be read
- **WHEN** the poll loop ticks with stalled agents reported by the broker
- **THEN** it SHALL send no keystrokes and forward no question on that tick
