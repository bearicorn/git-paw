## ADDED Requirements

### Requirement: The pre-commit guard refuses commits on a frozen branch

When the committing worktree's gitdir holds a `paw-frozen` marker, the pre-commit hook SHALL refuse the commit with exit status 1 and a message naming the frozen SHA, that the branch is under verification, and that an `agent.verified` or `agent.feedback` lifts the freeze. The check SHALL apply whatever `strict_branch_guard` is set to.

#### Scenario: A frozen worker cannot commit

- **GIVEN** agent `feat-x`'s gitdir holds `paw-frozen` with SHA `abc123`
- **WHEN** the agent runs `git commit`
- **THEN** the commit SHALL fail and the message SHALL name `abc123` and say the branch is under verification

#### Scenario: Without the marker commits proceed

- **GIVEN** agent `feat-x`'s gitdir holds no `paw-frozen`
- **WHEN** the agent commits on its own branch
- **THEN** the pre-commit hook SHALL NOT refuse it
