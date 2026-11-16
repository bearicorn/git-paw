## MODIFIED Requirements

### Requirement: add refills a live session that has no agents

`git paw add` SHALL accept a live session that records zero agents — one whose last agent was removed with `git paw remove` while its tmux session, dashboard and broker kept running — and attach the new agent to it exactly as it would to a session that has agents. It SHALL NOT refuse such a session as stopped or ask for `git paw start`.

#### Scenario: add after removing the last agent attaches the new agent

- **GIVEN** a live session whose only agent was removed with `git paw remove`, leaving its tmux session running
- **WHEN** the user runs `git paw add feat/next`
- **THEN** the command SHALL succeed and create a pane, worktree and session entry for `feat/next`
- **AND** SHALL NOT print `is stopped — recover it with \`git paw start\``

Test: `tests/emptied_session_refill_e2e.rs::add_refills_a_live_session_emptied_by_remove`

#### Scenario: add on a genuinely stopped session is still refused

- **GIVEN** a session whose tmux session no longer exists
- **WHEN** the user runs `git paw add feat/next`
- **THEN** the command SHALL still refuse and name `git paw start`

Test: `tests/emptied_session_refill_e2e.rs::add_on_a_stopped_session_is_refused_naming_start` (added by this change; no test covered the scenario)
