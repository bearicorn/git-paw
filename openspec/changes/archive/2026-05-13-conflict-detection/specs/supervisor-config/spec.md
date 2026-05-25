## ADDED Requirements

### Requirement: ConflictConfig sub-table

The system SHALL extend `SupervisorConfig` with a nested `conflict: ConflictConfig` field. `ConflictConfig` SHALL contain:

- `window_seconds: u64` — defaults to `120` when the field or section is absent. Used as the in-flight-conflict escalation window.
- `warn_on_intent_overlap: bool` — defaults to `true` when the field or section is absent. When `false`, forward-conflict warnings are suppressed; the active-intent tracker SHALL still record intents.
- `escalate_on_violation: bool` — defaults to `true` when the field or section is absent. When `false`, ownership violations still emit `agent.feedback` to the violator but SHALL NOT emit a follow-up `agent.question` to the supervisor inbox.

The `[supervisor.conflict]` table SHALL be optional in `.git-paw/config.toml`. A config file with `[supervisor]` but no `[supervisor.conflict]` SHALL load with all three fields at their default values.

#### Scenario: ConflictConfig defaults when section absent

- **GIVEN** a config file with `[supervisor] enabled = true` and no `[supervisor.conflict]` section
- **WHEN** the config is loaded
- **THEN** `supervisor.conflict.window_seconds` SHALL be `120`
- **AND** `supervisor.conflict.warn_on_intent_overlap` SHALL be `true`
- **AND** `supervisor.conflict.escalate_on_violation` SHALL be `true`

#### Scenario: ConflictConfig with all fields populated

- **GIVEN** a config file with `[supervisor] enabled = true` and `[supervisor.conflict]` containing `window_seconds = 300`, `warn_on_intent_overlap = false`, `escalate_on_violation = false`
- **WHEN** the config is loaded
- **THEN** the loaded `ConflictConfig` matches the TOML values exactly

#### Scenario: ConflictConfig with partial fields

- **GIVEN** a config file with `[supervisor.conflict]` containing only `window_seconds = 60`
- **WHEN** the config is loaded
- **THEN** `window_seconds` SHALL be `60`
- **AND** `warn_on_intent_overlap` SHALL be `true`
- **AND** `escalate_on_violation` SHALL be `true`

#### Scenario: ConflictConfig round-trips through save and load

- **GIVEN** a `SupervisorConfig` with `ConflictConfig { window_seconds: 90, warn_on_intent_overlap: false, escalate_on_violation: true }`
- **WHEN** saved to TOML and loaded back
- **THEN** the loaded values match the original

#### Scenario: Pre-v0.5 configs load without error

- **GIVEN** a `.git-paw/config.toml` produced before v0.5.0 (no `[supervisor.conflict]` section)
- **WHEN** the config is loaded with the v0.5.0 binary
- **THEN** loading SHALL succeed without error
- **AND** `supervisor.conflict` SHALL contain default values
