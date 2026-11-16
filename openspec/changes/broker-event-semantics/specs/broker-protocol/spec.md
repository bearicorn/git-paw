## ADDED Requirements

### Requirement: Artifacts carry the commit they announce

The `agent.artifact` payload SHALL accept an optional `sha` string naming the commit the artifact announces. It SHALL be omitted from the wire when absent, and an artifact without it SHALL decode and route exactly as before.

#### Scenario: An artifact with sha round-trips

- **GIVEN** `{"type":"agent.artifact","agent_id":"feat-x","payload":{"status":"committed","modified_files":["a.rs"],"sha":"abc123"}}`
- **WHEN** it is decoded and re-encoded
- **THEN** the payload's `sha` SHALL be `abc123`

#### Scenario: An artifact without sha is unchanged

- **GIVEN** an `agent.artifact` with no `sha`
- **WHEN** it is encoded
- **THEN** the output SHALL contain no `sha` key

### Requirement: Verified messages carry the verified tip

The `agent.verified` payload SHALL accept an optional `sha` naming the branch tip the verification ran against. The broker SHALL record the latest `sha` per verified agent for the merge check. A verified message without `sha` SHALL be accepted and SHALL record no tip.

#### Scenario: The verified tip is recorded

- **GIVEN** a running broker
- **WHEN** it accepts `agent.verified` for `feat-x` with `sha = aaa111`
- **THEN** the broker's last verified tip for `feat-x` SHALL be `aaa111`

### Requirement: Freeze message variant

The broker SHALL accept `orchestrator.freeze` with an `agent_id` and a `sha`, require the session authority credential for it as for other verdicts, route it to that agent's inbox only, and record the agent as frozen at that SHA. An `agent.verified` or orchestrator `agent.feedback` for that agent SHALL clear the record.

#### Scenario: A freeze reaches only its agent

- **GIVEN** agents `feat-x` and `feat-y` with inboxes
- **WHEN** an authorised `orchestrator.freeze` for `feat-x` with `sha = abc123` is published
- **THEN** only `feat-x`'s inbox SHALL receive it and `/status` SHALL show `feat-x` frozen at `abc123`

#### Scenario: A freeze without the credential is refused

- **GIVEN** a broker with an authority credential loaded
- **WHEN** an `orchestrator.freeze` is published without the credential header
- **THEN** the broker SHALL answer `403` and record nothing

#### Scenario: A verdict clears the freeze

- **GIVEN** `feat-x` frozen at `abc123`
- **WHEN** an `agent.verified` for `feat-x` is accepted
- **THEN** `/status` SHALL no longer show `feat-x` frozen
