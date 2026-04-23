## MODIFIED Requirements

### Requirement: Embedded coordination skill

The embedded `coordination.md` skill content SHALL be updated to require proactive status publishing at three trigger points:

1. When starting work on a new file or task
2. After editing or creating any file (with `modified_files` populated)
3. After each `git commit`

The embedded content SHALL include a new section: `### Cherry-pick peer commits` with the exact `git cherry-pick` command agents should use when a peer's artifact arrives in their inbox.

The existing requirements for the four broker operation examples (`agent.status`, `/messages/{{BRANCH_ID}}`, `agent.artifact`, `agent.blocked`) remain unchanged.

#### Scenario: Coordination skill requires proactive status on file edit

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains instructions to publish `agent.status` when editing or creating files

#### Scenario: Coordination skill requires status after commit

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains the phrase "after each commit" (or equivalent instruction) adjacent to the `agent.status` publish command

#### Scenario: Coordination skill contains cherry-pick instructions

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains the substring `git cherry-pick`

#### Scenario: Existing four operations still present after update

- **WHEN** the updated embedded coordination skill is inspected
- **THEN** it still contains `agent.status`, `agent.artifact`, `agent.blocked`, and `${GIT_PAW_BROKER_URL}/messages/{{BRANCH_ID}}`
