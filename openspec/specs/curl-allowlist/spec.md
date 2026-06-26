# curl-allowlist Specification

## Purpose
TBD - created by archiving change auto-approve-patterns. Update Purpose after archive.
## Requirements
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

### Requirement: Allowlist file format

The system SHALL write the curl allowlist to the appropriate agent CLI configuration file with the correct format.

#### Scenario: Allowlist written to Claude settings

- **GIVEN** Claude CLI is used as supervisor
- **WHEN** allowlist is created
- **THEN** it SHALL be written to `.claude/settings.json`
- **AND** use the `allowed_bash_prefixes` format

#### Scenario: Allowlist format is valid JSON

- **WHEN** allowlist file is created
- **THEN** it SHALL be valid JSON
- **AND** contain an `allowed_bash_prefixes` array

### Requirement: Allowlist prevents permission prompts

The curl allowlist SHALL effectively prevent permission prompts for whitelisted commands.

#### Scenario: No permission prompt for allowlisted curl

- **GIVEN** curl command in allowlist
- **WHEN** agent executes the command
- **THEN** no permission prompt SHALL appear
- **AND** command executes immediately

#### Scenario: Permission prompt for non-allowlisted commands

- **GIVEN** curl command not in allowlist
- **WHEN** agent executes the command
- **THEN** permission prompt SHALL appear normally

### Requirement: Allowlist updates

The system SHALL support updating the curl allowlist when broker URL changes or new endpoints are added.

#### Scenario: Allowlist updated on broker URL change

- **GIVEN** session with broker URL change
- **WHEN** allowlist is regenerated
- **THEN** it SHALL contain the new broker URL

#### Scenario: New endpoints added to allowlist

- **GIVEN** new broker endpoint `/feedback`
- **WHEN** allowlist is updated
- **THEN** it SHALL include the new endpoint

