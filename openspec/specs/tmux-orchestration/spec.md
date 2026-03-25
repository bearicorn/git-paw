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

### Requirement: Session name override via builder

The builder SHALL support overriding the default `paw-<project>` session name with a custom name.

#### Scenario: Override replaces default name
- **GIVEN** `session_name("custom-session-name")` is set on the builder
- **WHEN** the session is built
- **THEN** the session name SHALL be `"custom-session-name"` and commands SHALL target it

Test: `tmux::tests::session_name_override_replaces_default`

### Requirement: Dynamic pane count matches input

The number of panes in the session SHALL match the number of `PaneSpec` entries added via the builder.

#### Scenario: Two panes created
- **GIVEN** 2 pane specs added
- **WHEN** the session is built
- **THEN** exactly 2 `send-keys` commands SHALL be emitted

Test: `tmux::tests::pane_count_matches_input_for_two_panes`

#### Scenario: Five panes created
- **GIVEN** 5 pane specs added
- **WHEN** the session is built
- **THEN** exactly 5 `send-keys` commands SHALL be emitted

Test: `tmux::tests::pane_count_matches_input_for_five_panes`

#### Scenario: Building with no panes is an error
- **GIVEN** no pane specs added
- **WHEN** `build()` is called
- **THEN** it SHALL return an error

Test: `tmux::tests::building_with_no_panes_is_an_error`

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
