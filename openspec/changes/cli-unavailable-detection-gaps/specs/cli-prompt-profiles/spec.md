## MODIFIED Requirements

### Requirement: The Claude Code profile is embedded as the default

The system SHALL embed a Claude Code profile compiled into the binary, and SHALL use it as the default for every field. When no profile configuration is present, resolved behaviour SHALL be identical to the behaviour before profiles existed. The embedded profile's `cli_unavailable_markers` SHALL include Claude Code's session/usage-limit, organisation spend-limit and authentication-failure phrasings, including `You've hit your session limit` and `org's monthly spend limit`. The spend-limit marker SHALL be scoped to that organisation-specific phrasing rather than the bare phrase `monthly spend limit`, which is plausible wording in an unrelated consumer application a worker might legitimately have open and would wrongly read as CLI unavailability. The embedded profile's `stream_error_markers` SHALL additionally include the mid-response connection-drop phrasings `API Error` and `Connection closed mid-response`, which denote a transient drop whose conversation context survives — a nudge resumes the worker — and SHALL NOT be placed in `cli_unavailable_markers`, which is reserved for terminal conditions requiring human intervention.

#### Scenario: Absent configuration reproduces current behaviour

- **GIVEN** an installation with no profile configuration
- **WHEN** readiness, approval detection, mid-response detection, mode detection, and prompt-shape identification run
- **THEN** each SHALL resolve the same literals it used before profiles were introduced

#### Scenario: Single-binary distribution preserved

- **WHEN** the binary is distributed without any accompanying profile file
- **THEN** the embedded Claude Code profile SHALL still resolve

#### Scenario: The embedded profile recognises the Claude Code session limit

- **GIVEN** no profile configuration and a pane capture whose tail contains `You've hit your session limit`
- **WHEN** the capture is matched against the resolved `cli_unavailable_markers`
- **THEN** it SHALL match

#### Scenario: The embedded profile recognises an organisation spend limit

- **GIVEN** no profile configuration and a pane capture whose tail contains `You've hit your org's monthly spend limit · run /usage-credits to ask your admin for a higher limit`
- **WHEN** the capture is matched against the resolved `cli_unavailable_markers`
- **THEN** it SHALL match
- **AND** it SHALL NOT match `stream_error_markers`

#### Scenario: A mid-response connection drop is a stream error, not CLI unavailability

- **GIVEN** no profile configuration and a pane capture whose tail contains `API Error: Connection closed mid-response`
- **WHEN** the capture is matched against the resolved marker sets
- **THEN** it SHALL match `stream_error_markers`
- **AND** it SHALL NOT match `cli_unavailable_markers`
- **AND** the wave SHALL NOT exit with outcome `cli-unavailable` on account of it
