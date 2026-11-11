## ADDED Requirements

### Requirement: The orchestrator delegates to delegate profiles first

The orchestrator skill's delegation section SHALL direct delegated work to a configured delegate profile whose role skill matches it. Only when none is configured SHALL it use the CLI's own isolated subagents, and only when neither is available SHALL it run the gate inline.

#### Scenario: Delegate profiles come first

- **WHEN** the rendered orchestrator skill's delegation section is read
- **THEN** it SHALL name `git paw delegate --list`
- **AND** it SHALL order the mechanisms as delegate profile, then the CLI's own isolated subagents, then inline

### Requirement: A delegate profile counts as an isolated subagent

A configured delegate profile SHALL satisfy the isolated-subagent condition of *The orchestrator skill delegates audit gates through a compressed-verdict contract* and *Delegation degrades to inline gates*, whatever the orchestrator's own CLI supports, so an orchestrator whose CLI has no subagents still delegates when a matching delegate profile exists.

#### Scenario: A delegate profile is used when the CLI has no subagents

- **GIVEN** an orchestrator whose CLI cannot run isolated subagents and a configured delegate profile whose role skill is `spec-auditor`
- **WHEN** the rendered delegation section is read
- **THEN** it SHALL direct the spec audit to that delegate profile and SHALL NOT direct the orchestrator to run it inline

### Requirement: The delegation section maps work to delegate roles

The delegation section SHALL tell the orchestrator to list delegate profiles with `git paw delegate --list`, to run each audit gate through a delegate profile whose role skill is that gate's auditor (`spec-auditor`, `doc-auditor`, `security-auditor`) and broad read-only exploration through one whose role skill is `explorer`, passing the branch, the re-resolved tip and the change id in the task (`git paw delegate <name> --task …`).

#### Scenario: Gates map to auditor roles

- **WHEN** the delegation section is read
- **THEN** it SHALL map the spec, doc and security audit gates to the `spec-auditor`, `doc-auditor` and `security-auditor` role skills, and exploration to `explorer`

### Requirement: Publishing stays with the orchestrator when it delegates

The delegation section SHALL state that at most four read-only delegates run at once and that a delegate's stdout is its verdict or map, and SHALL keep stating, as *Only the orchestrator publishes verdicts for delegated gates* requires, that delegates never publish and the orchestrator alone publishes `agent.verified` or one `feedback-gate` per finding.

#### Scenario: Publishing stays with the orchestrator

- **WHEN** the delegation section is read
- **THEN** it SHALL state that at most four read-only delegates run at once and that delegates never publish
- **AND** it SHALL state that the orchestrator publishes the verdict
