## ADDED Requirements

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
