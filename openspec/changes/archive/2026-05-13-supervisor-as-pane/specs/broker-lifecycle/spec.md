## MODIFIED Requirements

### Requirement: Start flow conditionally creates dashboard pane

When `[broker] enabled = true` in config, the `start` flow SHALL insert a dashboard pane that runs `git paw __dashboard`. The dashboard pane's index depends on whether supervisor mode is active:

- **Bare `git paw start` and `git paw start --from-specs` (no supervisor):** dashboard at pane 0; coding agent panes start at pane 1. Same as v0.4.
- **`git paw start --supervisor` (or `--from-specs --supervisor`):** dashboard at pane 1; supervisor agent at pane 0; coding agent panes start at pane 2. Updated in this change per the new supervisor-as-pane layout.

When `[broker] enabled = false` (or absent), the start flow SHALL behave identically to v0.2.0 with no dashboard pane (and supervisor mode is not meaningful since auto-approve, dashboard, and broker-status all require the broker).

The dashboard pane is in both cases a non-interactive TUI process; it does NOT receive a `tmux send-keys` boot block injection.

#### Scenario: Broker enabled in bare-start mode adds dashboard as pane 0

- **GIVEN** `[broker]\nenabled = true` in config and no supervisor mode
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 4 panes: pane 0 running `git paw __dashboard`, panes 1-3 running coding CLIs

#### Scenario: Broker enabled in supervisor mode places dashboard at pane 1

- **GIVEN** `[broker]\nenabled = true` and `[supervisor]\nenabled = true` in config
- **WHEN** `git paw start --supervisor` launches a session with 3 branches
- **THEN** the tmux session has 5 panes
- **AND** pane 0 SHALL be the supervisor agent (Claude with the supervisor skill as AGENTS.md)
- **AND** pane 1 SHALL be the dashboard (`git paw __dashboard`)
- **AND** panes 2-4 SHALL be the coding CLIs

#### Scenario: Broker disabled produces no dashboard pane

- **GIVEN** no `[broker]` section in config (or `enabled = false`)
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 3 panes, all running coding CLIs (same as v0.2.0)

#### Scenario: Dashboard pane title

- **GIVEN** broker enabled (in either bare-start or supervisor mode)
- **WHEN** the session is created
- **THEN** the dashboard pane has the title `"dashboard"` regardless of its index

### Requirement: Stop flow shuts down broker via pane 0 exit

The `stop` flow SHALL NOT add any broker-specific shutdown logic. Killing the tmux session kills the dashboard pane (pane 0 in bare-start mode, pane 1 in supervisor mode), which causes `run_dashboard` to exit, which drops `BrokerHandle`, which triggers graceful broker shutdown including the final log flush.

In supervisor mode, the dashboard's `__dashboard` subprocess ALSO hosts the auto-approve thread (relocated from `cmd_supervisor` per the new supervisor-as-pane architecture). The auto-approve thread SHALL terminate alongside the dashboard process when the pane is killed.

#### Scenario: Stop kills tmux and broker shuts down

- **GIVEN** an active session with broker enabled (any mode)
- **WHEN** `git paw stop` is executed
- **THEN** the tmux session is killed
- **AND** the broker port is freed within 5 seconds
- **AND** `broker.log` contains a final flush of all messages

#### Scenario: Stop in supervisor mode also terminates auto-approve

- **GIVEN** an active supervisor mode session with `[supervisor.auto_approve] enabled = true`
- **WHEN** `git paw stop` is executed
- **THEN** the auto-approve thread (running inside the `__dashboard` subprocess) terminates alongside the broker shutdown
- **AND** no further auto-approve fires after stop

## ADDED Requirements

### Requirement: Auto-approve thread location in dashboard subprocess

When supervisor mode is active AND `[supervisor.auto_approve] enabled = true`, the auto-approve poll thread SHALL run inside the dashboard's `__dashboard` subprocess (the long-lived process running in the dashboard pane), NOT inside the `cmd_supervisor` process (which returns immediately after launching the session per the new supervisor-as-pane architecture).

The auto-approve thread's responsibilities are unchanged: poll `/status` every `stall_threshold_seconds`, capture stalled panes, classify pending commands, dispatch approve keystrokes for safe commands, escalate unknowns via `agent.question`. Only the host process changes.

#### Scenario: Auto-approve thread runs inside the dashboard subprocess

- **GIVEN** an active supervisor mode session with auto-approve enabled
- **WHEN** the dashboard's `__dashboard` subprocess starts
- **THEN** it spawns the auto-approve poll thread alongside the broker + TUI rendering
- **AND** `cmd_supervisor` SHALL NOT spawn a parallel auto-approve thread

#### Scenario: Auto-approve thread terminates when dashboard pane is killed

- **GIVEN** an active supervisor mode session with auto-approve running
- **WHEN** the user kills the dashboard pane (via `tmux kill-pane` or pane exit)
- **THEN** the `__dashboard` subprocess exits
- **AND** the auto-approve thread terminates with it
- **AND** the broker shuts down (per the existing "Stop flow" requirement)
