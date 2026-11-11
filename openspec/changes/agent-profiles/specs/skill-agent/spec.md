## ADDED Requirements

### Requirement: The orchestrator delegates to delegate profiles first

The orchestrator skill's delegation section SHALL direct delegated work to a configured delegate profile whose role skill matches it, which SHALL count as an isolated subagent whatever the orchestrator's own CLI supports. Only when none is configured SHALL it use the CLI's own isolated subagents, and only when neither is available SHALL it run the gate inline. The section SHALL name no vendor product, model, or persona file format or path.

#### Scenario: Delegate profiles come first

- **WHEN** the rendered orchestrator skill's delegation section is read
- **THEN** it SHALL name `git paw delegate --list`
- **AND** it SHALL order the mechanisms as delegate profile, then the CLI's own isolated subagents, then inline

#### Scenario: The section stays vendor-neutral

- **WHEN** the delegation section is audited
- **THEN** it SHALL contain no vendor product name, model name or vendor persona path

### Requirement: The delegation section maps work to delegate roles

The delegation section SHALL tell the orchestrator to list delegate profiles with `git paw delegate --list`, to run each audit gate through a delegate profile whose role skill is that gate's auditor (`spec-auditor`, `doc-auditor`, `security-auditor`) and broad read-only exploration through one whose role skill is `explorer`, passing the branch, the re-resolved tip and the change id in the task (`git paw delegate <name> --task …`).

#### Scenario: Gates map to auditor roles

- **WHEN** the delegation section is read
- **THEN** it SHALL map the spec, doc and security audit gates to the `spec-auditor`, `doc-auditor` and `security-auditor` role skills, and exploration to `explorer`

### Requirement: Publishing stays with the orchestrator when it delegates

The delegation section SHALL state that at most four read-only delegates run at once, that a delegate's stdout is its verdict or map, that delegates never publish, and that the orchestrator alone publishes `agent.verified` or one `feedback-gate` per finding.

#### Scenario: Publishing stays with the orchestrator

- **WHEN** the delegation section is read
- **THEN** it SHALL state that delegates never publish and that the orchestrator publishes the verdict
