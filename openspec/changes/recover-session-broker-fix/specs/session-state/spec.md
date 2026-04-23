## MODIFIED Requirements

### Requirement: Recovery data survives tmux crashes

After a tmux crash, the persisted session SHALL contain all data needed to reconstruct the session.

#### Scenario: Crashed session has all recovery data including broker fields
- **GIVEN** a saved session with worktrees and broker enabled
- **WHEN** tmux crashes and the session is loaded from disk
- **THEN** it SHALL have the session name, repo path, all worktree details, AND broker_port, broker_bind, broker_log_path

#### Scenario: Session recovery recreates dashboard pane when broker was enabled
- **GIVEN** a saved session with `broker_port = Some(9119)` and `broker_bind = Some("127.0.0.1")`
- **WHEN** `recover_session()` is called
- **THEN** the rebuilt tmux session SHALL have:
  - Dashboard pane in pane 0 running `git-paw __dashboard`
  - `GIT_PAW_BROKER_URL` environment variable set to `http://127.0.0.1:9119`
  - All original worktree panes in subsequent indices

#### Scenario: Session recovery uses original broker config, not current config
- **GIVEN** a saved session with `broker_port = Some(9119)`
- **AND** current repo config has `broker.enabled = false`
- **WHEN** `recover_session()` is called
- **THEN** the dashboard pane SHALL still be created with the original broker URL

#### Scenario: Session recovery without original broker creates no dashboard
- **GIVEN** a saved session with `broker_port = None`
- **WHEN** `recover_session()` is called
- **THEN** no dashboard pane SHALL be created