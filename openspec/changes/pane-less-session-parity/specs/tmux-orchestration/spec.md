## ADDED Requirements

### Requirement: Every pane-less session uses the agent grid

Every launch path — bare `start`, spec-driven `start`, and supervised `start` — and the recovery of a stopped session SHALL build the window with the agent-grid geometry defined by "Supervisor-mode pane layout": the dashboard as the full-width top row when the broker is enabled, coding agents row-major below at no more than five per row, each row rebalanced to equal width, and the 25-agent cap. When the session has no dashboard (broker disabled), there SHALL be no top row: coding agents SHALL start at pane 0 and the agent rows SHALL share the full window height evenly. The whole-window `select-layout tiled` and `main-horizontal` layouts SHALL NOT be used to arrange coding agents.

#### Scenario: Bare start builds the grid

- **GIVEN** `[broker] enabled = true` and no supervisor mode
- **WHEN** `git paw start` launches 7 branches
- **THEN** pane 0 SHALL be the dashboard spanning the full width
- **AND** panes 1-5 SHALL form the first agent row and panes 6-7 the second, each row at equal widths

#### Scenario: Spec-driven start builds the grid

- **GIVEN** a spec-driven `git paw start --specs a b c` without supervisor mode and the broker enabled
- **WHEN** the session is built
- **THEN** pane 0 SHALL be the dashboard and panes 1-3 SHALL be a single equal-width agent row

#### Scenario: Broker-less start uses the full height

- **GIVEN** `[broker] enabled = false`
- **WHEN** `git paw start` launches 6 branches
- **THEN** there SHALL be no dashboard pane
- **AND** panes 0-4 SHALL form the first agent row and pane 5 the second, the two rows sharing the full height evenly

#### Scenario: Recovery rebuilds a grid

- **GIVEN** a stopped bare session saved with the earlier tiled layout and 3 recorded worktrees with the broker enabled
- **WHEN** `git paw start` recovers it
- **THEN** the rebuilt window SHALL be the grid (dashboard row + one agent row of 3)
- **AND** the re-saved session SHALL record the grid layout

## MODIFIED Requirements

### Requirement: Launch-readiness gate before boot-block injection

The system SHALL verify that a pane's CLI process has actually started and reached an interactive ready state before injecting any boot block, framing prompt, or task prompt into that pane, rather than relying solely on a fixed wall-clock sleep. The system SHALL poll the pane (via `tmux capture-pane`) for a CLI-readiness marker — content that distinguishes the launched CLI's interactive UI from a bare shell prompt — up to a bounded timeout. The boot block SHALL NOT be injected while the pane is still a bare shell, because the shell would interpret the multi-line boot block line-by-line as failing commands (the v0.8.0 G1 dogfood failure).

If the readiness timeout elapses while the pane is still a bare shell, the system SHALL relaunch the CLI command into that pane (re-clearing the input line first) and poll again, up to a bounded number of relaunch attempts. The readiness gate and its timeout/attempt budget SHALL apply uniformly to EVERY launch path that injects text into an agent pane: bare `start` (which then pre-fills without submitting), spec-driven `start`, supervised `start`, `git paw add`, and the submission of a held boot prompt when a paused session is restarted. A pane the gate reports as blocked on an unanswered first-run dialog SHALL NOT receive the injected text and SHALL fail that launch path loudly (per "Launch gate handles a first-run permission/trust acceptance dialog"); the one exception is restart from pause, which SHALL keep that pane's held prompt for the next restart, report the blocked pane on stderr, and continue restarting the session.

The readiness gate SHALL be conservative: a CLI that the gate cannot positively classify within the budget (e.g. an unrecognised custom CLI whose UI the marker heuristic does not match) SHALL fall back to injecting the boot block after the budget elapses rather than failing the launch, so launch behaviour is never worse than the previous fixed-sleep behaviour for an unrecognised CLI.

#### Scenario: Boot block withheld until the pane is CLI-ready

- **GIVEN** a freshly split agent pane whose CLI process has not yet reached its interactive prompt
- **WHEN** the launch flow reaches the boot-block injection step for that pane
- **THEN** the system SHALL poll the pane for a CLI-readiness marker and SHALL NOT inject the boot block while the pane still shows only a bare shell prompt

#### Scenario: Boot block injected once the CLI is ready

- **GIVEN** an agent pane whose CLI has reached its interactive ready state within the readiness timeout
- **WHEN** the readiness poll observes the CLI-readiness marker
- **THEN** the system SHALL inject the boot block into that pane exactly once

#### Scenario: CLI relaunched when the pane is still a bare shell

- **GIVEN** an agent pane whose CLI never started (the pane is still a bare shell when the readiness timeout elapses)
- **WHEN** the readiness gate exhausts its timeout for that pane
- **THEN** the system SHALL relaunch the CLI command into that pane and poll for readiness again, up to the relaunch-attempt budget, before falling back to injection

#### Scenario: Unrecognised CLI falls back to fixed-budget injection

- **GIVEN** an agent pane running a custom CLI whose interactive UI does not match any known readiness marker
- **WHEN** the readiness budget elapses without a positive readiness classification
- **THEN** the system SHALL inject the boot block after the budget rather than rejecting the launch, so behaviour is no worse than the prior fixed-sleep launch for an unrecognised CLI

#### Scenario: Bare start is gated

- **GIVEN** a bare `git paw start` with the broker enabled whose first agent pane is still a bare shell when the launch reaches injection
- **WHEN** the launch injects the boot block pre-fill
- **THEN** the gate SHALL run first and the pre-fill SHALL NOT be typed into the bare shell

#### Scenario: Spec-driven start stuck on a first-run dialog fails loudly

- **GIVEN** a spec-driven `git paw start --specs a` whose agent pane is blocked on an unanswered first-run acceptance dialog
- **WHEN** the launch reaches injection for that pane
- **THEN** the launch SHALL fail with an error naming the pane and the attach command
- **AND** the boot block SHALL NOT be typed on top of the dialog

#### Scenario: Restart from pause gates a held prompt

- **GIVEN** a paused session holding a boot prompt for an agent added while paused, whose pane is blocked on a first-run dialog
- **WHEN** `git paw start` restarts the session
- **THEN** the held prompt SHALL NOT be submitted to that pane
- **AND** it SHALL remain held for the next restart
- **AND** stderr SHALL name the blocked pane while the rest of the session restarts
