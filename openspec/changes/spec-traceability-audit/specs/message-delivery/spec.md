## MODIFIED Requirements

### Requirement: Publish updates sender's agent record

When `publish_message` is called with a **status-publishing** message variant — `Status`, `Artifact`, `Blocked`, or `Intent` — the system SHALL update the sender's `AgentRecord` in `BrokerStateInner`:

- Set `last_seen` to the current instant
- Set `status` to the message's status label (e.g. `"working"`, `"done"`, `"blocked"`)
- Set `last_message` to a clone of the published message
- If no `AgentRecord` exists for the sender's `agent_id`, one SHALL be created automatically (lazy registration)
- If no inbox queue exists for the sender's `agent_id`, one SHALL be created automatically

Identity and routing variants (`Verified`, `Feedback`, `Answer`, `Question`) SHALL NOT create or mutate any `AgentRecord` — see "Roster updates exclude identity and routing variants". This scoping is the W15-16 phantom-row fix: a verifier, feedback sender, answer sender, or questioner is not publishing its own status, so it must not mint a roster row or bump one's `last_seen`.

#### Scenario: First publish from an agent creates its record

- **GIVEN** a `BrokerState` with no known agents
- **WHEN** `publish_message` is called with an `agent.status` message from `agent_id = "feat-errors"`
- **THEN** `BrokerStateInner.agents` contains a record for `"feat-errors"`
- **AND** the record's `status` is `"working"`
- **AND** the record's `last_seen` is approximately `Instant::now()`

#### Scenario: Subsequent publish updates an existing record

- **GIVEN** a `BrokerState` with an existing record for `"feat-errors"` with status `"working"`
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"` with status `"done"`
- **THEN** the record's `status` is updated to `"done"`
- **AND** `last_seen` is updated

#### Scenario: Publish creates an inbox for the sender

- **GIVEN** a `BrokerState` with no known agents
- **WHEN** `publish_message` is called from `"feat-errors"`
- **THEN** `BrokerStateInner.queues` contains an inbox entry for `"feat-errors"`

## REMOVED Requirements

### Requirement: Agent record updated for new message types

**Reason:** contradicts the shipped, tested W15-16 phantom-row fix — `Verified` and `Feedback` deliberately do NOT upsert the sender's roster record. Replaced by "Roster updates exclude identity and routing variants".

### Requirement: Agent record updated for Question variant

**Reason:** contradicts the shipped, tested W15-16 phantom-row fix — `Question` is routed to the supervisor inbox but does NOT create or mutate the questioner's roster record. Folded into "Roster updates exclude identity and routing variants".

## ADDED Requirements

### Requirement: Roster updates exclude identity and routing variants

The system SHALL NOT create or mutate any `AgentRecord` when `agent.verified`, `agent.feedback`, `agent.answer`, or `agent.question` is published. These variants are routed and logged (per their delivery requirements) but are not status publications, so they SHALL NOT mint a phantom roster row for a sender that has not otherwise published its status, nor bump an existing row's `last_seen`, `status`, or `last_message`. Only the status-publishing variants (`Status`, `Artifact`, `Blocked`, `Intent`) update the roster (see "Publish updates sender's agent record" and "Agent record updated for Intent variant").

#### Scenario: Verified does not mutate the verifier's roster record

- **WHEN** `publish_message` is called with an `agent.verified` from `"supervisor"`
- **THEN** no `AgentRecord` is created for `"supervisor"` by this publish
- **AND** any pre-existing `"supervisor"` record's `last_seen`, `status`, and `last_message` are left unchanged

Test: `broker::delivery::tests::verified_does_not_mutate_verifier_record`

#### Scenario: Feedback does not mutate the sender's roster record

- **WHEN** `publish_message` is called with an `agent.feedback` from a sender
- **THEN** the sender's `AgentRecord` is neither created nor mutated by this publish

Test: `broker::delivery::tests::feedback_does_not_mutate_sender_record`

#### Scenario: Question does not create a sender roster row

- **GIVEN** no `AgentRecord` exists for `"feat-x"`
- **WHEN** `publish_message` is called with an `agent.question` from `"feat-x"`
- **THEN** no roster row is created for `"feat-x"` (the question is still routed to the supervisor inbox)

Test: `broker::delivery::tests::question_does_not_create_sender_roster_row`

#### Scenario: Question leaves an existing sender row unchanged

- **GIVEN** an existing `AgentRecord` for `"feat-x"`
- **WHEN** `publish_message` is called with an `agent.question` from `"feat-x"`
- **THEN** that row's `last_seen`, `status`, and `last_message` are unchanged

Test: `broker::delivery::tests::question_leaves_existing_sender_row_unchanged`
