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
15. **Branch on `--unattended`:**
    - When `--unattended` is **absent**: **print an attach-hint and return `Ok(())`**: `cmd_supervisor()` does NOT block on a foreground supervisor CLI. The user runs `tmux attach -t paw-<project>` to interact with the supervisor pane. (v0.5.0 behaviour, unchanged.)
    - When `--unattended` is **present**: instead of returning immediately, `cmd_supervisor()` SHALL run the in-process unattended drive loop (per the `unattended-operation` capability) which blocks until a completion, escalation-summary, stuck, or heartbeat exit condition is reached, then prints the exit summary and returns. The drive loop SHALL NOT require an attached interactive terminal. The unattended path SHALL NOT replace the foreground terminal with an interactive supervisor CLI.

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

#### Scenario: cmd_supervisor returns immediately with attach hint when --unattended is absent

- **GIVEN** `cmd_supervisor()` completes the launch sequence successfully WITHOUT `--unattended`
- **WHEN** all panes are created and prompts injected
- **THEN** stdout SHALL contain "Supervisor session 'paw-<project>' launched" and the manual-attach command (`tmux attach -t paw-<project>`)
- **AND** `cmd_supervisor()` SHALL return `Ok(())` without blocking on any process
- **AND** the foreground terminal SHALL NOT be replaced with an interactive supervisor CLI

#### Scenario: cmd_supervisor drives the loop in-process when --unattended is present

- **GIVEN** `cmd_supervisor()` completes the launch sequence successfully WITH `--unattended`
- **WHEN** all panes are created and prompts injected
- **THEN** `cmd_supervisor()` SHALL run the in-process unattended drive loop (per the `unattended-operation` capability) rather than returning immediately
- **AND** the drive loop SHALL block until a completion, escalation-summary, stuck, or heartbeat exit condition is reached
- **AND** the foreground terminal SHALL NOT be replaced with an interactive supervisor CLI
- **AND** on exit the process SHALL print the drive-loop summary and return

#### Scenario: cmd_supervisor does NOT call the Rust merge loop

- **GIVEN** any valid supervisor session
- **WHEN** `cmd_supervisor()` runs to completion
- **THEN** no `run_merge_loop` (or equivalent Rust merge orchestration) SHALL execute
- **AND** the merge orchestration SHALL be the supervisor agent's responsibility per the `agent-skills` "Merge orchestration" requirement
