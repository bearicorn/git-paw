## MODIFIED Requirements

### Requirement: Manual mode injection timing

The system SHALL inject boot blocks in manual mode immediately after tmux session creation, before returning control to the user.

#### Scenario: Boot blocks injected during session setup

- **GIVEN** `git paw start --from-all-specs --cli claude` (manual mode)
- **WHEN** tmux session is created
- **THEN** boot blocks SHALL be pre-filled before command returns

### Requirement: Boot-block injection in cmd_start_from_specs

When `git paw start --from-all-specs` is invoked in bare mode (the bare spec-driven path, routed by the dispatcher per `cli-parsing` to `cmd_start_from_specs`) AND `[broker] enabled = true` is set in config, the system SHALL inject a broker boot block into each coding agent pane after the tmux session is executed, and SHALL submit it.

The injection SHALL use the split-send + settle-delay submission mechanism required by `cli-resolution` ("Boot prompt submitted via split-send + settle delay"): the prompt text is sent literally, then `Enter` is sent as a separate keystroke after the per-CLI settle delay. A spec-driven launch SHALL NOT leave the prompt unsubmitted.

The injected prompt SHALL carry the boot block, the task prompt's prose and the worktree rule. The boot block carries the agent's `BRANCH_ID`, broker URL, and the publish patterns it needs to participate in broker coordination. The task prompt SHALL name the agent's assignment, and the spec's backend SHALL bring the agent into its format-native workflow — when the backend declares a workflow entry point (the apply invocation), that entry point SHALL be submitted as its own line per "A declared workflow entry point is submitted as its own composer line" — so the agent drives the spec workflow — walking its task list and marking tasks off — rather than improvising a private one. Without it an agent may complete the work while leaving every task unchecked, so `openspec status` and any orchestrator reading progress see none.

When `[broker] enabled = false`, no boot-block injection occurs (matching the existing `cmd_start` behaviour — the boot block is broker-specific content). The task prompt's prose and the worktree rule SHALL still be injected and submitted for a spec-driven launch, and a declared entry point still submitted after them, since the agent's assignment does not depend on the broker.

#### Scenario: Boot block is injected per agent pane in spec-mode-with-broker

- **GIVEN** `[broker] enabled = true` and the start mode resolves to bare (spec-mode-only)
- **AND** three pending spec changes are discovered
- **WHEN** `git paw start --from-all-specs` is invoked
- **THEN** the system SHALL inject once per spec pane (panes 1, 2, 3 with broker enabled and dashboard at pane 0)
- **AND** each injection SHALL carry the boot block produced for that pane's branch

#### Scenario: The spec-driven boot prompt is submitted

- **GIVEN** a bare-mode spec-driven launch
- **WHEN** the boot prompt has been injected into an agent pane
- **THEN** a separate `Enter` SHALL be sent after the settle delay
- **AND** the pane SHALL NOT be left with an unsubmitted prompt in its input line

#### Scenario: The task prompt is injected alongside the boot block

- **GIVEN** a bare-mode spec-driven launch, for a spec with a known backend
- **WHEN** the agent's prompt is built
- **THEN** it SHALL include the task prompt's prose naming the assignment
- **AND** the prompt SHALL match the one the coordinated launch path builds for the same spec
- **AND** a backend that declares an entry point SHALL have it submitted as a separate line, not inside the prompt

#### Scenario: Task prompt is injected even with the broker disabled

- **GIVEN** `[broker] enabled = false` and a bare-mode spec-driven launch
- **WHEN** agent panes are created
- **THEN** no boot block SHALL be injected
- **AND** the task prompt's prose and the worktree rule SHALL still be injected and submitted

#### Scenario: No boot-block injection when broker is disabled

- **GIVEN** `[broker] enabled = false`
- **AND** spec changes are discovered
- **WHEN** `git paw start --from-all-specs` is invoked
- **THEN** no broker boot block SHALL be injected
- **AND** the launch SHALL still proceed (panes are created, just without broker boot blocks)

#### Scenario: Boot-block injection failure is non-fatal

- **GIVEN** `[broker] enabled = true` and a pending spec change
- **AND** the underlying `tmux send-keys` invocation returns a non-zero exit (simulating a transient tmux issue)
- **WHEN** `git paw start --from-all-specs` is invoked
- **THEN** the launch SHALL proceed without erroring out
- **AND** the session SHALL still be saved
- **AND** the user SHALL still be guided to attach (per the non-TTY handling requirement in `cli-parsing`, or the actual attach when TTY is present)

#### Scenario: Pane offset accounts for dashboard

- **GIVEN** `[broker] enabled = true` and N pending spec changes
- **WHEN** the launch flow injects boot blocks
- **THEN** the first spec's boot block SHALL target pane index `1` (dashboard is at index `0`)
- **AND** the Nth spec's boot block SHALL target pane index `N`
