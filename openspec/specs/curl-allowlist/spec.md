# curl-allowlist Specification

## Purpose
TBD - created by archiving change auto-approve-patterns. Update Purpose after archive.
## Requirements
### Requirement: Curl allowlist setup

The system SHALL automatically create and configure a curl command allowlist during session startup to prevent permission prompts for broker communication.

#### Scenario: Allowlist created on session start

- **GIVEN** supervisor mode session with broker enabled
- **WHEN** `cmd_supervisor()` starts the session
- **THEN** a curl allowlist SHALL be created
- **AND** it SHALL include common broker endpoints

#### Scenario: Allowlist contains broker URLs

- **GIVEN** broker URL `http://127.0.0.1:9119`
- **WHEN** allowlist is created
- **THEN** it SHALL contain entries for:
  - `curl -s http://127.0.0.1:9119/publish`
  - `curl -s http://127.0.0.1:9119/status`
  - `curl -s http://127.0.0.1:9119/poll`

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

