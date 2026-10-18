## MODIFIED Requirements

### Requirement: Pane-to-agent resolution via pane_current_path

The drive loop SHALL resolve each surface to its agent by asking the terminal backend which working tree owns the surface and matching that path against the session's per-agent `worktree_path` by exact canonical-path equality, through the single core owner-resolution function that the `remove`, `add` reconciliation and restart-from-pause paths also use. The loop SHALL NOT resolve coding agents by index or by CLI-argument order, because indices are neither alphabetical nor argument-ordered and drift on layout changes, and SHALL NOT match on a path's basename or on a path prefix. A surface whose owner matches no agent worktree is never a coding agent: the dashboard is identified by the session's layout record (the recorded dashboard position, converted to a surface by the backend), not by its owner path, and any other unmatched surface resolves as unknown. The tmux backend SHALL answer the owner query from `pane_current_path`; no module outside the tmux backend SHALL read `pane_current_path` directly.

#### Scenario: Agent resolved by working directory, not index

- **GIVEN** a session whose surface indices do not match the alphabetical or CLI-argument order of agents, and an agent on worktree `/path/to/repo-feat-b`
- **WHEN** the drive loop resolves the surface whose owner the backend reports as `/path/to/repo-feat-b`
- **THEN** the loop SHALL attribute that surface to the `feat/b` agent
- **AND** SHALL NOT rely on the surface index to make the attribution

#### Scenario: A captured prompt is attributed to the correct agent

- **GIVEN** two coding-agent surfaces in different worktrees, one showing a live prompt
- **WHEN** the drive loop captures both and asks the backend for each owner
- **THEN** the prompt SHALL be attributed to the agent whose `worktree_path` equals the prompting surface's owner

#### Scenario: A basename collision does not misattribute

- **GIVEN** agent `feat-a` in worktree `/work/repo/.git-paw/worktrees/feat-a` and an unrelated surface owned by `/elsewhere/other-feat-a`
- **WHEN** both surfaces are resolved
- **THEN** only the first SHALL resolve to `feat-a`

#### Scenario: A path below a worktree root is not an owner match

- **GIVEN** agent `feat-a` in worktree `/work/repo/.git-paw/worktrees/feat-a` and a surface whose owner the backend reports as `/work/repo/.git-paw/worktrees/feat-a/src`
- **WHEN** the drive loop resolves that surface
- **THEN** it SHALL NOT resolve to `feat-a`

#### Scenario: The dashboard is found by the layout record, not by its owner

- **GIVEN** a broker-enabled session whose dashboard surface and an unrelated surface are both owned by the repository root
- **WHEN** the drive loop resolves the session's surfaces
- **THEN** only the surface at the layout record's dashboard position SHALL resolve as the dashboard
- **AND** the other repository-root surface SHALL resolve as unknown, never as a coding agent

#### Scenario: tmux backend answers the owner query from pane_current_path

- **GIVEN** the tmux backend and a surface whose tmux pane reports `pane_current_path` `/path/to/repo-feat-b`
- **WHEN** the drive loop asks the backend for that surface's owner
- **THEN** the backend SHALL return `/path/to/repo-feat-b`
- **AND** the drive loop's own source SHALL contain no reference to `pane_current_path`
