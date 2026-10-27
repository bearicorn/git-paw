# supervisor-launch Specification

## Purpose
Orchestrates the full supervisor session launch via `cmd_supervisor()` — computing the layout, creating worktrees and the tmux pane structure (supervisor, dashboard, coding-agent grid), injecting the broker URL and per-pane boot prompts, and branching between the return-with-attach-hint and `--unattended` in-process drive-loop paths. It also resolves whether `git paw start` enters supervisor mode from CLI flags, config, and interactive prompts (with launch- and purge-time git safety guards warning on uncommitted specs before `--from-specs` launch and on unmerged worktree commits before `purge`); ensures the first coding agent's pane runs its CLI in that agent's own worktree rather than the supervisor's repo root (assigning split-time `-c <cwd>` values to compensate for the pane-1/2 swap); and applies tmux visual affordances (double-line pane borders, per-pane role labels via a stable `@paw_role` option, a reverse-video border-format header bar, and active-pane border styling) gated by the `[layout].border_affordances` config field and degrading gracefully on older tmux.
## Requirements
### Requirement: Supervisor auto-start flow

The system SHALL implement `cmd_supervisor()` (in `src/commands/supervisor.rs`) that orchestrates the full supervised-session launch. The function SHALL execute the following steps in order:

1. Load config. Resolve the orchestrator CLI from `[supervisor].cli`, falling back to `default_cli`, when either is set. An absent orchestrator CLI SHALL NOT fail the launch — it is used only to seed the roster's `supervisor` row and by headless escalation, never to run a pane.
2. Scan specs via the spec mode or resolve branches from flags.
3. **Hard-cap check**: agent count SHALL NOT exceed 25; above this, return a `PawError` with an actionable "split into multiple sessions" hint.
4. **Compute layout proportions**: `agents_per_row = 5`, `agent_rows = ceil(agents / 5)`, `total_rows = agent_rows + 1`; the dashboard row takes 50% of the height and each agent row `50 / agent_rows` percent.
5. Create worktrees for each branch (with `-b` fallback for new branches).
6. Generate each worktree's sidecar `AGENTS.md` view with spec content, file ownership, coordination skill, and inter-agent rules.
7. **Build the tmux session**: pane 0 = dashboard (`git-paw __dashboard` in `repo_root`) as the full-width top row; panes 1..N = coding agents, row-major, up to `agents_per_row` per row. No pane SHALL run an agent CLI at the repository root.
8. Inject `GIT_PAW_BROKER_URL` via `tmux set-environment` before pane creation.
9. For each agent pane: construct the CLI launch command with approval flags resolved from `agent_approval`.
10. Execute the tmux session in detached mode.
11. Wait for each coding-agent pane to pass the launch-readiness gate.
12. **Inject the initial prompt into each coding-agent pane** (boot block + task prompt). The dashboard pane receives no keystrokes.
13. Render the orchestrator context into the repo-root sidecar (per "Supervisor AGENTS.md from supervisor skill template") and print its path alongside the attach hint. The launcher SHALL NOT publish any broker message on behalf of the orchestrator.
14. Save session state with `mode = Supervisor`, `dashboard_pane = Some(0)`, and no orchestrator entry.
15. **Branch on `--unattended`:**
    - When `--unattended` is **absent**: print the attach hint and the orchestrator-context hint, then return `Ok(())`. `cmd_supervisor()` does NOT block on any process.
    - When `--unattended` is **present**: run the in-process unattended drive loop (per `supervisor-unattended-operation`), which blocks until a completion, escalation-summary, stuck, or heartbeat exit condition is reached, then print the exit summary and return. The drive loop SHALL NOT require an attached interactive terminal.

The Rust merge loop SHALL NOT be invoked from `cmd_supervisor`. Merge orchestration is the external orchestrator's responsibility per the orchestrator skill.

#### Scenario: Supervisor auto-start launches the dashboard and coding panes only

- **GIVEN** a valid supervisor config and two spec branches
- **WHEN** `cmd_supervisor()` is called
- **THEN** a tmux session named `paw-<project>` SHALL exist in detached mode
- **AND** pane 0 SHALL be the dashboard (`git-paw __dashboard`)
- **AND** panes 1 and 2 SHALL run the coding-agent CLI commands in their worktrees
- **AND** no pane SHALL run an agent CLI whose working directory is `repo_root`

#### Scenario: Hard cap rejects more than 25 agents

- **GIVEN** a configuration that resolves to 26 or more spec branches
- **WHEN** `cmd_supervisor()` is called
- **THEN** the function SHALL return a `PawError` before any tmux command runs
- **AND** the error message SHALL state the requested count, the maximum (25), and a hint suggesting `--branches <subset>` for splitting

#### Scenario: Layout proportions match agent count

- **GIVEN** `agents = 10` (computed `agent_rows = 2`, `total_rows = 3`)
- **WHEN** the tmux session is built
- **THEN** the dashboard row SHALL occupy 50% of the vertical space and each agent row SHALL occupy 25%

- **GIVEN** `agents = 20` (computed `agent_rows = 4`, `total_rows = 5`)
- **WHEN** the tmux session is built
- **THEN** the dashboard row SHALL occupy 50% and each of the 4 agent rows SHALL occupy 12.5%

#### Scenario: Dashboard row spans the full width

- **GIVEN** any valid supervised session with broker enabled
- **WHEN** the tmux layout is built
- **THEN** pane 0 (dashboard) SHALL span the full window width as the top row
- **AND** no other pane SHALL share the top row

#### Scenario: Broker URL is injected before pane creation

- **GIVEN** `cmd_supervisor()` is executing
- **WHEN** the tmux session is created
- **THEN** `GIT_PAW_BROKER_URL` SHALL be set in the tmux session environment before any pane is created

#### Scenario: Approval flags are injected per agent pane

- **GIVEN** `agent_approval = "full-auto"` in supervisor config and `cli = "claude"` for the agents
- **WHEN** `cmd_supervisor()` constructs each agent's launch command
- **THEN** the command SHALL include `--dangerously-skip-permissions`

#### Scenario: Launch does not require an orchestrator CLI

- **GIVEN** a config with `[supervisor] enabled = true`, no `cli` key, no `default_cli`, and `--cli claude` on the command line
- **WHEN** `cmd_supervisor()` is called
- **THEN** the launch SHALL succeed
- **AND** each coding-agent pane SHALL run `claude`

#### Scenario: cmd_supervisor returns immediately with hints when --unattended is absent

- **GIVEN** `cmd_supervisor()` completes the launch sequence successfully WITHOUT `--unattended`
- **WHEN** all panes are created and prompts injected
- **THEN** stdout SHALL contain "Supervisor session 'paw-<project>' launched", the manual-attach command (`tmux attach -t paw-<project>`), and the orchestrator-context path `.git-paw/AGENTS.local.md`
- **AND** `cmd_supervisor()` SHALL return `Ok(())` without blocking on any process

#### Scenario: cmd_supervisor drives the loop in-process when --unattended is present

- **GIVEN** `cmd_supervisor()` completes the launch sequence successfully WITH `--unattended`
- **WHEN** all panes are created and prompts injected
- **THEN** `cmd_supervisor()` SHALL run the in-process unattended drive loop rather than returning immediately
- **AND** the drive loop SHALL block until a completion, escalation-summary, stuck, or heartbeat exit condition is reached
- **AND** on exit the process SHALL print the drive-loop summary and return

#### Scenario: cmd_supervisor does NOT call the Rust merge loop

- **GIVEN** any valid supervised session
- **WHEN** `cmd_supervisor()` runs to completion
- **THEN** no `run_merge_loop` (or equivalent Rust merge orchestration) SHALL execute

### Requirement: Initial prompt injection via tmux send-keys

After the tmux session is created in detached mode, the system SHALL wait until each coding agent pane passes the launch-readiness gate (`tmux-orchestration`, "Launch-readiness gate before boot-block injection"), then inject the initial task prompt for that pane and submit it with the split-send mechanism (`cli-resolution`, "Boot prompt submitted via split-send + settle delay").

The initial task prompt SHALL be constructed by appending a per-agent **task prompt** to the standardized boot block (separated by a blank line), followed by the worktree rule (`boot-block`, "Launch prompts name the agent's worktree root"). The task prompt SHALL be derived from the agent's associated `SpecEntry` (if any) via the pure helper `build_task_prompt(spec_entry: Option<&SpecEntry>) -> TaskPrompt`, which returns the task prompt's prose and, when the backend's bundled task-prompt templates declare one, a workflow entry point: a single command line taken from the backend's template with its placeholders substituted. The system SHALL NOT hard-code an entry-point command anywhere but in those templates. The prose is what the initial task prompt carries; the entry point SHALL NOT be embedded in it and SHALL be submitted as its own line per `boot-block`, "A declared workflow entry point is submitted as its own composer line". Because the managed assignment block moved out of the worktree-root `AGENTS.md` into the gitignored sidecar `.git-paw/AGENTS.local.md` (`git_paw::agents::SIDECAR_REL_PATH`) — which coding CLIs do NOT auto-load — every arm of `build_task_prompt` SHALL first point the agent at that sidecar. `build_task_prompt` SHALL dispatch on `SpecEntry.backend`:

1. `SpecBackendKind::OpenSpec` — the prose SHALL point the agent at the sidecar and state that its workflow command follows as the next message. The prose SHALL NOT contain `/opsx:apply` and SHALL NOT contain the path prose `openspec/changes/` (the sidecar already carries the artifact map). The entry point SHALL be the slash command `/opsx:apply {id}` (`{id} = spec_entry.id`), submitted as its own line so paste-aware CLIs parse it as a slash-command invocation and a weaker model cannot narrate it instead of running it.
2. `SpecBackendKind::Markdown` — the prose SHALL point the agent at the sidecar for the project rules and full spec, AND name the sibling-artifact directory `openspec/changes/{id}/`. It SHALL NOT contain `/opsx:apply` and SHALL declare no entry point (this backend has no slash-command apply workflow).
3. `SpecBackendKind::SpecKit` — the prose SHALL point the agent at the sidecar, which carries the project rules and the full Spec Kit assignment inline, and SHALL NOT name an `openspec/changes/` directory (per `core-lang-agnostic`). It SHALL NOT contain `/opsx:apply` and SHALL declare no entry point.
4. `SpecBackendKind::Superpowers` — the prose SHALL point the agent at the sidecar for the project rules and the full superpowers plan (goal, tasks, exact file paths, per-step verification commands), instructing the agent to work the steps in order and flip `- [ ]` to `- [x]` as each lands. It SHALL declare no entry point.
5. When no spec is associated with the agent's branch (the `--branches` path), the prose SHALL be the verbatim fallback `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."` and there SHALL be no entry point.

The task prompt SHALL NOT contain the spec body itself, nor a truncated heading from the spec body. The full spec body remains the source of truth for the sidecar's generation (`WorktreeAssignment.spec_content` is unchanged); only the injected task-prompt portion changes per backend.

The initial prompt SHALL be submitted with exactly one `Enter`, sent as a separate keystroke after the settle delay; a declared entry point SHALL be the only other line the launch flow submits to that pane. The longer pointer prompts may still trip paste-aware CLIs' paste-buffer behaviour, which the supervisor agent recovers from via the paste-buffer-recovery skill (see the `agent-skills` capability).

#### Scenario: Initial prompt is injected once the pane is ready

- **GIVEN** two coding agent panes have been created
- **WHEN** the launch flow injects initial prompts
- **THEN** each agent pane SHALL first pass the launch-readiness gate
- **AND** each SHALL receive its composed prompt typed literally, followed by a separate `Enter` after the settle delay

#### Scenario: Default prompt when no spec content

- **GIVEN** an agent pane with no spec file assigned
- **WHEN** the initial prompt is injected
- **THEN** the injected task-prompt portion SHALL be the verbatim fallback `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."`

#### Scenario: Launch flow sends one Enter per submitted line

- **GIVEN** N coding agent panes
- **WHEN** the launch flow runs through the prompt-injection loop
- **THEN** a pane whose task prompt declares no entry point SHALL receive exactly one standalone `Enter`, submitting the composed prompt
- **AND** a pane whose task prompt declares an entry point SHALL receive exactly two: one submitting the composed prompt and one submitting the entry point
- **AND** the system SHALL NOT emit any other standalone `Enter` keystroke to the pane during the launch flow

#### Scenario: Paste-buffer recovery is delegated to the supervisor skill

- **GIVEN** a coding agent pane on a paste-aware CLI (e.g. Claude Code v2.1.x) whose injected long prompt has been captured as a paste-buffer placeholder rather than submitted
- **WHEN** the supervisor agent's monitoring loop next inspects the pane via `tmux capture-pane`
- **THEN** the supervisor SHALL apply the paste-buffer-recovery sub-case from the embedded skill (`agent-skills` capability)
- **AND** the launch flow itself SHALL have already exited; the launch flow is NOT responsible for retrying the keystroke

#### Scenario: OpenSpec task prompt declares opsx:apply as its entry point

- **GIVEN** a coding agent on branch `feat/my-change` whose associated spec entry has `id = "my-change"` and `backend = SpecBackendKind::OpenSpec`
- **WHEN** the launch flow builds the task prompt for that agent
- **THEN** the prose returned by `build_task_prompt(Some(&entry))` SHALL contain the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the prose SHALL NOT contain the substring `/opsx:apply` nor the substring `openspec/changes/`
- **AND** the prose SHALL NOT contain any portion of the spec's prompt body
- **AND** the entry point SHALL equal `/opsx:apply my-change`

#### Scenario: Markdown-backed task prompt uses the sidecar pointer

- **GIVEN** a coding agent on branch `feat/my-feature` whose associated spec entry has `id = "my-feature"` and `backend = SpecBackendKind::Markdown`
- **WHEN** the launch flow builds the task prompt for that agent
- **THEN** the prose SHALL contain the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the prose SHALL contain the substring `openspec/changes/my-feature`
- **AND** the prose SHALL NOT contain `/opsx:apply`
- **AND** there SHALL be no entry point

Test: `main::tests::task_prompt_markdown_backend_uses_sidecar_pointer`

#### Scenario: Spec Kit task prompt names no artifact directory and no entry point

- **GIVEN** a coding agent whose associated spec entry has `backend = SpecBackendKind::SpecKit`
- **WHEN** the launch flow builds the task prompt for that agent
- **THEN** the prose SHALL contain the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the prose SHALL NOT contain `openspec/changes/` nor `/opsx:apply`
- **AND** there SHALL be no entry point

Test: `main::tests::build_task_prompt_speckit_does_not_reference_openspec_changes_path`

#### Scenario: No-spec fallback points at the sidecar verbatim

- **WHEN** `build_task_prompt(None)` is called
- **THEN** its prose SHALL equal `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."` byte-for-byte
- **AND** there SHALL be no entry point

#### Scenario: The composed prompt ends with the worktree rule

- **GIVEN** a coding agent whose worktree root is `/repo/.git-paw/worktrees/feat-x`
- **WHEN** the launch flow composes its initial prompt
- **THEN** the prompt SHALL be the boot block (when the broker is enabled), the task prompt's prose, and the worktree rule naming `/repo/.git-paw/worktrees/feat-x`, in that order, separated by blank lines

#### Scenario: Backend dispatch is exhaustive over SpecBackendKind

- **GIVEN** `SpecBackendKind` enumerates the backends supported in the current build (`OpenSpec`, `Markdown`, `SpecKit`, `Superpowers`)
- **WHEN** the launch flow's task-prompt construction is inspected
- **THEN** the template selector `build_task_prompt` reads SHALL match every variant of `SpecBackendKind` exhaustively, naming both the prose template and the entry-point template in each arm
- **AND** that selector SHALL be the only such match in the codebase for task-prompt purposes, so the doctor's spec-workflow probe reads the same answer rather than keeping its own
- **AND** the compiler SHALL reject the selector if a future variant is added to `SpecBackendKind` without a corresponding match arm

#### Scenario: build_task_prompt remains a pure function

- **WHEN** the launch flow's task-prompt construction is inspected
- **THEN** it SHALL be implemented as a pure function `build_task_prompt(spec_entry: Option<&SpecEntry>) -> TaskPrompt`
- **AND** the function SHALL have no I/O side effects (no filesystem reads, no process spawns, no config lookups), nor SHALL the template selector it delegates to
- **AND** the function SHALL be callable from `cfg(test)` without launching tmux

### Requirement: Supervisor AGENTS.md from supervisor skill template

The system SHALL write the repo-root sidecar `.git-paw/AGENTS.local.md` (gitignored and git-excluded) as the orchestrator's **session context** before session state is saved: the session name, the broker URL, the bundled helper paths, a pointer to the per-repo discovery file, the governance section, the drive-loop coordination directive when `--unattended`, and a pointer to the standardized orchestrator skill at `.git-paw/skills/orchestrator/SKILL.md` for the procedure. `start` SHALL re-render `.git-paw/skills/orchestrator/SKILL.md` from the resolved orchestrator skill with the same session-independent inputs `git paw init` uses (the current gate commands and the configured spec format; no broker URL or session name) so it never lags the config, SHALL rewrite `.git-paw/skills/cli-setup/SKILL.md` from the resolved `cli-setup` skill so it never lags the binary, and SHALL announce the sidecar and orchestrator-skill paths through the launch command's injected output sink (`code-architecture`, *Command entry points are a callable library surface*), which the CLI writes to stdout and an in-process caller receives as data. The sidecar SHALL NOT duplicate the skill body; the session name and broker URL SHALL appear only in the sidecar, never in the exported skill.

#### Scenario: Session context is rendered to the repo-root sidecar

- **WHEN** `cmd_supervisor()` completes its launch sequence
- **THEN** `<repo_root>/.git-paw/AGENTS.local.md` SHALL exist and contain the session name, the broker URL and the path `.git-paw/skills/orchestrator/SKILL.md`
- **AND** the tracked `<repo_root>/AGENTS.md` SHALL be byte-identical to its pre-launch content

#### Scenario: Skill file is refreshed at start

- **GIVEN** `[supervisor].test_command` changed since `git paw init`
- **WHEN** `git paw start --supervisor` runs
- **THEN** `.git-paw/skills/orchestrator/SKILL.md` SHALL render `{{TEST_COMMAND}}` as the new value

#### Scenario: The cli-setup skill export is refreshed at start

- **GIVEN** a `.git-paw/skills/cli-setup/SKILL.md` whose content differs from the resolved `cli-setup` skill
- **WHEN** `git paw start --supervisor` runs
- **THEN** the file SHALL equal the resolved `cli-setup` skill

#### Scenario: Unattended context carries the drive-loop directive

- **GIVEN** a supervised session launched with `--unattended`
- **WHEN** the repo-root sidecar is inspected
- **THEN** it SHALL contain the drive-loop coordination directive

#### Scenario: Launch output names both paths

- **WHEN** `git paw start --supervisor` completes
- **THEN** stdout SHALL contain `.git-paw/AGENTS.local.md` and `.git-paw/skills/orchestrator/SKILL.md`

#### Scenario: An in-process launch receives the announcement through the sink

- **WHEN** the supervised launch use case is called in-process with a capturing output sink
- **THEN** the sink SHALL receive the lines naming `.git-paw/AGENTS.local.md` and `.git-paw/skills/orchestrator/SKILL.md`
- **AND** nothing SHALL be written to the process's stdout

### Requirement: Supervisor self-registration

The launcher SHALL NOT publish any broker message on behalf of the orchestrator. Specifically, `cmd_supervisor` SHALL NOT call `publish_to_broker_http(..., build_status_message("supervisor", ...))` or any equivalent self-registration POST at any point in its flow. The roster's `supervisor` row SHALL appear only when the external orchestrator publishes its own `agent.status` (via the bundled helper or the MCP publish surface) as its first action on picking up the orchestrator context — so a launch that no orchestrator ever drives shows no phantom `supervisor` row. The `supervisor` agent id remains a broker wire identity used by other senders — the drive loop publishes its escalation questions and branch-refresh feedback under it, and the broker seeds that row's CLI column from `[supervisor].cli` / `default_cli` — but none of these SHALL create the roster row, because only status-kind messages populate the roster.

#### Scenario: cmd_supervisor does not self-publish on behalf of the orchestrator

- **GIVEN** `cmd_supervisor()` is called with a valid supervisor config and broker enabled
- **WHEN** the launcher reaches its `Ok(())` return
- **THEN** no `agent.status` message with `agent_id = "supervisor"` SHALL have been published by the launcher process
- **AND** the broker's `/status` endpoint SHALL NOT contain an entry for `agent_id = "supervisor"`

#### Scenario: No phantom supervisor row before an orchestrator drives

- **GIVEN** a successfully launched supervised session that no external orchestrator has yet picked up
- **WHEN** the dashboard renders a frame
- **THEN** the agent table SHALL NOT contain a `supervisor` row
- **AND** no divider SHALL be rendered above the coding-agent rows

#### Scenario: Drive-loop escalations create no supervisor row

- **GIVEN** an `--unattended` supervised session whose drive loop has escalated a coding agent's permission prompt as an `agent.question` from `supervisor`, and no external orchestrator has published
- **WHEN** the broker's `/status` endpoint is read
- **THEN** it SHALL NOT contain an entry for `agent_id = "supervisor"`

#### Scenario: Orchestrator's own publish makes the row appear

- **GIVEN** a running supervised session
- **WHEN** the external orchestrator publishes `agent.status` with `agent_id = "supervisor"`
- **THEN** the broker's `/status` endpoint SHALL list `supervisor`
- **AND** the dashboard SHALL render the `supervisor` row pinned to the top

### Requirement: Boot-prompt injection

The system SHALL prepend a standardized boot instruction block to each coding-agent pane's initial prompt. The block SHALL instruct agents on the runtime events they must publish. Only coding-agent panes receive an injected prompt; the dashboard pane SHALL receive no keystrokes.

#### Scenario: Boot block is prepended to every coding-agent prompt

- **GIVEN** `cmd_supervisor()` is launching coding agents
- **WHEN** each coding pane's initial prompt is constructed
- **THEN** every coding-agent pane's prompt SHALL begin with the standardized boot instruction block

#### Scenario: Boot block uses template substitution

- **GIVEN** an agent on branch `feat/errors` with broker URL `http://127.0.0.1:9119`
- **WHEN** the boot block is generated
- **THEN** the block contains pre-expanded helper invocations with `{{BRANCH_ID}}` replaced by `feat-errors` and `{{GIT_PAW_BROKER_URL}}` replaced by `http://127.0.0.1:9119`

#### Scenario: Dashboard pane receives no prompt

- **WHEN** the launch flow injects initial prompts
- **THEN** no `send-keys` SHALL target pane 0

### Requirement: cmd_supervisor SHALL fall back to default SupervisorConfig when none configured

`cmd_supervisor` SHALL NOT error with "supervisor mode enabled but `[supervisor]` config missing" when the loaded `PawConfig` lacks a `[supervisor]` block. Instead, `cmd_supervisor` SHALL synthesize a `SupervisorConfig::default()` value. No orchestrator CLI is required to launch a supervised session: `[supervisor].cli` and `default_cli` are consulted only to seed the roster's `supervisor` row and for headless escalation, and their absence SHALL NOT fail the launch. The coding agents' CLI SHALL resolve through the existing chain — `--cli`, then `default_cli`, then `[supervisor].cli` (the existing fallback, kept so earlier configs launch unchanged), with spec-driven branches using the spec-driven CLI chain; only when no coding-agent CLI resolves SHALL the launch fail, with an error naming `--cli` and `default_cli`.

This requirement applies symmetrically to the supervised recovery path for resumed sessions.

Rationale: `resolve_supervisor_mode` already prompts the user when no `[supervisor]` block exists. The prompt was designed to ask the user to opt in to supervised mode without forcing them to hand-author a config block first.

#### Scenario: Interactive prompt yes accepts default supervisor config

- **GIVEN** a repo with `.git-paw/config.toml` containing only `default_cli = "echo"` (no `[supervisor]` section)
- **AND** `git paw start --branches a,b` is invoked from a TTY
- **WHEN** the prompt "Start in supervisor mode?" appears and the user answers yes
- **THEN** the launch SHALL exit 0 and print the standard supervised-session-launched message
- **AND** the stderr SHALL NOT contain "supervisor mode enabled but [supervisor] config missing"
- **AND** the synthesized `SupervisorConfig` has `enabled = false`, `cli = None`, `agent_approval = ApprovalLevel::default()`, all other fields at their respective `Default` values

#### Scenario: --supervisor flag without [supervisor] config works

- **GIVEN** a repo with `.git-paw/config.toml` containing only `default_cli = "echo"`
- **WHEN** `git paw start --supervisor --branches a,b` is invoked
- **THEN** the launch SHALL exit 0
- **AND** each coding-agent pane SHALL run `echo`
- **AND** no pane SHALL run an agent CLI at the repository root

#### Scenario: Missing coding-agent CLI still errors

- **GIVEN** a repo with `.git-paw/config.toml` containing no `[supervisor]` section AND no top-level `default_cli`
- **WHEN** `git paw start --supervisor --branches a,b` is invoked without `--cli`
- **THEN** the launch SHALL exit non-zero with an error naming `--cli` and `default_cli`

#### Scenario: A configured orchestrator CLI still serves as the agents' last fallback

- **GIVEN** a config with `[supervisor] cli = "claude"`, no `default_cli`, and no `--cli`
- **WHEN** `git paw start --supervisor --branches a,b` is invoked
- **THEN** each coding-agent pane SHALL run `claude`

#### Scenario: Supervised recovery applies the same fallback

- **GIVEN** a previously-launched supervised session that has been stopped
- **AND** the repo's `.git-paw/config.toml` no longer has a `[supervisor]` block
- **WHEN** `git paw start` is invoked and routes to supervised recovery
- **THEN** the recovery SHALL succeed using the default supervisor config
- **AND** SHALL NOT error on the missing `[supervisor]` section

### Requirement: Resumed coding-agent panes SHALL spawn in their worktree cwd

When `recover_session` rebuilds a stopped session (bare or supervised), every coding-agent pane's tmux working directory SHALL be the pane's `worktree_path` from the session JSON — NOT the repo root.

Implementation SHALL pass `-c <pane.worktree>` on every `split-window` that creates a coding-agent pane. The `cd <worktree> && <cli_command>` pattern via `send-keys` is forbidden for agent panes because it races with shell startup.

The dashboard pane SHALL spawn in the repo root via `new-session -c <repo_root>`.

#### Scenario: Bare-session recovery places each agent pane in its worktree

- **GIVEN** a stopped bare-mode session with two coding agents in worktrees `/path/to/repo-feat-a` and `/path/to/repo-feat-b`
- **WHEN** `git paw start` resumes the session and panes have settled
- **THEN** `tmux display-message -t <session>:0.1 -p "#{pane_current_path}"` SHALL output `/path/to/repo-feat-a`
- **AND** `tmux display-message -t <session>:0.2 -p "#{pane_current_path}"` SHALL output `/path/to/repo-feat-b`

#### Scenario: Supervised recovery places each agent pane in its worktree

- **GIVEN** a stopped supervised session with two coding agents in worktrees `/path/to/repo-feat-a` and `/path/to/repo-feat-b`
- **WHEN** `git paw start` resumes the session
- **THEN** the dashboard pane (`0.0`) SHALL have `pane_current_path = /path/to/repo`
- **AND** the agent panes (`0.1`, `0.2`) SHALL have `pane_current_path` equal to their respective `worktree_path` values
- **AND** the CLI command sent to each agent pane SHALL NOT be prefixed with `cd <worktree> &&`

#### Scenario: Agent split passes -c worktree

- **GIVEN** a supervised recovery flow building an agent pane via `split-window`
- **WHEN** the tmux command sequence is inspected
- **THEN** the `split-window` for that agent SHALL include `-c <agent.worktree>`
- **AND** the follow-up `send-keys` SHALL send only the bare CLI command

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

Before destroying worktrees, `git paw purge` SHALL check each worktree branch for commits not yet merged to the default branch, and SHALL never delete a branch that carries any. The system SHALL:

1. For each worktree branch, determine whether its tip is an ancestor of the default branch, or of the branch recorded as that worktree's base (`git merge-base --is-ancestor refs/heads/<branch> refs/heads/<target>`, every ref qualified so a same-named tag cannot answer for the branch) and, when it is an ancestor of neither, count the commits not in the recorded base — or, when none was recorded, not in the default branch
2. If any branch has unmerged commits, display a warning listing each such branch with its commit count and tip SHA, and stating that those branches will be **kept** — only their worktrees (including any uncommitted changes) and the session state are removed
3. Require either the `--force` flag or interactive confirmation ("Y" response) to proceed with worktree removal
4. If the user declines, exit without destroying any worktrees
5. Whether or not `--force` is given, retain every branch that has unmerged commits or whose merge state cannot be determined; delete only a branch git-paw created whose tip is an ancestor of the default branch or of its recorded base; and print, for each retained branch, its tip SHA and how to resume or discard it by hand

The default branch SHALL be resolved through the single default-branch resolver (see `git-operations`, "Default-branch resolution is single-source and tolerant of a missing origin/HEAD"), not a purge-local lookup. The `purge` help text (`about`, `long_about`, and the `--force` option) and the `stop` help text that describes purge SHALL state that branches with unmerged commits are kept.

#### Scenario: Purge with no unmerged commits proceeds without warning

- **GIVEN** all worktree branches have no commits beyond the default branch
- **WHEN** `git paw purge` is run
- **THEN** no unmerged commit warning SHALL be shown
- **AND** purge proceeds normally, deleting the branches git-paw created and printing each with the SHA it was at

#### Scenario: Purge with unmerged commits warns before destroying

- **GIVEN** one worktree branch has 3 commits not merged to main
- **WHEN** `git paw purge` is run without `--force`
- **THEN** a warning SHALL be displayed identifying the branch, its 3 unmerged commits, and its tip SHA
- **AND** the warning SHALL state that the branch will be kept
- **AND** the system SHALL prompt for confirmation before removing worktrees

#### Scenario: Purge --force skips confirmation but retains unmerged branches

- **GIVEN** one worktree branch git-paw created has unmerged commits
- **WHEN** `git paw purge --force` is run
- **THEN** the warning SHALL still be displayed
- **AND** purge SHALL proceed without waiting for interactive confirmation
- **AND** the worktree, tmux session, and session state SHALL be removed
- **AND** the branch SHALL still exist at the same tip afterwards
- **AND** the output SHALL name the retained branch with its tip SHA and a way to resume or discard it

#### Scenario: Confirmed purge also retains unmerged branches

- **GIVEN** one worktree branch git-paw created has unmerged commits
- **WHEN** `git paw purge` is run and the user answers "Y"
- **THEN** the worktree SHALL be removed
- **AND** the branch SHALL still exist at the same tip afterwards

#### Scenario: Purge cancelled by user preserves worktrees

- **GIVEN** one worktree branch has unmerged commits
- **WHEN** `git paw purge` is run and the user answers "N" to the confirmation
- **THEN** no worktrees SHALL be removed
- **AND** the system SHALL exit with a non-error message indicating purge was cancelled

#### Scenario: An undeterminable merge state is treated as unmerged

- **GIVEN** a worktree branch whose comparison against the default branch fails (the default branch cannot be resolved, or `git merge-base` errors)
- **WHEN** `git paw purge --force` is run
- **THEN** the branch SHALL be retained
- **AND** the output SHALL state that its merge state could not be determined

#### Scenario: Purge in a master-default repository without origin/HEAD compares against master

- **GIVEN** a repository whose default branch is `master`, with no `refs/remotes/origin/HEAD`, and a created worktree branch with one commit not in `master`
- **WHEN** `git paw purge --force` is run
- **THEN** the warning SHALL report the branch as having 1 unmerged commit
- **AND** the branch SHALL be retained

#### Scenario: Help text states unmerged branches are kept

- **WHEN** `git paw purge --help` is run
- **THEN** the output SHALL state that branches with unmerged commits are kept
- **AND** the `--force` description SHALL NOT imply that it deletes them

### Requirement: First agent pane launches in its own worktree

The supervised session build SHALL create every coding-agent pane, including the first, with a `split-window` that carries `-c <that agent's worktree>`, and SHALL send each pane's CLI command to the index that split produced. There is no pane swap and no compensated working-directory assignment: the dashboard is created by `new-session -c <repo_root>` and each agent by its own split.

#### Scenario: First agent's CLI runs in its worktree

- **GIVEN** a supervised session launched with at least one coding agent
- **WHEN** the layout is built and the first agent's CLI command is sent to its pane
- **THEN** that pane's working directory SHALL be the first agent's worktree, NOT the repo root

#### Scenario: Every agent split names its own worktree

- **WHEN** the supervised build's `split-window` commands are inspected
- **THEN** each agent's split SHALL carry `-c <its worktree>`
- **AND** no `swap-pane` command SHALL be emitted

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

The session builder SHALL set each pane's title via `tmux select-pane -t <pane> -T '<title>'` after pane creation. Pane 0 SHALL receive the title `dashboard`. Each agent pane SHALL receive a title equal to its branch_id (e.g. `feat/cold-start-ci-parity`).

In addition to `select-pane -T`, the session builder SHALL set a pane-scoped user option `@paw_role` to the same label via `tmux set-option -p -t <pane> @paw_role '<title>'`. This option is the authoritative, stable source of the border label: the agent CLI running in a pane emits OSC title escape sequences that overwrite `#{pane_title}` with its current activity, so the `select-pane -T` value does not survive past the CLI's first title update. The `@paw_role` pane option is git-paw's own and is never overwritten by the CLI. The `set-option -p @paw_role` call SHALL be a *soft* command (a non-zero exit on older tmux warns and the build continues).

#### Scenario: Each pane gets a stable @paw_role option

- **GIVEN** an agent attached at pane index N for branch `feat/foo`
- **WHEN** the session builder completes
- **THEN** `tmux show-options -p -t paw-<project>:0.N @paw_role` SHALL return `feat/foo`, and this value SHALL NOT change when the CLI subsequently sets `#{pane_title}` via an OSC sequence

#### Scenario: Dashboard pane title is dashboard

- **WHEN** the session builder completes
- **THEN** `tmux display-message -t paw-<project>:0.0 -p '#{pane_title}'` SHALL return `dashboard`

#### Scenario: Agent pane title is the branch id

- **GIVEN** an agent attached at pane index N for branch `feat/foo`
- **WHEN** the session builder completes
- **THEN** `tmux display-message -t paw-<project>:0.N -p '#{pane_title}'` SHALL return `feat/foo`

#### Scenario: Add via git paw add sets the new pane's title

- **GIVEN** an active session and the user runs `git paw add feat/bar`
- **WHEN** the new pane is created
- **THEN** the new pane's title SHALL be `feat/bar`

### Requirement: Pane border format renders the role label

The session builder SHALL set `pane-border-format` to a reverse-video label bar — `#[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]` — and `pane-border-status top` so each pane shows its index and role label as a colored header chip above the pane content. The format SHALL prefer the pane-scoped `@paw_role` option and fall back to `#{pane_title}` only when `@paw_role` is unset (e.g. a user-created pane).

#### Scenario: Border format is the reverse-video bar preferring @paw_role

- **WHEN** the session builder completes
- **THEN** the session's `pane-border-format` SHALL be exactly `#[fg=colour39,bold,reverse] #{pane_index}: #{?#{@paw_role},#{@paw_role},#{pane_title}} #[default]`, and `pane-border-status` SHALL be `top`

#### Scenario: Role label survives a CLI title overwrite

- **GIVEN** a built session where pane 1's `@paw_role` is `feat/foo`
- **WHEN** the CLI in pane 1 emits an OSC sequence that sets `#{pane_title}` to `Thinking…`
- **THEN** the rendered border label for pane 1 SHALL still read `1: feat/foo`

#### Scenario: Dashboard pane label is its role

- **WHEN** the built session's pane 0 border label is rendered
- **THEN** it SHALL read `0: dashboard`

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
leaving the user's tmux defaults in effect — except the
pane-scoped `@paw_role` option, which SHALL be set regardless.
`@paw_role` renders nothing on its own (only
`pane-border-format` reads it, and that option is skipped with
the rest), so setting it unconditionally leaves the user's tmux
styling untouched. It is set regardless because the agent-pane
liveness probe identifies coding-agent panes by it; gating it on
a styling preference would leave an affordances-off session
identifiable only by pane index, which shifts when `git paw
pause` kills the dashboard pane — reporting a live session's
agents as exited.

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
  none of the `select-pane -T` title sets SHALL be emitted; the
  session SHALL inherit the user's default tmux styling
- **AND** the pane-scoped `@paw_role` option SHALL still be set
  on every pane the builder creates

### Requirement: Graceful degradation on older tmux

The session builder SHALL tolerate `tmux set-option` failures
for options unsupported by older tmux versions. For an option
the user configured — the border affordances — the builder
SHALL emit a stderr warning naming the unsupported option and
SHALL continue building the session. For the pane-scoped
`@paw_role` option, which is git-paw's own bookkeeping rather
than a user-facing setting, the builder SHALL continue
**silently**: `set-option -p` requires tmux 3.0, the option is
set once per pane on every launch, `git paw add` and recovery,
and the failure is not actionable — the agent-pane liveness
probe falls back to pane positions when a pane carries no role.
Warning per pane on every launch would bury the warnings the
user can act on.

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

#### Scenario: An unsupported @paw_role set is silent

- **GIVEN** a tmux version where pane-scoped `set-option -p` is
  not recognised (pre-3.0)
- **WHEN** the session builder runs
- **THEN** the build SHALL complete without fatal error
- **AND** stderr SHALL NOT contain a warning for the
  `@paw_role` invocation

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

`docs/src/architecture.md` SHALL describe the supervised-session layout as it is after the orchestrator relocation: the dashboard at pane 0 as a full-width top row, coding-agent panes from index 1 in a row-major grid below, and the orchestrator running *outside* the tmux session (the operator's own agent session, reading the repo-root sidecar). The chapter SHALL NOT describe a supervisor pane at pane 0 as the current layout.

#### Scenario: Architecture chapter places the dashboard at pane 0

- **WHEN** the supervised-session layout description in `architecture.md` is inspected
- **THEN** it states that the dashboard is at pane 0 and agents start at pane 1
- **AND** it does NOT state that a supervisor pane occupies pane 0

#### Scenario: Architecture chapter locates the orchestrator outside the session

- **WHEN** `architecture.md` is inspected
- **THEN** it states that the orchestrator drives the session from outside tmux via the broker, the bundled helpers, or the MCP server

### Requirement: Quick Start Supervisor chapter is internally consistent

`docs/src/quick-start-orchestrator.md` SHALL replace `docs/src/quick-start-supervisor.md` (with an mdBook redirect from the old path) and SHALL describe a single, consistent layout throughout: dashboard at pane 0, agent panes at indices 1 onwards, no supervisor pane, and the orchestrator running in the operator's own agent session outside tmux. The chapter SHALL NOT contain any sentence placing a supervisor inside the session.

#### Scenario: Quick start chapter is internally consistent

- **WHEN** `docs/src/quick-start-orchestrator.md` is inspected
- **THEN** every reference to the dashboard pane resolves to pane index 0
- **AND** every reference to an agent pane resolves to an index of 1 or greater
- **AND** the chapter states that the orchestrator runs outside the tmux session

#### Scenario: Old path redirects

- **WHEN** `mdbook build docs/` runs
- **THEN** the built site SHALL serve a redirect from `quick-start-supervisor.html` to `quick-start-orchestrator.html`

### Requirement: Quick Start Supervisor chapter does not reference nonexistent broker messages

`docs/src/quick-start-orchestrator.md` SHALL NOT reference broker message types that do not exist in `src/broker/messages.rs`. Specifically the chapter SHALL NOT contain the substrings `agent.register` or `agent.done` as broker message variants.

#### Scenario: Quick start chapter does not mention agent.register

- **WHEN** `docs/src/quick-start-orchestrator.md` is inspected
- **THEN** it does NOT contain the substring `agent.register`

#### Scenario: Quick start chapter does not mention agent.done

- **WHEN** `docs/src/quick-start-orchestrator.md` is inspected
- **THEN** it does NOT contain the substring `agent.done`

### Requirement: Quick Start Supervisor chapter reflects shipped features

`docs/src/quick-start-orchestrator.md` SHALL describe the shipped workflow for driving a session from the operator's own agent: `git paw init` (installs the skill and the AGENTS.md pointer), `git paw start --supervisor`, loading `.git-paw/skills/orchestrator/SKILL.md` in the driving agent, sweeping and verifying via the bundled helpers or the MCP server, publishing verdicts via `sweep.sh verified`, and pairing with `--unattended`. It SHALL NOT advertise as "not yet supported" any feature that has shipped, and SHALL note that a remote-controllable driving session (e.g. via the CLI's own remote-control feature) drives git-paw without any extra topology.

#### Scenario: Quick start describes the external-orchestrator workflow

- **WHEN** the chapter is inspected
- **THEN** it SHALL mention `git paw init`, `git paw start --supervisor`, `.git-paw/skills/orchestrator/SKILL.md`, and `sweep.sh verified`

#### Scenario: Quick start does not mark shipped features as deferred

- **WHEN** the chapter is inspected
- **THEN** the substrings `conflict detection` and `learnings` do not appear inside a section that describes them as not yet supported

### Requirement: `docs/src/user-guide/supervisor.md` SHALL consolidate the supervisor surfaces

The user-guide supervisor chapter SHALL include the following subsections (or equivalent headings):

1. **Spec audit governance sub-step** — references `docs/src/user-guide/governance.md` and the five doc-checklist examples (DoD, ADR, security, test-strategy, constitution).
2. **Agent permissions** — states that git-paw does not approve permission prompts; that whether an agent's command runs without a prompt is decided by the agent CLI's own permission system (the `[supervisor] agent_approval` / `approval` level mapped to CLI flags, the CLI's own allow rules, or `full-auto` inside a sandbox); that live prompts are escalated for a human decision and answered explicitly with `sweep.sh approve <pane>`; and that `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` are deprecated and have no effect. It cross-links to `docs/src/configuration/README.md` and `docs/src/user-guide/security-posture.md`.
3. **Repo-configurable gate commands** — names the six `[supervisor]` gate-command keys (`test_command`, `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`) and the `(not configured)` graceful-skip behaviour; cross-links to `docs/src/configuration/README.md`.
4. **Broker-side conflict detector** — names the three failure shapes (forward, in-flight, ownership) and the `[conflict-detector]` token; cross-links to `docs/src/user-guide/conflict-detection.md`.
5. **Learnings aggregator** — at minimum a one-line cross-link to `docs/src/user-guide/learnings.md`.
6. **When the user types in your session** — mirrors the bundled-skill section of the same name, covering status questions, directives, and judgement-call asks.
7. **Merge orchestration** — mirrors the bundled-skill section, covering the topological order from `agent.blocked` events, per-branch `git merge --ff-only` + test loop, cycle handling, and the unattended rule (merge once every gate is verified without asking first; escalate only on a conflict, an exhausted correction cycle or a regression).
8. **Driving the session from your own agent** — the workflow: where the skill and session context live (`.git-paw/skills/orchestrator/SKILL.md`, `.git-paw/AGENTS.local.md`, the AGENTS.md pointer section), the helper and MCP surfaces the orchestrator uses, how verdicts are published, and how `--unattended` pairs with an external orchestrator.

#### Scenario: Supervisor user-guide names governance sub-step + cross-link

- **WHEN** `docs/src/user-guide/supervisor.md` is inspected
- **THEN** the content SHALL contain a heading or paragraph naming the governance sub-step inside spec audit
- **AND** SHALL link to `docs/src/user-guide/governance.md`

#### Scenario: Supervisor user-guide explains agent permissions

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section with a heading approximately "Agent permissions" or equivalent
- **AND** SHALL state that git-paw does not approve permission prompts and that the agent CLI's own permission system decides
- **AND** SHALL name `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` as deprecated with no effect

#### Scenario: Supervisor user-guide names the gate-command templating

- **WHEN** the file is inspected
- **THEN** it SHALL contain prose stating that orchestrator skill gate commands are repo-configurable
- **AND** SHALL name at least three of the six `[supervisor]` gate-command keys
- **AND** SHALL mention the `(not configured)` graceful-skip behaviour

#### Scenario: Supervisor user-guide names the broker-side conflict detector

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section describing the broker-side conflict detector
- **AND** SHALL name the three failure shapes (forward, in-flight, ownership)
- **AND** SHALL link to `docs/src/user-guide/conflict-detection.md`

#### Scenario: Supervisor user-guide cross-links the learnings aggregator chapter

- **WHEN** the file is inspected
- **THEN** the content SHALL include a link to `docs/src/user-guide/learnings.md`

#### Scenario: Supervisor user-guide mirrors "When the user types in your session"

- **WHEN** the file is inspected
- **THEN** the content SHALL include a section approximately named "When the user types in your session"
- **AND** SHALL describe at least the three categories of user input (status question, directive, judgment-call ask)
- **AND** SHALL NOT contain the heading "When the user types in your pane"

#### Scenario: Supervisor user-guide mirrors "Merge orchestration"

- **WHEN** the file is inspected
- **THEN** the content SHALL include a section describing orchestrator-driven merge orchestration
- **AND** SHALL mention the topological order derived from `agent.blocked` events
- **AND** SHALL mention `git merge --ff-only` as the per-branch merge command
- **AND** SHALL state that an unattended run merges without asking first and escalates only on a conflict, an exhausted correction cycle or a regression

#### Scenario: Supervisor user-guide documents driving from an external agent

- **WHEN** the file is inspected
- **THEN** it SHALL contain a section describing how to drive the session from the operator's own agent
- **AND** SHALL name `.git-paw/skills/orchestrator/SKILL.md` and `.git-paw/AGENTS.local.md`

### Requirement: Supervised launch honours the CLIs chosen in the interactive picker

When a supervised launch has no `--branches` and no spec selection, and so runs the interactive selection flow (branch picker, assignment-mode prompt and CLI picker), each coding agent SHALL launch with the CLI the selection mapped to its branch — the uniform choice, or that branch's own per-branch choice. The launch SHALL NOT discard the selection's CLI mapping in favour of `default_cli` or the supervisor CLI. The session-wide fallback (`--cli` > `default_cli` > the supervisor CLI) SHALL apply only to launch paths that show no CLI picker, such as `--branches` without `--cli`; spec-driven branches keep their spec CLI resolution chain.

#### Scenario: A uniform CLI pick is honoured

- **GIVEN** supervisor mode with `default_cli = "alpha"` and the supervisor CLI `alpha`, two available CLIs `alpha` and `beta`, and no `--branches`, `--specs` or `--cli`
- **WHEN** the user selects `feat/a`, chooses uniform mode and picks `beta`
- **THEN** the coding agent for `feat/a` SHALL launch with `beta`

#### Scenario: Per-branch CLI picks are honoured

- **GIVEN** supervisor mode with no `--branches`, `--specs` or `--cli`
- **WHEN** the user selects `feat/a` and `feat/b`, chooses per-branch mode, and picks `alpha` for `feat/a` and `beta` for `feat/b`
- **THEN** `feat/a`'s coding agent SHALL launch with `alpha` and `feat/b`'s with `beta`

#### Scenario: The flag path keeps the session-wide fallback

- **GIVEN** supervisor mode with `default_cli = "alpha"`
- **WHEN** the launch is given `--branches feat/a` and no `--cli`
- **THEN** no CLI picker SHALL be shown
- **AND** `feat/a`'s coding agent SHALL launch with `alpha`

### Requirement: Remote-control workflow chapter

The documentation SHALL include a chapter, `docs/src/user-guide/remote-control.md`, describing how to drive a git-paw session from a phone or another device by remote-controlling the operator's own driving agent session through that agent CLI's native remote-control feature, with git-paw's MCP server attached to that session. The chapter SHALL be the target of the "Remote control (documented workflow)" row of the controls table in `docs/src/user-guide/security-posture.md`, which names the workflow without linking to any chapter today. The chapter SHALL state that git-paw provides no bridge, tunnel or network transport of its own; SHALL include a table of which agent CLIs can act as a remote-controlled driving session, marked with the date its facts were last checked; SHALL state the caveats that the driving process must keep running on the local machine and that a remote session is lost after an outage of roughly ten minutes; and SHALL link the quick start and the MCP chapter. It SHALL NOT describe a remote-controlled supervisor pane or any topology in which an orchestrator runs inside the tmux session.

#### Scenario: Chapter states the topology and the local-process caveat

- **WHEN** the remote-control chapter is inspected
- **THEN** it SHALL state that the driving session runs on the local machine with git-paw's MCP server attached
- **AND** it SHALL state that the local process must stay running and that an outage of about ten minutes ends the remote session
- **AND** it SHALL state that git-paw ships no bridge or tunnel

#### Scenario: Chapter names supported CLIs with a check date

- **WHEN** the chapter's CLI table is inspected
- **THEN** it SHALL list each CLI's support as a remote-controlled driving session
- **AND** it SHALL carry the date the facts were last verified

#### Scenario: The security-posture controls table links the chapter

- **WHEN** the "Remote control" row of the controls table in `docs/src/user-guide/security-posture.md` is inspected
- **THEN** it SHALL link to `remote-control.md`

#### Scenario: Chapter describes no in-session orchestrator

- **WHEN** the chapter is inspected
- **THEN** it SHALL NOT contain the phrase `supervisor pane`

