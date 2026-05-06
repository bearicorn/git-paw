# supervisor-launch Specification

## Purpose
TBD - created by archiving change supervisor-agent. Update Purpose after archive.
## Requirements
### Requirement: Supervisor auto-start flow

The system SHALL implement `cmd_supervisor()` in `src/main.rs` that orchestrates the full supervisor session launch. The function SHALL execute the following steps in order:

1. Load config and resolve the supervisor CLI from `[supervisor]` config
2. Scan specs via `--from-specs` or resolve branches from flags
3. Create worktrees for each branch (with `-b` fallback for new branches)
4. Generate per-worktree AGENTS.md with spec content, file ownership, coordination skill, and inter-agent rules
5. Build the tmux session with pane 0 as the dashboard and panes 1-N as coding agents
6. Inject `GIT_PAW_BROKER_URL` via `tmux set-environment` before pane creation
7. For each agent pane: construct the CLI launch command with approval flags from `approval_flags(cli, level)`
8. Execute the tmux session in detached mode
9. Wait approximately 2 seconds for panes to boot
10. Inject the initial prompt for each agent pane via `tmux send-keys`
11. Start the supervisor CLI in the foreground terminal with the supervisor skill template as its AGENTS.md

#### Scenario: Supervisor auto-start launches all panes

- **GIVEN** a valid supervisor config with `cli = "claude"` and two spec branches
- **WHEN** `cmd_supervisor()` is called
- **THEN** a tmux session named `paw-<project>` SHALL exist in detached mode
- **AND** pane 0 SHALL be the dashboard (`git-paw __dashboard`)
- **AND** panes 1 and 2 SHALL have the coding agent CLI commands

#### Scenario: Broker URL is injected before pane creation

- **GIVEN** `cmd_supervisor()` is executing
- **WHEN** the tmux session is created
- **THEN** `GIT_PAW_BROKER_URL` SHALL be set in the tmux session environment before any coding agent pane is created

#### Scenario: Approval flags are injected per agent pane

- **GIVEN** `agent_approval = "full-auto"` in supervisor config and `cli = "claude"`
- **WHEN** `cmd_supervisor()` constructs each agent's launch command
- **THEN** the command SHALL include `--dangerously-skip-permissions`

#### Scenario: Supervisor CLI starts in the foreground

- **GIVEN** `cmd_supervisor()` completes pane setup
- **WHEN** the supervisor CLI is started
- **THEN** it runs in the current terminal (not in a detached tmux pane)
- **AND** `cmd_supervisor()` blocks until the supervisor CLI exits

### Requirement: Initial prompt injection via tmux send-keys

After the tmux session is created in detached mode, the system SHALL wait approximately 2 seconds for all panes to reach an interactive state, then inject the initial task prompt for each coding agent pane via `tmux send-keys`.

The initial prompt SHALL be derived from the agent's spec content if available, or a default "Begin your assigned task." message if no spec is configured.

#### Scenario: Initial prompt is injected after boot delay

- **GIVEN** two coding agent panes have been created
- **WHEN** `cmd_supervisor()` injects initial prompts
- **THEN** `tmux send-keys` SHALL be called for each agent pane with the task prompt followed by `Enter`

#### Scenario: Default prompt when no spec content

- **GIVEN** an agent pane with no spec file assigned
- **WHEN** the initial prompt is injected
- **THEN** the injected text SHALL be a default task prompt (not empty)

### Requirement: Supervisor AGENTS.md from supervisor skill template

The system SHALL load the supervisor skill template via `skills::resolve("supervisor")` and write it to a temporary location as the supervisor CLI's AGENTS.md before starting the supervisor CLI in the foreground.

#### Scenario: Supervisor CLI receives supervisor skill as AGENTS.md

- **GIVEN** the supervisor skill template is resolvable
- **WHEN** `cmd_supervisor()` prepares the supervisor environment
- **THEN** an AGENTS.md file SHALL exist at the supervisor's working directory containing the rendered supervisor skill content

### Requirement: Supervisor self-registration

The system SHALL automatically register the supervisor agent in the broker so it appears in the dashboard alongside other agents. This SHALL include publishing an initial `agent.status` message with `agent_id = "supervisor"`.

#### Scenario: Supervisor registers itself on startup

- **GIVEN** `cmd_supervisor()` is executing
- **WHEN** the supervisor CLI starts
- **THEN** an `agent.status` message SHALL be published with:
  - `agent_id = "supervisor"`
  - `status = "working"`
  - `message = "Supervisor booting"`

#### Scenario: Supervisor appears in dashboard

- **GIVEN** supervisor self-registration is complete
- **WHEN** the dashboard renders
- **THEN** the agent table includes an entry for "supervisor"
- **AND** the entry shows the supervisor's status and activity

### Requirement: Boot-prompt injection

The system SHALL prepend a standardized boot instruction block to each agent's task prompt. This block SHALL instruct agents on the four essential runtime events they must publish via curl.

#### Scenario: Boot block is prepended to agent prompts

- **GIVEN** `cmd_supervisor()` is launching agents
- **WHEN** an agent's initial prompt is constructed
- **THEN** the prompt SHALL begin with a boot instruction block containing:
  1. Register on boot instructions
  2. Publish done/blocked/question patterns
  3. Broker URL and branch ID substitution
  4. "Do not guess, ask and wait" instruction

#### Scenario: Boot block uses template substitution

- **GIVEN** agent on branch `feat/errors` with broker URL `http://127.0.0.1:9119`
- **WHEN** boot block is generated
- **THEN** the block contains pre-expanded curl commands with:
  - `{{BRANCH_ID}}` replaced with `"feat-errors"`
  - `{{GIT_PAW_BROKER_URL}}` replaced with `"http://127.0.0.1:9119"`

### Requirement: Auto-approve common command classes

The system SHALL detect and automatically approve common permission prompts to reduce manual intervention overhead.

#### Scenario: Permission prompts are detected via tmux capture

- **GIVEN** an agent pane showing a permission prompt
- **WHEN** stall detection identifies the agent as stuck
- **THEN** the system captures the pane content via `tmux capture-pane`
- **AND** analyzes it for known permission prompt patterns

#### Scenario: Safe commands are auto-approved

- **GIVEN** a detected permission prompt for a safe command (e.g., `cargo fmt`)
- **WHEN** auto-approve is triggered
- **THEN** the system sends `BTab Down Enter` to approve and remember the decision

