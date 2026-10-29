## ADDED Requirements

### Requirement: Session status reports the session mode in coordinated terms

The `get_session_status` result SHALL carry `session_mode`, equal to `"coordinated"` for a coordinated session and `"bare"` otherwise. Until v1.0.0 it SHALL also carry the deprecated `mode` field with its existing values (`"supervisor"` or `"bare"`), which SHALL be recorded in the deprecation registry as an output field replaced by `session_mode`. The tool's JSON Schema SHALL describe both fields and mark `mode` deprecated.

#### Scenario: A coordinated session reports both fields

- **GIVEN** an active coordinated session
- **WHEN** the MCP client calls `get_session_status()`
- **THEN** the result SHALL contain `"session_mode": "coordinated"` and `"mode": "supervisor"`

#### Scenario: A bare session reports bare in both fields

- **GIVEN** an active bare session
- **WHEN** the MCP client calls `get_session_status()`
- **THEN** the result SHALL contain `"session_mode": "bare"` and `"mode": "bare"`

#### Scenario: The schema marks mode deprecated

- **WHEN** the tool list is read
- **THEN** `get_session_status`'s result schema SHALL describe `session_mode` and SHALL mark `mode` as deprecated
