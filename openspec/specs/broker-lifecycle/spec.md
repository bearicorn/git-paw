# broker-lifecycle Specification

## Purpose
TBD - created by archiving change broker-integration. Update Purpose after archive.
## Requirements
### Requirement: Dashboard subcommand starts broker and dashboard

The system SHALL handle the hidden `__dashboard` subcommand by:

1. Loading `BrokerConfig` from `.git-paw/config.toml`
2. Constructing a `BrokerState` with `log_path` set to `<session_state_dir>/broker.log`
3. Calling `start_broker(config, state.clone())` to obtain a `BrokerHandle`
4. Calling `run_dashboard(state, handle)` which blocks until the user presses `q`
5. Returning `Ok(())` on clean exit

The subcommand SHALL refuse to run if the `$TMUX` environment variable is not set, returning an error indicating it is an internal command intended to run inside tmux.

#### Scenario: Dashboard subcommand starts broker and blocks

- **GIVEN** `$TMUX` is set and `[broker]` config is valid
- **WHEN** `git paw __dashboard` is executed
- **THEN** a broker starts listening on the configured port
- **AND** the dashboard renders in the terminal

#### Scenario: Dashboard subcommand refuses outside tmux

- **GIVEN** `$TMUX` is not set
- **WHEN** `git paw __dashboard` is executed
- **THEN** it returns an error mentioning "internal command" and "tmux"

### Requirement: Start flow conditionally creates dashboard pane

When `[broker] enabled = true` in config, the `start` flow SHALL insert a dashboard pane that runs `git paw __dashboard`. The dashboard pane's index depends on whether supervisor mode is active:

- **Bare `git paw start` and `git paw start --from-specs` (no supervisor):** dashboard at pane 0; coding agent panes start at pane 1. Same as v0.4.
- **`git paw start --supervisor` (or `--from-specs --supervisor`):** dashboard at pane 1; supervisor agent at pane 0; coding agent panes start at pane 2. Updated in this change per the new supervisor-as-pane layout.

When `[broker] enabled = false` (or absent), the start flow SHALL behave identically to v0.2.0 with no dashboard pane (and supervisor mode is not meaningful since auto-approve, dashboard, and broker-status all require the broker).

The dashboard pane is in both cases a non-interactive TUI process; it does NOT receive a `tmux send-keys` boot block injection.

#### Scenario: Broker enabled in bare-start mode adds dashboard as pane 0

- **GIVEN** `[broker]\nenabled = true` in config and no supervisor mode
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 4 panes: pane 0 running `git paw __dashboard`, panes 1-3 running coding CLIs

#### Scenario: Broker enabled in supervisor mode places dashboard at pane 1

- **GIVEN** `[broker]\nenabled = true` and `[supervisor]\nenabled = true` in config
- **WHEN** `git paw start --supervisor` launches a session with 3 branches
- **THEN** the tmux session has 5 panes
- **AND** pane 0 SHALL be the supervisor agent (Claude with the supervisor skill as AGENTS.md)
- **AND** pane 1 SHALL be the dashboard (`git paw __dashboard`)
- **AND** panes 2-4 SHALL be the coding CLIs

#### Scenario: Broker disabled produces no dashboard pane

- **GIVEN** no `[broker]` section in config (or `enabled = false`)
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 3 panes, all running coding CLIs (same as v0.2.0)

#### Scenario: Dashboard pane title

- **GIVEN** broker enabled (in either bare-start or supervisor mode)
- **WHEN** the session is created
- **THEN** the dashboard pane has the title `"dashboard"` regardless of its index

### Requirement: Broker URL injected into tmux environment

When broker is enabled, the `start` flow SHALL call `tmux set-environment -t <session-name> GIT_PAW_BROKER_URL <url>` before any pane CLI commands are sent. The URL SHALL be computed from `BrokerConfig::url()`.

All panes in the session SHALL inherit this environment variable automatically via tmux's session-level environment.

#### Scenario: GIT_PAW_BROKER_URL is set on the session

- **GIVEN** `[broker]\nenabled = true\nport = 9119` in config
- **WHEN** a session is created
- **THEN** `tmux show-environment -t <session> GIT_PAW_BROKER_URL` returns `GIT_PAW_BROKER_URL=http://127.0.0.1:9119`

#### Scenario: Env var is set before pane commands

- **WHEN** the tmux session builder emits commands
- **THEN** the `set-environment` command appears before any `send-keys` commands

#### Scenario: No env var when broker is disabled

- **GIVEN** broker is disabled
- **WHEN** a session is created
- **THEN** `tmux show-environment -t <session> GIT_PAW_BROKER_URL` returns "unknown variable" or empty

### Requirement: Stop flow shuts down broker via pane 0 exit

The `stop` flow SHALL kill the tmux session via `tmux::kill_session`. Killing the session kills every pane including the dashboard pane, which causes `run_dashboard` to exit, which drops `BrokerHandle`, which triggers graceful broker shutdown including the final log flush.

The stop flow SHALL render an interactive confirmation prompt before killing the session when stdin is a TTY AND `--force` is not set. The prompt SHALL:

- Name the destructive consequences (CLI processes killed, agent conversation context lost).
- Point at `git paw pause` as the soft-stop alternative.
- Point at `git paw purge` as the full-reset alternative.
- Default to `n` (no) — the user SHALL confirm with `y` to proceed.

When `--force` is set OR stdin is not a TTY, the prompt SHALL be skipped and the kill SHALL proceed immediately. This preserves CI / automation back-compat (non-TTY contexts behave as in v0.4) and gives scripts a `--force` opt-out for TTY contexts.

When the session's recorded status is `Paused`, the confirmation prompt SHALL additionally inform the user that the session is currently paused and that continuing will kill the still-running CLI processes.

#### Scenario: Stop kills tmux and broker shuts down

- **GIVEN** an active session with broker enabled
- **WHEN** `git paw stop --force` is executed
- **THEN** the tmux session SHALL be killed
- **AND** the broker port SHALL be freed within 5 seconds
- **AND** `broker.log` SHALL contain a final flush of all messages

#### Scenario: Stop from TTY without --force prompts before killing

- **GIVEN** an active session and stdin attached to a TTY
- **WHEN** `git paw stop` is executed (no `--force`)
- **THEN** a confirmation prompt SHALL appear
- **AND** the prompt SHALL mention `git paw pause` as the soft alternative
- **AND** the prompt SHALL default to `no`

#### Scenario: Stop from non-TTY without --force does not prompt

- **GIVEN** an active session and stdin not attached to a TTY (e.g. CI)
- **WHEN** `git paw stop` is executed (no `--force`)
- **THEN** no interactive prompt SHALL be rendered
- **AND** the stop SHALL proceed immediately (v0.4 back-compat)

#### Scenario: Stop after pause kills remaining CLI panes

- **GIVEN** a session with `status == Paused` and tmux still alive
- **WHEN** `git paw stop --force` is executed
- **THEN** the tmux session SHALL be killed
- **AND** every previously-still-running CLI process SHALL be terminated
- **AND** the session state SHALL be `status == Stopped`

#### Scenario: Stop after pause from TTY prompt mentions paused state

- **GIVEN** a session with `status == Paused` and stdin attached to a TTY
- **WHEN** `git paw stop` is executed (no `--force`)
- **THEN** the confirmation prompt SHALL inform the user the session is currently paused
- **AND** the prompt SHALL state that continuing will kill the still-running CLIs

### Requirement: Purge flow cleans up broker log

The `purge` flow SHALL delete `broker.log` from the session state directory if the session state contains a `broker_log_path` field. Deletion SHALL be best-effort — missing or already-deleted log files SHALL NOT cause an error.

#### Scenario: Purge deletes broker.log

- **GIVEN** an active session with broker enabled and a `broker.log` file
- **WHEN** `git paw purge --force` is executed
- **THEN** the `broker.log` file is deleted
- **AND** the session state file is deleted
- **AND** worktrees are removed

#### Scenario: Purge succeeds when broker.log does not exist

- **GIVEN** a session state with `broker_log_path` pointing to a nonexistent file
- **WHEN** `git paw purge --force` is executed
- **THEN** purge completes successfully

### Requirement: Status shows broker information

When a session is active and has broker fields in its state, `git paw status` SHALL display broker information including the configured URL. The system SHALL attempt to probe `GET /status` against the broker URL:

- If the probe succeeds: display the broker URL, agent count, and uptime from the response.
- If the probe fails AND the session's effective status is `Paused`: display the broker URL with `(paused — run 'git paw start' to resume)`.
- If the probe fails AND the session's effective status is `Active` or `Stopped`: display the broker URL with `(not responding)`.

`git paw status` SHALL render the three session statuses with distinguishable visual treatment (e.g. different emoji / labels for `active`, `paused`, `stopped`). The paused row SHALL include a one-line restart hint pointing at `git paw start`.

#### Scenario: Status shows running broker with agents

- **GIVEN** an active session with broker enabled and 3 agents registered
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL contain the broker URL, `running`, and `3 agents`

#### Scenario: Status shows paused session with broker offline

- **GIVEN** a session with `status == Paused`, tmux alive, broker stopped
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL show the paused state distinctly from `active` and `stopped`
- **AND** the output SHALL contain a restart hint mentioning `git paw start`
- **AND** the broker line SHALL indicate the broker is paused, not "not responding" in error terms

#### Scenario: Status shows broker not responding for crashed active session

- **GIVEN** a session state with `status == Active` and broker fields but the dashboard pane has crashed
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL contain the broker URL and `not responding`

#### Scenario: Status shows no broker info when disabled

- **GIVEN** a session without broker fields in state
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL NOT mention broker, port, or agents

### Requirement: Auto-approve thread location in dashboard subprocess

When supervisor mode is active AND `[supervisor.auto_approve] enabled = true`, the auto-approve poll thread SHALL run inside the dashboard's `__dashboard` subprocess (the long-lived process running in the dashboard pane), NOT inside the `cmd_supervisor` process (which returns immediately after launching the session per the new supervisor-as-pane architecture).

The auto-approve thread's responsibilities are unchanged: poll `/status` every `stall_threshold_seconds`, capture stalled panes, classify pending commands, dispatch approve keystrokes for safe commands, escalate unknowns via `agent.question`. Only the host process changes.

#### Scenario: Auto-approve thread runs inside the dashboard subprocess

- **GIVEN** an active supervisor mode session with auto-approve enabled
- **WHEN** the dashboard's `__dashboard` subprocess starts
- **THEN** it spawns the auto-approve poll thread alongside the broker + TUI rendering
- **AND** `cmd_supervisor` SHALL NOT spawn a parallel auto-approve thread

#### Scenario: Auto-approve thread terminates when dashboard pane is killed

- **GIVEN** an active supervisor mode session with auto-approve running
- **WHEN** the user kills the dashboard pane (via `tmux kill-pane` or pane exit)
- **THEN** the `__dashboard` subprocess exits
- **AND** the auto-approve thread terminates with it
- **AND** the broker shuts down (per the existing "Stop flow" requirement)

### Requirement: Pause flow detaches client and stops broker without killing tmux

`git paw pause` SHALL perform a soft-stop that:

1. Detaches every client currently attached to the session by running `tmux detach-client -s <session-name>`. With no clients attached, the command SHALL be a no-op and SHALL NOT error.
2. Stops the broker by killing the dashboard pane only (`tmux kill-pane -t <session-name>:0.<dashboard-pane-index>`). The dashboard subprocess receives SIGHUP, the `BrokerHandle` drop runs, the broker shuts down gracefully, and `broker.log` flushes.
3. Updates the on-disk session state's `status` field from `Active` to `Paused` (see the session-state delta in this change).
4. Leaves the tmux session and every coding-agent CLI pane running.
5. Prints a one-line confirmation: `"Session '<name>' paused. <N> CLI pane(s) still running. Run 'git paw start' to resume."`

The dashboard pane index SHALL be read from the saved session's `dashboard_pane` field (see the session-state delta). For sessions saved by v0.4.0 (where the field is absent and defaults to `None`), the index SHALL default to `0` (the bare-start dashboard location).

The pause flow SHALL NOT call `tmux::kill_session` at any point.

#### Scenario: Pause detaches the client

- **GIVEN** an active session with a tmux client attached
- **WHEN** `git paw pause` is executed
- **THEN** `tmux list-clients -t <session>` SHALL return no clients
- **AND** the tmux session SHALL still be alive (`tmux has-session -t <session>` exits 0)

#### Scenario: Pause stops the broker

- **GIVEN** an active session with broker enabled and listening on port P
- **WHEN** `git paw pause` is executed
- **THEN** within 5 seconds, port P SHALL be free (no listener)
- **AND** the broker's `broker.log` SHALL contain a final flush of all messages

#### Scenario: Pause leaves coding-agent panes alive

- **GIVEN** an active session with 3 coding-agent panes
- **WHEN** `git paw pause` is executed
- **THEN** the tmux session SHALL still report 3 panes (dashboard pane removed)
- **AND** each coding-agent CLI process SHALL still be running (PID alive)

#### Scenario: Pause updates session state to paused

- **GIVEN** an active session
- **WHEN** `git paw pause` is executed
- **THEN** loading the session via `session::load_session` SHALL return a session with `status == SessionStatus::Paused`

#### Scenario: Pause prints a resume hint

- **WHEN** `git paw pause` completes successfully
- **THEN** stdout SHALL contain the session name
- **AND** stdout SHALL contain the phrase "Run `git paw start` to resume" (or words conveying the same)

### Requirement: Pause is idempotent

Running `git paw pause` against a session that is already in `Paused` state (no clients attached, broker stopped, tmux alive) SHALL be a no-op that exits successfully with an informational message. The second invocation SHALL NOT error, SHALL NOT re-publish broker shutdown events, and SHALL NOT alter the session state file.

#### Scenario: Pause on an already-paused session

- **GIVEN** a session with `status == Paused` and tmux alive
- **WHEN** `git paw pause` is executed
- **THEN** the command SHALL exit 0
- **AND** stdout SHALL contain a message indicating the session is already paused
- **AND** the session state file SHALL be unchanged

#### Scenario: Pause on a stopped session

- **GIVEN** a session with `status == Stopped` (tmux not alive)
- **WHEN** `git paw pause` is executed
- **THEN** the command SHALL exit 0
- **AND** stdout SHALL inform the user the session is already stopped and pause has no effect
- **AND** the session state SHALL remain `Stopped`

#### Scenario: Pause when no session exists

- **GIVEN** no session file exists for the current repo
- **WHEN** `git paw pause` is executed
- **THEN** the command SHALL exit 0
- **AND** stdout SHALL contain "No active session for this repo." (or words conveying the same)

### Requirement: Start flow restarts a paused session

When `git paw start` is invoked against a session whose effective status is `Paused` (recorded `Paused` AND tmux alive), the start flow SHALL:

1. Recreate the dashboard pane at the saved `dashboard_pane` index (or `0` if absent — v0.4 fallback) by running `tmux split-window` / `tmux new-window` / equivalent layout-restore tmux invocation appropriate to the original pane arrangement.
2. Send the `git paw __dashboard` command into the new dashboard pane via `tmux send-keys`.
3. Update the session state's `status` field from `Paused` to `Active`.
4. Attach to the tmux session via `tmux attach -t <session-name>`.

The restart-from-pause flow SHALL NOT create worktrees, SHALL NOT spawn coding-agent CLI processes, and SHALL NOT inject boot prompts. Coding-agent panes are already running and retain their in-memory conversation state.

#### Scenario: Start against paused session reattaches and restarts broker

- **GIVEN** a session with `status == Paused` and tmux alive
- **WHEN** `git paw start` is executed
- **THEN** the broker SHALL be listening on its configured port within 5 seconds
- **AND** the user's tmux client SHALL be attached to the session
- **AND** the session state SHALL be `status == Active`

#### Scenario: Start against paused session does not respawn CLIs

- **GIVEN** a paused session whose coding-agent CLI processes have PIDs P1..Pn
- **WHEN** `git paw start` is executed
- **THEN** the coding-agent panes SHALL still hold processes with PIDs P1..Pn
- **AND** no `tmux send-keys` SHALL be issued to the coding-agent panes during the restart

#### Scenario: Start against paused-but-tmux-dead falls through to recover

- **GIVEN** a session with recorded `status == Paused` but `tmux has-session` exits non-zero
- **WHEN** `git paw start` is executed
- **THEN** `effective_status` SHALL evaluate to `Stopped`
- **AND** the start flow SHALL run the existing cold-recovery path (fresh CLI spawn), NOT the restart-from-pause path

