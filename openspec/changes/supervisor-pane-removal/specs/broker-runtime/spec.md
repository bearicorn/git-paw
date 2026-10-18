## MODIFIED Requirements

### Requirement: Start flow conditionally creates dashboard pane

When `[broker] enabled = true` in config, the `start` flow SHALL insert a dashboard pane that runs `git paw __dashboard` at pane 0 in every mode:

- **Bare `git paw start` and `git paw start --from-specs` (no supervisor):** dashboard at pane 0; coding agent panes start at pane 1. Same as v0.4.
- **`git paw start --supervisor` (or `--from-specs --supervisor`, or `[supervisor] enabled = true`):** dashboard at pane 0 as the full-width top row; coding agent panes start at pane 1 in the supervised grid. No orchestrator pane is created.

When `[broker] enabled = false` (or absent), the start flow SHALL behave identically to v0.2.0 with no dashboard pane (and supervised mode is not meaningful since the dashboard, broker status, and the drive loop's escalations all require the broker).

The dashboard pane is in both cases a non-interactive TUI process; it does NOT receive a `tmux send-keys` boot block injection, and it sends no keystrokes to any other pane.

#### Scenario: Broker enabled in bare-start mode adds dashboard as pane 0

- **GIVEN** `[broker]\nenabled = true` in config and no supervisor mode
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 4 panes: pane 0 running `git paw __dashboard`, panes 1-3 running coding CLIs

#### Scenario: Broker enabled in supervised mode also places dashboard at pane 0

- **GIVEN** `[broker]\nenabled = true` and `[supervisor]\nenabled = true` in config
- **WHEN** `git paw start --supervisor` launches a session with 3 branches
- **THEN** the tmux session has 4 panes
- **AND** pane 0 SHALL be the dashboard (`git paw __dashboard`)
- **AND** panes 1-3 SHALL be the coding CLIs

#### Scenario: Broker disabled produces no dashboard pane

- **GIVEN** no `[broker]` section in config (or `enabled = false`)
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 3 panes, all running coding CLIs (same as v0.2.0)

#### Scenario: Dashboard pane title

- **GIVEN** broker enabled (in either bare-start or supervised mode)
- **WHEN** the session is created
- **THEN** the dashboard pane has the title `"dashboard"`
