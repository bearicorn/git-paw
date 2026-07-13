## ADDED Requirements

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
