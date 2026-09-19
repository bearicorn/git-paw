## MODIFIED Requirements

### Requirement: Stop flow shuts down broker via pane 0 exit

The `stop` flow SHALL kill the tmux session via `tmux::kill_session`. Killing the session kills every pane including the dashboard pane, which causes `run_dashboard` to exit, which drops `BrokerHandle`, which triggers graceful broker shutdown including the final log flush.

The stop flow SHALL NOT render a confirmation prompt, in any context. Stopping is recoverable: worktrees and session state survive, and `git paw start` recovers the session. When the session's recorded status is `Paused`, stopping SHALL kill the still-running CLI processes as for any other session.

#### Scenario: Stop kills tmux and broker shuts down

- **GIVEN** an active session with broker enabled
- **WHEN** `git paw stop` is executed
- **THEN** the tmux session SHALL be killed
- **AND** the broker port SHALL be freed within 5 seconds
- **AND** the session's broker log SHALL contain a final flush of all messages

#### Scenario: Stop never prompts

- **GIVEN** an active session, with stdin attached to a TTY or not
- **WHEN** `git paw stop` is executed
- **THEN** no interactive prompt SHALL be rendered
- **AND** the stop SHALL proceed immediately

#### Scenario: Stop after pause kills remaining CLI panes

- **GIVEN** a session with `status == Paused` and tmux still alive
- **WHEN** `git paw stop` is executed
- **THEN** the tmux session SHALL be killed
- **AND** every previously-still-running CLI process SHALL be terminated
- **AND** the session state SHALL be `status == Stopped`
