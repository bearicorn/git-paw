## MODIFIED Requirements

### Requirement: Supervisor auto-start flow

The system SHALL implement `cmd_supervisor()` in `src/main.rs` that orchestrates the full supervisor session launch. The function SHALL execute the following steps in order:

1. Load config and resolve the supervisor CLI from `[supervisor]` config.
2. Scan specs via `--from-specs` or resolve branches from flags.
3. **Hard-cap check**: agent count SHALL NOT exceed 25; above this, return a `PawError` with an actionable "split into multiple sessions" hint. (Configurable layout deferred to v1.0.0.)
4. **Compute layout proportions**: based on agent count, derive `agents_per_row = 5`, `agent_rows = ceil(agents / 5)`, `total_rows = agent_rows + 1`, and the top-row / agent-row height percentages from the layout table.
5. Create worktrees for each branch (with `-b` fallback for new branches).
6. Generate per-worktree AGENTS.md with spec content, file ownership, coordination skill, and inter-agent rules.
7. **Build the tmux session with the new pane structure**: pane 0 = supervisor agent (Claude in `repo_root` with the supervisor skill as AGENTS.md), pane 1 = dashboard (`git-paw __dashboard` in `repo_root`), panes 2..N+1 = coding agents. Top row splits 50/50 horizontally between supervisor and dashboard. Agent grid below uses up to `agents_per_row` columns per row, with row heights matching the layout table.
8. Inject `GIT_PAW_BROKER_URL` via `tmux set-environment` before pane creation.
9. For each agent pane: construct the CLI launch command with approval flags from `approval_flags(cli, level)`.
10. Execute the tmux session in detached mode.
11. Wait approximately 2 seconds for panes to boot.
12. **Inject the initial prompt for ALL panes including the supervisor (pane 0)** via `tmux send-keys`. The supervisor's initial prompt is its boot block + a "Begin observing" message.
13. **Self-register the supervisor in the broker** via an HTTP POST publishing `agent.status` with `agent_id = "supervisor"`, `status = "working"`, `message = "Supervisor booting"`.
14. Save session state.
15. **Print an attach-hint and return `Ok(())`**: `cmd_supervisor()` does NOT block on a foreground supervisor CLI any more. The user runs `tmux attach -t paw-<project>` to interact with the supervisor pane.

The Rust merge loop SHALL NOT be invoked from `cmd_supervisor`. Merge orchestration is supervisor-skill territory (see the `agent-skills` capability and the "Merge orchestration" requirement on the supervisor skill).

#### Scenario: Supervisor auto-start launches all panes including the supervisor pane

- **GIVEN** a valid supervisor config with `cli = "claude"` and two spec branches
- **WHEN** `cmd_supervisor()` is called
- **THEN** a tmux session named `paw-<project>` SHALL exist in detached mode
- **AND** pane 0 SHALL be the supervisor agent (Claude in `repo_root` with the rendered `supervisor.md` content as `AGENTS.md`)
- **AND** pane 1 SHALL be the dashboard (`git-paw __dashboard`)
- **AND** panes 2 and 3 SHALL have the coding agent CLI commands

#### Scenario: Hard cap rejects more than 25 agents

- **GIVEN** a configuration that resolves to 26 or more spec branches
- **WHEN** `cmd_supervisor()` is called
- **THEN** the function SHALL return a `PawError` before any tmux command runs
- **AND** the error message SHALL state the requested count, the maximum (25), and a hint suggesting `--branches <subset>` for splitting

#### Scenario: Layout proportions match agent count

- **GIVEN** `agents = 10` (computed `agent_rows = 2`, `total_rows = 3`)
- **WHEN** the tmux session is built
- **THEN** the top row SHALL occupy 40% of vertical space and each agent row SHALL occupy 30%

- **GIVEN** `agents = 20` (computed `agent_rows = 4`, `total_rows = 5`)
- **WHEN** the tmux session is built
- **THEN** the top row SHALL occupy 28% of vertical space and each of the 4 agent rows SHALL occupy 18%

#### Scenario: Top row is split 50/50 between supervisor and dashboard

- **GIVEN** any valid supervisor session with broker enabled
- **WHEN** the tmux layout is built
- **THEN** the top row SHALL be split horizontally into two equal panes: pane 0 (supervisor, 50%) and pane 1 (dashboard, 50%)

#### Scenario: Broker URL is injected before pane creation

- **GIVEN** `cmd_supervisor()` is executing
- **WHEN** the tmux session is created
- **THEN** `GIT_PAW_BROKER_URL` SHALL be set in the tmux session environment before any pane is created

#### Scenario: Approval flags are injected per agent pane

- **GIVEN** `agent_approval = "full-auto"` in supervisor config and `cli = "claude"`
- **WHEN** `cmd_supervisor()` constructs each agent's launch command
- **THEN** the command SHALL include `--dangerously-skip-permissions`

#### Scenario: cmd_supervisor returns immediately with attach hint

- **GIVEN** `cmd_supervisor()` completes the launch sequence successfully
- **WHEN** all panes are created and prompts injected
- **THEN** stdout SHALL contain "Supervisor session 'paw-<project>' launched" and the manual-attach command (`tmux attach -t paw-<project>`)
- **AND** `cmd_supervisor()` SHALL return `Ok(())` without blocking on any process
- **AND** the foreground terminal SHALL NOT be replaced with an interactive supervisor CLI

#### Scenario: cmd_supervisor does NOT call the Rust merge loop

- **GIVEN** any valid supervisor session
- **WHEN** `cmd_supervisor()` runs to completion
- **THEN** no `run_merge_loop` (or equivalent Rust merge orchestration) SHALL execute
- **AND** the merge orchestration SHALL be the supervisor agent's responsibility per the `agent-skills` "Merge orchestration" requirement

### Requirement: Initial prompt injection via tmux send-keys

After the tmux session is created in detached mode, the system SHALL wait approximately 2 seconds for all panes to reach an interactive state, then inject the initial task prompt for each pane (including the supervisor pane at index 0) via `tmux send-keys`.

The initial prompt SHALL be derived from:
- For pane 0 (supervisor): the boot block (with `BRANCH_ID = supervisor`) followed by a "Begin observing" framing message instructing the supervisor agent to start its autonomous loop and respond to user input per the supervisor skill.
- For panes 2..N+1 (coding agents): the boot block (with `BRANCH_ID = <slugified-branch>`) followed by the spec content if available, or a default "Begin your assigned task." message if no spec is configured.

The dashboard pane (index 1) does NOT receive a `send-keys` prompt — it runs the `__dashboard` subcommand which is a non-interactive TUI process.

#### Scenario: Initial prompt is injected after boot delay

- **GIVEN** a supervisor pane (index 0) and two coding agent panes (indices 2 and 3) have been created
- **WHEN** `cmd_supervisor()` injects initial prompts
- **THEN** `tmux send-keys` SHALL be called for pane 0 (supervisor) with the supervisor boot block + framing message
- **AND** `tmux send-keys` SHALL be called for panes 2 and 3 (agents) with their respective boot blocks + task prompts
- **AND** `tmux send-keys` SHALL NOT be called for pane 1 (dashboard)

#### Scenario: Supervisor pane prompt uses BRANCH_ID = supervisor

- **GIVEN** the supervisor pane is being initialised
- **WHEN** the boot block is built for pane 0
- **THEN** the boot block's `BRANCH_ID` placeholder SHALL be substituted with `"supervisor"`

#### Scenario: Default prompt when no spec content

- **GIVEN** a coding agent pane with no spec file assigned
- **WHEN** the initial prompt is injected
- **THEN** the injected text SHALL be the boot block followed by a default task prompt (not empty)

### Requirement: Supervisor AGENTS.md from supervisor skill template

The system SHALL load the supervisor skill template via `skills::resolve("supervisor")` and write it to the supervisor pane's working directory (the repo root, NOT a worktree) as the supervisor CLI's `AGENTS.md` before starting the supervisor pane. This makes the supervisor skill available to the supervisor agent's Claude on startup.

#### Scenario: Supervisor pane reads supervisor.md as AGENTS.md

- **GIVEN** the supervisor skill template is resolvable
- **WHEN** `cmd_supervisor()` prepares the supervisor pane's environment
- **THEN** an `AGENTS.md` file SHALL exist at the repo root containing the rendered supervisor skill content
- **AND** when the supervisor pane's Claude starts (in `repo_root`), it SHALL read that `AGENTS.md`

### Requirement: Supervisor self-registration

The system SHALL automatically register the supervisor agent in the broker so it appears in the dashboard alongside other agents. This SHALL include publishing an initial `agent.status` message with `agent_id = "supervisor"` BEFORE the supervisor pane's Claude finishes booting, so the dashboard reflects the supervisor's presence immediately.

#### Scenario: Supervisor registers itself on startup

- **GIVEN** `cmd_supervisor()` is executing
- **WHEN** the launch sequence reaches the self-registration step
- **THEN** an `agent.status` message SHALL be published with:
  - `agent_id = "supervisor"`
  - `status = "working"`
  - `message = "Supervisor booting"`

#### Scenario: Supervisor appears in dashboard pane

- **GIVEN** supervisor self-registration is complete
- **WHEN** the dashboard renders
- **THEN** the agent table includes an entry for `"supervisor"`
- **AND** the entry shows the supervisor's status and activity

### Requirement: Boot-prompt injection

The system SHALL prepend a standardized boot instruction block to each agent pane's initial prompt — INCLUDING the supervisor pane (pane 0). The block SHALL instruct agents on the runtime events they must publish via curl.

#### Scenario: Boot block is prepended to all agent prompts including supervisor

- **GIVEN** `cmd_supervisor()` is launching the supervisor and coding agents
- **WHEN** each pane's initial prompt is constructed
- **THEN** every coding agent pane's prompt SHALL begin with the standardized boot instruction block
- **AND** the supervisor pane's prompt SHALL ALSO begin with the boot instruction block (with `BRANCH_ID = supervisor`)

#### Scenario: Boot block uses template substitution

- **GIVEN** an agent on branch `feat/errors` with broker URL `http://127.0.0.1:9119`
- **WHEN** the boot block is generated
- **THEN** the block contains pre-expanded curl commands with:
  - `{{BRANCH_ID}}` replaced with `"feat-errors"`
  - `{{GIT_PAW_BROKER_URL}}` replaced with `"http://127.0.0.1:9119"`

### Requirement: Auto-approve common command classes

The system SHALL detect and automatically approve common permission prompts to reduce manual intervention overhead. The auto-approve subsystem SHALL run inside the dashboard's `__dashboard` subprocess (which is long-lived for the duration of the dashboard pane), NOT inside the `cmd_supervisor` process (which now returns immediately after launching the session).

#### Scenario: Permission prompts are detected via tmux capture

- **GIVEN** an agent pane showing a permission prompt
- **WHEN** the auto-approve subsystem (running inside the `__dashboard` process) polls
- **THEN** the prompt SHALL be detected via `tmux capture-pane`

#### Scenario: Auto-approve dies when dashboard pane is closed

- **GIVEN** an active supervisor session with auto-approve enabled
- **WHEN** the user kills the dashboard pane (pane 1)
- **THEN** the auto-approve subsystem SHALL stop firing (it is a thread inside the `__dashboard` process)
- **AND** subsequent permission prompts SHALL require manual approval

