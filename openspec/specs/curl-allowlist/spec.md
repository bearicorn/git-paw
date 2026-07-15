# curl-allowlist Specification

## Purpose
Seeds a least-privilege, path-based command allowlist into the agent CLI's settings at session startup so agents can invoke the bundled broker helper without a permission prompt, and keeps that allowlist current as broker URLs or endpoints change.
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

### Requirement: Helper allowlist seeded per agent worktree

Under the same gating that governs the repo-root helper allowlist (broker enabled for the broker/sweep helper prefixes; docs base URL configured for the docs-fetch prefix), the system SHALL merge the helper-path allowlist into `<worktree>/.claude/settings.json` for every agent worktree at start, add, and session recovery — the same events that provision the helper scripts themselves. Merge semantics match the repo-root target; failures are non-fatal warnings.

#### Scenario: Worktree carries the helper grants next to the helper scripts

- **GIVEN** a broker-enabled session
- **WHEN** an agent worktree is attached
- **THEN** its `.claude/settings.json` `allowed_bash_prefixes` SHALL include the `.git-paw/scripts/broker.sh` path-scoped prefix
- **AND** the worktree SHALL also contain the provisioned helper scripts (per `agent-broker-helper`)

#### Scenario: Broker disabled seeds no broker prefix

- **GIVEN** a session with `[broker] enabled = false`
- **WHEN** an agent worktree is attached
- **THEN** the worktree settings SHALL NOT gain the broker helper prefix from this seeder

