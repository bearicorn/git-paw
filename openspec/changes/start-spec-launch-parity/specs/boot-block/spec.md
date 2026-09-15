## MODIFIED Requirements

### Requirement: Manual mode boot block pre-fill

In manual broker mode on a **bare, human-driven launch** — `git paw start` with no spec selection and no supervisor mode — the system SHALL pre-fill the boot instruction block into each agent pane's input line without sending an Enter key. This allows users to paste their actual task after the boot instructions. This pre-fill-only behaviour SHALL NOT apply to a spec-driven launch, where the task is supplied by the spec rather than typed by a human and an unsubmitted prompt would strand the agent; a spec-driven launch submits per "Boot-block injection in cmd_start_from_specs".

#### Scenario: Boot block pre-filled without Enter

- **GIVEN** broker-enabled session in manual mode launched with no spec selection
- **WHEN** agent panes are created
- **THEN** boot block SHALL be sent to input line
- **AND** no Enter key SHALL be sent
- **AND** cursor SHALL remain at end of boot block

#### Scenario: User can append task after boot block

- **GIVEN** boot block pre-filled in agent pane
- **WHEN** user pastes task instructions
- **THEN** task appears after boot block
- **AND** user can press Enter to submit combined content

#### Scenario: Spec-driven launch is not pre-fill-only

- **GIVEN** broker-enabled session launched with `--specs` or `--from-all-specs` and no supervisor mode
- **WHEN** agent panes are created
- **THEN** the boot prompt SHALL be submitted rather than left pre-filled

### Requirement: Boot-block injection in cmd_start_from_specs

When `git paw start --from-specs` is invoked WITHOUT supervisor mode (the bare from-specs path, routed by the dispatcher per `cli-parsing` to `cmd_start_from_specs`) AND `[broker] enabled = true` is set in config, the system SHALL inject a broker boot block into each coding agent pane after the tmux session is executed, and SHALL submit it.

The injection SHALL use the split-send + settle-delay submission mechanism required by `cli-resolution` ("Boot prompt submitted via split-send + settle delay"): the prompt text is sent literally, then `Enter` is sent as a separate keystroke after the per-CLI settle delay. A spec-driven launch SHALL NOT leave the prompt unsubmitted.

The injected prompt SHALL carry both the boot block and the task prompt. The boot block carries the agent's `BRANCH_ID`, broker URL, and the publish patterns it needs to participate in broker coordination. The task prompt SHALL name the agent's assignment and the format-native apply invocation for the spec's backend, so the agent drives the spec workflow — walking its task list and marking tasks off — rather than improvising a private one. Without it an agent may complete the work while leaving every task unchecked, so `openspec status` and any orchestrator reading progress see none.

When `[broker] enabled = false`, no boot-block injection occurs (matching the existing `cmd_start` behaviour — the boot block is broker-specific content). The task prompt SHALL still be injected and submitted for a spec-driven launch, since the agent's assignment does not depend on the broker.

#### Scenario: Boot block is injected per agent pane in spec-mode-with-broker

- **GIVEN** `[broker] enabled = true` and `[supervisor]` is not configured (spec-mode-only)
- **AND** three pending spec changes are discovered
- **WHEN** `git paw start --from-specs` is invoked
- **THEN** the system SHALL inject once per spec pane (panes 1, 2, 3 with broker enabled and dashboard at pane 0)
- **AND** each injection SHALL carry the boot block produced for that pane's branch

#### Scenario: The spec-driven boot prompt is submitted

- **GIVEN** a spec-driven launch without supervisor mode
- **WHEN** the boot prompt has been injected into an agent pane
- **THEN** a separate `Enter` SHALL be sent after the settle delay
- **AND** the pane SHALL NOT be left with an unsubmitted prompt in its input line

#### Scenario: The task prompt is injected alongside the boot block

- **GIVEN** a spec-driven launch without supervisor mode, for a spec with a known backend
- **WHEN** the agent's prompt is built
- **THEN** it SHALL include the task prompt naming the assignment and the backend's apply invocation
- **AND** the prompt SHALL match the one the supervisor path builds for the same spec

#### Scenario: Task prompt is injected even with the broker disabled

- **GIVEN** `[broker] enabled = false` and a spec-driven launch without supervisor mode
- **WHEN** agent panes are created
- **THEN** no boot block SHALL be injected
- **AND** the task prompt SHALL still be injected and submitted

#### Scenario: No boot-block injection when broker is disabled

- **GIVEN** `[broker] enabled = false`
- **AND** spec changes are discovered
- **WHEN** `git paw start --from-specs` is invoked
- **THEN** no broker boot block SHALL be injected
- **AND** the launch SHALL still proceed (panes are created, just without broker boot blocks)

#### Scenario: Boot-block injection failure is non-fatal

- **GIVEN** `[broker] enabled = true` and a pending spec change
- **AND** the underlying `tmux send-keys` invocation returns a non-zero exit (simulating a transient tmux issue)
- **WHEN** `git paw start --from-specs` is invoked
- **THEN** the launch SHALL proceed without erroring out
- **AND** the session SHALL still be saved
- **AND** the user SHALL still be guided to attach (per the non-TTY handling requirement in `cli-parsing`, or the actual attach when TTY is present)

#### Scenario: Pane offset accounts for dashboard

- **GIVEN** `[broker] enabled = true` and N pending spec changes
- **WHEN** the launch flow injects boot blocks
- **THEN** the first spec's boot block SHALL target pane index `1` (dashboard is at index `0`)
- **AND** the Nth spec's boot block SHALL target pane index `N`
