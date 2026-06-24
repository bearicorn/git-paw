## MODIFIED Requirements

### Requirement: Curl allowlist setup

The system SHALL automatically create and configure an allowlist during
session startup to prevent permission prompts for broker communication.
The seeded grant SHALL be the single stable path of the bundled
agent-broker helper (`.git-paw/scripts/broker.sh`, the
`agent-broker-helper` capability) — a least-privilege, path-based grant.
The system SHALL NOT seed a broad `curl *` grant, and SHALL NOT depend
on per-endpoint `curl <broker-url><endpoint>` prefixes for the agent's
boot-time broker interactions.

#### Scenario: Allowlist created on session start

- **GIVEN** supervisor mode session with broker enabled
- **WHEN** `cmd_supervisor()` starts the session
- **THEN** an allowlist SHALL be created
- **AND** it SHALL grant the agent-broker helper path

#### Scenario: Allowlist grants the helper path, not broad curl

- **GIVEN** broker URL `http://127.0.0.1:9119`
- **WHEN** the allowlist is created
- **THEN** it SHALL contain a prefix authorising
  `.git-paw/scripts/broker.sh`
- **AND** it SHALL NOT contain a `curl *` (broad curl) grant

#### Scenario: Helper grant removes the boot-publish dead-stall

- **GIVEN** an agent whose first boot action publishes its register
  status via `.git-paw/scripts/broker.sh status booting`
- **WHEN** the agent runs that boot action with the helper-path grant
  seeded
- **THEN** no permission prompt SHALL appear
- **AND** the agent SHALL register with the broker without stalling
