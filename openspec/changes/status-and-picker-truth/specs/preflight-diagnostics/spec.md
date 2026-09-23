## MODIFIED Requirements

### Requirement: Hygiene check

The command SHALL verify required `.gitignore` entries are present (including
`.git-paw/worktrees/` being ignored under child placement) and detect stale session state or
orphaned worktree registrations, reporting ⚠ with a remedy to run `git paw purge --stale`.

Stale session state SHALL mean a receipt that claims active while its tmux session is absent.
A session whose tmux session exists but whose agent panes have all exited (past the launch
grace period) SHALL NOT be reported as stale and SHALL NOT be given the `git paw purge --stale`
remedy, because `purge --stale` does not purge it. The command SHALL instead report it as ⚠.
The finding SHALL name the session, state that its agent panes have exited, and give the remedy
`git paw start`, which recovers the session.

#### Scenario: Missing gitignore entry

- **GIVEN** a required `.gitignore` entry is absent
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ with a remedy

#### Scenario: Stale session detected

- **GIVEN** session state that claims active but whose tmux session is gone
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ with a remedy to run `git paw purge --stale`

#### Scenario: Session whose agent panes exited

- **GIVEN** session state that claims active, whose tmux session exists, and whose agent panes have all exited past the launch grace period
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ naming the session and stating that its agent panes exited
- **AND** the remedy SHALL be to run `git paw start`
- **AND** the finding SHALL NOT describe the tmux session as gone and SHALL NOT recommend `git paw purge --stale`
