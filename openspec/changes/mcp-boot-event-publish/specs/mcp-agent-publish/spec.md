## ADDED Requirements

### Requirement: MCP publish tools cover exactly the four agent boot events

The system SHALL expose MCP tools that publish the four agent runtime events — register/status, done/artifact, blocked, and question — to the broker. Each tool SHALL publish the same `BrokerMessage` shape the bundled shell helper publishes for that event, so the two paths are wire-identical. Each tool SHALL advertise a JSON Schema for its parameters and return shape.

The publish tool set SHALL be fixed. The system SHALL NOT expose a generic tool that publishes a caller-supplied message type or an arbitrary `BrokerMessage`.

#### Scenario: Four publish tools are advertised

- **WHEN** an MCP client lists the available tools
- **THEN** the registry SHALL include a publish tool for each of status, artifact, blocked, and question
- **AND** each SHALL carry a name, description, and input schema

#### Scenario: Wire shape matches the shell helper

- **WHEN** an agent publishes an event via the MCP publish tool
- **THEN** the resulting broker message SHALL have the same type and payload shape as the equivalent bundled-helper invocation

#### Scenario: No generic publish tool exists

- **WHEN** the tool registry is inspected
- **THEN** it SHALL NOT contain a tool that accepts an arbitrary message type or an arbitrary `BrokerMessage` body

### Requirement: Publish tools act as the calling agent only

Each publish tool SHALL derive the publishing `agent_id` from the resolved worktree branch of the calling session, using the established branch-to-`agent_id` conversion. A publish tool SHALL NOT accept an `agent_id` parameter from the caller. An agent SHALL NOT be able to publish a message attributed to a different agent.

#### Scenario: Agent id is derived, not supplied

- **WHEN** a publish tool's input schema is inspected
- **THEN** it SHALL NOT contain an `agent_id` parameter

#### Scenario: Published message carries the caller's own id

- **GIVEN** an MCP session resolved to a worktree on branch `feat/x`
- **WHEN** that session publishes a status event via the MCP publish tool
- **THEN** the published message's `agent_id` SHALL be the slugified form of `feat/x`

#### Scenario: Impersonation is not possible

- **GIVEN** an MCP session resolved to one worktree
- **WHEN** it attempts to publish an event attributed to a different agent
- **THEN** no message attributed to that other agent SHALL be published

### Requirement: Supervisor authority verbs are excluded from the MCP write surface

The system SHALL NOT expose `agent.verified` or `agent.feedback` as MCP publish tools. These are authority messages: `agent.verified` is what authorises a branch to be merged, and exposing it on a surface a coding agent can call would let that agent self-verify and bypass the supervisor's verification framework entirely.

#### Scenario: No verified publish tool

- **WHEN** the tool registry is inspected
- **THEN** it SHALL NOT contain a tool that publishes an `agent.verified` message

#### Scenario: No feedback publish tool

- **WHEN** the tool registry is inspected
- **THEN** it SHALL NOT contain a tool that publishes an `agent.feedback` message

#### Scenario: Agent cannot self-verify via MCP

- **GIVEN** an agent with access to the MCP server
- **WHEN** it attempts to mark its own work verified through any advertised tool
- **THEN** no `agent.verified` message SHALL be published on its behalf

### Requirement: Publish failures degrade without terminating the session

A publish tool SHALL return an MCP-level error when the broker is unreachable or rejects the message, and SHALL NOT crash the server or terminate the client session. Stdout SHALL remain reserved for protocol frames.

#### Scenario: Broker unreachable returns an error

- **GIVEN** a broker that is not running
- **WHEN** an agent invokes a publish tool
- **THEN** the tool SHALL return an MCP-level error
- **AND** the server SHALL continue running

#### Scenario: Rejected message does not corrupt the protocol stream

- **GIVEN** a message the broker rejects as malformed
- **WHEN** the publish tool handles the rejection
- **THEN** no diagnostic SHALL be written to stdout
