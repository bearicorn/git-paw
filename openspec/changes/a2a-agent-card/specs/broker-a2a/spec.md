## ADDED Requirements

### Requirement: Agent Card is served at the well-known URI

While the A2A face is served, the broker SHALL answer `GET /.well-known/agent-card.json` with the session's Agent Card, as JSON with `Content-Type: application/json`. The route SHALL NOT require authentication. While the A2A face is not served, the route SHALL answer `404`.

#### Scenario: Card served without credentials

- **GIVEN** `[broker.a2a] enabled = true` on the default bind
- **WHEN** `GET /.well-known/agent-card.json` is sent with no `Authorization` header
- **THEN** the response status SHALL be `200` with `Content-Type: application/json`
- **AND** the body SHALL parse as an A2A `AgentCard`

#### Scenario: Card absent while A2A is not served

- **GIVEN** `[broker.a2a]` absent
- **WHEN** `GET /.well-known/agent-card.json` is sent
- **THEN** the response status SHALL be `404`

#### Scenario: Legacy card path is not served

- **GIVEN** `[broker.a2a] enabled = true`
- **WHEN** `GET /.well-known/agent.json` is sent
- **THEN** the response status SHALL be `404`

### Requirement: Agent Card describes the session broker

The Agent Card SHALL carry every field the A2A 1.0 proto marks required. It SHALL describe the broker as follows:

- `name` SHALL be `git-paw session <session name>`.
- `version` SHALL be the running git-paw version.
- `supportedInterfaces` SHALL hold exactly one entry, with `url` equal to the broker URL followed by `/a2a`, `protocolBinding` equal to `"JSONRPC"`, and `protocolVersion` equal to `"1.0"`.
- `capabilities` SHALL set `streaming` to `true` only while streaming is served, and SHALL set `pushNotifications` and `extendedAgentCard` to `false`.
- `capabilities.extensions` SHALL list the coordination extension URI `https://bearicorn.github.io/git-paw/a2a/coordination/v1`, with `required` set to `false`.
- `securitySchemes` SHALL declare one HTTP authentication scheme with `scheme` equal to `"Bearer"`, and `securityRequirements` SHALL require it.
- `defaultInputModes` and `defaultOutputModes` SHALL each be `["text/plain", "application/json"]`.

The card SHALL NOT contain the session credential, any filesystem path, or any agent id.

#### Scenario: Interface and capabilities match the served face

- **GIVEN** a session `paw-demo` whose broker URL is `http://127.0.0.1:9119`, with streaming not served
- **WHEN** the card is fetched
- **THEN** `supportedInterfaces` SHALL equal `[{"url":"http://127.0.0.1:9119/a2a","protocolBinding":"JSONRPC","protocolVersion":"1.0"}]`
- **AND** `name` SHALL equal `git-paw session paw-demo`
- **AND** `capabilities.streaming`, `capabilities.pushNotifications` and `capabilities.extendedAgentCard` SHALL all be `false`
- **AND** `capabilities.extensions` SHALL contain the coordination extension URI with `required` set to `false`

#### Scenario: Bearer scheme is declared and required

- **WHEN** the card is fetched
- **THEN** `securitySchemes` SHALL contain one entry whose `httpAuthSecurityScheme.scheme` is `"Bearer"`
- **AND** `securityRequirements` SHALL reference that entry

#### Scenario: Card carries no secret, path or agent id

- **GIVEN** a running session with credential `C` and worker `feat-x` in a worktree at path `P`
- **WHEN** the card is fetched
- **THEN** the body SHALL contain none of `C`, `P` or `feat-x`

### Requirement: Agent Card skills describe session capabilities, not agents

The card SHALL declare exactly three skills: `observe-workers`, `steer-worker` and `record-verdict`. Each SHALL have a non-empty name and description, and at least one tag. The description of `record-verdict` SHALL state that it requires the coordination extension. Adding or removing workers SHALL NOT change the card.

#### Scenario: Exactly three capability skills

- **WHEN** the card is fetched
- **THEN** the `skills` ids SHALL be exactly `observe-workers`, `steer-worker` and `record-verdict`
- **AND** each skill SHALL have at least one tag

#### Scenario: Card is unchanged when a worker is added

- **GIVEN** the card fetched with ETag `E` while the session has one worker
- **WHEN** a second worker is registered and the card is fetched again
- **THEN** the body SHALL be identical, and the ETag SHALL still be `E`

### Requirement: Agent Card responses are cacheable

The card response SHALL carry `Cache-Control: max-age=60` and an `ETag` derived from the card content. A request whose `If-None-Match` equals the current ETag SHALL receive `304` with an empty body.

#### Scenario: Conditional request returns 304

- **GIVEN** a card fetched with ETag `E`
- **WHEN** the card is requested again with `If-None-Match: E`
- **THEN** the response status SHALL be `304` with an empty body

#### Scenario: Cache headers present

- **WHEN** the card is fetched
- **THEN** the response SHALL carry `Cache-Control: max-age=60` and a non-empty `ETag`
