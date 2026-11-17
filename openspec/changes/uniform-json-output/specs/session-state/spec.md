## ADDED Requirements

### Requirement: Status JSON is a versioned document with terminal-neutral liveness and broker state

`git paw status --json` SHALL follow the `cli-machine-output` contract. When no session exists for the repository it SHALL print `{"schema_version": 1, "session": null}`. Otherwise the document SHALL contain `schema_version`, `session`, `status` (`active`, `paused`, `stopped`, or `stale`), `terminal_alive`, `broker`, and `worktrees`. `terminal_alive` SHALL report whether the session's terminal session exists (a session whose agent CLIs have all exited but whose terminal session is still up reports `true`). The document SHALL NOT contain `tmux_running`; `terminal_alive` replaces it. `broker` SHALL be `null` when the session recorded no broker, and otherwise an object with `url` and `state`, where `state` is one of `running`, `paused`, `not_responding`, or `foreign`, decided by the same broker probe the human output uses. Each `worktrees` entry SHALL keep `branch`, `cli`, `worktree_path`, and `cli_session_id`. The human-readable output SHALL be unchanged.

#### Scenario: No session yields a versioned null document

- **GIVEN** a repository with no session
- **WHEN** `git paw status --json` is run
- **THEN** stdout SHALL be a single object equal to `{"schema_version": 1, "session": null}`

#### Scenario: Liveness is reported as terminal_alive

- **GIVEN** an active session whose terminal session is alive
- **WHEN** `git paw status --json` is run
- **THEN** `terminal_alive` SHALL be `true`

#### Scenario: tmux_running is absent

- **GIVEN** any session, alive, stopped, or stale
- **WHEN** `git paw status --json` is run
- **THEN** the document SHALL NOT contain a `tmux_running` key

#### Scenario: Exited agents in a live terminal session report alive

- **GIVEN** an active session whose agent CLIs have all exited while its terminal session is still up
- **WHEN** `git paw status --json` is run
- **THEN** `terminal_alive` SHALL be `true`

#### Scenario: A stale receipt reports not alive

- **GIVEN** a session whose receipt says `active` but whose terminal session no longer exists
- **WHEN** `git paw status --json` is run
- **THEN** `status` SHALL be `"stale"` and `terminal_alive` SHALL be `false`

#### Scenario: Broker state is reported

- **GIVEN** an active session that recorded a broker URL and whose broker answers
- **WHEN** `git paw status --json` is run
- **THEN** `broker` SHALL be an object whose `url` is the recorded URL and whose `state` is `"running"`

#### Scenario: A session without a broker reports null

- **GIVEN** a session started with the broker disabled
- **WHEN** `git paw status --json` is run
- **THEN** `broker` SHALL be `null`

#### Scenario: Other existing keys are unchanged

- **GIVEN** a session that produced a status JSON before this change
- **WHEN** `git paw status --json` is run after it
- **THEN** `session`, `status`, and every `worktrees` entry's keys SHALL carry the same values as before

## MODIFIED Requirements

### Requirement: Liveness reflects dead agent panes, not just session existence

The tmux-liveness probe that feeds `effective_status` SHALL report a session as not
alive when the tmux session object still exists but its agent panes' CLI processes
have all exited (the `is_tmux_alive` input becomes false) — so a session whose panes
died shortly after launch is reported `Stopped`, not `Active`. Liveness SHALL NOT be
satisfied by the mere existence of the named tmux session; it SHALL require at
least one live agent pane (a pane whose launched CLI process is still running) —
except when the session's receipt records zero agents (every agent was removed
with `git paw remove`) and no live agent pane exists, which SHALL be reported
alive: no agent exited, the session is empty, and its dashboard and broker are
still serving. The pane probe itself stays receipt-blind; the exception is
decided from the receipt, so a session that still records agents whose panes
were all closed SHALL keep the agent-panes-exited outcome. This
ensures `git paw status` and `git paw doctor` never report a session healthy after
its panes have all died (the full-auto boot-exit failure mode). The probe SHALL
tolerate the brief launch window in which panes are still starting their CLIs (it
SHALL NOT flap a just-launched session to `Stopped`).

The probe SHALL report "agent panes exited" as a separate liveness outcome from "tmux
session absent". `🔴 stale` and the JSON `"stale"` value SHALL mean only that the tmux
session is absent. `git paw status` SHALL NOT report a session as stale when its tmux
session exists and all its agent panes have exited. For such a session it SHALL:

- display `🟡 stopped`;
- print a hint that the agent panes exited and that `git paw start` recovers them;
- report the tmux session as running: `Tmux: running` in the human output and
  `"terminal_alive": true` in JSON;
- never say that the tmux session no longer exists.

`start` auto-invalidation and `purge --stale` SHALL keep deciding staleness with the single
`tmux has-session` probe. As a result, `git paw start` recovers a session whose agent panes
exited, and `purge --stale` never purges it.

The probe SHALL identify coding-agent panes by their pane-scoped `@paw_role` option rather
than by pane position, because a pane index shifts when an earlier pane is killed. It SHALL
classify each pane independently: a pane whose role is `dashboard` or `supervisor` is never a
coding-agent pane, any other role is, and a pane carrying no role falls back to the positional
rule. git-paw SHALL set `@paw_role` on every pane it creates, whether or not
`[layout].border_affordances` is enabled. Consequently a paused session — `git paw pause`
kills the dashboard pane, and tmux renumbers the survivors — SHALL NOT be reported as having
exited agent panes while any of its agents is still running.

#### Scenario: A session whose agent panes all exited reports Stopped

- **GIVEN** a session recorded `Active` whose tmux session object still exists but all agent panes' CLI processes have exited
- **WHEN** its effective status is resolved via the liveness probe
- **THEN** the liveness probe SHALL report the session not alive
- **AND** the effective status SHALL be `Stopped` (not `Active`)

#### Scenario: A session with at least one live agent pane remains Active

- **GIVEN** a session recorded `Active` with at least one agent pane whose CLI process is still running
- **WHEN** its effective status is resolved
- **THEN** the liveness probe SHALL report the session alive
- **AND** the effective status SHALL be `Active`

#### Scenario: Exited agent panes are a distinct liveness outcome from an absent session

- **GIVEN** a tmux session that exists, created past the launch grace period, whose every agent pane runs a bare shell
- **WHEN** the agent-pane liveness probe runs
- **THEN** it SHALL return the agent-panes-exited outcome, not the outcome for an absent tmux session
- **AND** for a tmux session that does not exist the same probe SHALL return the absent-session (stale) outcome

#### Scenario: Status shows exited agent panes as stopped with tmux running

- **GIVEN** a session whose receipt says `active`, whose tmux session exists, and whose agent panes have all exited past the launch grace period
- **WHEN** the user runs `git paw status`
- **THEN** the output SHALL display `🟡 stopped`, not `🔴 stale`
- **AND** it SHALL print a hint that the agent panes exited and that `git paw start` recovers them
- **AND** it SHALL print `Tmux: running`
- **AND** it SHALL NOT state that the tmux session no longer exists

#### Scenario: JSON status for exited agent panes reports stopped with tmux running

- **GIVEN** the same session (tmux session present, all agent panes exited)
- **WHEN** the user runs `git paw status --json`
- **THEN** the response object SHALL contain `"status": "stopped"` and `"terminal_alive": true`

#### Scenario: An absent tmux session still reports stale with tmux not running

- **GIVEN** a session whose receipt says `active` and whose tmux session does not exist
- **WHEN** the user runs `git paw status --json`
- **THEN** the response object SHALL contain `"status": "stale"` and `"terminal_alive": false`

#### Scenario: purge --stale leaves a session whose agent panes exited

- **GIVEN** a session in the current repository whose receipt says `active`, whose tmux session exists, and whose agent panes have all exited
- **WHEN** the user runs `git paw purge --stale`
- **THEN** that session's receipt SHALL NOT be purged
- **AND** the command SHALL report that there is nothing to purge

#### Scenario: A paused session whose dashboard pane was killed reports paused, not exited

- **GIVEN** a paused session whose dashboard pane `git paw pause` killed, whose remaining agent pane tmux has renumbered into the dashboard's old index, and whose agent process is still running
- **WHEN** the user runs `git paw status` and `git paw doctor`
- **THEN** status SHALL display `🔵 paused` and SHALL NOT print the agent-panes-exited hint
- **AND** doctor's session-state finding SHALL NOT say that its agent panes exited

#### Scenario: A live session emptied by remove stays active

- **GIVEN** a session recorded `Active`, created past the launch grace period, whose last agent was removed with `git paw remove` so its receipt records zero agents and its tmux session holds only infrastructure panes (dashboard, and supervisor where present)
- **WHEN** its effective status is resolved
- **THEN** the session SHALL be reported alive, not with the agent-panes-exited outcome
- **AND** `git paw status` SHALL NOT display `🟡 stopped` or the agent-panes-exited hint for it

Test: `session::tests::an_emptied_session_with_no_agent_pane_is_alive`, `tests/emptied_session_refill_e2e.rs::add_refills_a_live_session_emptied_by_remove`

#### Scenario: A session whose agents' panes were all closed still reports Stopped

- **GIVEN** a session recorded `Active`, created past the launch grace period, whose receipt still records an agent but whose window holds no agent pane
- **WHEN** its effective status is resolved and the user runs `git paw start`
- **THEN** the session SHALL keep the agent-panes-exited outcome and display `🟡 stopped`
- **AND** `start` SHALL recover it rather than reattach to an empty grid

Test: `session::tests::a_session_recording_agents_with_no_live_pane_stays_agents_exited`, `tests/emptied_session_refill_e2e.rs::closed_agent_panes_with_agents_recorded_still_recover`
