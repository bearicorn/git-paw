## ADDED Requirements

### Requirement: An unattended re-start reports an ignored launch selection

When `git paw start --unattended` binds a drive loop to an existing session that records agents and was given `--branches`, `--specs` or `--from-all-specs`, it SHALL print the same one-line notice the attended path prints: the launch selection was not applied, naming `git paw add`. It SHALL NOT apply the selection.

#### Scenario: An unattended re-start with a different selection prints the notice

- **GIVEN** a live session recording an agent for `feat/alpha`
- **WHEN** the user runs `git paw start --unattended --branches feat/beta`
- **THEN** stderr SHALL say the launch selection was not applied and name `git paw add`
- **AND** no worktree or pane for `feat/beta` SHALL be created

Test: `tests/unattended_drive_e2e.rs::unattended_rebinds_a_loop_to_a_live_session_after_heartbeat`
