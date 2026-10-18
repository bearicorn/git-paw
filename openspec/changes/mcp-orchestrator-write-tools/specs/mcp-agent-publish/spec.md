## MODIFIED Requirements

### Requirement: MCP publish tools cover exactly the four agent boot events

The system SHALL expose MCP tools that publish the four agent runtime events — register/status, done/artifact, blocked, and question — to the broker, to every session; and SHALL expose two further tools, `publish_verified` and `publish_feedback`, to a root session only. Each tool SHALL publish the same `BrokerMessage` shape the bundled shell helpers publish for that event (`broker.sh` for the four worker events, `sweep.sh verified` / `feedback-gate` for the two authority events), so the paths are wire-identical. Each tool SHALL advertise a JSON Schema for its parameters and return shape.

The publish tool set SHALL be fixed. The system SHALL NOT expose a generic tool that publishes a caller-supplied message type or an arbitrary `BrokerMessage`.

#### Scenario: Four publish tools are advertised to every session

- **WHEN** an MCP client lists the available tools from any session
- **THEN** the registry SHALL include a publish tool for each of status, artifact, blocked, and question
- **AND** each SHALL carry a name, description, and input schema

#### Scenario: Two authority publish tools are advertised to a root session only

- **WHEN** an MCP client lists tools from a root session and from a worktree session
- **THEN** the root list SHALL include `publish_verified` and `publish_feedback`
- **AND** the worktree list SHALL NOT

#### Scenario: Wire shape matches the shell helpers

- **WHEN** an agent publishes an event via an MCP publish tool
- **THEN** the resulting broker message SHALL have the same type and payload shape as the equivalent bundled-helper invocation

#### Scenario: No generic publish tool exists

- **WHEN** the tool registry is inspected
- **THEN** it SHALL NOT contain a tool that accepts an arbitrary message type or an arbitrary `BrokerMessage` body

### Requirement: Publish tools act as the calling agent only

Each publish tool SHALL derive the publishing identity from the calling session: in a worktree session, the slugified worktree branch (the established branch-to-`agent_id` conversion); in a root session, the wire identity `supervisor`. A publish tool SHALL NOT accept an `agent_id` parameter naming the publisher; the authority tools take the *target* agent as a parameter, as the helpers do, and set `verified_by` / `from` to `supervisor` themselves. A session SHALL NOT be able to publish a message attributed to a different publisher.

#### Scenario: Publisher id is derived, not supplied

- **WHEN** any publish tool's input schema is inspected
- **THEN** it SHALL NOT contain a parameter naming the publishing identity

#### Scenario: Worktree message carries the branch slug

- **GIVEN** an MCP session resolved to a worktree on branch `feat/x`
- **WHEN** that session publishes a status event
- **THEN** the published message's `agent_id` SHALL be the slugified form of `feat/x`

#### Scenario: Root verdict carries supervisor as the verifier

- **GIVEN** a root MCP session
- **WHEN** it calls `publish_verified({ "agent": "feat-x", "message": "gates green" })`
- **THEN** the published `agent.verified` SHALL have `agent_id = "feat-x"` and `verified_by = "supervisor"`

#### Scenario: Impersonation is not possible

- **GIVEN** an MCP session resolved to one worktree
- **WHEN** it attempts to publish an event attributed to a different agent
- **THEN** no message attributed to that other agent SHALL be published

### Requirement: Supervisor authority verbs are excluded from the MCP write surface

The system SHALL NOT expose `agent.verified` or `agent.feedback` to a worktree MCP session: neither tool SHALL be advertised there, and a by-name call SHALL return an MCP-level error. A root session SHALL have `publish_verified` and `publish_feedback`, which attach the session's authority credential (read from the user-data `Session`) as the `X-Paw-Authority` header, so the broker accepts them; the tools SHALL NOT accept a credential parameter. A coding agent therefore cannot self-verify through any MCP surface it can reach.

#### Scenario: No verified tool in a worktree session

- **WHEN** the tool registry is inspected from a worktree session
- **THEN** it SHALL NOT contain a tool that publishes an `agent.verified` message

#### Scenario: No feedback tool in a worktree session

- **WHEN** the tool registry is inspected from a worktree session
- **THEN** it SHALL NOT contain a tool that publishes an `agent.feedback` message

#### Scenario: Agent cannot self-verify via MCP

- **GIVEN** a coding agent with access to its worktree MCP session
- **WHEN** it attempts to mark its own work verified through any advertised tool, or calls `publish_verified` by name
- **THEN** no `agent.verified` message SHALL be published on its behalf

#### Scenario: Root session verdict is accepted by the broker

- **GIVEN** a root MCP session and a running broker with a session credential
- **WHEN** the session calls `publish_verified` for `feat-x`
- **THEN** the broker SHALL respond 202 and broadcast the verdict

#### Scenario: Credential is never a tool parameter

- **WHEN** the `publish_verified` and `publish_feedback` input schemas are inspected
- **THEN** neither SHALL contain a credential or header parameter
