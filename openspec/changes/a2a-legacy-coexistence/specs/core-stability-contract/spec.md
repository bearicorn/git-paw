## ADDED Requirements

### Requirement: The legacy broker wire and helpers stay stable alongside A2A

From release 1.2.0, the major-1 contract manifest SHALL keep the following as stable entries:

- the five legacy broker endpoints: `POST /publish`, `POST /watch`, `GET /messages/:agent_id`, `GET /status` and `GET /log`;
- every broker message type;
- the install path and documented subcommands of each bundled helper script.

No such entry SHALL appear in the deprecation registry of any 1.y release unless an agent-side replacement for it is a stable entry in the same release. The stability chapter SHALL state that the legacy broker wire and the helpers are git-paw's agent-side transport, and that A2A is its external face. It SHALL state that the legacy path is not deprecated, and that any future removal follows the deprecation rule: a registry entry, at least two minor releases that warn, and removal only at a later major.

#### Scenario: Major-1 manifest keeps the legacy entries

- **GIVEN** the crate version is `1.2.0` and the manifest is generated for major 1
- **WHEN** the manifest guard test runs
- **THEN** the manifest SHALL contain the five legacy broker endpoints, every broker message type, and the subcommands of `broker.sh` and `sweep.sh`

#### Scenario: Legacy entries are not in the deprecation registry

- **WHEN** the deprecation registry of a 1.y release is read
- **THEN** it SHALL contain no broker endpoint, no broker message type and no helper subcommand

#### Scenario: Stability chapter states the two roles

- **WHEN** `docs/src/stability.md` is read
- **THEN** it SHALL describe the legacy broker wire and helpers as the agent-side transport and A2A as the external face
- **AND** it SHALL state that the legacy path is not deprecated and how a future removal would proceed

### Requirement: A2A entries are recorded in the major-1 contract

From release 1.2.0, the major-1 contract manifest SHALL record these stable entries:

- the configuration key `[broker.a2a] enabled`;
- the endpoints `POST /a2a` and `GET /.well-known/agent-card.json`;
- the declared A2A protocol version `1.0`;
- the coordination extension URI `https://bearicorn.github.io/git-paw/a2a/coordination/v1`.

The manifest guard test SHALL fail when any of these is missing from the manifest or from the live surface.

#### Scenario: A2A entries present in the manifest

- **WHEN** the manifest is generated for crate version `1.2.0`
- **THEN** it SHALL contain `[broker.a2a] enabled`, `POST /a2a`, `GET /.well-known/agent-card.json`, A2A protocol version `1.0` and the coordination extension URI

#### Scenario: Dropping the card route fails the guard

- **GIVEN** a major-1 manifest that records `GET /.well-known/agent-card.json`
- **WHEN** the route is removed from the code and the guard test runs under a `1.y` version
- **THEN** the test SHALL fail naming the route
