## ADDED Requirements

### Requirement: The drive loop follows the live agent roster

The drive loop SHALL re-read its session's coding-agent roster (branch, agent id, worktree path) from the persisted session state at the start of every sweep, rather than fixing it at loop start, so an agent attached with `git paw add` while the loop runs is resolved as a coding agent and driven, and an agent detached with `git paw remove` stops being driven. A sweep whose roster read fails SHALL keep the previous roster and continue. The completion and exit-summary decisions SHALL use the roster as last read.

#### Scenario: An added agent is driven by a running loop

- **GIVEN** an `--unattended` session whose drive loop started with two agents
- **WHEN** `git paw add feat/third` attaches a third agent that then shows a live permission prompt
- **THEN** a subsequent sweep SHALL resolve that surface as a coding agent and escalate the prompt as a `permission-prompt` naming `feat-third`
- **AND** SHALL send no keystroke answering it

#### Scenario: A removed agent is no longer driven

- **GIVEN** an `--unattended` session whose drive loop is driving `feat/x`
- **WHEN** `git paw remove feat/x` completes
- **THEN** no later sweep SHALL send keystrokes attributed to `feat/x`
- **AND** the exit summary SHALL NOT list `feat/x` as an agent of the wave

#### Scenario: An unreadable roster keeps the previous one

- **GIVEN** a running drive loop whose session file cannot be read on one sweep
- **WHEN** that sweep runs
- **THEN** the loop SHALL continue with the roster it last read
