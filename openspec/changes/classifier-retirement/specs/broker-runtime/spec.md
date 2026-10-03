## MODIFIED Requirements

### Requirement: Start flow conditionally creates dashboard pane

When `[broker] enabled = true` in config, the `start` flow SHALL insert a dashboard pane that runs `git paw __dashboard`. The dashboard pane's index depends on whether supervisor mode is active:

- **Bare `git paw start` and `git paw start --from-specs` (no supervisor):** dashboard at pane 0; coding agent panes start at pane 1. Same as v0.4.
- **`git paw start --supervisor` (or `--from-specs --supervisor`):** dashboard at pane 1; supervisor agent at pane 0; coding agent panes start at pane 2. Updated in this change per the new supervisor-as-pane layout.

When `[broker] enabled = false` (or absent), the start flow SHALL behave identically to v0.2.0 with no dashboard pane (and supervisor mode is not meaningful since the dashboard, broker status, and the drive loop's escalations all require the broker).

The dashboard pane is in both cases a non-interactive TUI process; it does NOT receive a `tmux send-keys` boot block injection, and it sends no keystrokes to any other pane.

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

## REMOVED Requirements

### Requirement: Auto-approve thread location in dashboard subprocess

**Reason**: The dashboard auto-approve thread is deleted. git-paw no longer approves permission prompts (`approval-auto`, *Approval keystrokes are sent only by an explicit approve action*).
**Migration**: The `__dashboard` process hosts the broker and the TUI only. Under `--unattended` the drive loop escalates live prompts. In attended sessions the orchestrator's `sweep.sh detect-stuck` surfaces stuck-on-prompt agents, and approvals are made explicitly with `sweep.sh approve <pane>`.
