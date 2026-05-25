# supervisor-launch Specification

## Purpose
TBD - created by archiving change supervisor-agent. Update Purpose after archive.
## Requirements
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

After the tmux session is created in detached mode, the system SHALL wait approximately 2 seconds for all panes to reach an interactive state, then inject the initial task prompt for each coding agent pane via a single `tmux send-keys` invocation.

The initial task prompt SHALL be constructed by appending a per-agent **task prompt** to the standardized boot block (separated by a blank line). The task prompt SHALL be derived from the agent's associated `SpecEntry` (if any) via the pure helper `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String`, which SHALL dispatch on `SpecEntry.backend`:

1. When a spec is associated with the agent's branch (the `--from-specs` path) and `spec_entry.backend == SpecBackendKind::OpenSpec`, the task prompt SHALL be exactly the slash-command invocation `format!("/opsx:apply {id}", id = spec_entry.id)`. The task prompt SHALL NOT contain any prose surrounding the slash command, SHALL NOT contain `AGENTS.md`, and SHALL NOT contain `openspec/changes/`. The slash command SHALL be the entire returned string so that paste-aware CLIs parse it as a slash-command invocation at the start of the agent's first turn.
2. When a spec is associated with the agent's branch and `spec_entry.backend == SpecBackendKind::Markdown` (or any other non-OpenSpec backend that lacks a slash-command apply workflow), the task prompt SHALL point the agent at the worktree's `AGENTS.md` for the full spec body AND include the spec's identifier so the agent can locate sibling artifacts (proposal, design, specs, tasks) under `openspec/changes/<id>/`. The task prompt SHALL NOT contain the spec body itself, nor a truncated heading from the spec body.
3. When no spec is associated with the agent's branch (the `--branches` path), use the default fallback `"Begin your assigned task as described in AGENTS.md."` verbatim.

The full spec body remains the source of truth for `AGENTS.md` generation (`WorktreeAssignment.spec_content` is unchanged); only the injected boot prompt's task-prompt portion changes per backend.

The single `tmux send-keys` invocation SHALL pass the constructed prompt followed by the `Enter` keystroke. On paste-aware CLIs the slash-command form (OpenSpec branch) is short enough that paste-buffer capture is unlikely; the longer Markdown-branch pointer may still trip paste-buffer behaviour, which the supervisor agent recovers from via the paste-buffer-recovery skill (see the `agent-skills` capability).

#### Scenario: Initial prompt is injected after boot delay

- **GIVEN** two coding agent panes have been created
- **WHEN** `cmd_supervisor()` injects initial prompts
- **THEN** `tmux send-keys` SHALL be called for each agent pane with the task prompt followed by `Enter`

#### Scenario: Default prompt when no spec content

- **GIVEN** an agent pane with no spec file assigned
- **WHEN** the initial prompt is injected
- **THEN** the injected task-prompt portion SHALL be the default fallback string `"Begin your assigned task as described in AGENTS.md."`

#### Scenario: Launch flow sends exactly one Enter per pane

- **GIVEN** N coding agent panes
- **WHEN** the supervisor launch flow runs through the prompt-injection loop
- **THEN** the system SHALL invoke `tmux send-keys` exactly once per pane
- **AND** the invocation SHALL include the prompt text and the `Enter` keystroke
- **AND** the system SHALL NOT emit any additional standalone `Enter` keystrokes to the pane during the launch flow

#### Scenario: Paste-buffer recovery is delegated to the supervisor skill

- **GIVEN** a coding agent pane on a paste-aware CLI (e.g. Claude Code v2.1.x) whose injected long prompt has been captured as a paste-buffer placeholder rather than submitted
- **WHEN** the supervisor agent's monitoring loop next inspects the pane via `tmux capture-pane`
- **THEN** the supervisor SHALL apply the paste-buffer-recovery sub-case from the embedded skill (`agent-skills` capability)
- **AND** the launch flow itself SHALL have already exited; the launch flow is NOT responsible for retrying the keystroke

#### Scenario: OpenSpec-backed task prompt invokes the opsx:apply slash command

- **GIVEN** a coding agent on branch `feat/governance-config` whose associated spec entry has `id = "governance-config"` and `backend = SpecBackendKind::OpenSpec`
- **WHEN** the supervisor launch flow builds the task prompt for that agent
- **THEN** `build_task_prompt(Some(&entry))` SHALL return exactly the string `"/opsx:apply governance-config"`
- **AND** the returned string SHALL NOT contain the substring `AGENTS.md`
- **AND** the returned string SHALL NOT contain the substring `openspec/changes/`
- **AND** the returned string SHALL NOT contain any portion of the spec's prompt body

#### Scenario: Markdown-backed task prompt uses the generic AGENTS.md pointer

- **GIVEN** a coding agent on branch `feat/my-feature` whose associated spec entry has `id = "my-feature"` and `backend = SpecBackendKind::Markdown`
- **WHEN** the supervisor launch flow builds the task prompt for that agent
- **THEN** the returned string SHALL contain the substring `AGENTS.md`
- **AND** the returned string SHALL contain the substring `openspec/changes/my-feature`
- **AND** the returned string SHALL NOT begin with `/opsx:apply`
- **AND** the returned string SHALL instruct the agent to read AGENTS.md and the sibling artifacts before starting

#### Scenario: Backend dispatch is exhaustive over SpecBackendKind

- **GIVEN** `SpecBackendKind` enumerates the backends supported in the current build (initially `OpenSpec` and `Markdown`)
- **WHEN** the supervisor launch flow's task-prompt construction is inspected
- **THEN** `build_task_prompt` SHALL match every variant of `SpecBackendKind` exhaustively
- **AND** the compiler SHALL reject `build_task_prompt` if a future variant (e.g. `SpecKit`) is added to `SpecBackendKind` without a corresponding match arm

#### Scenario: build_task_prompt remains a pure function

- **WHEN** the supervisor launch flow's task-prompt construction is inspected
- **THEN** it SHALL be implemented as a pure function `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String`
- **AND** the function SHALL have no I/O side effects (no filesystem reads, no process spawns, no config lookups)
- **AND** the function SHALL be callable from `cfg(test)` without launching tmux

### Requirement: Supervisor AGENTS.md from supervisor skill template

The system SHALL load the supervisor skill template via `skills::resolve("supervisor")` and write it to the supervisor pane's working directory (the repo root, NOT a worktree) as the supervisor CLI's `AGENTS.md` before starting the supervisor pane. This makes the supervisor skill available to the supervisor agent's Claude on startup.

#### Scenario: Supervisor pane reads supervisor.md as AGENTS.md

- **GIVEN** the supervisor skill template is resolvable
- **WHEN** `cmd_supervisor()` prepares the supervisor pane's environment
- **THEN** an `AGENTS.md` file SHALL exist at the repo root containing the rendered supervisor skill content
- **AND** when the supervisor pane's Claude starts (in `repo_root`), it SHALL read that `AGENTS.md`

### Requirement: Supervisor self-registration

The system SHALL register the supervisor agent in the broker so it appears in the dashboard alongside other agents. Registration SHALL be performed by the supervisor agent itself, from inside the supervisor pane, via the existing skill-driven curl POST to `/publish`. The launcher (`cmd_supervisor`) SHALL NOT publish any `agent.status` on behalf of the supervisor before returning.

Specifically:

1. `cmd_supervisor` SHALL NOT call `publish_to_broker_http(..., build_status_message("supervisor", ...))` (or any equivalent self-registration POST) at any point in its flow.
2. The supervisor pane's boot block (rendered by `build_boot_block("supervisor", broker_url)`) and the supervisor skill SHALL together instruct the supervisor agent's CLI to publish an initial `agent.status` message as the very first action after reading `AGENTS.md`. The published message SHALL have `agent_id = "supervisor"`, a phase-appropriate `status` label, and a populated `cli` field identifying the supervisor's CLI (resolved by the supervisor agent from its environment or skill template substitution).
3. When the supervisor pane fails to start (e.g. layout error after `tmux_session.execute()`, missing CLI on PATH, system-level pane spawn failure, any abort path that does not actually launch a supervisor CLI process), no `agent.status` for `agent_id = "supervisor"` SHALL exist in the broker — the dashboard SHALL correctly render no supervisor row in such failure cases.

The "Supervisor row placement" rule on the dashboard (`dashboard` capability) governs how the row is rendered once it does appear; this requirement governs only WHEN the row is allowed to appear.

#### Scenario: cmd_supervisor does not self-publish on behalf of the supervisor

- **GIVEN** `cmd_supervisor()` is called with a valid supervisor config and broker enabled
- **WHEN** the launcher completes all of its tmux-session, save-state, and send-keys steps
- **AND** the launcher reaches its `Ok(())` return
- **THEN** no `agent.status` message with `agent_id = "supervisor"` SHALL have been published by the launcher process
- **AND** the broker's `/status` endpoint SHALL NOT yet contain an entry for `agent_id = "supervisor"` from the launcher's side
- **AND** the broker's message log SHALL NOT contain any entry whose origin is the launcher publishing as `agent_id = "supervisor"`

#### Scenario: Supervisor pane publishes its own initial agent.status

- **GIVEN** a successfully-launched supervisor session in which the supervisor pane's CLI has booted and read its AGENTS.md
- **WHEN** the supervisor agent executes its boot-block instructions
- **THEN** an `agent.status` message with `agent_id = "supervisor"` SHALL be published from inside the supervisor pane via curl
- **AND** the published message's payload SHALL include `cli = Some(<supervisor CLI name>)`
- **AND** the broker's `/status` endpoint SHALL then list `supervisor` among the known agents

#### Scenario: Aborted launch leaves no phantom supervisor row

- **GIVEN** a launch path where `cmd_supervisor()` returns an error (or follows any abort path) AFTER the broker is running but BEFORE the supervisor pane's CLI has executed its boot block
- **WHEN** the dashboard renders a frame
- **THEN** the agent table SHALL NOT contain a `supervisor` row
- **AND** no divider SHALL be rendered above the coding-agent rows

#### Scenario: Non-interactive launch leaves no phantom supervisor row before pane bootstrap

- **GIVEN** a launch path that successfully completes `cmd_supervisor()` but where the supervisor pane's CLI has not yet executed its boot-block curl (the time window between `cmd_supervisor` returning and the supervisor agent's first curl)
- **WHEN** the dashboard renders a frame during this window
- **THEN** the agent table SHALL NOT contain a `supervisor` row
- **AND** the dashboard SHALL render no divider
- **AND** any subsequent frame rendered AFTER the supervisor agent's first curl SHALL include the `supervisor` row pinned to the top (per the dashboard capability's supervisor-row-placement rule)

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

### Requirement: cmd_supervisor SHALL fall back to default SupervisorConfig when none configured

`cmd_supervisor` SHALL NOT error with "supervisor mode enabled but `[supervisor]` config missing" when the loaded `PawConfig` lacks a `[supervisor]` block. Instead, `cmd_supervisor` SHALL synthesize a `SupervisorConfig::default()` value and resolve the supervisor CLI through the existing chain `[supervisor].cli > default_cli > error`.

The error path SHALL be reached only when **both** `[supervisor].cli` and the top-level `default_cli` are unset. The error message in that case SHALL remain: `"supervisor mode requires either [supervisor].cli or default_cli to be set"`.

This requirement applies symmetrically to `recover_supervisor_session` for resumed sessions.

Rationale: `resolve_supervisor_mode` already prompts the user when no `[supervisor]` block exists. The prompt was designed to ask the user opt-in to supervisor mode without forcing them to hand-author a config block first. The pre-existing hard error in `cmd_supervisor` defeated the prompt's design intent.

#### Scenario: Interactive prompt yes accepts default supervisor config

- **GIVEN** a repo with `.git-paw/config.toml` containing only `default_cli = "echo"` (no `[supervisor]` section)
- **AND** `git paw start --branches a,b` is invoked from a TTY
- **WHEN** the prompt "Start in supervisor mode?" appears and the user answers yes
- **THEN** the launch SHALL exit 0 and print the standard supervisor-session-launched message
- **AND** the stderr SHALL NOT contain "supervisor mode enabled but [supervisor] config missing"
- **AND** the synthesized `SupervisorConfig` has `enabled = false`, `cli = None`, `agent_approval = ApprovalLevel::default()`, all other fields at their respective `Default` values

#### Scenario: --supervisor flag without [supervisor] config works

- **GIVEN** a repo with `.git-paw/config.toml` containing only `default_cli = "echo"`
- **WHEN** `git paw start --supervisor --branches a,b` is invoked
- **THEN** the launch SHALL exit 0
- **AND** the supervisor pane SHALL run the `default_cli` value as the supervisor CLI

#### Scenario: Both [supervisor].cli and default_cli missing still errors

- **GIVEN** a repo with `.git-paw/config.toml` containing no `[supervisor]` section AND no top-level `default_cli`
- **WHEN** `git paw start --supervisor --branches a,b` is invoked
- **THEN** the launch SHALL exit non-zero with the error "supervisor mode requires either [supervisor].cli or default_cli to be set"

#### Scenario: recover_supervisor_session applies the same fallback

- **GIVEN** a previously-launched supervisor session that has been stopped
- **AND** the repo's `.git-paw/config.toml` no longer has a `[supervisor]` block (e.g. user deleted it between sessions)
- **WHEN** `git paw start` is invoked and routes to `recover_supervisor_session`
- **THEN** the recovery SHALL succeed using the default supervisor config
- **AND** SHALL NOT error on the missing `[supervisor]` section

### Requirement: Resumed coding-agent panes SHALL spawn in their worktree cwd

When `recover_session` rebuilds a stopped session (via either `recover_bare_session` or `recover_supervisor_session`), every coding-agent pane's tmux working directory SHALL be the pane's `worktree_path` from the session JSON — NOT the repo root.

Implementation SHALL pass `-c <pane.worktree>` on every `split-window` that creates a coding-agent pane. The previous `cd <worktree> && <cli_command>` pattern via `send-keys` is forbidden for new agent panes because it races with shell startup: when send-keys fires before the shell is ready to accept input the `cd` prefix is lost and the CLI launches in whichever cwd the pane inherited from its parent (typically the repo root).

The supervisor pane and dashboard pane SHALL continue to spawn in the repo root via `new-session -c <repo_root>` and the `-c <repo_root>` parameter on their respective splits.

#### Scenario: Bare-session recovery places each agent pane in its worktree

- **GIVEN** a stopped bare-mode session with two coding agents in worktrees `/path/to/repo-feat-a` and `/path/to/repo-feat-b`
- **WHEN** `git paw start` resumes the session
- **AND** the session is fully built and panes have settled
- **THEN** `tmux display-message -t <session>:0.1 -p "#{pane_current_path}"` SHALL output `/path/to/repo-feat-a`
- **AND** `tmux display-message -t <session>:0.2 -p "#{pane_current_path}"` SHALL output `/path/to/repo-feat-b`

#### Scenario: Supervisor-mode recovery places each agent pane in its worktree

- **GIVEN** a stopped supervisor-mode session with the v0.5 layout (supervisor / dashboard / agent grid)
- **AND** two coding agents in worktrees `/path/to/repo-feat-a` and `/path/to/repo-feat-b`
- **WHEN** `git paw start` resumes the session
- **THEN** the supervisor pane (`0.0`) and dashboard pane (`0.1`) have `pane_current_path = /path/to/repo` (the repo root)
- **AND** the agent panes (`0.2`, `0.3`) have `pane_current_path` equal to their respective `worktree_path` values
- **AND** the CLI command sent to each agent pane via `send-keys` SHALL NOT be prefixed with `cd <worktree> &&`

#### Scenario: First-agent split passes -c worktree (not just send-keys cd)

- **GIVEN** a supervisor-mode recovery flow building the first agent pane via `split-window`
- **WHEN** the tmux command sequence is inspected
- **THEN** the `split-window` for the first agent SHALL include `-c <first_agent.worktree>` as arguments
- **AND** the follow-up `send-keys` SHALL send only the bare CLI command (no `cd <worktree> &&` prefix)

