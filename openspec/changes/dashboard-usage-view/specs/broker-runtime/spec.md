## MODIFIED Requirements

### Requirement: Start flow conditionally creates dashboard pane

The `start` flow SHALL insert a dashboard pane that runs `git paw __dashboard` at pane 0 only when the dashboard is enabled for the launch AND the session runs the broker host. This applies to bare `git paw start`, spec-driven `start` and coordinated `start` (`--coordinated`, or `default_mode = "coordinated"`).

The dashboard is enabled for a launch when `git paw start --dashboard` is passed, or otherwise when the merged configuration sets `[dashboard] enabled = true`. It is disabled by default.

- **With a dashboard:** the dashboard is at pane 0 as the full-width top row, and the coding-agent panes start at pane 1 in the agent grid.
- **Without a dashboard:** the coding-agent panes start at pane 0 and the agent grid uses the whole window. This is also the case when the broker runs.

The panes above are the panes of the session's window 0. When the session runs the broker host it also has a detached `broker` service window holding the host; it holds no agent and is not counted among the session's panes.

When `--dashboard` is passed and the session would run no broker, the start flow SHALL fail before creating any worktree, pane or session state, with an error stating that the dashboard needs the broker and naming `[broker] enabled` and coordinated mode. When the dashboard is enabled only by configuration and the session runs no broker, the start flow SHALL create no dashboard pane and SHALL NOT fail. When the session runs no broker, the start flow SHALL otherwise behave as it does without the broker.

The dashboard pane is a non-interactive TUI process; it does NOT receive a `tmux send-keys` boot block injection, and it sends no keystrokes to any other pane. Recovery and restart from pause SHALL rebuild the dashboard pane the session recorded (per `session-state`), not the one the current flag or configuration would select.

#### Scenario: A coordinated session has no dashboard by default

- **GIVEN** a version-2 config with `[broker] enabled = true`, `default_mode = "coordinated"` and no `[dashboard] enabled`
- **WHEN** `git paw start --coordinated` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 3 panes, panes 0-2 running the coding CLIs
- **AND** no pane runs `git paw __dashboard`

#### Scenario: A bare broker session has no dashboard by default

- **GIVEN** a version-2 config with `[broker] enabled = true` and no `[dashboard] enabled`
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 3 panes, all running coding CLIs

#### Scenario: The config key enables the dashboard

- **GIVEN** `[broker] enabled = true` and `[dashboard] enabled = true`
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 4 panes: pane 0 running `git paw __dashboard`, panes 1-3 running coding CLIs

#### Scenario: The flag enables the dashboard

- **GIVEN** `[broker] enabled = true` and no `[dashboard] enabled`
- **WHEN** `git paw start --coordinated --dashboard` launches a session with 3 branches
- **THEN** pane 0 SHALL be the dashboard (`git paw __dashboard`) and panes 1-3 SHALL be the coding CLIs

#### Scenario: The flag without a broker fails before launching

- **GIVEN** a bare launch whose session runs no broker
- **WHEN** `git paw start --dashboard --branches feat/a` runs
- **THEN** the command SHALL exit non-zero with an error naming `[broker] enabled` and coordinated mode
- **AND** no worktree, tmux session or session state SHALL be created

#### Scenario: The config key without a broker adds no pane

- **GIVEN** `[dashboard] enabled = true` and a bare launch whose session runs no broker
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 3 panes, all running coding CLIs, and the launch SHALL succeed

#### Scenario: Broker disabled produces no dashboard pane

- **GIVEN** no `[broker]` section in config (or `enabled = false`) and a bare launch
- **WHEN** `git paw start` launches a session with 3 branches
- **THEN** window 0 of the tmux session has 3 panes, all running coding CLIs

#### Scenario: Dashboard pane title

- **GIVEN** a launch that creates a dashboard pane (in either mode)
- **WHEN** the session is created
- **THEN** the dashboard pane has the title `"dashboard"`

### Requirement: Dashboard subcommand runs a thin broker client

The hidden `git paw __dashboard` subcommand SHALL be a read-only client. It:

- SHALL hold no `BrokerState`;
- SHALL start no broker, watcher, conflict detector or learnings aggregator;
- SHALL write no session, configuration or broker state;
- SHALL NOT require `$TMUX`.

It SHALL resolve the session and its broker URL from the saved session — the broker URL from the saved session's `broker_port` and `broker_bind` whatever the session's recorded status, unlike the MCP server, which resolves a URL only for an `Active` session, because restart from pause recreates the dashboard while the session is still recorded `Paused` — and, while no session is saved yet (a fresh launch saves its receipt after the panes exist), SHALL retry the resolution on its poll cadence in the degraded state. A saved session that records no broker (a coordinated launch with the broker disabled) SHALL render a "broker disabled" state rather than the unreachable indicator. It SHALL render its usage panel from the read-only usage query that `git paw status --json` uses, and its broker log and indicators from `GET /status` and `GET /log?since=<seq>`. It SHALL keep running in a degraded state when the broker is unreachable. Killing the dashboard SHALL NOT affect the broker.

#### Scenario: Dashboard holds no broker

- **GIVEN** a running session with a dashboard
- **WHEN** the dashboard pane is killed
- **THEN** `GET /status` SHALL continue to answer
- **AND** watcher, conflict-detector and learnings behavior SHALL be unchanged

#### Scenario: Dashboard runs outside tmux

- **GIVEN** a shell with `$TMUX` unset and a saved session with a live broker
- **WHEN** `git paw __dashboard` is started
- **THEN** it SHALL render the usage panel and the broker log

#### Scenario: Dashboard with no live broker renders degraded

- **GIVEN** a saved session whose broker is not running
- **WHEN** `git paw __dashboard` is started
- **THEN** it SHALL render with a "broker unreachable" indicator rather than exiting or busy-looping
- **AND** pressing `q` SHALL exit with status 0

#### Scenario: Dashboard of a session still recorded paused connects

- **GIVEN** a session recorded `Paused` whose broker host is answering
- **WHEN** `git paw __dashboard` is started before the session is recorded `Active`
- **THEN** it SHALL render the broker log from the broker without being restarted

#### Scenario: Dashboard started before the receipt connects once it is saved

- **GIVEN** a fresh launch whose dashboard pane starts before the session receipt is saved
- **WHEN** the launcher saves the receipt
- **THEN** the dashboard SHALL resolve the broker URL on a later poll and render live data

### Requirement: Broker host lifecycle across start, recovery, pause, stop

When the broker is enabled, the `start` flow (bare, spec-driven and coordinated) SHALL launch the broker host in a detached service surface of the session provided by the terminal backend — on tmux, a window named `broker` that is not part of window 0's pane grid — as soon as the session exists: the backend opens the session with `GIT_PAW_BROKER_URL` in its environment, the dashboard pane at pane 0 when the launch has a dashboard (per "Start flow conditionally creates dashboard pane"), and the coding-agent panes in one step, and a service surface can only be added to a session that exists. The launcher SHALL hand the host the session name and the per-session log path, and SHALL wait, bounded by 10 seconds, for `GET /status` to answer with the `git_paw` marker and this repository's `repo_root` before it registers any agent, saves the receipt, or injects any boot prompt. The dashboard pane, when the launch has one, resolves the broker URL on its own poll cadence and renders degraded until the host answers. Once the host answers, and before any boot prompt is injected, the launcher SHALL register every coding agent with the host through `POST /watch`, carrying the agent's spec id when it was launched from a spec — the path `git paw add` uses. The launcher SHALL save the session receipt only after the panes exist and the host has answered, so a launch that fails — including a host that never answers — leaves the persisted session state unchanged (*A failed start preserves persisted session state*); a fresh launch whose host fails SHALL close the session it opened, since no receipt names it, and a recovery whose host fails SHALL close the rebuilt session and restore the receipt it had saved, so the next `git paw start` recovers again. Before starting a host the launcher SHALL wait, bounded, for a broker of this repository that still answers (a host stopped a moment ago) to release the port, and SHALL fail naming the port if it does not.

- **Recovery** of a stopped session SHALL recreate the broker host the same way, and SHALL recreate the dashboard pane only when the session has a dashboard; the host seeds its agents from the saved receipt.
- **`stop`** SHALL kill the session, which terminates the host's surface and triggers the graceful flush.
- **`pause`** SHALL stop the host by closing its service surface, and SHALL leave the dashboard pane, if the session has one, running. When a session started before the host split (no `broker` surface) is paused, `pause` SHALL fall back to killing the dashboard pane at the recorded `dashboard_pane`.

Window 0's pane indices SHALL be unaffected by the host in every flow.

#### Scenario: Start launches the host before the first boot prompt

- **GIVEN** `[broker] enabled = true`
- **WHEN** `git paw start` launches a session
- **THEN** a tmux window named `broker` SHALL exist in the session running `git paw __broker`
- **AND** `GET /status` SHALL have answered with the `git_paw` marker before the first injection into a coding-agent pane

#### Scenario: A fresh launch registers its agents with the host

- **GIVEN** `[broker] enabled = true`, learnings enabled, and a spec-driven launch of agent `feat-x` from spec `003-user-list`
- **WHEN** `git paw start --from-specs` launches the session
- **THEN** the host SHALL have accepted a `POST /watch` for `feat-x` carrying spec id `003-user-list` before `feat-x`'s boot prompt was injected
- **AND** `/status` SHALL list `feat-x` once its worktree shows activity, on the same terms as an agent seeded from a saved session

#### Scenario: A launch that fails after the host starts saves no receipt

- **GIVEN** a repository with no saved session
- **WHEN** a fresh `git paw start` starts its broker host and the host never answers `/status`
- **THEN** no session receipt SHALL have been saved for the repository
- **AND** the terminal session the launch opened SHALL be closed

#### Scenario: Pane indices are unaffected by the host

- **GIVEN** a coordinated launch with 3 agents and the dashboard enabled
- **WHEN** the session is built
- **THEN** window 0 SHALL contain exactly 4 panes (dashboard + 3 agents) with the dashboard at index 0

#### Scenario: A session without a dashboard still runs the host

- **GIVEN** a coordinated launch with 3 agents and the dashboard not enabled
- **WHEN** the session is built
- **THEN** a `broker` window running `git paw __broker` SHALL exist
- **AND** window 0 SHALL contain exactly 3 panes, the coding agents at indices 0-2

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
- **AND** the host SHALL watch every agent's worktree of the saved session, so each appears in `/status` once its worktree shows activity

#### Scenario: Pause stops the host and keeps the dashboard

- **GIVEN** an active session with a `broker` window and a dashboard at pane 0
- **WHEN** `git paw pause` runs
- **THEN** the `broker` window SHALL be gone and the broker port free within 5 seconds
- **AND** pane 0 SHALL still be running `git paw __dashboard`

#### Scenario: Pause of a session started before the split falls back

- **GIVEN** an active session with no `broker` window whose dashboard pane hosts the broker
- **WHEN** `git paw pause` runs
- **THEN** it SHALL kill the dashboard pane at the recorded `dashboard_pane` so the broker stops as before

### Requirement: Start flow restarts a paused session

When `git paw start` is invoked against a session whose effective status is `Paused` (recorded `Paused` AND tmux alive), the start flow SHALL:

1. Recreate the broker host in its detached service surface (`broker` window), handing it the session name and the re-stamped log path, and wait, bounded by 10 seconds, for `GET /status` to answer.
2. When the session has a dashboard (per `session-state`), recreate the dashboard pane at the saved `dashboard_pane` index (or `0` if absent) ONLY when no pane is currently running `git paw __dashboard`. An existing dashboard pane SHALL be left alone and SHALL reconnect on its own. A session without a dashboard SHALL NOT gain one, whatever the current flag or configuration says.
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

#### Scenario: A paused session without a dashboard does not gain one

- **GIVEN** a paused session that records no dashboard and a config that now sets `[dashboard] enabled = true`
- **WHEN** `git paw start` is executed
- **THEN** no pane SHALL run `git paw __dashboard` and the coding-agent panes SHALL keep their indices

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
