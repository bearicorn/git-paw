## MODIFIED Requirements

### Requirement: Supervisor auto-start flow

The system SHALL implement `cmd_supervisor()` (in `src/commands/supervisor.rs`) that orchestrates the full coordinated-session launch. The function SHALL execute the following steps in order:

1. Load config. Resolve the orchestrator CLI from `[orchestrator] cli`, falling back to `default_cli`, when either is set. An absent orchestrator CLI SHALL NOT fail the launch: it is used only to seed the roster's root row and by headless escalation, never to run a pane. Resolve whether the launch has a dashboard: `--dashboard`, else `[dashboard] enabled`, else none (per `broker-runtime`'s "Start flow conditionally creates dashboard pane").
2. Scan specs via the spec mode or resolve branches from flags.
3. **Cap check**: resolve the effective grid settings (`--max-agents`, else `[layout] max_agents`, else 25; `[layout] agents_per_row`, else 5) per `tmux-orchestration`'s "Agent grid dimensions are configurable". The agent count SHALL NOT exceed the effective `max_agents`; above it, return a `PawError` with an actionable hint (split with `--branches`, or raise the cap).
4. **Compute layout proportions**: `agent_rows = ceil(agents / agents_per_row)`. With a dashboard, `total_rows = agent_rows + 1`, the dashboard row takes 50% of the height and each agent row `50 / agent_rows` percent. Without one, `total_rows = agent_rows` and each agent row takes `100 / agent_rows` percent.
5. Create worktrees for each branch (with `-b` fallback for new branches).
6. Generate each worktree's sidecar `AGENTS.md` view with spec content, file ownership, coordination skill, and inter-agent rules.
7. **Build the tmux session**: with a dashboard, pane 0 = dashboard (`git-paw __dashboard` in `repo_root`) as the full-width top row and panes 1..N = coding agents; without one, panes 0..N-1 = coding agents. Agents are laid out row-major, up to `agents_per_row` per row. No pane SHALL run an agent CLI at the repository root.
8. Inject `GIT_PAW_BROKER_URL` via `tmux set-environment` before pane creation.
9. For each agent pane: construct the CLI launch command with approval flags resolved from `[workers] approval`.
10. Execute the tmux session in detached mode.
11. Wait for each coding-agent pane to pass the launch-readiness gate.
12. **Inject the initial prompt into each coding-agent pane** (boot block + task prompt). The dashboard pane, when present, receives no keystrokes.
13. Render the orchestrator context into the repo-root sidecar (per "Supervisor AGENTS.md from supervisor skill template") and print its path alongside the attach hint. The launcher SHALL NOT publish any broker message on behalf of the orchestrator.
14. Save session state with the coordinated mode, `dashboard_pane = Some(0)` only when a dashboard pane was created, no orchestrator entry, and the effective grid settings.
15. **Branch on `--unattended`:**
    - When `--unattended` is **absent**: print the attach hint and the orchestrator-context hint, then return `Ok(())`. `cmd_supervisor()` does NOT block on any process.
    - When `--unattended` is **present**: run the in-process unattended drive loop (per `supervisor-unattended-operation`), which blocks until a completion, escalation-summary, stuck, or heartbeat exit condition is reached, then print the exit summary and return. The drive loop SHALL NOT require an attached interactive terminal.

The Rust merge loop SHALL NOT be invoked from `cmd_supervisor`. Merge orchestration is the external orchestrator's responsibility per the orchestrator skill.

#### Scenario: Coordinated auto-start launches only coding panes by default

- **GIVEN** a valid coordinated config without `[dashboard] enabled` and two spec branches
- **WHEN** `cmd_supervisor()` is called
- **THEN** a tmux session named `paw-<project>` SHALL exist in detached mode
- **AND** panes 0 and 1 SHALL run the coding-agent CLI commands in their worktrees
- **AND** no pane SHALL run `git-paw __dashboard`, and no pane SHALL run an agent CLI whose working directory is `repo_root`

#### Scenario: Coordinated auto-start with the dashboard enabled

- **GIVEN** a valid coordinated config, `[dashboard] enabled = true` (or `--dashboard`) and two spec branches
- **WHEN** `cmd_supervisor()` is called
- **THEN** pane 0 SHALL be the dashboard (`git-paw __dashboard`)
- **AND** panes 1 and 2 SHALL run the coding-agent CLI commands in their worktrees

#### Scenario: Cap rejects more agents than the effective maximum

- **GIVEN** default grid settings and a configuration that resolves to 26 or more spec branches
- **WHEN** `cmd_supervisor()` is called
- **THEN** the function SHALL return a `PawError` before any tmux command runs
- **AND** the error message SHALL state the requested count, the maximum (25), a hint suggesting `--branches <subset>` for splitting, and how to raise the cap

#### Scenario: Layout proportions match agent count

- **GIVEN** default grid settings, a dashboard and `agents = 10` (computed `agent_rows = 2`, `total_rows = 3`)
- **WHEN** the tmux session is built
- **THEN** the dashboard row SHALL occupy 50% of the vertical space and each agent row SHALL occupy 25%

- **GIVEN** default grid settings, a dashboard and `agents = 20` (computed `agent_rows = 4`, `total_rows = 5`)
- **WHEN** the tmux session is built
- **THEN** the dashboard row SHALL occupy 50% and each of the 4 agent rows SHALL occupy 12.5%

- **GIVEN** default grid settings, no dashboard and `agents = 10` (computed `agent_rows = 2`, `total_rows = 2`)
- **WHEN** the tmux session is built
- **THEN** each agent row SHALL occupy 50% of the vertical space

#### Scenario: Dashboard row spans the full width

- **GIVEN** any valid coordinated session with a dashboard
- **WHEN** the tmux layout is built
- **THEN** pane 0 (dashboard) SHALL span the full window width as the top row
- **AND** no other pane SHALL share the top row

#### Scenario: Broker URL is injected before pane creation

- **GIVEN** `cmd_supervisor()` is executing
- **WHEN** the tmux session is created
- **THEN** `GIT_PAW_BROKER_URL` SHALL be set in the tmux session environment before any pane is created

#### Scenario: Approval flags are injected per agent pane

- **GIVEN** `[workers] approval = "full-auto"` and `cli = "claude"` for the agents
- **WHEN** `cmd_supervisor()` constructs each agent's launch command
- **THEN** the command SHALL include `--dangerously-skip-permissions`

#### Scenario: Launch does not require an orchestrator CLI

- **GIVEN** a config with `default_mode = "coordinated"`, no `[orchestrator] cli` key, no `default_cli`, and `--cli claude` on the command line
- **WHEN** `cmd_supervisor()` is called
- **THEN** the launch SHALL succeed
- **AND** each coding-agent pane SHALL run `claude`

#### Scenario: cmd_supervisor returns immediately with hints when --unattended is absent

- **GIVEN** `cmd_supervisor()` completes the launch sequence successfully WITHOUT `--unattended`
- **WHEN** all panes are created and prompts injected
- **THEN** stdout SHALL contain the coordinated launch line naming `paw-<project>`, the manual-attach command (`tmux attach -t paw-<project>`), and the orchestrator-context path `.git-paw/AGENTS.local.md`
- **AND** `cmd_supervisor()` SHALL return `Ok(())` without blocking on any process

#### Scenario: cmd_supervisor drives the loop in-process when --unattended is present

- **GIVEN** `cmd_supervisor()` completes the launch sequence successfully WITH `--unattended`
- **WHEN** all panes are created and prompts injected
- **THEN** `cmd_supervisor()` SHALL run the in-process unattended drive loop rather than returning immediately
- **AND** the drive loop SHALL block until a completion, escalation-summary, stuck, or heartbeat exit condition is reached
- **AND** on exit the process SHALL print the drive-loop summary and return

#### Scenario: cmd_supervisor does NOT call the Rust merge loop

- **GIVEN** any valid coordinated session
- **WHEN** `cmd_supervisor()` runs to completion
- **THEN** no `run_merge_loop` (or equivalent Rust merge orchestration) SHALL execute
