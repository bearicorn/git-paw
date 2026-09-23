## MODIFIED Requirements

### Requirement: Conflict-event signal

The aggregator SHALL track conflict events by subscribing to `agent.feedback` and `agent.question` messages whose error/question text begins with the `[conflict-detector]` tag (per the conflict-detection capability's emission convention).

For each tagged message, the aggregator SHALL classify the event into one of:
- `forward-conflict-intra-spec` — both implicated agent_ids belong to the same `SpecEntry` family
- `forward-conflict-cross-spec` — the agent_ids belong to different `SpecEntry` families
- `in-flight-conflict` — text matches the in-flight pattern
- `ownership-violation` — text matches the ownership pattern

Each classified event contributes one bullet to the markdown file's "Conflict events" section at the next flush. Intra-vs-cross-spec classification SHALL use the agent → `SpecEntry` mapping the broker session tracks at the time of the event.

When the learnings aggregator is attached to a session, that mapping SHALL be populated from the `spec_id` each worktree entry of the persisted session records, keyed by the agent id the broker uses for that entry. A forward conflict involving an agent with no recorded `spec_id` SHALL classify as `forward-conflict-cross-spec`, and its unknown spec id SHALL NOT be rendered as a named spec.

#### Scenario: Forward-conflict-intra-spec is classified

- **GIVEN** the conflict detector emitted `agent.feedback` to agents X and Y, both belonging to spec `003-user-list`, with text containing `[conflict-detector] forward conflict`
- **WHEN** the aggregator processes those messages
- **THEN** one entry SHALL be recorded with category `forward-conflict-intra-spec` referencing the same agent pair
- **AND** the next flush SHALL append a corresponding bullet under "Conflict events"

#### Scenario: Forward-conflict-cross-spec is classified

- **GIVEN** the conflict detector emitted `agent.feedback` to agents X (spec `003-user-list`) and Y (spec `004-error-handling`)
- **WHEN** the aggregator processes those messages
- **THEN** one entry SHALL be recorded with category `forward-conflict-cross-spec` and the entry SHALL name both spec ids

#### Scenario: Ownership violation is classified

- **WHEN** the conflict detector emits `agent.feedback` with text containing `[conflict-detector] ownership violation`
- **THEN** the aggregator SHALL record an entry with category `ownership-violation` naming the violator and owner agent ids and the file path

#### Scenario: The spec mapping is seeded from the persisted session

- **GIVEN** a persisted session whose worktree entries for agents X and Y both record `spec_id = "003-user-list"`
- **WHEN** the aggregator is attached for that session and then processes a `[conflict-detector] forward conflict` message naming X and Y
- **THEN** it SHALL record a `forward-conflict-intra-spec` entry with `spec_id` `003-user-list`
- **AND** SHALL NOT require any call other than attaching it to the session to learn the mapping

#### Scenario: An agent without a recorded spec classifies cross-spec

- **GIVEN** a persisted session in which agent X records `spec_id = "003-user-list"` and agent Y records no `spec_id`
- **WHEN** the aggregator processes a forward conflict naming X and Y
- **THEN** it SHALL record a `forward-conflict-cross-spec` entry
- **AND** the rendered bullet SHALL name `003-user-list` and SHALL NOT render an empty spec id
