## MODIFIED Requirements

### Requirement: Liveness reflects dead agent panes, not just session existence

The tmux-liveness probe that feeds `effective_status` SHALL report a session as not
alive when the tmux session object still exists but its agent panes' CLI processes
have all exited (the `is_tmux_alive` input becomes false) — so a session whose panes
died shortly after launch is reported `Stopped`, not `Active`. Liveness SHALL NOT be
satisfied by the mere existence of the named tmux session; it SHALL require at
least one live agent pane (a pane whose launched CLI process is still running) —
except when the tmux session has no coding-agent pane at all (every agent was
removed with `git paw remove`), which SHALL be reported alive: no agent exited,
the session is empty, and its dashboard and broker are still serving. This
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
  `"tmux_running": true` in JSON;
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
- **THEN** the response object SHALL contain `"status": "stopped"` and `"tmux_running": true`

#### Scenario: An absent tmux session still reports stale with tmux not running

- **GIVEN** a session whose receipt says `active` and whose tmux session does not exist
- **WHEN** the user runs `git paw status --json`
- **THEN** the response object SHALL contain `"status": "stale"` and `"tmux_running": false`

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

- **GIVEN** a session recorded `Active`, created past the launch grace period, whose last agent was removed with `git paw remove` so its tmux session holds only infrastructure panes (dashboard, and supervisor where present)
- **WHEN** its effective status is resolved
- **THEN** the liveness probe SHALL report the session alive, not the agent-panes-exited outcome
- **AND** `git paw status` SHALL NOT display `🟡 stopped` or the agent-panes-exited hint for it
