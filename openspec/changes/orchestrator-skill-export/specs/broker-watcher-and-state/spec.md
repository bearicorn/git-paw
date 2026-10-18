## MODIFIED Requirements

### Requirement: Supervisor publishes on merge to main

The bundled orchestrator skill (formerly the supervisor skill) SHALL teach the LLM to publish an
`agent.advanced-main` event after every successful `git merge`
targeting `main` (or the configured `[git] main_branch`). The
skill SHALL include a concrete example publishing it through the
bundled helper — `.git-paw/scripts/sweep.sh advanced-main
<merged-branch> <summary>` — which sends the message shape from
this capability; the example SHALL NOT be a raw `curl` request.

#### Scenario: Skill prose names the publish trigger explicitly

- **WHEN** the merge-orchestration section of the orchestrator skill is
  read
- **THEN** the prose SHALL include a publish step that fires
  immediately after a successful merge to main, with a
  concrete `sweep.sh advanced-main` example
- **AND** the section SHALL NOT contain a `curl` request to `/publish`

#### Scenario: Publish includes the resolved base name

- **WHEN** the skill prose shows the publish step
- **THEN** the `base` field SHALL be documented as the
  resolved `[git] main_branch` value, not hardcoded `"main"`
