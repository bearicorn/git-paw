## ADDED Requirements

### Requirement: Coordination skill — poll the inbox before each commit

The coordination skill SHALL instruct the coding agent to poll its broker inbox, and act on any `agent.feedback` or `orchestrator.freeze` it finds, before every commit, so orchestrator instructions are read before the work they affect is committed.

#### Scenario: Skill orders poll before commit

- **WHEN** the embedded coordination skill's commit guidance is inspected
- **THEN** it instructs the agent to run the helper's `poll` before each `git commit` and to handle feedback and freeze messages first
