## Purpose

Orchestrate tmux sessions with multiple panes, each running an AI CLI in a git worktree. Uses a builder pattern for testability and dry-run support, with configurable mouse mode and automatic tiled layout.
## Requirements
### Requirement: Check tmux availability with actionable error

The system SHALL verify tmux is installed on PATH and provide install instructions if missing.

#### Scenario: tmux is present on PATH
- **GIVEN** tmux is installed
- **WHEN** `ensure_tmux_installed()` is called
- **THEN** it SHALL return `Ok(())`

Test: `tmux::tests::ensure_tmux_installed_succeeds_when_present`

### Requirement: Create named sessions derived from project name

The system SHALL name tmux sessions as `paw-<project_name>`.

#### Scenario: Session named after project
- **GIVEN** project name `"my-project"`
- **WHEN** a session is built
- **THEN** the session name SHALL be `"paw-my-project"`

Test: `tmux::tests::session_is_named_after_project`

#### Scenario: Session creation command uses correct name
- **GIVEN** project name `"app"`
- **WHEN** a session is built
- **THEN** the commands SHALL include `new-session` with `paw-app`

Test: `tmux::tests::session_creation_command_uses_session_name`

### Requirement: Session creation passes explicit dimensions for headless environments

Both session builders (the basic `TmuxSessionBuilder` and `build_supervisor_session`) SHALL emit `tmux new-session` with `-x 200 -y 50` so the session has explicit window dimensions when created without an attached client. The user's real terminal resizes the session on `tmux attach`.

Without explicit dimensions, tmux on Linux (apt-shipped tmux 3.4+) errors with `size missing` on subsequent `split-window` operations because the layout engine can't resolve percentages without a known window size.

Additionally, immediately after the `new-session` command, both builders SHALL emit `tmux set-option -g default-size 200x50`. This pins the global default-size so subsequent split-window / resize-pane operations have a fallback size context even when no client is attached. macOS tmux honours per-session `-x/-y` for splits; Linux tmux 3.4+ requires the server-level fallback.

#### Scenario: Basic session passes -x/-y to new-session
- **GIVEN** any `TmuxSessionBuilder` with one or more panes
- **WHEN** `command_strings()` is invoked
- **THEN** the first command in the output SHALL be a `new-session` containing the substrings `-x 200` and `-y 50`

#### Scenario: Basic session sets global default-size after new-session
- **GIVEN** any `TmuxSessionBuilder` with one or more panes
- **WHEN** `command_strings()` is invoked
- **THEN** the second command in the output SHALL be `set-option -g default-size 200x50`

#### Scenario: Supervisor session passes -x/-y to new-session
- **GIVEN** a `build_supervisor_session` invocation with supervisor + dashboard + N agent panes
- **WHEN** the command list is built
- **THEN** the first emitted command SHALL be a `new-session` containing `-x 200` and `-y 50`

#### Scenario: Supervisor session sets global default-size after new-session
- **GIVEN** a `build_supervisor_session` invocation
- **WHEN** the command list is built
- **THEN** the second emitted command SHALL be `set-option -g default-size 200x50`

#### Scenario: Headless supervisor launch succeeds under socket isolation
- **GIVEN** a cold tmux server (no pre-existing client, isolated socket via `TMUX_TMPDIR`)
- **WHEN** `git paw start --supervisor --branches a,b` is invoked
- **THEN** the supervisor session SHALL launch successfully
- **AND** stderr SHALL NOT contain `Tmux error: size missing`
- **AND** the exit code SHALL be 0

Test: `tmux::tests::built_session_can_be_executed_and_killed`, `tmux::tests::supervisor_top_row_split_50_50`, and the `cli_supervisor_no_config::supervisor_without_section_uses_default_when_default_cli_present` integration test.

#### Scenario: Supervisor splits use `-l <N>%` syntax (tmux 3.1+) not deprecated `-p <N>`
- **GIVEN** a `build_supervisor_session` invocation with any agent count
- **WHEN** the command list is built
- **THEN** every emitted `split-window` command SHALL use the `-l <N>%` length flag, NOT the deprecated `-p <N>` percentage flag

Rationale: Linux apt-tmux 3.4 (Ubuntu 24.04) emits `cmd-split-window.c: "size missing"` when `-p` cannot resolve the percentage against the parent pane's laid-out size — on a detached server with no attached client the pane geometry is unresolved. `-l <N>%` resolves against the window's `-y` dimension (set by `new-session -x 200 -y 50`) instead, which is well-defined in headless mode. macOS tmux 3.6a tolerates either form.

Test: `tmux::tests::supervisor_splits_use_l_percent_not_p`.

### Requirement: Session name override via builder

The builder SHALL support overriding the default `paw-<project>` session name with a custom name.

#### Scenario: Override replaces default name
- **GIVEN** `session_name("custom-session-name")` is set on the builder
- **WHEN** the session is built
- **THEN** the session name SHALL be `"custom-session-name"` and commands SHALL target it

Test: `tmux::tests::session_name_override_replaces_default`

### Requirement: Dynamic pane count matches input

The number of panes in the session's window 0 SHALL match the number of `PaneSpec` entries added via the builder. When broker is enabled, the builder SHALL receive an additional `PaneSpec` for the dashboard in pane 0, increasing the total pane count by one. A detached service window (the broker host's `broker` window) is not part of this count.

#### Scenario: Two agent panes plus dashboard created
- **GIVEN** broker is enabled and 2 agent pane specs are added
- **WHEN** the session is built
- **THEN** exactly 3 panes SHALL exist in window 0: pane 0 (dashboard) + panes 1-2 (agents)

#### Scenario: Two panes without broker
- **GIVEN** broker is disabled and 2 pane specs are added
- **WHEN** the session is built
- **THEN** exactly 2 panes SHALL exist in window 0 (same as v0.2.0)

### Requirement: Correct commands sent to each pane

Each pane SHALL receive a `cd <worktree> && <cli_command>` command targeting the correct pane index.

#### Scenario: Each pane receives cd and CLI command
- **GIVEN** two panes with different worktrees and CLIs
- **WHEN** the session is built
- **THEN** each `send-keys` command SHALL contain `cd <worktree> && <cli>`

Test: `tmux::tests::each_pane_receives_cd_and_cli_command`

#### Scenario: Commands are submitted with Enter
- **GIVEN** a pane spec
- **WHEN** the session is built
- **THEN** the `send-keys` command SHALL include `Enter`

Test: `tmux::tests::pane_commands_are_submitted_with_enter`

#### Scenario: Each pane targets a distinct index
- **GIVEN** 3 panes
- **WHEN** the session is built
- **THEN** `send-keys` SHALL target `:0.0`, `:0.1`, and `:0.2` respectively

Test: `tmux::tests::each_pane_targets_a_distinct_pane_index`

### Requirement: Pane titles show branch and CLI

Each pane SHALL be titled with `<branch> → <cli_command>` and border status configured.

#### Scenario: Pane titles contain branch and CLI
- **GIVEN** panes with branches and CLIs
- **WHEN** the session is built
- **THEN** `select-pane -T` commands SHALL set titles like `"feat/auth → claude"`

Test: `tmux::tests::each_pane_is_titled_with_branch_and_cli`

#### Scenario: Pane border status configured
- **GIVEN** any session
- **WHEN** the session is built
- **THEN** `pane-border-status` SHALL be set to `top` and `pane-border-format` SHALL use `#{pane_title}`

Test: `tmux::tests::pane_border_status_is_configured`

### Requirement: Configurable mouse mode per session

Mouse mode SHALL be enabled by default and be disableable via the builder.

#### Scenario: Mouse mode enabled by default
- **GIVEN** no explicit mouse mode setting
- **WHEN** the session is built
- **THEN** a `mouse on` command SHALL be emitted

Test: `tmux::tests::mouse_mode_enabled_by_default`

#### Scenario: Mouse mode can be disabled
- **GIVEN** `mouse_mode(false)` is set on the builder
- **WHEN** the session is built
- **THEN** no `mouse on` command SHALL be emitted

Test: `tmux::tests::mouse_mode_can_be_disabled`

### Requirement: Attach to a tmux session

The system SHALL attach the current terminal to a named tmux session, returning an error if the session does not exist.

#### Scenario: Attaching to a nonexistent session fails
- **GIVEN** no tmux session with the given name exists
- **WHEN** `attach()` is called
- **THEN** it SHALL return an error

Test: `e2e_tests::attach_fails_for_nonexistent_session`

### Requirement: Session liveness check

The system SHALL check whether a tmux session is alive by name.

#### Scenario: Nonexistent session reports not alive
- **GIVEN** no tmux session with the queried name exists
- **WHEN** `is_session_alive()` is called
- **THEN** it SHALL return `false`

Test: `tmux::tests::is_session_alive_returns_false_for_nonexistent`

### Requirement: Session lifecycle management

The system SHALL support creating, checking, and killing tmux sessions.

#### Scenario: Full create-check-kill lifecycle
- **GIVEN** a tmux session is created
- **WHEN** `is_session_alive()` is called, then `kill_session()`, then `is_session_alive()` again
- **THEN** it SHALL be alive after creation and not alive after killing

Test: `tmux::tests::session_lifecycle_create_check_kill`

#### Scenario: Built session can be executed and killed
- **GIVEN** a session built via `TmuxSessionBuilder`
- **WHEN** `execute()` is called
- **THEN** the tmux session SHALL be alive, and after `kill_session()` it SHALL be gone

Test: `tmux::tests::built_session_can_be_executed_and_killed`

### Requirement: Session name collision resolution

The system SHALL resolve name collisions by appending `-2`, `-3`, etc. to the base session name.

#### Scenario: No collision returns base name
- **GIVEN** no existing session with the base name
- **WHEN** `resolve_session_name()` is called
- **THEN** it SHALL return `paw-<project_name>`

Test: `tmux::tests::resolve_session_name_returns_base_when_no_collision`

#### Scenario: Collision appends numeric suffix
- **GIVEN** a session with the base name already exists
- **WHEN** `resolve_session_name()` is called
- **THEN** it SHALL return `paw-<project_name>-2`

Test: `tmux::tests::resolve_session_name_appends_suffix_on_collision`

### Requirement: Tmux session lifecycle SHALL work against a real tmux server

#### Scenario: Create and kill session lifecycle
- **GIVEN** a tmux session is created via the builder
- **WHEN** `execute()`, `is_session_alive()`, and `kill_session()` are called
- **THEN** the session SHALL be alive after creation and gone after killing

Test: `e2e_tests::tmux_session_create_and_kill_lifecycle`

#### Scenario: Five panes with different CLIs
- **GIVEN** 5 pane specs with different branch/CLI pairs
- **WHEN** the session is executed
- **THEN** tmux SHALL have 5 panes with correct titles

Test: `e2e_tests::tmux_session_with_five_panes_and_different_clis`

#### Scenario: Mouse mode enabled by default against live tmux
- **GIVEN** a session built with default settings
- **WHEN** `tmux show-option` is queried
- **THEN** mouse SHALL be "on"

Test: `e2e_tests::tmux_mouse_mode_enabled_by_default`

#### Scenario: is_session_alive returns false for nonexistent (e2e)
- **GIVEN** no session with the queried name
- **WHEN** `is_session_alive()` is called
- **THEN** it SHALL return `false`

Test: `e2e_tests::tmux_is_session_alive_returns_false_for_nonexistent`

#### Scenario: Attach succeeds for live session
- **GIVEN** a live tmux session
- **WHEN** `attach()` is called and the client is detached programmatically
- **THEN** the function SHALL execute without panic

Test: `e2e_tests::attach_succeeds_for_live_session`

### Requirement: E2E commands SHALL behave correctly against real repos

#### Scenario: Dry run shows session plan
- **GIVEN** a git repo with branches and `--dry-run --cli echo --branches feat/a,feat/b`
- **WHEN** the binary runs
- **THEN** stdout SHALL contain "Dry run", branch names, and the CLI name

Test: `e2e_tests::dry_run_with_flags_shows_plan`

#### Scenario: Preset not found returns error
- **GIVEN** a git repo with no presets configured
- **WHEN** `start --preset nonexistent` is run
- **THEN** it SHALL fail with stderr mentioning "not found"

Test: `e2e_tests::preset_not_found_returns_error`

#### Scenario: Stop with no session
- **GIVEN** a git repo with no active session
- **WHEN** `stop` is run
- **THEN** it SHALL succeed with stdout mentioning "No active session"

Test: `e2e_tests::stop_with_no_session`

#### Scenario: Purge with no session
- **GIVEN** a git repo with no active session
- **WHEN** `purge --force` is run
- **THEN** it SHALL succeed with stdout mentioning "No session to purge"

Test: `e2e_tests::purge_with_no_session`

#### Scenario: Status with no session
- **GIVEN** a git repo with no active session
- **WHEN** `status` is run
- **THEN** it SHALL succeed with stdout mentioning "No session"

Test: `e2e_tests::status_with_no_session`

#### Scenario: Stop from non-git directory fails
- **GIVEN** a directory that is not a git repository
- **WHEN** `stop` is run
- **THEN** it SHALL fail with "Not a git repository"

Test: `e2e_tests::stop_from_non_git_dir_fails`

#### Scenario: Status from non-git directory fails
- **GIVEN** a directory that is not a git repository
- **WHEN** `status` is run
- **THEN** it SHALL fail with "Not a git repository"

Test: `e2e_tests::status_from_non_git_dir_fails`

### Requirement: TmuxSession supports pipe-pane command

The `TmuxSession` builder SHALL support queuing a `pipe-pane` command to attach logging to a specific pane.

#### Scenario: pipe-pane queued in builder
- **WHEN** `pipe_pane()` is called on a `TmuxSession` with a pane target and log path
- **THEN** the command queue SHALL contain a `pipe-pane -o -t <pane> "cat >> <path>"` entry

#### Scenario: pipe-pane in dry-run output
- **WHEN** a session with `pipe_pane()` is rendered as dry-run
- **THEN** the output SHALL include the `tmux pipe-pane` command string

#### Scenario: pipe-pane executed after pane creation
- **WHEN** the session commands are executed
- **THEN** the `pipe-pane` command SHALL execute after the corresponding `split-window` and `send-keys` commands for that pane

### Requirement: TmuxSession supports session-level environment variables

The `TmuxSessionBuilder` SHALL support setting session-level environment variables via a `set_environment(key, value)` method. The resulting `set-environment -t <session> <key> <value>` command SHALL be emitted before any `send-keys` commands to ensure all panes inherit the variable.

#### Scenario: set_environment emits correct tmux command
- **GIVEN** `set_environment("GIT_PAW_BROKER_URL", "http://127.0.0.1:9119")` is called on the builder
- **WHEN** the session is built
- **THEN** the command queue SHALL contain `set-environment -t <session> GIT_PAW_BROKER_URL http://127.0.0.1:9119`

#### Scenario: set_environment appears before send-keys
- **GIVEN** a builder with environment variables and pane specs
- **WHEN** the session is built
- **THEN** all `set-environment` commands SHALL appear before any `send-keys` commands in the command queue

#### Scenario: set_environment in dry-run output
- **GIVEN** a builder with `set_environment` called
- **WHEN** the session is rendered as dry-run
- **THEN** the output SHALL include the `tmux set-environment` command string

#### Scenario: Multiple environment variables
- **GIVEN** `set_environment("A", "1")` and `set_environment("B", "2")` are both called
- **WHEN** the session is built
- **THEN** both `set-environment` commands SHALL appear in the command queue

### Requirement: Supervisor-mode pane layout

When the tmux session is built for a supervised session (per the `supervisor-launch` capability), the system SHALL produce a layout with these structural properties:

- **Top row**: pane 0 (dashboard) spanning the full window width.
- **Agent grid below**: dynamically sized by agent count, with up to 5 columns per row. The agent grid is a sequence of horizontal rows; each row holds up to 5 agent panes side-by-side.
- **Pane indices**: pane 0 = dashboard; panes 1..N = coding agents in row-major order (left-to-right, top-to-bottom). No pane runs an orchestrator CLI.
- **Vertical proportions**: the dashboard row SHALL occupy 50% of the session height regardless of agent count, and the agent rows SHALL share the remaining 50% evenly — each agent row occupying `50 / agent_rows` percent. Adding agents SHALL shrink the agent rows, never the dashboard row.

  | Agent rows | Dashboard row height | Each agent row height |
  |---|---|---|
  | 1 (1-5 agents) | 50% | 50% |
  | 2 (6-10 agents) | 50% | 25% each |
  | 3 (11-15 agents) | 50% | 16.7% each |
  | 4 (16-20 agents) | 50% | 12.5% each |
  | 5 (21-25 agents) | 50% | 10% each |

- **Equal-width agent panes within a row**: every agent pane in the same agent row SHALL be rendered at equal width (within a one-column rounding tolerance). Because agents are added to a row by successive `tmux split-window -h` — and each `-h` split halves the *current* pane — a row populated by raw splits renders unequal widths, so the system SHALL rebalance each agent row to equal width after its panes are created, by applying `tmux select-layout even-horizontal` scoped to that row's agent panes OR by issuing `tmux resize-pane` to set each agent pane to `100 / agents_in_row` percent of the row's width. The rebalance SHALL NOT alter the dashboard row's full width nor the vertical height proportions described above.

- **Minimum usable pane width**: the layout SHALL keep agent panes wide enough to be usable up to the agent cap; at the maximum 5 columns per row the equal-width target is 20% of the window width per pane. The system SHALL NOT produce agent rows with more than 5 panes (`SUPERVISOR_AGENTS_PER_ROW`).

- **Hard cap**: 25 agents per session. Above 25, the system SHALL reject the launch with a clear "split into multiple sessions" error before any tmux command runs.

The layout SHALL be built using `tmux split-window -h` and `-v` with explicit percentages, then enforced via `tmux resize-pane -y <pct>` for the height proportions and the per-row equal-width rebalance described above. `select-layout tiled` (or other whole-window auto-layouts) SHALL NOT be used for the supervised layout because they do not preserve the predictable pane-index ordering this layout relies on; `select-layout even-horizontal` MAY be used only when scoped so it rebalances a single agent row's widths without reordering pane indices or disturbing the dashboard row. No `swap-pane` SHALL be emitted.

#### Scenario: 5-agent supervised layout has 1 agent row

- **GIVEN** a supervised session with 5 agent branches
- **WHEN** the tmux layout is built
- **THEN** pane 0 SHALL be the dashboard spanning the full window width
- **AND** panes 1-5 SHALL be agents arranged in a single row below the dashboard
- **AND** the dashboard row's height SHALL be 50% and the agent row's height SHALL be 50%

#### Scenario: 10-agent supervised layout has 2 agent rows

- **GIVEN** a supervised session with 10 agent branches
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 3 (1 dashboard + 2 agent rows)
- **AND** the dashboard row's height SHALL be 50%
- **AND** each agent row's height SHALL be 25%
- **AND** the first agent row SHALL contain panes 1-5, the second agent row SHALL contain panes 6-10

#### Scenario: 20-agent supervised layout has 4 agent rows

- **GIVEN** a supervised session with 20 agent branches
- **WHEN** the tmux layout is built
- **THEN** total row count SHALL be 5 (1 dashboard + 4 agent rows)
- **AND** the dashboard row's height SHALL be 50%
- **AND** each of the 4 agent rows' height SHALL be 12.5%

#### Scenario: 26-agent supervised session is rejected

- **GIVEN** 26 agent branches resolved
- **WHEN** the supervised launch flow runs
- **THEN** the launch SHALL be rejected with a `PawError` stating the requested count (26), the maximum (25), and a hint suggesting `--branches <subset>`
- **AND** no tmux session SHALL be created

#### Scenario: Pane indices match row-major order

- **GIVEN** a supervised session with 7 agents
- **WHEN** the tmux layout is built
- **THEN** pane 1 SHALL be the first agent (top-left of the agent grid)
- **AND** pane 5 SHALL be the fifth agent (top-right of the first agent row)
- **AND** pane 6 SHALL be the sixth agent (start of the second agent row)

#### Scenario: Three agents in a row render at equal width

- **GIVEN** a supervised session with 3 agent branches (a single agent row)
- **WHEN** the tmux layout is built and applied to a live tmux window
- **THEN** the three agent panes SHALL each occupy approximately one third of the window width (within a one-column rounding tolerance)

#### Scenario: Row rebalance leaves the dashboard row full width

- **GIVEN** a supervised session whose agent row is rebalanced to equal width
- **WHEN** the layout is applied to a live tmux window
- **THEN** pane 0 (dashboard) SHALL remain at the full window width
- **AND** the vertical height proportions SHALL be unchanged: the dashboard row at 50% and each agent row at `50 / agent_rows` percent

#### Scenario: Full agent row stays at five equal-width columns

- **GIVEN** a supervised session with 5 agents in a single row
- **WHEN** the layout is applied to a live tmux window
- **THEN** each of the 5 agent panes SHALL occupy approximately 20% of the window width
- **AND** no agent row SHALL contain more than 5 panes

### Requirement: Launch-readiness gate before boot-block injection

The system SHALL verify that a pane's CLI process has actually started and reached an interactive ready state before injecting any boot block, framing prompt, or task prompt into that pane, rather than relying solely on a fixed wall-clock sleep. The system SHALL poll the pane (via `tmux capture-pane`) for a CLI-readiness marker — content that distinguishes the launched CLI's interactive UI from a bare shell prompt — up to a bounded timeout. The boot block SHALL NOT be injected while the pane is still a bare shell, because the shell would interpret the multi-line boot block line-by-line as failing commands (the v0.8.0 G1 dogfood failure).

If the readiness timeout elapses while the pane is still a bare shell, the system SHALL relaunch the CLI command into that pane (re-clearing the input line first) and poll again, up to a bounded number of relaunch attempts. The readiness gate and its timeout/attempt budget SHALL apply uniformly to EVERY launch path that injects text into an agent pane: bare `start` (which then pre-fills without submitting), spec-driven `start`, supervised `start`, `git paw add`, and the submission of a held boot prompt when a paused session is restarted. A pane the gate reports as blocked on an unanswered first-run dialog SHALL NOT receive the injected text and SHALL fail that launch path loudly (per "Launch gate handles a first-run permission/trust acceptance dialog"); the one exception is restart from pause, which SHALL keep that pane's held prompt for the next restart, report the blocked pane on stderr, and continue restarting the session. On restart from pause a held prompt SHALL be submitted only to the pane resolved live from its agent's worktree: when no live pane resolves, the prompt SHALL stay held and the agent SHALL be reported on stderr, and the prompt SHALL NOT be submitted at a computed pane index.

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

#### Scenario: A held prompt whose pane cannot be found stays held

- **GIVEN** a paused session holding a boot prompt for an agent whose worktree no longer resolves to a live pane, with another ready CLI at that agent's computed pane index
- **WHEN** `git paw start` restarts the session
- **THEN** the held prompt SHALL NOT be submitted to any pane
- **AND** it SHALL remain held for the next restart
- **AND** stderr SHALL name the agent as "pane not found" while the rest of the session restarts

### Requirement: The dry-run plan preview reflects worktree placement

The `git paw start --dry-run` session-plan preview SHALL render each agent's
worktree path according to the configured [`WorktreePlacement`], rather than
assuming the sibling layout: `.git-paw/worktrees/<branch-slug>` (relative to the
repo root) for child placement, and `../<project>-<branch-slug>` for sibling
placement. This applies uniformly to the plain, `--supervisor`, and
`--from-specs` dry-run previews. Computing the preview path MUST NOT create any
directory (the dry run stays read-only).

**Test:** `git::tests::worktree_display_path_reflects_placement`,
`git::tests::worktree_path_for_creates_nothing`

#### Scenario: Child placement preview shows the in-repo path
- **GIVEN** a repository configured with child worktree placement
- **WHEN** the dry-run plan renders the worktree path for branch `feat/auth-flow`
- **THEN** it SHALL show `.git-paw/worktrees/feat-auth-flow`
- **AND** it SHALL NOT show a `../`-prefixed sibling path

#### Scenario: Sibling placement preview shows the sibling path
- **GIVEN** a repository configured with sibling worktree placement
- **WHEN** the dry-run plan renders the worktree path for branch `feat/auth-flow`
- **THEN** it SHALL show `../<project>-feat-auth-flow` beside the repository

#### Scenario: Rendering the preview path creates nothing
- **WHEN** the dry-run preview computes a child-placement worktree path
- **THEN** no worktree directory or `.git-paw/worktrees/` entry is created on disk

### Requirement: Launch gate handles a first-run permission/trust acceptance dialog

The readiness gate SHALL, when the launch-readiness poll observes a pane presenting
a first-run acceptance dialog for its CLI — a one-time bypass-permissions
confirmation (e.g. the dialog Claude Code shows the first time
`--dangerously-skip-permissions` runs against a config directory that never accepted
it) or a "trust this folder?" prompt — either answer the dialog to reach the
interactive ready state, or fail the launch loudly. It SHALL NOT treat the dialog as a bare shell and relaunch
(which would re-open the dialog), and SHALL NOT treat it as ready and inject the
boot block on top of it. A pane left blocked on an unanswered acceptance dialog
SHALL NOT be reported as a healthy, running agent. Dialog detection SHALL be
per-CLI and conservative: a CLI whose acceptance dialog the gate does not recognise
SHALL fall back to the existing readiness / relaunch / fixed-budget behaviour, so
launch is never worse than before.

#### Scenario: First-run bypass acceptance dialog is answered, not relaunched

- **GIVEN** an agent pane launched with a full-auto bypass flag against a config directory that has never accepted bypass, showing the first-run acceptance dialog
- **WHEN** the readiness gate polls the pane
- **THEN** the gate SHALL answer the acceptance dialog (or fail the launch loudly) rather than relaunching the CLI into the dialog

#### Scenario: A pane stuck on an acceptance dialog is not reported healthy

- **GIVEN** an agent pane blocked on an unanswered first-run acceptance dialog after the launch budget elapses
- **WHEN** launch completes
- **THEN** the pane SHALL NOT be reported as a healthy running agent (the launch fails loudly or the pane's dead/blocked state is surfaced)

#### Scenario: Unrecognised CLI acceptance dialog falls back to prior behaviour

- **GIVEN** a custom CLI whose first-run dialog the gate does not recognise
- **WHEN** the readiness budget elapses
- **THEN** the gate SHALL fall back to the existing readiness / relaunch / fixed-budget injection behaviour

### Requirement: Start reattaches to an existing live session instead of forking

`git paw start` SHALL reattach to the repository's existing live session rather than creating a numerically-suffixed parallel session (`paw-<project>-2`) when that repository already has a live session (resolved by repository path, per `session-state` find-by-repository). The `-N` collision suffix SHALL be reserved for genuinely distinct sessions (e.g. two different repositories that share a project name), not a re-invocation of `start` against the same repository's running session. In a non-interactive context where reattaching is not possible, `start` SHALL refuse with an actionable error (or require an explicit opt-in flag) rather than silently forking a parallel wave.

With `--unattended`, `start` SHALL instead bind a drive loop to the live session, whether or not the invocation is interactive (per `supervisor-unattended-operation`, *A drive loop can be bound to a live supervised session*). It SHALL NOT attach the terminal, SHALL NOT create a suffixed parallel session, and SHALL refuse only for the reasons that requirement and *At most one drive loop drives a session instance* state. Without `--unattended` the behaviour above is unchanged.

#### Scenario: Re-start of a repo with a live session reattaches

- **GIVEN** a repository with a live session `paw-x`
- **WHEN** the user runs `git paw start` again in that repository (interactive)
- **THEN** the system SHALL reattach to `paw-x` and SHALL NOT create `paw-x-2`

#### Scenario: A distinct repository sharing a name still gets its own session

- **GIVEN** a live session `paw-x` belonging to a different repository
- **WHEN** `git paw start` runs in a second repository whose project name is also `x`
- **THEN** the system SHALL resolve a distinct session name (`paw-x-2`) for the second repository

#### Scenario: Non-interactive re-start refuses rather than forking

- **GIVEN** a repository with a live session and a non-interactive (non-TTY) invocation
- **WHEN** `git paw start` runs again in that repository without `--unattended`
- **THEN** the system SHALL refuse with an actionable error (or require an explicit opt-in flag) rather than silently creating a suffixed parallel session

Test: `tests/supervisor_reattach_integration.rs::supervisor_restart_refuses_rather_than_forking_a_parallel_session` (unchanged).

#### Scenario: Non-interactive re-start with --unattended binds a drive loop

- **GIVEN** a repository with a live supervised session `paw-x`, no drive loop running, and a non-interactive (non-TTY) invocation
- **WHEN** `git paw start --unattended` runs again in that repository
- **THEN** the system SHALL bind a drive loop to `paw-x` and run it to an exit condition
- **AND** SHALL NOT create `paw-x-2`
- **AND** SHALL NOT print the non-interactive reattach refusal

Test: `tests/unattended_drive_e2e.rs::unattended_rebinds_a_loop_to_a_live_session_after_heartbeat`.

### Requirement: Every pane-less session uses the agent grid

Every launch path — bare `start`, spec-driven `start`, and supervised `start` — and the recovery of a stopped session SHALL build the window with the agent-grid geometry defined by "Supervisor-mode pane layout": the dashboard as the full-width top row when the broker is enabled, coding agents row-major below at no more than five per row, each row rebalanced to equal width, and the 25-agent cap. When the session has no dashboard (broker disabled), there SHALL be no top row: coding agents SHALL start at pane 0 and the agent rows SHALL share the full window height evenly. The whole-window `select-layout tiled` and `main-horizontal` layouts SHALL NOT be used to arrange coding agents. Every coding agent's pane index SHALL be its row-major position — the session's first agent index plus the agent's position — at any agent count: each split that creates an agent pane, including the one that opens a new row and the one `git paw add` performs, SHALL target the session's last pane, so the new pane takes the next index and no existing pane is renumbered.

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

#### Scenario: Every agent's pane index holds that agent

- **GIVEN** a bare session started with 7 branches, with the broker enabled and again with it disabled
- **WHEN** the window is built, and again when `git paw add` attaches a sixth agent to a 5-agent session
- **THEN** pane `first_agent + i` SHALL run in the worktree of the i-th agent and run that agent's CLI, for every agent
- **AND** no launch command or prompt SHALL be typed into a pane other than the one its agent occupies

#### Scenario: Recovery rebuilds a grid

- **GIVEN** a stopped bare session saved with the earlier tiled layout and 3 recorded worktrees with the broker enabled
- **WHEN** `git paw start` recovers it
- **THEN** the rebuilt window SHALL be the grid (dashboard row + one agent row of 3)
- **AND** the re-saved session SHALL record the grid layout

