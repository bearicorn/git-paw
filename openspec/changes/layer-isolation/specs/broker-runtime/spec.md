## MODIFIED Requirements

### Requirement: Start flow conditionally creates dashboard pane

The `start` flow SHALL insert a dashboard pane that runs `git paw __dashboard` at pane 0 only when the dashboard is enabled for the launch AND the session's resolved layers include the Broker layer. The Broker layer is present when `[broker] enabled = true` or when the session is coordinated, per `core-layer-isolation`. The rule applies to bare `git paw start`, spec-driven `start` and coordinated `start` (`--coordinated`, or `default_mode = "coordinated"`).

The dashboard is enabled for a launch when `git paw start --dashboard` is passed, or otherwise when the merged configuration sets `[dashboard] enabled = true`. It is disabled by default. The dashboard is an optional view of the Broker layer, not a layer of its own.

- **With a dashboard:** the dashboard is at pane 0 as the full-width top row, and the coding-agent panes start at pane 1 in the agent grid.
- **Without a dashboard:** the coding-agent panes start at pane 0 and the agent grid uses the whole window. This is also the case when the session has the Broker layer.

The panes above are the panes of the session's window 0. When the session runs the broker host it also has a detached `broker` service window holding the host; it holds no agent and is not counted among the session's panes.

When `--dashboard` is passed and the session's layers do not include Broker, the start flow SHALL fail before creating any worktree, pane or session state, with an error stating that the dashboard needs the broker and naming `[broker] enabled` and coordinated mode. When the dashboard is enabled only by configuration and the session's layers do not include Broker, the start flow SHALL create no dashboard pane and SHALL NOT fail. When the session's layers do not include Broker, the start flow SHALL otherwise behave as it does without the broker.

The dashboard pane is a non-interactive TUI process; it does NOT receive a `tmux send-keys` boot block injection, and it sends no keystrokes to any other pane. Recovery and restart from pause SHALL rebuild the dashboard pane the session recorded (per `session-state`), not the one the current flag or configuration would select.

#### Scenario: A coordinated session has no dashboard by default

- **GIVEN** a version-2 config with `[broker] enabled = true`, `default_mode = "coordinated"` and no `[dashboard] enabled`
- **WHEN** `git paw start --coordinated` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 3 panes, panes 0-2 running the coding CLIs
- **AND** no pane runs `git paw __dashboard`

#### Scenario: A bare broker session has no dashboard by default

- **GIVEN** a version-2 config with `[broker] enabled = true` and no `[dashboard] enabled`
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 3 panes, all running coding CLIs

#### Scenario: The config key enables the dashboard in a bare broker session

- **GIVEN** `[broker] enabled = true`, `[dashboard] enabled = true` and coordinated mode not selected
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 4 panes: pane 0 running `git paw __dashboard`, panes 1-3 running coding CLIs

#### Scenario: The flag enables the dashboard in a coordinated session

- **GIVEN** `[broker] enabled = true` and no `[dashboard] enabled`
- **WHEN** `git paw start --coordinated --dashboard` launches a session with 3 branches
- **THEN** pane 0 SHALL be the dashboard (`git paw __dashboard`) and panes 1-3 SHALL be the coding CLIs

#### Scenario: A coordinated session without a broker section can show the dashboard

- **GIVEN** `default_mode = "coordinated"`, `[dashboard] enabled = true` and no `[broker]` section in config
- **WHEN** `git paw start --coordinated` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 4 panes with pane 0 running `git paw __dashboard`

#### Scenario: The flag without the Broker layer fails before launching

- **GIVEN** no `[broker]` section in config and coordinated mode not selected
- **WHEN** `git paw start --dashboard --branches feat/a` runs
- **THEN** the command SHALL exit non-zero with an error naming `[broker] enabled` and coordinated mode
- **AND** no worktree, tmux session or session state SHALL be created

#### Scenario: The config key without the Broker layer adds no pane

- **GIVEN** `[dashboard] enabled = true`, no `[broker]` section in config and coordinated mode not selected
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 3 panes, all running coding CLIs, and the launch SHALL succeed

#### Scenario: Broker disabled produces no dashboard pane

- **GIVEN** no `[broker]` section in config (or `enabled = false`) and coordinated mode not selected
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 3 panes, all running coding CLIs

#### Scenario: Dashboard pane title

- **GIVEN** a launch that creates a dashboard pane (in either mode)
- **WHEN** the session is created
- **THEN** the dashboard pane has the title `"dashboard"`
