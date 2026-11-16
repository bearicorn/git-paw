## ADDED Requirements

### Requirement: remove withdraws the agent from the broker before teardown

When the session has a live broker, `git paw remove <branch>` SHALL withdraw the agent's worktree through `DELETE /watch` before running the `on_remove` hook or deleting the worktree. When the broker is unreachable it SHALL print a warning and continue. With `--keep-worktree` it SHALL still withdraw the agent.

#### Scenario: Teardown raises no false conflict

- **GIVEN** a session with `feat-x` and `feat-y` whose worktrees both modify `src/a.rs`
- **WHEN** the user runs `git paw remove feat-x --force`
- **THEN** the broker SHALL receive no `agent.status` for `feat-x` listing deleted files
- **AND** no `agent.question` or feedback naming `feat-x` SHALL be emitted afterwards

#### Scenario: An unreachable broker does not block removal

- **GIVEN** a session whose broker is not answering
- **WHEN** the user runs `git paw remove feat-x`
- **THEN** the worktree SHALL be removed and the output SHALL contain a warning that the broker could not be told
