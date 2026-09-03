## ADDED Requirements

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
