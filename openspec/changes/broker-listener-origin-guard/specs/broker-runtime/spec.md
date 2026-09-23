## ADDED Requirements

### Requirement: Broker listener validates the Host header on a loopback bind

When the broker is bound to a loopback address, it SHALL check the host part of every request's `Host` header before routing the request. The accepted host parts SHALL be:

- `localhost`, compared ASCII case-insensitively;
- an IPv4 literal in `127.0.0.0/8`;
- `[::1]`.

The port part SHALL NOT be checked.

A request whose `Host` is missing, or whose host part is not accepted, SHALL receive HTTP `403` with the body `{"error":"forbidden host"}`. The request SHALL NOT be routed and SHALL NOT change any broker state. The check SHALL run before routing, so it covers every route and the `404`/`405` fallbacks alike; a request that passes it SHALL be routed exactly as before.

When the broker is bound to a non-loopback address, it SHALL NOT perform this check. In that case the process that starts the broker SHALL print a warning on stderr when the broker starts. The warning SHALL name the bind address and SHALL state that the broker is reachable from the network without authentication.

#### Scenario: Loopback Host is served

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET /status` is sent with `Host: 127.0.0.1:9119`
- **THEN** the response status SHALL be `200`

#### Scenario: Tunnelled loopback Host on another port is served

- **GIVEN** a broker bound to `127.0.0.1` on port 9119
- **WHEN** `GET /status` is sent with `Host: localhost:19119`
- **THEN** the response status SHALL be `200`

#### Scenario: Rebinding Host is refused without state change

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `POST /publish` with a valid `agent.answer` targeting `feat-x` is sent with `Host: attacker.example:9119`
- **THEN** the response status SHALL be `403` with the body `{"error":"forbidden host"}`
- **AND** `GET /messages/feat-x` SHALL return no `agent.answer`

#### Scenario: Non-loopback bind skips the Host check and warns

- **GIVEN** `[broker] bind = "0.0.0.0"`
- **WHEN** the broker starts
- **THEN** stderr SHALL contain a warning naming `0.0.0.0` and stating that the broker is reachable from the network without authentication
- **AND** `GET /status` sent with `Host: 192.168.1.5:9119` SHALL return `200`

### Requirement: Broker listener validates the Origin header

The broker SHALL check every request that carries an `Origin` header before routing it. This SHALL apply whatever the bind address. The origin's host SHALL be one that the Host check accepts. A request whose origin's host is not accepted SHALL receive HTTP `403` with the body `{"error":"forbidden origin"}`, as SHALL a request whose `Origin` is the serialized value `null`. Such a request SHALL NOT be routed and SHALL NOT change any broker state. A request without an `Origin` header SHALL NOT be affected by this check.

Every in-tree broker client SHALL remain accepted by both checks on the default bind: the bundled `broker.sh` and `sweep.sh` helpers, the bundled post-commit git hook, and the Rust HTTP clients (the publish, watch-registration, log and status clients, and the stale-broker probe) that the CLI, the MCP server and the dashboard use.

#### Scenario: Foreign origin is refused even with a loopback Host

- **WHEN** `POST /publish` is sent with `Host: 127.0.0.1:9119` and `Origin: http://attacker.example`
- **THEN** the response status SHALL be `403` with the body `{"error":"forbidden origin"}`

#### Scenario: Null origin is refused

- **WHEN** `GET /log` is sent with `Origin: null`
- **THEN** the response status SHALL be `403`

#### Scenario: Loopback origin is served

- **WHEN** `GET /status` is sent with `Host: 127.0.0.1:9119` and `Origin: http://localhost:3000`
- **THEN** the response status SHALL be `200`

#### Scenario: Bundled shell clients are unaffected

- **GIVEN** a running broker on the default bind
- **WHEN** `broker.sh status "x"`, `broker.sh poll`, `sweep.sh status`, `sweep.sh status-publish "x"` and the bundled post-commit hook each talk to it
- **THEN** every publish SHALL be accepted with `202` and every read SHALL return `200`

#### Scenario: In-tree Rust clients are unaffected

- **GIVEN** a running broker on the default bind
- **WHEN** `publish_to_broker_http`, `register_watch_target_http`, `fetch_log_entries_over_http`, `fetch_log_over_http`, `fetch_status_agents_over_http`, `fetch_status_over_http` and the stale-broker probe each talk to it
- **THEN** each SHALL succeed as it does without the guard
- **AND** the probe SHALL still recognise the listener as a live git-paw broker
