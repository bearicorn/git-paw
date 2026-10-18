## ADDED Requirements

### Requirement: Broker host subcommand

The system SHALL provide a hidden `git paw __broker` subcommand that hosts the broker for the current repository's session. The launcher SHALL hand it the session name and the per-session broker log path (`<session_state_dir>/<session_name>.broker.log`) as command-line arguments — neither is secret — because on a fresh launch the session receipt is saved only after the panes exist. It SHALL load `BrokerConfig`; construct the `BrokerState` with that log path (one log per session), the verify-on-commit nudge setting, the seeded orchestrator CLI, the opsx role-gating context, and the learnings aggregator when enabled; when a saved `Session` with that name exists (recovery, restart from pause), seed the watch targets and the agent → spec mapping from it, while on a fresh launch the agents arrive through `POST /watch`; start the broker — with the stale-broker probe that reattaches only to this repository's own broker — using those watch targets and the conflict-detector configuration; and block until shutdown. It SHALL exit — dropping `BrokerHandle` so the final broker-log flush and the learnings flush run — on `SIGHUP`, when reparented to init, or when a controlling terminal it was given hangs up. It SHALL ignore `SIGINT`. It SHALL NOT render a TUI, SHALL NOT enter the alternate screen, and SHALL NOT require `$TMUX`. A bind or probe failure SHALL exit non-zero with a diagnostic rather than retry. It SHALL be hidden from `git paw --help`.

#### Scenario: Host serves the broker

- **GIVEN** a saved session with the broker enabled on a free port
- **WHEN** `git paw __broker` is started in the repository with that session's name and log path
- **THEN** `GET <url>/status` SHALL answer with the `git_paw` marker and this repository's `repo_root` within 5 seconds
- **AND** the process SHALL still be running

#### Scenario: Host of a fresh launch writes the per-session log

- **GIVEN** a fresh launch whose session receipt has not been saved yet
- **WHEN** the host, handed session name `paw-demo` and its log path, accepts a published message
- **THEN** the message SHALL be flushed to `<session_state_dir>/paw-demo.broker.log`
- **AND** nothing SHALL be written to the `git-paw.broker.log` fallback

#### Scenario: SIGHUP shuts the host down with a final flush

- **GIVEN** a running `__broker` that has accepted at least one message
- **WHEN** `SIGHUP` is delivered to it
- **THEN** the port SHALL be free within 5 seconds
- **AND** the session's broker log SHALL contain every accepted message

#### Scenario: Host renders nothing

- **WHEN** `git paw __broker` runs for 2 seconds attached to a pseudo-terminal
- **THEN** it SHALL write no alternate-screen or cursor-addressing escape sequences to that terminal

#### Scenario: Bind failure exits instead of looping

- **GIVEN** a foreign HTTP server bound to the configured port
- **WHEN** `git paw __broker` starts
- **THEN** it SHALL exit non-zero with a diagnostic naming the port
- **AND** SHALL NOT remain running

#### Scenario: Host runs outside tmux

- **GIVEN** a shell with `$TMUX` unset
- **WHEN** `git paw __broker` is started for a session
- **THEN** it SHALL start and serve `/status`

### Requirement: Dashboard subcommand runs a thin broker client

The hidden `git paw __dashboard` subcommand SHALL hold no `BrokerState`, SHALL start no broker, watcher, conflict detector or learnings aggregator, and SHALL NOT require `$TMUX`. It SHALL resolve the broker URL from the saved session's `broker_port` and `broker_bind` whatever the session's recorded status — unlike the MCP server, which resolves a URL only for an `Active` session, because restart from pause recreates the dashboard while the session is still recorded `Paused` — and, while no session is saved yet (a fresh launch saves its receipt after the panes exist), SHALL retry the resolution on its poll cadence in the degraded state. It SHALL render from `GET /status` and `GET /log?since=<seq>`, and continue running in a degraded state when the broker is unreachable. Killing the dashboard SHALL NOT affect the broker.

#### Scenario: Dashboard holds no broker

- **GIVEN** a running session
- **WHEN** the dashboard pane is killed
- **THEN** `GET /status` SHALL continue to answer
- **AND** watcher, conflict-detector and learnings behavior SHALL be unchanged

#### Scenario: Dashboard runs outside tmux

- **GIVEN** a shell with `$TMUX` unset and a saved session with a live broker
- **WHEN** `git paw __dashboard` is started
- **THEN** it SHALL render the agent table from `/status`

#### Scenario: Dashboard with no live broker renders degraded

- **GIVEN** a saved session whose broker is not running
- **WHEN** `git paw __dashboard` is started
- **THEN** it SHALL render with a "broker unreachable" indicator rather than exiting or busy-looping
- **AND** pressing `q` SHALL exit with status 0

#### Scenario: Dashboard of a session still recorded paused connects

- **GIVEN** a session recorded `Paused` whose broker host is answering
- **WHEN** `git paw __dashboard` is started before the session is recorded `Active`
- **THEN** it SHALL render the agent table from `/status` without being restarted

#### Scenario: Dashboard started before the receipt connects once it is saved

- **GIVEN** a fresh launch whose dashboard pane starts before the session receipt is saved
- **WHEN** the launcher saves the receipt
- **THEN** the dashboard SHALL resolve the broker URL on a later poll and render live data

### Requirement: Broker host lifecycle across start, recovery, pause, stop

When the broker is enabled, the `start` flow (bare, spec-driven and supervised) SHALL launch the broker host in a detached service surface of the session provided by the terminal backend — on tmux, a window named `broker` that is not part of window 0's pane grid — immediately after `GIT_PAW_BROKER_URL` is injected and before any coding-agent pane is created, handing it the session name and the per-session log path; SHALL wait, bounded by 10 seconds, for `GET /status` to answer with the `git_paw` marker and this repository's `repo_root`; and SHALL then create the dashboard pane at pane 0 and the coding-agent panes. Once a coding agent's pane exists, and before its boot prompt is injected, the launcher SHALL register the agent with the host through `POST /watch`, carrying the agent's spec id when it was launched from a spec — the path `git paw add` uses. The launcher SHALL save the session receipt only after the panes exist, as before, so a launch that fails leaves the persisted session state unchanged (*A failed start preserves persisted session state*). Recovery of a stopped session SHALL recreate the broker host the same way; the host seeds its agents from the saved receipt. `stop` SHALL kill the session, which terminates the host's surface and triggers the graceful flush. `pause` SHALL stop the host by closing its service surface and SHALL leave the dashboard pane running. When a session started before this change (no `broker` surface) is paused, `pause` SHALL fall back to killing the dashboard pane at the recorded `dashboard_pane`. Window 0's pane indices SHALL be unaffected by the host in every flow.

#### Scenario: Start launches the host before the first boot prompt

- **GIVEN** `[broker] enabled = true`
- **WHEN** `git paw start` launches a session
- **THEN** a tmux window named `broker` SHALL exist in the session running `git paw __broker`
- **AND** `GET /status` SHALL have answered with the `git_paw` marker before the first injection into a coding-agent pane

#### Scenario: A fresh launch registers its agents with the host

- **GIVEN** `[broker] enabled = true`, learnings enabled, and a spec-driven launch of agent `feat-x` from spec `003-user-list`
- **WHEN** `git paw start --from-specs` launches the session
- **THEN** the host SHALL have accepted a `POST /watch` for `feat-x` carrying spec id `003-user-list` before `feat-x`'s boot prompt was injected
- **AND** `/status` SHALL list `feat-x`

#### Scenario: A launch that fails after the host starts saves no receipt

- **GIVEN** a repository with no saved session
- **WHEN** a fresh `git paw start` starts its broker host and then errors before its panes exist
- **THEN** no session receipt SHALL have been saved for the repository

#### Scenario: Pane indices are unaffected by the host

- **GIVEN** a supervised launch with 3 agents
- **WHEN** the session is built
- **THEN** window 0 SHALL contain exactly 4 panes (dashboard + 3 agents) with the dashboard at index 0

#### Scenario: Host that never answers fails the launch clearly

- **GIVEN** a broker host that cannot start (port held by a foreign process)
- **WHEN** `git paw start` waits for `/status`
- **THEN** the launch SHALL fail within the bound with an error naming the broker port
- **AND** the persisted session state SHALL be preserved for recovery

#### Scenario: Recovery recreates the host

- **GIVEN** a stopped session with `broker_port = Some(P)`
- **WHEN** `git paw start` recovers it
- **THEN** a `broker` window running `git paw __broker` SHALL exist
- **AND** `GET http://127.0.0.1:P/status` SHALL answer
- **AND** `/status` SHALL list every agent of the saved session

#### Scenario: Pause stops the host and keeps the dashboard

- **GIVEN** an active session with a `broker` window and a dashboard at pane 0
- **WHEN** `git paw pause` runs
- **THEN** the `broker` window SHALL be gone and the broker port free within 5 seconds
- **AND** pane 0 SHALL still be running `git paw __dashboard`

#### Scenario: Pause of a session started before the split falls back

- **GIVEN** an active session with no `broker` window whose dashboard pane hosts the broker
- **WHEN** `git paw pause` runs
- **THEN** it SHALL kill the dashboard pane at the recorded `dashboard_pane` so the broker stops as before

## MODIFIED Requirements

### Requirement: Stop flow shuts down broker via pane 0 exit

The `stop` flow SHALL kill the tmux session via `tmux::kill_session`. Killing the session terminates the broker host's service surface (the `broker` window), which delivers `SIGHUP` to `git paw __broker`, which drops `BrokerHandle`, which triggers graceful broker shutdown including the final log flush. For a session started before the host split, killing the session kills the dashboard pane that hosts the broker, with the same effect.

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

### Requirement: Pause flow detaches client and stops broker without killing tmux

`git paw pause` SHALL perform a soft-stop that:

1. Detaches every client currently attached to the session by running `tmux detach-client -s <session-name>`. With no clients attached, the command SHALL be a no-op and SHALL NOT error.
2. Stops the broker by closing the broker host's service surface (the `broker` window). The host receives `SIGHUP`, the `BrokerHandle` drop runs, the broker shuts down gracefully, and the session's broker log flushes. When the session has no `broker` surface (started before the host split), pause SHALL instead kill the dashboard pane at the recorded `dashboard_pane` (default `0`).
3. Updates the on-disk session state's `status` field from `Active` to `Paused`.
4. Leaves the tmux session, the dashboard pane, and every coding-agent CLI pane running.
5. Prints a one-line confirmation: `"Session '<name>' paused. <N> CLI pane(s) still running. Run 'git paw start' to resume."`

The pause flow SHALL NOT call `tmux::kill_session` at any point.

#### Scenario: Pause detaches the client

- **GIVEN** an active session with a tmux client attached
- **WHEN** `git paw pause` is executed
- **THEN** `tmux list-clients -t <session>` SHALL return no clients
- **AND** the tmux session SHALL still be alive

#### Scenario: Pause stops the broker

- **GIVEN** an active session with broker enabled and listening on port P
- **WHEN** `git paw pause` is executed
- **THEN** within 5 seconds, port P SHALL be free
- **AND** the session's broker log SHALL contain a final flush of all messages

#### Scenario: Pause leaves coding-agent panes and the dashboard alive

- **GIVEN** an active session with 3 coding-agent panes and a dashboard
- **WHEN** `git paw pause` is executed
- **THEN** window 0 SHALL still report 4 panes (dashboard + 3 agents)
- **AND** each coding-agent CLI process SHALL still be running
- **AND** the dashboard SHALL render its broker-unreachable state

#### Scenario: Pause updates session state to paused

- **GIVEN** an active session
- **WHEN** `git paw pause` is executed
- **THEN** loading the session via `session::load_session` SHALL return a session with `status == SessionStatus::Paused`

#### Scenario: Pause prints a resume hint

- **WHEN** `git paw pause` completes successfully
- **THEN** stdout SHALL contain the session name
- **AND** stdout SHALL contain the phrase "Run `git paw start` to resume" (or words conveying the same)

### Requirement: Start flow restarts a paused session

When `git paw start` is invoked against a session whose effective status is `Paused` (recorded `Paused` AND tmux alive), the start flow SHALL:

1. Recreate the broker host in its detached service surface (`broker` window), handing it the session name and the re-stamped log path, and wait, bounded by 10 seconds, for `GET /status` to answer.
2. Recreate the dashboard pane at the saved `dashboard_pane` index (or `0` if absent) ONLY when no pane is currently running `git paw __dashboard`; an existing dashboard pane SHALL be left alone and SHALL reconnect on its own.
3. Update the session state's `status` field from `Paused` to `Active`.
4. Attach to the tmux session via `tmux attach -t <session-name>`.

The restart-from-pause flow SHALL NOT create worktrees, SHALL NOT spawn coding-agent CLI processes, and SHALL NOT inject boot prompts other than prompts held for agents added while paused.

#### Scenario: Start against paused session reattaches and restarts broker

- **GIVEN** a session with `status == Paused` and tmux alive
- **WHEN** `git paw start` is executed
- **THEN** the broker SHALL be listening on its configured port within 10 seconds
- **AND** the user's tmux client SHALL be attached to the session
- **AND** the session state SHALL be `status == Active`

#### Scenario: Existing dashboard pane is reused

- **GIVEN** a paused session whose dashboard pane is still running
- **WHEN** `git paw start` is executed
- **THEN** no second dashboard pane SHALL be created
- **AND** the existing dashboard SHALL render live data within 5 seconds of the broker answering

#### Scenario: Start against paused session does not respawn CLIs

- **GIVEN** a paused session whose coding-agent CLI processes have PIDs P1..Pn
- **WHEN** `git paw start` is executed
- **THEN** the coding-agent panes SHALL still hold processes with PIDs P1..Pn
- **AND** no keystrokes SHALL be sent to the coding-agent panes during the restart other than a held boot prompt

#### Scenario: Start against paused-but-tmux-dead falls through to recover

- **GIVEN** a session with recorded `status == Paused` but `tmux has-session` exits non-zero
- **WHEN** `git paw start` is executed
- **THEN** `effective_status` SHALL evaluate to `Stopped`
- **AND** the start flow SHALL run the existing cold-recovery path

### Requirement: Status shows broker information

When a session is active and has broker fields in its state, `git paw status` SHALL display broker information including the configured URL. The system SHALL probe `GET /status` against the broker URL **with the session's repository**, so a broker on the port that serves a different repository is never reported as this session's broker, and SHALL act on the result as follows:

- If the probe finds a git-paw broker for this session's repository AND the session's broker host is alive — its `broker` service surface exists, as the terminal backend reports; for a session started before the host split, which has no such surface, its terminal session is alive: display the broker URL, agent count, and uptime from the response.
- If the probe finds a git-paw broker for this session's repository but the session's broker host is not alive (for example after `git paw stop`, while a process that outlived the session still answers on the port): display the broker URL noting that a leftover broker answers with no live session host. The system SHALL NOT display it as running.
- If the probe finds a git-paw broker for a different — or unreported — repository: display the broker URL noting it is held by another repository's git-paw broker. The system SHALL NOT display it as running.
- If the probe fails AND the session's effective status is `Paused`: display the broker URL with `(paused — run 'git paw start' to resume)`.
- If the probe fails AND the session's effective status is `Active` or `Stopped`: display the broker URL with `(not responding)`.

The broker's state SHALL be taken from the probe and the host's liveness, never from the session receipt alone. `git paw status` SHALL render the three session statuses with distinguishable visual treatment (e.g. different emoji / labels for `active`, `paused`, `stopped`). The paused row SHALL include a one-line restart hint pointing at `git paw start`.

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

- **GIVEN** a session state with `status == Active` and broker fields but the broker host has crashed
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL contain the broker URL and `not responding`

#### Scenario: Status after stop does not report the broker running

- **GIVEN** a session stopped with `git paw stop`
- **WHEN** `git paw status` is executed right afterwards, whether or not a leftover process still answers on the broker port
- **THEN** the broker line SHALL NOT report the broker as running

#### Scenario: Status shows no broker info when disabled

- **GIVEN** a session without broker fields in state
- **WHEN** `git paw status` is executed
- **THEN** the output SHALL NOT mention broker, port, or agents

#### Scenario: Status reports a port held by another repository

- **GIVEN** a session in repository B whose configured port is held by a live git-paw broker for repository A
- **WHEN** `git paw status` is executed in repository B
- **THEN** the broker line SHALL say the port is held by another repository
- **AND** the output SHALL NOT report the broker as running

### Requirement: Signal handling

The broker host process SHALL install a `SIGINT` handler that prevents an accidental Ctrl+C from terminating the broker. The handler SHALL NOT call `std::process::exit` and SHALL NOT trigger broker shutdown. Clean shutdown of the broker SHALL only occur via:

- The `BrokerHandle` being dropped
- `SIGHUP` delivered to the host (its service surface closed)
- `SIGTERM` or `SIGKILL` from outside the process (e.g. `git paw stop` killing tmux)

#### Scenario: SIGINT does not terminate the broker

- **GIVEN** a running broker host
- **WHEN** `SIGINT` is delivered to it
- **THEN** the broker continues to respond to HTTP requests
- **AND** the process does not exit

#### Scenario: Dropping BrokerHandle still shuts down cleanly

- **GIVEN** a running broker
- **WHEN** `BrokerHandle` is dropped
- **THEN** the broker shuts down and the process owning the handle continues running

### Requirement: Panic isolation

A panic in any single HTTP request handler or background tokio task MUST NOT terminate the broker runtime or the broker host process. The system SHALL rely on axum's default panic-catching for handlers and SHALL spawn any background broker tasks via `tokio::spawn` so panics remain isolated to a single task.

#### Scenario: Panic in a request handler returns 500 and broker keeps serving

- **GIVEN** a running broker whose handler implementation contains a deliberate `panic!` for one route (test-only setup)
- **WHEN** a request to that route arrives
- **THEN** the response status is 500
- **AND** subsequent requests to other routes continue to succeed

### Requirement: Live watch-target registration endpoint

The broker SHALL expose `POST /watch` accepting a JSON body with an agent id, worktree path, cli label, and an optional spec id, and SHALL add that path to its live filesystem-watch-target set so the watcher begins surfacing the worktree's activity without a broker restart. When a spec id is present and the learnings aggregator is attached, the broker SHALL record the agent → spec mapping for conflict classification. The endpoint SHALL be idempotent: registering an already-watched path SHALL NOT create a duplicate target. It SHALL bind to loopback only, consistent with the other broker endpoints. A body without a spec id SHALL be accepted exactly as before.

A coding agent registered through `POST /watch` — hot-added via `git paw add`, or registered by the launcher of a fresh `git paw start` once its pane exists — SHALL appear in `/status` from worktree activity on the same terms as an agent seeded from the saved session, independent of whether its CLI has self-published a status.

#### Scenario: Registering a target surfaces the worktree via the watcher

- **GIVEN** a running broker and a worktree not among its start-time watch targets
- **WHEN** a client `POST`s the worktree (agent id + path + cli) to `/watch`
- **AND** that worktree subsequently has uncommitted changes
- **THEN** the watcher SHALL surface the agent in `/status` without a broker restart and without requiring the agent's CLI to have self-published

#### Scenario: Registration is idempotent

- **GIVEN** a worktree already registered as a watch target
- **WHEN** the same path is `POST`ed to `/watch` again
- **THEN** the broker SHALL NOT create a duplicate target and SHALL return success

#### Scenario: Endpoint binds to loopback only

- **WHEN** the broker starts with `/watch` enabled
- **THEN** the endpoint SHALL be reachable only on the loopback interface, consistent with `/publish`, `/status`, and `/messages`

#### Scenario: A registration with a spec id maps the agent

- **GIVEN** a running broker with the learnings aggregator attached
- **WHEN** a client `POST`s `/watch` for agent `feat-x` with spec id `003-user-list`
- **THEN** a later forward conflict between `feat-x` and another agent mapped to `003-user-list` SHALL be classified `forward-conflict-intra-spec`

## REMOVED Requirements

### Requirement: Dashboard subcommand starts broker and dashboard

**Reason**: The dashboard no longer hosts the broker; hosting moved to the `__broker` subcommand so the broker outlives any view. The per-session log path and repository-identity probe introduced by `broker-repo-identity` carry over into "Broker host subcommand".

**Migration**: See "Broker host subcommand" and "Dashboard subcommand runs a thin broker client" in this capability.
