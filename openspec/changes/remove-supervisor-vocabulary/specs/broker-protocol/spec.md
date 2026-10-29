## MODIFIED Requirements

### Requirement: The root session's broker identity is orchestrator

The broker SHALL use `orchestrator` as the reserved identity of the repository-root session: it SHALL be a valid `agent_id`, the inbox that receives `agent.question` messages and the verify nudge SHALL be the `orchestrator` inbox, the verify nudge's `type` SHALL be `orchestrator.verify-now`, and every message git-paw itself publishes as the root session — from the drive loop, the bundled `sweep.sh` and the root-side MCP tools — SHALL carry `orchestrator`. The retired spelling `supervisor` SHALL NOT be accepted on the live protocol: a request that carries `supervisor` as the envelope `agent_id`, as the sender field `verified_by` or `from` (of `agent.feedback`, `agent.answer`, `agent.blocked` or `agent.advanced-main`), or as the `agent.learning` payload `agent_id`, a `GET /messages/supervisor`, and a publish whose `type` is `supervisor.verify-now` SHALL each be answered `400` with an error body that names `orchestrator`, and SHALL NOT be stored, routed or logged. Records persisted by releases before 1.0.0 and read back by git-paw SHALL still decode, with the old spellings mapped to `orchestrator` and `orchestrator.verify-now`.

#### Scenario: The retired agent id is refused

- **WHEN** a client publishes `{"type":"agent.status","agent_id":"supervisor","payload":{"status":"working","modified_files":[]}}`
- **THEN** the broker SHALL answer `400` with a body naming `orchestrator`
- **AND** `GET /status` SHALL list no agent `supervisor`

#### Scenario: A retired sender field is refused, not dropped

- **WHEN** a client publishes `agent.blocked` from `feat-a` with `from = "supervisor"`
- **THEN** the broker SHALL answer `400` naming `orchestrator`

#### Scenario: The retired spelling cannot bypass verdict authority

- **GIVEN** a broker holding a root credential
- **WHEN** a client without the credential publishes `agent.feedback` for `feat-a` with `from = "supervisor"`
- **THEN** the publish SHALL be refused
- **AND** no `agent.feedback` for `feat-a` SHALL be stored or delivered

#### Scenario: The retired inbox path is refused

- **WHEN** a client reads `GET /messages/supervisor`
- **THEN** the broker SHALL answer `400` naming `/messages/orchestrator`

#### Scenario: The verify nudge uses the orchestrator type

- **GIVEN** `verify_on_commit_nudge` enabled and an agent that publishes `agent.artifact` with status `committed`
- **WHEN** the `orchestrator` inbox is read
- **THEN** it SHALL contain a message whose `type` is `orchestrator.verify-now`
- **AND** a publish whose `type` is `supervisor.verify-now` SHALL be answered `400`

#### Scenario: Records from earlier releases still decode

- **GIVEN** an event record written by a 0.x release containing `agent_id = "supervisor"` and a `supervisor.verify-now` message
- **WHEN** git-paw reads the record back
- **THEN** both entries SHALL decode, carrying `orchestrator` and `orchestrator.verify-now`

#### Scenario: Git-paw's own root publishes use the orchestrator identity

- **WHEN** `sweep.sh status-publish`, `sweep.sh verified` and `sweep.sh learn` publish against a recording endpoint
- **THEN** every captured body SHALL carry `orchestrator` as its root identity and none SHALL contain `supervisor`
