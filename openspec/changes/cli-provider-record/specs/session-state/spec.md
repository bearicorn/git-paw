## MODIFIED Requirements

### Requirement: Recovery relaunches a worker with its recorded session id

When `git paw start` recovers a stopped session, the system SHALL relaunch each worktree with the
pane command built by the single launch-command builder (per `cli-provider`) for the entry's
recorded CLI: the executable, the CLI's `args`, the approval flags for the configured
`agent_approval`, and — when the entry carries a `cli_session_id` AND its CLI resolves a resume
template — that template's arguments, so the worker's conversation context is restored. A worktree
whose entry has no id, or whose CLI resolves no resume template, SHALL be relaunched without resume
arguments. Recovery SHALL NOT fail because a resume template is absent.

#### Scenario: Recovered Claude worker is resumed

- **GIVEN** a stopped session whose worktree entry for `feat/x` has `cli = "claude"` and
  `cli_session_id = Some("<uuid>")`, and no `[clis.claude]` override
- **WHEN** the session is recovered
- **THEN** the pane command for `feat/x` SHALL contain `--resume <uuid>`

#### Scenario: Recovered worker keeps its approval flags

- **GIVEN** a stopped session whose entry for `feat/x` has `cli = "claude"` and
  `agent_approval = "auto"` in the current config
- **WHEN** the session is recovered
- **THEN** the pane command for `feat/x` SHALL contain `--permission-mode acceptEdits`

#### Scenario: Entry without an id launches fresh

- **GIVEN** a stopped session whose worktree entry has `cli_session_id = None`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL contain no resume arguments
- **AND** it SHALL equal the command a fresh launch of that CLI at the same approval level would
  build, without session-id arguments

#### Scenario: CLI without a resume template launches fresh

- **GIVEN** a worktree entry with `cli = "my-agent"`, `cli_session_id = Some("<id>")`, and no
  `[clis.my-agent].resume_args`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL contain no resume arguments
- **AND** recovery SHALL succeed
