## ADDED Requirements

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

When `[broker] enabled = true` in config, the `start` flow SHALL insert a dashboard pane as pane 0 that runs `git paw __dashboard`. Coding agent panes SHALL start at pane 1. When `[broker] enabled = false` (or absent), the start flow SHALL behave identically to v0.2.0 with no dashboard pane.

#### Scenario: Broker enabled adds dashboard as pane 0

- **GIVEN** `[broker]\nenabled = true` in config
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 4 panes: pane 0 running `git paw __dashboard`, panes 1-3 running coding CLIs

#### Scenario: Broker disabled produces no dashboard pane

- **GIVEN** no `[broker]` section in config (or `enabled = false`)
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** the tmux session has 3 panes, all running coding CLIs (same as v0.2.0)

#### Scenario: Dashboard pane title

- **GIVEN** broker enabled
- **WHEN** the session is created
- **THEN** pane 0 has the title `"dashboard"`

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

The `stop` flow SHALL NOT add any broker-specific shutdown logic. Killing the tmux session kills pane 0, which causes `run_dashboard` to exit, which drops `BrokerHandle`, which triggers graceful broker shutdown including the final log flush.

#### Scenario: Stop kills tmux and broker shuts down

- **GIVEN** an active session with broker enabled
- **WHEN** `git paw stop` is executed
- **THEN** the tmux session is killed
- **AND** the broker port is freed within 5 seconds
- **AND** `broker.log` contains a final flush of all messages

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

- If the probe succeeds: display the broker URL, agent count, and uptime from the response
- If the probe fails: display the broker URL with `(not responding)`

#### Scenario: Status shows running broker with agents

- **GIVEN** an active session with broker enabled and 3 agents registered
- **WHEN** `git paw status` is executed
- **THEN** the output contains the broker URL, `running`, and `3 agents`

#### Scenario: Status shows broker not responding

- **GIVEN** a session state with broker fields but pane 0 has crashed
- **WHEN** `git paw status` is executed
- **THEN** the output contains the broker URL and `not responding`

#### Scenario: Status shows no broker info when disabled

- **GIVEN** a session without broker fields in state
- **WHEN** `git paw status` is executed
- **THEN** the output does not mention broker, port, or agents
