## ADDED Requirements

### Requirement: sweep.sh learn ids follow the aggregator's deterministic-id scheme

The bundled `sweep.sh learn` subcommand SHALL compute the `id` of the `agent.learning` message it publishes with the scheme defined by "Deterministic id for idempotent re-emission" — over its `category`, an absent `branch_id` (supervisor learnings are cross-cutting), its `body`, and the current UTC hour bucket — so that the helper and the aggregator assign the same id to the same logical record and broker consumers can dedupe across both publishers. The helper SHALL NOT use a different hash function or a different canonical serialisation.

#### Scenario: learn's id matches the aggregator's id for the same record

- **GIVEN** a `learn` invocation with category `tooling_friction` and body `{"friction":"git commit re-prompts","occurrences":3}` made within one UTC hour
- **WHEN** the helper publishes the record
- **THEN** its `id` SHALL equal the id the aggregator's deterministic-id function computes for category `tooling_friction`, no branch id, the same body, and the same hour bucket

#### Scenario: The title does not change the id

- **GIVEN** two `learn` invocations within one UTC hour with the same category and body but different titles
- **WHEN** both publish
- **THEN** both messages SHALL carry the same `id`
