# supervisor-launch Specification

## Purpose
Orchestrates the full supervisor session launch via `cmd_supervisor()` — computing the layout, creating worktrees and the tmux pane structure (supervisor, dashboard, coding-agent grid), injecting the broker URL and per-pane boot prompts, and branching between the return-with-attach-hint and `--unattended` in-process drive-loop paths. It also resolves whether `git paw start` enters supervisor mode from CLI flags, config, and interactive prompts (with launch- and purge-time git safety guards warning on uncommitted specs before `--from-specs` launch and on unmerged worktree commits before `purge`); ensures the first coding agent's pane runs its CLI in that agent's own worktree rather than the supervisor's repo root (assigning split-time `-c <cwd>` values to compensate for the pane-1/2 swap); and applies tmux visual affordances (double-line pane borders, per-pane role labels via a stable `@paw_role` option, a reverse-video border-format header bar, and active-pane border styling) gated by the `[layout].border_affordances` config field and degrading gracefully on older tmux.
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
15. **Branch on `--unattended`:**
    - When `--unattended` is **absent**: **print an attach-hint and return `Ok(())`**: `cmd_supervisor()` does NOT block on a foreground supervisor CLI. The user runs `tmux attach -t paw-<project>` to interact with the supervisor pane. (existing behaviour, unchanged.)
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

### Requirement: Initial prompt injection via tmux send-keys

After the tmux session is created in detached mode, the system SHALL wait approximately 2 seconds for all panes to reach an interactive state, then inject the initial task prompt for each coding agent pane via a single `tmux send-keys` invocation.

The initial task prompt SHALL be constructed by appending a per-agent **task prompt** to the standardized boot block (separated by a blank line). The task prompt SHALL be derived from the agent's associated `SpecEntry` (if any) via the pure helper `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String`. Because the managed assignment block moved out of the worktree-root `AGENTS.md` into the gitignored sidecar `.git-paw/AGENTS.local.md` (`git_paw::agents::SIDECAR_REL_PATH`) — which coding CLIs do NOT auto-load — every arm of `build_task_prompt` SHALL first point the agent at that sidecar. `build_task_prompt` SHALL dispatch on `SpecEntry.backend`:

1. `SpecBackendKind::OpenSpec` — the task prompt SHALL point the agent at the sidecar and then invoke the slash command `/opsx:apply {id}` (`{id} = spec_entry.id`). It SHALL contain the substring `/opsx:apply {id}` and SHALL NOT contain the path prose `openspec/changes/` (the sidecar already carries the artifact map). The slash command is retained so paste-aware CLIs parse it as a slash-command invocation.
2. `SpecBackendKind::Markdown` or `SpecBackendKind::SpecKit` — the task prompt SHALL point the agent at the sidecar for the project rules and full spec, AND name the sibling-artifact directory `openspec/changes/{id}/`. It SHALL NOT contain `/opsx:apply` (these backends have no slash-command apply workflow).
3. `SpecBackendKind::Superpowers` — the task prompt SHALL point the agent at the sidecar for the project rules and the full superpowers plan (goal, tasks, exact file paths, per-step verification commands), instructing the agent to work the steps in order and flip `- [ ]` to `- [x]` as each lands.
4. When no spec is associated with the agent's branch (the `--branches` path), the task prompt SHALL be the verbatim fallback `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."`.

The task prompt SHALL NOT contain the spec body itself, nor a truncated heading from the spec body. The full spec body remains the source of truth for the sidecar's generation (`WorktreeAssignment.spec_content` is unchanged); only the injected task-prompt portion changes per backend.

The single `tmux send-keys` invocation SHALL pass the constructed prompt followed by the `Enter` keystroke. The longer pointer prompts may still trip paste-aware CLIs' paste-buffer behaviour, which the supervisor agent recovers from via the paste-buffer-recovery skill (see the `agent-skills` capability).

#### Scenario: Initial prompt is injected after boot delay

- **GIVEN** two coding agent panes have been created
- **WHEN** `cmd_supervisor()` injects initial prompts
- **THEN** `tmux send-keys` SHALL be called for each agent pane with the task prompt followed by `Enter`

#### Scenario: Default prompt when no spec content

- **GIVEN** an agent pane with no spec file assigned
- **WHEN** the initial prompt is injected
- **THEN** the injected task-prompt portion SHALL be the verbatim fallback `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."`

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

#### Scenario: OpenSpec-backed task prompt points at the sidecar then invokes opsx:apply

- **GIVEN** a coding agent on branch `feat/my-change` whose associated spec entry has `id = "my-change"` and `backend = SpecBackendKind::OpenSpec`
- **WHEN** the supervisor launch flow builds the task prompt for that agent
- **THEN** `build_task_prompt(Some(&entry))` SHALL return a string containing the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the returned string SHALL contain the substring `/opsx:apply my-change`
- **AND** the returned string SHALL NOT contain the substring `openspec/changes/`
- **AND** the returned string SHALL NOT contain any portion of the spec's prompt body

Test: `main::tests::task_prompt_openspec_backend_points_at_sidecar_then_invokes_opsx_apply`

#### Scenario: Markdown-backed task prompt uses the sidecar pointer

- **GIVEN** a coding agent on branch `feat/my-feature` whose associated spec entry has `id = "my-feature"` and `backend = SpecBackendKind::Markdown`
- **WHEN** the supervisor launch flow builds the task prompt for that agent
- **THEN** the returned string SHALL contain the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the returned string SHALL contain the substring `openspec/changes/my-feature`
- **AND** the returned string SHALL NOT contain `/opsx:apply`

Test: `main::tests::task_prompt_markdown_backend_uses_sidecar_pointer`

#### Scenario: No-spec fallback points at the sidecar verbatim

- **WHEN** `build_task_prompt(None)` is called
- **THEN** the returned string SHALL equal `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."` byte-for-byte

Test: `main::tests::task_prompt_without_spec_points_at_sidecar_verbatim`

#### Scenario: Backend dispatch is exhaustive over SpecBackendKind

- **GIVEN** `SpecBackendKind` enumerates the backends supported in the current build (`OpenSpec`, `Markdown`, `SpecKit`, `Superpowers`)
- **WHEN** the supervisor launch flow's task-prompt construction is inspected
- **THEN** `build_task_prompt` SHALL match every variant of `SpecBackendKind` exhaustively
- **AND** the compiler SHALL reject `build_task_prompt` if a future variant is added to `SpecBackendKind` without a corresponding match arm

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

- **GIVEN** a stopped supervisor-mode session with the standard layout (supervisor / dashboard / agent grid)
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

### Requirement: Supervisor pane approval flags

When building the supervisor pane's CLI launch command, the system SHALL append the flags resolved from the supervisor's effective approval level (per `supervisor-config`'s "Supervisor-specific approval level resolution") to the supervisor CLI command. This SHALL apply on every path that constructs the supervisor pane command: the `cmd_supervisor` auto-start flow AND session recovery. Coding-agent pane commands SHALL keep resolving their flags from `agent_approval`.

The `--dry-run` plan output SHALL report the supervisor's effective approval level and the agents' approval level as separate lines when they differ.

#### Scenario: Fresh start applies supervisor flags to pane 0 only

- **GIVEN** a config with `[supervisor]` containing `cli = "claude"`, `approval = "full-auto"`, `agent_approval = "auto"`
- **WHEN** `cmd_supervisor()` builds the tmux session
- **THEN** pane 0's command SHALL be `claude --dangerously-skip-permissions`
- **AND** the coding-agent panes' commands SHALL NOT contain `--dangerously-skip-permissions`

#### Scenario: Recovery rebuilds the supervisor pane with the same flags

- **GIVEN** a recoverable session whose config sets `approval = "full-auto"` and `cli = "claude"`
- **WHEN** the session is recovered
- **THEN** the rebuilt supervisor pane command SHALL include `--dangerously-skip-permissions`

#### Scenario: Dry run reports split approval levels

- **GIVEN** a config with `approval = "full-auto"` and `agent_approval = "auto"`
- **WHEN** `git paw start --supervisor --dry-run` prints the session plan
- **THEN** the plan SHALL report the supervisor approval level (`FullAuto`) and the agent approval level (`Auto`) distinctly

#### Scenario: No approval key produces byte-identical commands to v0.10.0

- **GIVEN** a config with `[supervisor]` containing `agent_approval = "auto"` and no `approval` key
- **WHEN** the supervisor session launch commands are built
- **THEN** the supervisor pane and agent pane commands SHALL be identical to those v0.10.0 would build for the same config

### Requirement: Supervisor mode resolution chain

The system SHALL determine whether to enter supervisor mode using the following resolution chain, evaluated in order:

1. If `--no-supervisor` flag is present → disable supervisor mode (no prompt, regardless of any other input)
2. If `--supervisor` flag is present → enable supervisor mode (no prompt)
3. If `[supervisor] enabled = true` in config → enable supervisor mode (no prompt)
4. If `[supervisor] enabled = false` in config → disable supervisor mode (no prompt)
5. If `[supervisor]` section is absent (`None`) → prompt "Start in supervisor mode? (y/n)"
6. If `--dry-run` is present and step 5 would apply → assume no supervisor (skip prompt)

`--no-supervisor` and `--supervisor` SHALL be mutually exclusive at parse time (per the `cli-parsing` requirement); the resolver therefore never sees both flags `true` simultaneously.

When supervisor mode is enabled (steps 2 or 3), the system SHALL call `cmd_supervisor()`. When disabled (steps 1, 4, or 6), the system SHALL proceed with normal `cmd_start()`.

#### Scenario: --no-supervisor disables regardless of config (config enabled)

- **GIVEN** a config with `[supervisor] enabled = true`
- **WHEN** `git paw start --no-supervisor` is run
- **THEN** supervisor mode SHALL NOT be entered
- **AND** `cmd_supervisor()` SHALL NOT be called
- **AND** no interactive prompt SHALL be shown

#### Scenario: --no-supervisor with no config section also disables

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** `git paw start --no-supervisor` is run
- **THEN** supervisor mode SHALL NOT be entered
- **AND** no interactive prompt SHALL be shown

#### Scenario: --no-supervisor with --dry-run also disables

- **GIVEN** any config state
- **WHEN** `git paw start --no-supervisor --dry-run` is run
- **THEN** supervisor mode SHALL NOT be entered
- **AND** the dry-run plan SHALL reflect supervisor-disabled state

#### Scenario: --supervisor flag enables regardless of config

- **GIVEN** a config with `[supervisor] enabled = false`
- **WHEN** `git paw start --supervisor` is run
- **THEN** supervisor mode SHALL be enabled
- **AND** `cmd_supervisor()` SHALL be called

#### Scenario: Config enabled = true enables without prompt

- **GIVEN** a config with `[supervisor] enabled = true`
- **WHEN** `git paw start` is run with no flags
- **THEN** supervisor mode SHALL be enabled without any interactive prompt

#### Scenario: Config enabled = false disables without prompt

- **GIVEN** a config with `[supervisor] enabled = false`
- **WHEN** `git paw start` is run with no flags
- **THEN** supervisor mode SHALL NOT be entered
- **AND** no interactive prompt SHALL be shown

#### Scenario: No supervisor section prompts the user

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** `git paw start` is run with no flags
- **THEN** the system SHALL prompt "Start in supervisor mode?"

#### Scenario: dry-run skips supervisor prompt

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** `git paw start --dry-run` is run
- **THEN** no interactive prompt SHALL be shown
- **AND** supervisor mode SHALL NOT be entered

### Requirement: Validate specs are committed before launching

When `git paw start --from-specs` is used, the system SHALL verify that spec files discovered in the working directory are also present in the git index. This applies to both OpenSpec format (`openspec/changes/`) and Markdown format (the configured `[specs] dir`).

If any spec change directory or file exists in the working tree but is untracked or has uncommitted changes, the system SHALL warn: "N spec(s) have uncommitted changes. Worktree agents will not see uncommitted specs. Commit first or use --force to proceed."

The system SHALL NOT launch unless the user confirms or `--force` is passed.

#### Scenario: Uncommitted OpenSpec changes trigger warning

- **GIVEN** `openspec/changes/my-change/` exists but is not tracked by git
- **WHEN** `git paw start --from-specs` is run
- **THEN** the system SHALL warn about uncommitted specs
- **AND** SHALL NOT launch without user confirmation

#### Scenario: Uncommitted Markdown specs trigger warning

- **GIVEN** a Markdown spec file in the configured `[specs] dir` has uncommitted modifications
- **WHEN** `git paw start --from-specs` is run
- **THEN** the system SHALL warn about uncommitted specs

#### Scenario: All specs committed launches normally

- **GIVEN** all spec files are committed and clean
- **WHEN** `git paw start --from-specs` is run
- **THEN** no warning is shown and the session launches normally

#### Scenario: Force flag bypasses warning

- **GIVEN** uncommitted spec changes exist
- **WHEN** `git paw start --from-specs --force` is run
- **THEN** the session launches without warning
- **AND** if `just check` fails, the supervisor SHALL stop and report the failure

### Requirement: Purge warns about unmerged commits

Before destroying worktrees, `git paw purge` SHALL check each worktree branch for commits not yet merged to the default branch. The system SHALL:

1. For each worktree branch, run `git log <branch> --not <default-branch> --oneline`
2. If any branch has unmerged commits, display a warning listing each branch and its commit count
3. Require either `--force` flag or interactive confirmation ("Y" response) to proceed
4. If the user declines, exit without destroying any worktrees

The default branch SHALL be resolved from `git symbolic-ref refs/remotes/origin/HEAD`, falling back to `main` if unavailable.

#### Scenario: Purge with no unmerged commits proceeds without warning

- **GIVEN** all worktree branches have no commits beyond the default branch
- **WHEN** `git paw purge` is run
- **THEN** no unmerged commit warning SHALL be shown
- **AND** purge proceeds normally

#### Scenario: Purge with unmerged commits warns before destroying

- **GIVEN** one worktree branch has 3 commits not merged to main
- **WHEN** `git paw purge` is run without `--force`
- **THEN** a warning SHALL be displayed identifying the branch and the number of unmerged commits
- **AND** the system SHALL prompt for confirmation before proceeding

#### Scenario: Purge --force skips confirmation but still warns

- **GIVEN** one worktree branch has unmerged commits
- **WHEN** `git paw purge --force` is run
- **THEN** the warning SHALL still be displayed
- **AND** purge SHALL proceed without waiting for interactive confirmation

#### Scenario: Purge cancelled by user preserves worktrees

- **GIVEN** one worktree branch has unmerged commits
- **WHEN** `git paw purge` is run and the user answers "N" to the confirmation
- **THEN** no worktrees SHALL be removed
- **AND** the system SHALL exit with a non-error message indicating purge was cancelled

### Requirement: First agent pane launches in its own worktree

The supervisor session build SHALL ensure the first coding agent's pane runs
its CLI in that agent's worktree, never in the supervisor's repo-root working
directory. Because the build swaps panes 1 and 2 (to order dashboard before
the agent area) and sends each pane's CLI command after the swap by index,
the split-time `-c <cwd>` values SHALL be assigned to compensate for the
swap: the agent-area split takes the dashboard's cwd and the dashboard split
takes the first agent's worktree, so that post-swap each index's cwd matches
the command sent to it.

#### Scenario: First agent's CLI runs in its worktree

- **GIVEN** a supervisor session launched with at least one coding agent
- **WHEN** the layout is built and the first agent's CLI command is sent to
  its pane
- **THEN** that pane's working directory SHALL be the first agent's worktree
  (so its commits land on the agent's own branch), NOT the repo root

#### Scenario: Compensated split cwds

- **WHEN** the supervisor build's two top-region splits are inspected
- **THEN** the agent-area (`split-window -v`) SHALL carry `-c <dashboard
  cwd>` and the dashboard (`split-window -h`) SHALL carry `-c <first agent
  worktree>`, the assignment that, after the pane-1/2 swap, places the first
  agent's worktree under the agent's command

#### Scenario: Later agents unaffected

- **GIVEN** a supervisor session with two or more coding agents
- **THEN** the second and later agents (created by their own
  `split-window -c <worktree>` with no swap) SHALL each run in their own
  worktree, as before

### Requirement: Session builder applies double-line borders

The tmux session builder SHALL set
`pane-border-lines double` on the `paw-<project>` session
immediately after the session is created. The option SHALL be
scoped to the session (`tmux set-option -t <session>`), not
to the tmux server or to other windows. Double lines (`═║`) read
as a stronger row separator than single/heavy lines; tmux has no
inter-pane margin or padding (panes tile flush), so the divider
weight and the label bar are the only levers for perceived
separation between rows.

#### Scenario: Double-line border option is set on the session

- **WHEN** the session builder constructs a new
  `paw-<project>` session
- **THEN** the resulting `tmux set-option` invocations
  SHALL include `-t paw-<project> pane-border-lines double`

#### Scenario: Option does not leak to other sessions

- **GIVEN** another tmux session unrelated to git-paw
- **WHEN** the git-paw session builder runs
- **THEN** the other session's `pane-border-lines` setting
  SHALL be unchanged (verified via
  `tmux show-options -t <other-session> -v
  pane-border-lines`)

### Requirement: Per-pane title labelling

The session builder SHALL set each pane's title via
`tmux select-pane -t <pane> -T '<title>'` after pane
creation. Pane 0 SHALL receive the title `supervisor`. Pane 1
SHALL receive the title `dashboard`. Each agent pane SHALL
receive a title equal to its branch_id (e.g.
`feat/cold-start-ci-parity`).

In addition to `select-pane -T`, the session builder SHALL set a
pane-scoped user option `@paw_role` to the same label via
`tmux set-option -p -t <pane> @paw_role '<title>'`. This option is
the authoritative, stable source of the border label: the agent CLI
running in a pane emits OSC title escape sequences that overwrite
`#{pane_title}` with its current activity (e.g. `Searching files…`),
so the `select-pane -T` value does not survive past the CLI's first
title update. The `@paw_role` pane option is git-paw's own and is
never overwritten by the CLI, so the role label remains stable for
the life of the pane. The `set-option -p @paw_role` call SHALL be a
*soft* command (a non-zero exit on older tmux warns and the build
continues, matching the border affordances).

#### Scenario: Each pane gets a stable @paw_role option

- **GIVEN** an agent attached at pane index N for branch `feat/foo`
- **WHEN** the session builder completes
- **THEN** `tmux show-options -p -t paw-<project>:0.N @paw_role`
  SHALL return `feat/foo`, and this value SHALL NOT change when the
  CLI subsequently sets `#{pane_title}` via an OSC sequence

#### Scenario: Supervisor pane title is supervisor

- **WHEN** the session builder completes
- **THEN** `tmux display-message -t paw-<project>:0.0 -p
  '#{pane_title}'` SHALL return `supervisor`

#### Scenario: Dashboard pane title is dashboard

- **WHEN** the session builder completes
- **THEN** `tmux display-message -t paw-<project>:0.1 -p
  '#{pane_title}'` SHALL return `dashboard`

#### Scenario: Agent pane title is the branch id

- **GIVEN** an agent attached at pane index N for branch
  `feat/foo`
- **WHEN** the session builder completes
- **THEN** `tmux display-message -t paw-<project>:0.N -p
  '#{pane_title}'` SHALL return `feat/foo`

#### Scenario: Add via git paw add sets the new pane's title

- **GIVEN** an active session and the user runs
  `git paw add feat/bar` per [[git-paw-add]]
- **WHEN** the new pane is created
- **THEN** the new pane's title SHALL be `feat/bar`

### Requirement: Pane border format renders the role label

The session builder SHALL set `pane-border-format` to a reverse-video
label bar —
`#[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]`
— and `pane-border-status top` so each pane shows its index and role
label as a colored header chip above the pane content (the reverse-video
styling makes the label read as a header bar rather than plain text on
the divider line, aiding row separation). The format SHALL prefer the
pane-scoped `@paw_role` option (set per [Per-pane title labelling]) and
fall back to `#{pane_title}` only when `@paw_role` is unset (e.g. a
user-created pane). This keeps the role label stable even after the agent
CLI overwrites `#{pane_title}` with its current activity via OSC title
escape sequences.

#### Scenario: Border format is the reverse-video bar preferring @paw_role

- **WHEN** the session builder completes
- **THEN** the session's `pane-border-format` SHALL be exactly
  `#[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]`,
  and `pane-border-status` SHALL be `top`

#### Scenario: Role label survives a CLI title overwrite

- **GIVEN** a built session where pane 0's `@paw_role` is `supervisor`
- **WHEN** the CLI in pane 0 emits an OSC sequence that sets
  `#{pane_title}` to `Thinking…`
- **THEN** the rendered border label for pane 0 SHALL still read
  `0: supervisor` (the format resolves `@paw_role`, not `#{pane_title}`)

### Requirement: Active pane visually distinct

The session builder SHALL set
`pane-active-border-style fg=colour45,bold` and
`pane-border-style fg=colour238` so the focused pane's
border colour visually stands out from the others.

#### Scenario: Active border style is applied

- **WHEN** the session builder completes
- **THEN** the session's `pane-active-border-style` SHALL
  contain `colour45,bold`, and `pane-border-style` SHALL contain
  `colour238`

### Requirement: border_affordances config field

The system SHALL accept `[layout].border_affordances` as a
boolean config field defaulting to `true`. When `false`, the
session builder SHALL skip every `set-option` invocation and
every `select-pane -T` call described in this capability,
leaving the user's tmux defaults in effect.

#### Scenario: Default true applies all affordances

- **GIVEN** no `[layout]` section in config (or
  `border_affordances` unset)
- **WHEN** the session builder runs
- **THEN** all five `set-option` invocations and the
  per-pane title sets SHALL be emitted

#### Scenario: Explicit false skips all affordances

- **GIVEN** `[layout].border_affordances = false`
- **WHEN** the session builder runs
- **THEN** none of the five `set-option` invocations and
  none of the per-pane title sets SHALL be emitted; the
  session SHALL inherit the user's default tmux styling

### Requirement: Graceful degradation on older tmux

The session builder SHALL tolerate `tmux set-option` failures
for options unsupported by older tmux versions. The builder
SHALL emit a stderr warning naming the unsupported option and
SHALL continue building the session.

#### Scenario: Unsupported option produces a stderr warning

- **GIVEN** a tmux version where `pane-border-lines double`
  is not recognised (pre-3.2)
- **WHEN** the session builder runs
- **THEN** the build SHALL complete without fatal error,
  and stderr SHALL contain a warning naming the unsupported
  option

#### Scenario: Other affordances still apply when one fails

- **GIVEN** the same older-tmux scenario where
  `pane-border-lines double` fails
- **WHEN** the session builder runs
- **THEN** the other affordances (title format, status
  position, active-border style) SHALL still be set, since
  they have shipped in tmux since 2.3

### Requirement: Applies to both supervisor and non-supervisor sessions

The pane affordances SHALL apply to every git-paw-managed
tmux session regardless of supervisor mode. The
`[layout].border_affordances` config field SHALL govern both
`git paw start` (no supervisor) and `git paw start
--supervisor` paths.

#### Scenario: Non-supervisor session also receives affordances

- **GIVEN** a `git paw start` (no `--supervisor`) session
  with `border_affordances = true`
- **WHEN** the session builder completes
- **THEN** all the documented affordances SHALL be applied
  to the non-supervisor session's panes

### Requirement: README Supervisor Mode quick start documents the flags

The README's "Quick Start: Supervisor Mode" section SHALL
document the `--no-supervisor` opt-out flag and the
`start --force` flag for bypassing the uncommitted-spec validation
warning. Each flag SHALL appear at least once in a command-line
example within the section.

#### Scenario: Quick start supervisor mentions --no-supervisor

- **WHEN** the README's Quick Start: Supervisor Mode section is inspected
- **THEN** it contains the substring `--no-supervisor`

#### Scenario: Quick start supervisor mentions --force

- **WHEN** the README's Quick Start: Supervisor Mode section is inspected
- **THEN** it contains the substring `--force` (in the context of `start --force`)

### Requirement: Architecture chapter pins the supervisor-as-pane layout

`docs/src/architecture.md` SHALL describe the supervisor-mode
tmux layout established by the `supervisor-as-pane` archive:
pane 0 is the supervisor, pane 1 is the dashboard, and the agent
panes occupy indices 2 onwards in a row-major grid below the top
row. The chapter SHALL NOT describe the v0.4 layout (dashboard
at pane 0) as the current layout.

#### Scenario: Architecture chapter places supervisor at pane 0

- **WHEN** the supervisor-mode layout description in `architecture.md` is inspected
- **THEN** it states that the supervisor is at pane 0
- **AND** it states that the dashboard is at pane 1
- **AND** it does NOT state that the dashboard is at pane 0 as the default

### Requirement: Quick Start Supervisor chapter is internally consistent

`docs/src/quick-start-supervisor.md` SHALL describe a single,
consistent pane layout throughout. The chapter SHALL NOT contain
contradictory statements about which pane is the supervisor and
which is the dashboard. The canonical layout is:
supervisor at pane 0, dashboard at pane 1, agent panes at
indices 2 onwards.

#### Scenario: Quick start supervisor chapter is internally consistent on pane indices

- **WHEN** `docs/src/quick-start-supervisor.md` is inspected
- **THEN** every reference to the supervisor pane resolves to pane index 0
- **AND** every reference to the dashboard pane resolves to pane index 1
- **AND** the chapter does NOT contain any sentence stating that the dashboard is at pane 0 in supervisor mode

### Requirement: Quick Start Supervisor chapter does not reference nonexistent broker messages

`docs/src/quick-start-supervisor.md` SHALL NOT reference broker
message types that do not exist in `src/broker/messages.rs`.
Specifically the chapter SHALL NOT contain the substrings
`agent.register` or `agent.done` as broker message variants.

#### Scenario: Quick start supervisor chapter does not mention agent.register

- **WHEN** `docs/src/quick-start-supervisor.md` is inspected
- **THEN** it does NOT contain the substring `agent.register`

#### Scenario: Quick start supervisor chapter does not mention agent.done

- **WHEN** `docs/src/quick-start-supervisor.md` is inspected
- **THEN** it does NOT contain the substring `agent.done`

### Requirement: Quick Start Supervisor chapter reflects shipped features

`docs/src/quick-start-supervisor.md` SHALL NOT advertise as
"not yet supported" any feature that has shipped. The
v0.4-era "What's NOT Yet Supported in v0.4.0" section listing
conflict detection and learnings mode as deferred SHALL be
removed or rewritten so that readers see those features as
shipped.

#### Scenario: Quick start supervisor chapter does not mark conflict detection as deferred

- **WHEN** the chapter is inspected
- **THEN** the substrings `conflict detection` and `learnings`
  do not appear inside a section that describes them as not yet
  supported

### Requirement: `docs/src/user-guide/supervisor.md` SHALL consolidate the supervisor surfaces

The user-guide supervisor chapter SHALL include the following subsections (or equivalent headings):

1. **Spec audit governance sub-step** — references `docs/src/user-guide/governance.md` and the five doc-checklist examples (DoD, ADR, security, test-strategy, constitution).
2. **Common dev-command allowlist** — describes the preset, opt-out via `[supervisor.common_dev_allowlist].enabled = false`, and the `extra` field; cross-links to `docs/src/configuration/README.md`.
3. **Repo-configurable gate commands** — names the six `[supervisor]` gate-command keys (`test_command`, `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`) and the `(not configured)` graceful-skip behaviour; cross-links to `docs/src/configuration/README.md`.
4. **Broker-side conflict detector** — names the three failure shapes (forward, in-flight, ownership) and the `[conflict-detector]` token; cross-links to `docs/src/user-guide/conflict-detection.md`.
5. **Learnings aggregator** — at minimum a one-line cross-link to `docs/src/user-guide/learnings.md`.
6. **When the user types in your pane** — mirrors the bundled-skill section of the same name, covering status questions, directives, and judgement-call asks.
7. **Merge orchestration** — mirrors the bundled-skill section, covering the topological order from `agent.blocked` events, per-branch `git merge --ff-only` + test loop, cycle handling.

#### Scenario: Supervisor user-guide names governance sub-step + cross-link

- **WHEN** `docs/src/user-guide/supervisor.md` is inspected
- **THEN** the content SHALL contain a heading or paragraph naming the governance sub-step inside spec audit
- **AND** SHALL link to `docs/src/user-guide/governance.md`

#### Scenario: Supervisor user-guide names the common dev-command allowlist

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section with a heading approximately "Common dev-command allowlist" or equivalent
- **AND** SHALL mention `[supervisor.common_dev_allowlist]` and the `extra` field

#### Scenario: Supervisor user-guide names the gate-command templating

- **WHEN** the file is inspected
- **THEN** it SHALL contain prose stating that supervisor skill gate commands are repo-configurable
- **AND** SHALL name at least three of the six new `[supervisor]` gate-command keys
- **AND** SHALL mention the `(not configured)` graceful-skip behaviour

#### Scenario: Supervisor user-guide names the broker-side conflict detector

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section describing the broker-side conflict detector
- **AND** SHALL name the three failure shapes (forward, in-flight, ownership)
- **AND** SHALL link to `docs/src/user-guide/conflict-detection.md`

#### Scenario: Supervisor user-guide cross-links the learnings aggregator chapter

- **WHEN** the file is inspected
- **THEN** the content SHALL include a link to `docs/src/user-guide/learnings.md`

#### Scenario: Supervisor user-guide mirrors "When the user types in your pane"

- **WHEN** the file is inspected
- **THEN** the content SHALL include a section approximately named "When the user types in your pane" (or substantively equivalent)
- **AND** SHALL describe at least the three categories of user input (status question, directive, judgment-call ask)

#### Scenario: Supervisor user-guide mirrors "Merge orchestration"

- **WHEN** the file is inspected
- **THEN** the content SHALL include a section describing supervisor-driven merge orchestration
- **AND** SHALL mention the topological order derived from `agent.blocked` events
- **AND** SHALL mention `git merge --ff-only` as the per-branch merge command
