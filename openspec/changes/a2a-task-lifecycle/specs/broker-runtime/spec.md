## MODIFIED Requirements

### Requirement: Unknown routes return 404

The system SHALL respond with HTTP `404 Not Found` for any request that passes the listener guard and whose path does not match a documented route. The documented routes SHALL be the five legacy routes: `POST /publish`, `POST /watch`, `GET /messages/:agent_id`, `GET /status` and `GET /log`. While the A2A face is served, the documented routes SHALL also include the routes defined by the `broker-a2a` capability. While the A2A face is not served, those routes SHALL answer `404` like any other unknown path.

The listener guard runs before routing, so a request it refuses receives `403` rather than `404` even when its path matches no route.

#### Scenario: Unknown path returns 404

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET /unknown/route` is sent with `Host: 127.0.0.1:9119`
- **THEN** the response status is `404`

#### Scenario: Unknown path with a refused Host returns 403, not 404

- **GIVEN** a broker bound to `127.0.0.1`
- **WHEN** `GET /unknown/route` is sent with `Host: attacker.example:9119`
- **THEN** the response status is `403`

#### Scenario: Live watch registration route is documented

- **WHEN** `POST /watch` is sent with a valid body
- **THEN** the response status is not `404`

#### Scenario: A2A route is unknown while the face is not served

- **GIVEN** a broker with `[broker.a2a]` absent
- **WHEN** `POST /a2a` is sent
- **THEN** the response status is `404`
