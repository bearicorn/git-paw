## MODIFIED Requirements

### Requirement: Liveness reflects dead agent panes, not just session existence

The tmux-liveness probe that feeds `effective_status` SHALL report a session as not
alive when the tmux session object still exists but its agent panes' CLI processes
have all exited (the `is_tmux_alive` input becomes false) — so a session whose panes
died shortly after launch is reported `Stopped`, not `Active`. Liveness SHALL NOT be
satisfied by the mere existence of the named tmux session; it SHALL require at
least one live agent pane (a pane whose launched CLI process is still running). This
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
