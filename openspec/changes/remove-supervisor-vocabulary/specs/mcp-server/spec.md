## MODIFIED Requirements

### Requirement: Session status reports the session mode in coordinated terms

The `get_session_status` result SHALL carry `session_mode`, equal to `"coordinated"` for a coordinated session and `"bare"` otherwise, and SHALL NOT carry a `mode` field. The tool's JSON Schema SHALL describe `session_mode` and SHALL NOT describe `mode`.

#### Scenario: A coordinated session reports session_mode only

- **GIVEN** an active coordinated session
- **WHEN** the MCP client calls `get_session_status()`
- **THEN** the result SHALL contain `"session_mode": "coordinated"`
- **AND** it SHALL NOT contain a `mode` key

#### Scenario: A bare session reports bare

- **GIVEN** an active bare session
- **WHEN** the MCP client calls `get_session_status()`
- **THEN** the result SHALL contain `"session_mode": "bare"`

#### Scenario: The schema has no mode field

- **WHEN** the tool list is read
- **THEN** `get_session_status`'s result schema SHALL describe `session_mode` and SHALL NOT describe `mode`
