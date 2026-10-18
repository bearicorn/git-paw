## ADDED Requirements

### Requirement: Hot-attach works on every grid session

The `add` and `remove` subcommands SHALL operate on every live session whose recorded pane layout is the agent grid — bare, spec-driven and supervised alike — and on supervised sessions saved by earlier binaries, using the session's recorded first coding-agent index. They SHALL NOT refuse a session because of its `SessionMode`. A live session saved with the earlier tiled layout (a bare session with no recorded grid layout) SHALL be refused before any worktree, pane or session state is touched, with an error that names the tiled layout as the reason and tells the user to stop the session and run `git paw start` to relaunch it in the grid layout; the message SHALL NOT cite a release version.

#### Scenario: Add to a bare grid session

- **GIVEN** a live bare session (no supervisor mode) started with two branches and the broker enabled
- **WHEN** the user runs `git paw add feat/third`
- **THEN** a third agent pane SHALL be created at pane 3 running the session's CLI in the new worktree
- **AND** the agent grid SHALL match what a 3-agent `start` would produce

#### Scenario: Remove from a bare grid session

- **GIVEN** a live bare grid session with agents `feat/a`, `feat/b`, `feat/c`
- **WHEN** the user runs `git paw remove feat/b`
- **THEN** only `feat/b`'s pane SHALL be closed and the remaining grid SHALL re-flow to the 2-agent layout

#### Scenario: Add to a broker-less grid session

- **GIVEN** a live session with the broker disabled whose agents start at pane 0
- **WHEN** the user runs `git paw add feat/new`
- **THEN** the new agent SHALL receive the next pane index after the existing agents

#### Scenario: An earlier tiled session is refused explicitly

- **GIVEN** a live bare session saved without a recorded grid layout
- **WHEN** the user runs `git paw add feat/x` or `git paw remove feat/y`
- **THEN** the command SHALL exit non-zero with an error naming the tiled layout and telling the user to stop and start the session again
- **AND** the error SHALL NOT contain a version string such as `v0.6.0`
- **AND** no worktree, pane or session entry SHALL be created or removed

#### Scenario: A supervised session saved earlier stays hot-attachable

- **GIVEN** a live supervised session saved by an earlier binary with `dashboard_pane = Some(1)` and agents from pane 2
- **WHEN** the user runs `git paw add feat/new`
- **THEN** the new agent SHALL be added after the existing agents and the existing agents' panes SHALL keep their indices

## MODIFIED Requirements

### Requirement: Pane spawn with grid re-tiling

When adding a pane, the system SHALL recompute the agent-grid layout for the new total agent count and re-apply it so all panes match the layout `git paw start` would have produced for that count. Existing agent panes SHALL retain their pane indices so that in-flight `send-keys` targeting continues to address the correct panes.

The re-tile SHALL preserve every OTHER agent's pane: adding an agent SHALL NOT close, drop, or orphan any existing agent's pane, and after the re-tile the live tmux window SHALL contain exactly one pane per session-JSON agent plus the dashboard pane when the session has one (the v0.8.0 G2 dogfood failure dropped a different agent's pane during the re-tile). After re-tiling, each agent row SHALL be rebalanced to equal width per the `tmux-orchestration` "Supervisor-mode pane layout" requirement, so an incrementally-added grid matches a start-time grid of the same agent count in both pane count and pane widths. The first coding-agent index SHALL come from the session's recorded layout, not from a mode constant.

#### Scenario: Grid re-tiles for the new agent count

- **GIVEN** an active session with 4 agent panes (single row)
- **WHEN** the user runs `git paw add feat/fifth`
- **THEN** the agent grid SHALL be laid out as the 5-pane layout (single row of 5), matching what a 5-agent `start` would produce

#### Scenario: Existing pane indices are preserved

- **GIVEN** an active session whose agents occupy pane indices 1 through 5
- **WHEN** the user runs `git paw add feat/new`
- **THEN** the existing panes SHALL retain indices 1 through 5 and the new pane SHALL receive index 6, verified by `pane_current_path` mapping

#### Scenario: No existing agent pane is dropped by the re-tile

- **GIVEN** an active session with 3 agent panes mapped to their worktrees via `pane_current_path`
- **WHEN** the user runs `git paw add feat/fourth`
- **THEN** the live tmux window SHALL contain a pane for each of the original 3 agents plus the new agent (4 agent panes total), with no original agent left without a pane

#### Scenario: Added grid matches a start-time grid width-for-width

- **GIVEN** an active session with 2 agent panes
- **WHEN** the user runs `git paw add feat/third` and the re-tile is applied to a live tmux window
- **THEN** the 3 agent panes SHALL each render at approximately one third of the window width (within a one-column rounding tolerance), matching what a 3-agent `start` would produce

#### Scenario: Adding past the agent cap is rejected

- **GIVEN** an active session already at the 25-agent cap
- **WHEN** the user runs `git paw add feat/twenty-six`
- **THEN** the command SHALL exit non-zero with the same "split into multiple sessions" message `start` uses, and SHALL NOT create a worktree or pane

### Requirement: Pane closure with grid re-tiling

When removing an agent, the system SHALL kill the agent's tmux pane and re-apply the agent-grid layout for the new (smaller) agent count so the grid re-flows without a hole. Remaining panes' relative order SHALL be preserved.

The system SHALL resolve the target pane by mapping the removed branch's worktree to a live pane via `pane_current_path` and SHALL kill that pane by its tmux pane id, regardless of the process running in it — a bare shell (a failed/never-started CLI), a CLI, or any other process. Killing by resolved pane id (not by a position computed from the session JSON) ensures a failed agent whose pane never launched a CLI is still closed rather than orphaned, and that the kill targets the removed agent's pane and never a different agent's pane (the v0.8.0 G2 dogfood failure killed/dropped a different agent's pane because the index was computed from JSON position while a stale orphan pane shifted the grid). The re-tile SHALL preserve every OTHER agent's pane: after the removal the live tmux window SHALL contain exactly one pane per remaining session-JSON agent plus the dashboard pane when the session has one, and each agent row SHALL be rebalanced to equal width per the `tmux-orchestration` "Supervisor-mode pane layout" requirement.

#### Scenario: Grid re-flows after a removal

- **GIVEN** an active session with 5 agent panes (single row)
- **WHEN** the user runs `git paw remove feat/middle`
- **THEN** the agent grid SHALL be laid out as the 4-pane layout, matching what a 4-agent `start` would produce, and the order of the remaining 4 agents SHALL be preserved

#### Scenario: Branch→pane mapping is re-derived after removal

- **GIVEN** an active session whose branch→pane mapping was established via `pane_current_path`
- **WHEN** an agent in the middle of the grid is removed
- **THEN** subsequent sweeps by the drive loop or the orchestrator's helpers SHALL re-derive the branch→pane mapping via `pane_current_path` and SHALL continue to target the correct panes for the remaining agents

#### Scenario: Removing a failed shell-occupied pane still closes it

- **GIVEN** an active session whose agent `feat/x` pane is a bare shell (its CLI never started, the v0.8.0 G1 condition)
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the system SHALL resolve `feat/x`'s pane via `pane_current_path` and kill it by pane id, leaving no orphan pane, even though the pane is running a shell rather than the expected CLI

#### Scenario: Removal does not drop a different agent's pane

- **GIVEN** an active session with agents `feat/a`, `feat/b`, `feat/c` each mapped to a live pane via `pane_current_path`
- **WHEN** the user runs `git paw remove feat/b`
- **THEN** only `feat/b`'s pane SHALL be killed
- **AND** `feat/a` and `feat/c` SHALL each still have exactly one live pane after the re-tile (no collateral pane loss, no orphan)

### Requirement: Supervisor discovers the added agent passively

The system SHALL NOT directly signal the orchestrator or the drive loop on add. The external orchestrator SHALL discover the new agent through the broker `/status` roster and the per-repo discovery file, and an in-process drive loop SHALL pick it up on its next sweep from the same sources, without the `add` command restarting or prompting either.

#### Scenario: The new agent appears in the roster sources

- **GIVEN** an active session
- **WHEN** an agent is added
- **THEN** the per-repo discovery file SHALL list the new agent with its pane index
- **AND** the broker watcher SHALL be registered for the new worktree so the agent appears in `/status`
- **AND** no keystroke SHALL be sent to any surface other than the new agent's pane

#### Scenario: A running drive loop sweeps the new agent

- **GIVEN** an `--unattended` session whose drive loop is running
- **WHEN** an agent is added and later shows a live permission prompt
- **THEN** the drive loop SHALL escalate it as a `permission-prompt` naming the added agent on a subsequent sweep
- **AND** SHALL send no keystroke answering it

### Requirement: Supervisor discovers removal passively

The system SHALL NOT directly signal the orchestrator or the drive loop on remove. The removed agent SHALL disappear from the per-repo discovery file immediately, and the external orchestrator and any running drive loop SHALL stop acting on it from their next read of the roster sources.

#### Scenario: The removed agent leaves the roster sources

- **GIVEN** an active session containing `feat/x`
- **WHEN** the user runs `git paw remove feat/x`
- **THEN** the per-repo discovery file SHALL no longer list `feat/x`
- **AND** no keystroke SHALL be sent to any surface as part of the removal

#### Scenario: A running drive loop stops sweeping the removed agent

- **GIVEN** an `--unattended` session whose drive loop is running and contains `feat/x`
- **WHEN** `feat/x` is removed
- **THEN** the drive loop SHALL send no further keystrokes attributed to `feat/x`

## REMOVED Requirements

### Requirement: Bare-session add and remove refusal states the limitation plainly

**Reason**: `add` and `remove` now work on every session built with the agent grid, bare and supervised alike (*Hot-attach works on every grid session*), so refusing a session because it is bare contradicts the new behavior. The only refusal left is for a live session saved with the earlier tiled layout, which names the layout rather than the session mode.

**Migration**: Bare sessions started by this version accept `git paw add` / `git paw remove` directly. A live bare session started by an earlier binary (tiled layout) is refused with an error that names the tiled layout and tells the user to stop the session and run `git paw start` again; like the removed refusal, it cites no release version. Recovery of a stopped tiled session rebuilds it as a grid.
