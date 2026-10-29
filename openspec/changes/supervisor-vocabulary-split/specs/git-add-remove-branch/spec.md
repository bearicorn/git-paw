## ADDED Requirements

### Requirement: remove refuses the root identity

`git paw remove` SHALL refuse the root session's identity `orchestrator` and, until v1.0.0, its deprecated spelling `supervisor`, failing before any pane, worktree or session state is touched with a message that names `git paw stop` as the way to end the whole session.

#### Scenario: Removing the orchestrator is refused

- **GIVEN** an active session
- **WHEN** the user runs `git paw remove orchestrator`, and again `git paw remove supervisor`
- **THEN** each command SHALL exit non-zero naming `git paw stop`
- **AND** no pane, worktree or session entry SHALL change
