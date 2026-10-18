## MODIFIED Requirements

### Requirement: Manual mode boot block pre-fill

In manual broker mode on a **bare, human-driven launch** — `git paw start` with no spec selection and no supervisor mode — the system SHALL pre-fill the boot instruction block, followed by the worktree rule ("Launch prompts name the agent's worktree root"), into each agent pane's input line without sending an Enter key. This allows users to paste their actual task after the boot instructions. This pre-fill-only behaviour SHALL NOT apply to a spec-driven launch, where the task is supplied by the spec rather than typed by a human and an unsubmitted prompt would strand the agent; a spec-driven launch submits per "Boot-block injection in cmd_start_from_specs".

#### Scenario: Boot block pre-filled without Enter

- **GIVEN** broker-enabled session in manual mode launched with no spec selection
- **WHEN** agent panes are created
- **THEN** boot block SHALL be sent to input line, followed by the worktree rule naming that pane's worktree root
- **AND** no Enter key SHALL be sent
- **AND** cursor SHALL remain at end of the pre-filled text

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

The injected prompt SHALL carry the boot block, the task prompt's prose and the worktree rule. The boot block carries the agent's `BRANCH_ID`, broker URL, and the publish patterns it needs to participate in broker coordination. The task prompt SHALL name the agent's assignment, and the spec's backend SHALL bring the agent into its format-native workflow — when the backend declares a workflow entry point (the apply invocation), that entry point SHALL be submitted as its own line per "A declared workflow entry point is submitted as its own composer line" — so the agent drives the spec workflow — walking its task list and marking tasks off — rather than improvising a private one. Without it an agent may complete the work while leaving every task unchecked, so `openspec status` and any orchestrator reading progress see none.

When `[broker] enabled = false`, no boot-block injection occurs (matching the existing `cmd_start` behaviour — the boot block is broker-specific content). The task prompt's prose and the worktree rule SHALL still be injected and submitted for a spec-driven launch, and a declared entry point still submitted after them, since the agent's assignment does not depend on the broker.

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
- **THEN** it SHALL include the task prompt's prose naming the assignment
- **AND** the prompt SHALL match the one the supervisor path builds for the same spec
- **AND** a backend that declares an entry point SHALL have it submitted as a separate line, not inside the prompt

#### Scenario: Task prompt is injected even with the broker disabled

- **GIVEN** `[broker] enabled = false` and a spec-driven launch without supervisor mode
- **WHEN** agent panes are created
- **THEN** no boot block SHALL be injected
- **AND** the task prompt's prose and the worktree rule SHALL still be injected and submitted

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

## ADDED Requirements

### Requirement: Launch prompts name the agent's worktree root

Every prompt git-paw injects into a coding agent pane at launch — the bare pre-fill, the spec-driven and supervised `start` prompts, the `git paw add` prompt, and a prompt held for an agent added while paused — SHALL end with the worktree rule: a bundled, project-agnostic template that names the absolute path of that agent's worktree root and states that every file the agent creates or edits SHALL lie under that directory, and never outside it — neither by an absolute path nor by a relative path that climbs out of it (such as `../` into the repository's main checkout). The path SHALL be the worktree path the session records for the agent, substituted at launch; the template SHALL name no project, tool or CLI.

#### Scenario: A child worktree's prompt names its absolute root

- **GIVEN** child placement and an agent on `feat/x` whose worktree is `<repo>/.git-paw/worktrees/feat-x`
- **WHEN** its launch prompt is composed
- **THEN** the prompt SHALL end with the worktree rule naming the absolute path `<repo>/.git-paw/worktrees/feat-x`
- **AND** the rule SHALL state that edits outside that directory, including through `../`, are not allowed

#### Scenario: Every launch path carries the rule

- **WHEN** the prompts built by the bare pre-fill, spec-driven `start`, supervised `start`, `git paw add`, and `add` to a paused session are inspected
- **THEN** each SHALL end with the worktree rule naming that agent's recorded worktree path

### Requirement: A declared workflow entry point is submitted as its own composer line

When an agent's task prompt declares a workflow entry point, the launch path that submits its prompt — spec-driven `start`, supervised `start`, `git paw add` to an active session, and the submit of a held prompt when a paused session is restarted — SHALL submit the entry point as a separate line after the prompt has been submitted, as follows:

1. The system SHALL wait, bounded by a fixed budget and for all such panes concurrently, until the pane is idle: no live permission prompt, no mid-response marker, and an input box that is empty or not recognised, judged by the CLI prompt-shape profile (`cli-prompt-profiles`) on two consecutive captures taken after the prompt's `Enter`. A CLI whose markers the profile does not recognise SHALL be treated as idle.
2. Immediately before typing, the system SHALL capture the pane again and SHALL type nothing unless it is still idle.
3. The system SHALL then inject the entry point literally and submit it with a separate `Enter` after the settle delay, using the split-send mechanism (`cli-resolution`).
4. When the budget elapses before the pane is idle, the system SHALL NOT type into the pane, SHALL print to stderr the pane, the agent and the exact entry point to submit once the agent is idle, and SHALL continue the launch.

An agent added while the session is paused SHALL hold its entry point together with its held prompt, in an additive optional field of the session record. Restarting the session SHALL submit the held prompt and then the held entry point under the rule above; when the held prompt stays held (its pane is blocked on a first-run dialog), the entry point SHALL stay held with it. The system SHALL NOT type an entry point into a pane whose task prompt declares none, and the entry point's text SHALL come only from the backend's bundled template.

#### Scenario: The OpenSpec entry point is submitted after the first message

- **GIVEN** a spec-driven launch of an OpenSpec change `my-change` whose agent's CLI finishes its first turn within the budget
- **WHEN** the launch submits the agent's prompt
- **THEN** the pane SHALL receive the composed prompt and its `Enter`
- **AND** after the pane is idle, the line `/opsx:apply my-change` typed literally and its own separate `Enter`

#### Scenario: A pane at a permission prompt gets nothing typed

- **GIVEN** a spec-driven launch whose agent pane shows a live permission prompt for the whole budget
- **WHEN** the budget elapses
- **THEN** no keystroke SHALL be sent to that pane after the prompt's `Enter`
- **AND** stderr SHALL name the pane and `/opsx:apply my-change`

#### Scenario: A busy pane is not typed into

- **GIVEN** an agent pane showing the mid-response marker of its CLI profile
- **WHEN** the system waits to submit the entry point
- **THEN** it SHALL NOT type until two consecutive captures show the pane idle

#### Scenario: A backend without an entry point is unchanged

- **GIVEN** a spec-driven launch of a Markdown, Spec Kit or Superpowers spec
- **WHEN** the agent's prompt is submitted
- **THEN** no further line SHALL be typed into its pane and the launch SHALL NOT wait for it to become idle

#### Scenario: An agent added while paused holds its entry point

- **GIVEN** a paused session
- **WHEN** `git paw add feat/x --from-spec my-change` is run and then `git paw start` restarts the session
- **THEN** the session record SHALL hold the entry point `/opsx:apply my-change` until the restart
- **AND** the restart SHALL submit the held prompt, then the entry point once the pane is idle, and clear both

#### Scenario: A session saved without the field restarts as before

- **GIVEN** a paused session saved by an earlier binary, with a held prompt and no held entry point
- **WHEN** `git paw start` restarts it
- **THEN** the held prompt SHALL be submitted and no entry point typed
