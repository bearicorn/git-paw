## MODIFIED Requirements

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

### Requirement: Supervisor AGENTS.md from supervisor skill template

The system SHALL render the orchestrator skill (resolved via `skills::resolve("supervisor")`) with the configured gate commands, the session's spec-path doctrine, the "Governance documents" section (per `boot-block`), and — when `--unattended` — the drive-loop coordination directive, and SHALL write the result to the repo-root sidecar `.git-paw/AGENTS.local.md` (gitignored and git-excluded, via the same sidecar mechanism used for worktree assignments) before session state is saved. No pane reads this file: it is the orchestrator context for the operator's own agent session driving the run from outside tmux. `git paw start` SHALL print the sidecar path so the operator knows where to point that session.

#### Scenario: Orchestrator context is rendered to the repo-root sidecar

- **GIVEN** the supervisor skill template is resolvable
- **WHEN** `cmd_supervisor()` completes its launch sequence
- **THEN** `<repo_root>/.git-paw/AGENTS.local.md` SHALL exist and contain the rendered orchestrator skill content
- **AND** the tracked `<repo_root>/AGENTS.md` SHALL be byte-identical to its pre-launch content

#### Scenario: Configured governance documents reach the orchestrator context

- **GIVEN** `[governance] dod = "docs/dod.md"` and a supervised launch
- **WHEN** the repo-root sidecar is inspected after `cmd_supervisor()` completes
- **THEN** it SHALL contain the heading `## Governance documents` and a bullet naming `docs/dod.md`

#### Scenario: Unattended context carries the drive-loop directive

- **GIVEN** a supervised session launched with `--unattended`
- **WHEN** the repo-root sidecar is inspected
- **THEN** it SHALL contain the drive-loop coordination directive

#### Scenario: Launch output names the sidecar

- **WHEN** `git paw start --supervisor` completes
- **THEN** stdout SHALL contain `.git-paw/AGENTS.local.md`

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

`docs/src/quick-start-supervisor.md` SHALL describe a single, consistent pane layout throughout: dashboard at pane 0, agent panes at indices 1 onwards, and no supervisor pane. The chapter SHALL NOT contain any sentence placing a supervisor at pane 0 or the dashboard at pane 1.

#### Scenario: Quick start chapter is internally consistent on pane indices

- **WHEN** `docs/src/quick-start-supervisor.md` is inspected
- **THEN** every reference to the dashboard pane resolves to pane index 0
- **AND** every reference to an agent pane resolves to an index of 1 or greater
- **AND** the chapter does NOT contain a sentence stating that a supervisor pane exists inside the session

## REMOVED Requirements

### Requirement: Supervisor pane approval flags

**Reason**: There is no supervisor pane to launch. The orchestrator runs outside the session under the operator's own permission mode.

**Migration**: `[supervisor].approval` remains parseable and is reserved for the orchestrator's own git-paw-initiated invocations (bound by the `orchestrator-escalation-handoff` change). Coding-agent pane flags keep resolving from `agent_approval`; `--dry-run` reports the agent approval level only.
