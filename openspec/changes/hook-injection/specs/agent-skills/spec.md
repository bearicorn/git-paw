## MODIFIED Requirements

### Requirement: Embedded coordination skill

The coordination skill SHALL be updated to reflect that status publishing is now automated. The skill SHALL:

- Remove the "MUST publish agent.status" requirement (automated by filesystem watcher)
- Remove the status curl command from the "required" section
- Keep `agent.blocked` and `agent.artifact` (with exports) curl commands as opt-in actions
- Add a note: "git-paw automatically publishes your working status when you edit files and commits artifacts when you `git commit`. You only need to publish manually if you are blocked or done with specific exports to announce."
- Keep cherry-pick instructions and messages-you-may-receive reference

#### Scenario: Coordination skill documents automatic status

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains text indicating that status publishing is automatic
- **AND** it does NOT contain "MUST publish agent.status"

#### Scenario: Coordination skill still has blocked and artifact commands

- **WHEN** the embedded coordination skill is inspected
- **THEN** it contains curl commands for `agent.blocked` and `agent.artifact`
