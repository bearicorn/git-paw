## ADDED Requirements

### Requirement: Bundled task prompts name only real broker messages and promise no future releases

The bundled per-backend task prompts SHALL refer only to broker message types that exist. They SHALL NOT instruct an agent to publish a message type the broker does not define; where they describe signalling completion, they SHALL name the mechanism that actually exists (for code changes, committing — the post-commit hook publishes the artifact). They SHALL NOT state a version in which a feature is planned to arrive, because shipped prompt text outlives roadmaps and an agent cannot act on a promise.

#### Scenario: Task prompts reference only defined message types

- **WHEN** every bundled task prompt is inspected
- **THEN** each broker message type it names SHALL be one the broker defines
- **AND** none SHALL instruct publishing `agent.done`

#### Scenario: Task prompts make no release-version promises

- **WHEN** every bundled task prompt is inspected
- **THEN** none SHALL state a future version in which a feature is planned
