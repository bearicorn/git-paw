## ADDED Requirements

### Requirement: The broker host supplies the agent-to-spec mapping

The process that hosts the broker SHALL, when it attaches the learnings aggregator, seed the agent → spec mapping from the `spec_id` recorded on every worktree entry of the saved session when one exists (recovery, restart from pause; per *Conflict-event signal*), and SHALL record the mapping for every agent registered through `POST /watch` with a spec id — by the launcher of a fresh launch, whose receipt is saved only after the panes exist, or by `git paw add` — so the classification of forward conflicts as intra-spec or cross-spec uses real launch data, including for a hot-added agent. An agent with no recorded spec id SHALL remain unmapped.

#### Scenario: Two agents launched from the same spec are classified intra-spec

- **GIVEN** a session launched with agents `feat-x` and `feat-y`, both recorded with spec id `003-user-list`, and learnings enabled
- **WHEN** the conflict detector emits a forward conflict between `feat-x` and `feat-y`
- **THEN** the aggregator SHALL record a `forward-conflict-intra-spec` entry for the pair

#### Scenario: Agents from different specs are classified cross-spec

- **GIVEN** `feat-x` recorded with spec id `003-user-list` and `feat-y` with `004-error-handling`
- **WHEN** the conflict detector emits a forward conflict between them
- **THEN** the aggregator SHALL record a `forward-conflict-cross-spec` entry naming both spec ids

#### Scenario: The mapping has a production source

- **WHEN** a spec-driven session with learnings enabled is launched fresh, or recovered from its saved receipt
- **THEN** the aggregator SHALL hold a spec id for every agent launched from a spec, without any test-only setup
