## MODIFIED Requirements

### Requirement: Approval level coarse switch

The system SHALL accept a coarse `approval_level` field that maps to common policy presets.

#### Scenario: `safe` preset

- **GIVEN** `approval_level = "safe"`
- **WHEN** the config is loaded
- **THEN** the effective whitelist SHALL be the **composed** built-in defaults — the stack-neutral built-ins plus the resolved dev-allowlist patterns (per `safe-command-classification`) — with no user-supplied `safe_commands` extras
- **AND** `enabled` SHALL be `true`

#### Scenario: `conservative` preset

- **GIVEN** `approval_level = "conservative"`
- **WHEN** the config is loaded
- **THEN** the effective whitelist SHALL exclude `git push` and `curl` entries
- **AND** `enabled` SHALL be `true`

#### Scenario: `off` preset

- **GIVEN** `approval_level = "off"`
- **WHEN** the config is loaded
- **THEN** `enabled` SHALL be forced to `false` regardless of other fields
