## ADDED Requirements

### Requirement: The root session's broker identity is orchestrator

The broker SHALL use `orchestrator` as the reserved identity of the repository-root session: it SHALL be a valid `agent_id`, the inbox that receives `agent.question` messages and the verify nudge SHALL be the `orchestrator` inbox, the verify nudge's `type` SHALL be `orchestrator.verify-now`, and every message git-paw itself publishes as the root session — from the drive loop, the bundled `sweep.sh` and the root-side MCP tools — SHALL carry `orchestrator`. Until v1.0.0 the broker SHALL also accept the deprecated spelling `supervisor` wherever the root identity appears in a request: the envelope `agent_id`, the sender fields `verified_by` and `from` (of `agent.feedback`, `agent.answer`, `agent.blocked` and `agent.advanced-main`), the `agent.learning` payload `agent_id`, and the `GET /messages/:agent_id` path. It SHALL rewrite that spelling to `orchestrator` when it decodes the request, before validation, the verdict-authority check, routing and logging, so that no stored, served or logged message carries `supervisor`, and SHALL print one deprecation warning per broker process through the shared helper the first time it does so. Decoding SHALL accept `supervisor.verify-now` as the type `orchestrator.verify-now`. Requirements that name the agent id `supervisor`, the `supervisor` inbox or `supervisor.verify-now` SHALL be read as naming `orchestrator`, the `orchestrator` inbox and `orchestrator.verify-now`.

#### Scenario: The old spelling is stored as the new one

- **GIVEN** a running broker
- **WHEN** a client publishes `{"type":"agent.status","agent_id":"supervisor","payload":{"status":"working","modified_files":[]}}`
- **THEN** the publish SHALL succeed
- **AND** `GET /status` SHALL list an agent `orchestrator` and no agent `supervisor`

#### Scenario: Both inbox paths serve the root inbox

- **GIVEN** a worker that published an `agent.question`
- **WHEN** a client reads `GET /messages/orchestrator` and `GET /messages/supervisor`
- **THEN** both responses SHALL contain the question

#### Scenario: Sender fields are normalized

- **GIVEN** a broker whose root session has published an `agent.status` as `orchestrator`
- **WHEN** a client publishes `agent.blocked` from `feat-a` with `from = "supervisor"`
- **THEN** the message delivered and logged SHALL carry `from = "orchestrator"`
- **AND** it SHALL be routed to the `orchestrator` inbox

#### Scenario: The verify nudge uses the new type and the old one decodes

- **GIVEN** `verify_on_commit_nudge` enabled and an agent that publishes `agent.artifact` with status `committed`
- **WHEN** the `orchestrator` inbox is read
- **THEN** it SHALL contain a message whose `type` is `orchestrator.verify-now`
- **AND** decoding the JSON `{"type":"supervisor.verify-now","branch_id":"feat-a"}` SHALL produce the same message as `{"type":"orchestrator.verify-now","branch_id":"feat-a"}`

#### Scenario: The old spelling cannot bypass verdict authority

- **GIVEN** a broker holding a root credential
- **WHEN** a client without the credential publishes `agent.feedback` for `feat-a` with `from = "supervisor"`, and again with `from = "orchestrator"`
- **THEN** both publishes SHALL be refused with the same authority error

#### Scenario: Git-paw's own root publishes use the new identity

- **WHEN** `sweep.sh status-publish`, `sweep.sh verified` and `sweep.sh learn` publish against a recording endpoint
- **THEN** every captured body SHALL carry `orchestrator` as its root identity and none SHALL contain `supervisor`
