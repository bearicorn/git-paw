## MODIFIED Requirements

### Requirement: remove refuses the root identity

`git paw remove` SHALL refuse the root session's identity `orchestrator`, failing before any pane, worktree or session state is touched with a message that names `git paw stop` as the way to end the whole session. The name `supervisor` SHALL be treated as an ordinary branch name.

#### Scenario: Removing the orchestrator is refused

- **GIVEN** an active session
- **WHEN** the user runs `git paw remove orchestrator`
- **THEN** the command SHALL exit non-zero naming `git paw stop`
- **AND** no pane, worktree or session entry SHALL change

#### Scenario: supervisor is an ordinary branch name

- **GIVEN** an active session whose agent list does not include `supervisor`
- **WHEN** the user runs `git paw remove supervisor`
- **THEN** the command SHALL fail with the not-in-session error listing the live agents, not with the root-identity refusal
