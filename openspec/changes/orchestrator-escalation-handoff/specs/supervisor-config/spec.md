## MODIFIED Requirements

### Requirement: SupervisorConfig struct with defaults

The system SHALL define a `SupervisorConfig` struct with the following fields:

- `enabled: bool` — defaults to `false` when the field or section is absent
- `cli: Option<String>` — defaults to `None` when absent. Names the orchestrator CLI used for headless escalation invocations and for seeding the roster's `supervisor` row; not required to launch a session.
- `test_command: Option<String>` — defaults to `None` when absent
- `agent_approval: ApprovalLevel` — defaults to `Auto` when absent
- `approval: Option<ApprovalLevel>` — defaults to `None` when absent. The orchestrator's own approval level, applied to git-paw-initiated headless orchestrator invocations. When `None`, those invocations inherit `agent_approval`. The field SHALL be annotated `#[serde(default, skip_serializing_if = "Option::is_none")]` so older TOMLs parse unchanged and unset values omit on round-trip.

The `ApprovalLevel` enum SHALL have three variants: `Manual`, `Auto`, `FullAuto`, serialized as kebab-case (`"manual"`, `"auto"`, `"full-auto"`).

#### Scenario: SupervisorConfig is None when section absent

- **GIVEN** a config file with no `[supervisor]` section
- **WHEN** the config is loaded
- **THEN** `supervisor` SHALL be `None`

#### Scenario: SupervisorConfig with all fields populated

- **GIVEN** a config file with `[supervisor]` containing `enabled = true`, `cli = "claude"`, `test_command = "just check"`, `agent_approval = "full-auto"`
- **WHEN** the config is loaded
- **THEN** all fields SHALL match the TOML values

#### Scenario: SupervisorConfig with partial fields

- **GIVEN** a config file with `[supervisor]` containing only `enabled = true`
- **WHEN** the config is loaded
- **THEN** `enabled` SHALL be `true`
- **AND** `cli`, `test_command` SHALL be `None`
- **AND** `agent_approval` SHALL be `Auto`
- **AND** `approval` SHALL be `None`

#### Scenario: Invalid agent_approval value is rejected

- **GIVEN** a config file with `[supervisor]` containing `agent_approval = "yolo"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error mentioning the invalid value

#### Scenario: supervisor approval field parses all three levels

- **GIVEN** a config file with `[supervisor]` containing `approval = "full-auto"`
- **WHEN** the config is loaded
- **THEN** `supervisor.approval` SHALL be `Some(ApprovalLevel::FullAuto)`
- **AND** the equivalent holds for `"manual"` and `"auto"`

#### Scenario: Invalid supervisor approval value is rejected

- **GIVEN** a config file with `[supervisor]` containing `approval = "yolo"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error mentioning the invalid value

#### Scenario: Pre-v0.11 configs load with approval = None

- **GIVEN** a `.git-paw/config.toml` produced before v0.11.0 (no `approval` key)
- **WHEN** the config is loaded
- **THEN** loading SHALL succeed without error
- **AND** `supervisor.approval` SHALL be `None`

#### Scenario: SupervisorConfig round-trips through save and load

- **GIVEN** a `SupervisorConfig` with all fields populated (including `approval = Some(FullAuto)`)
- **WHEN** saved to TOML and loaded back
- **THEN** all fields SHALL match the original

### Requirement: Supervisor-specific approval level resolution

The system SHALL resolve the orchestrator's effective approval level as `supervisor.approval` when the field is `Some(level)`, and as `supervisor.agent_approval` when it is `None`, and SHALL append the flags resolved for `([supervisor].cli, effective level)` to every headless orchestrator invocation the drive loop spawns. Coding-agent panes SHALL continue to resolve from `agent_approval` alone — setting `approval` SHALL NOT change any coding-agent pane's launch flags. When the effective level is `FullAuto` and flag resolution yields `""` (no built-in row and no `[clis.<name>].approval_args` override), the system SHALL print a warning naming the CLI and the `[clis.<name>]` override and SHALL spawn the invocation without flags. The launch SHALL NOT fail.

#### Scenario: approval set to full-auto relaxes only the headless invocation

- **GIVEN** `[supervisor]` containing `agent_approval = "auto"`, `approval = "full-auto"` and `cli = "claude"`
- **WHEN** a headless orchestrator invocation is spawned and the coding-agent pane commands are built
- **THEN** the invocation argv SHALL include `--dangerously-skip-permissions`
- **AND** every coding-agent pane command SHALL NOT include it

#### Scenario: Absent approval inherits agent_approval for the invocation

- **GIVEN** `[supervisor]` containing `agent_approval = "full-auto"`, no `approval` key and `cli = "claude"`
- **WHEN** a headless orchestrator invocation is spawned
- **THEN** its argv SHALL include `--dangerously-skip-permissions`

#### Scenario: Full-auto with an unmapped CLI warns and degrades

- **GIVEN** `[supervisor]` containing `approval = "full-auto"`, `cli = "my-agent"`, `[clis.my-agent].headless_args = "--batch"` and no `[clis.my-agent].approval_args`
- **WHEN** the drive loop starts and later spawns an invocation
- **THEN** a warning SHALL be printed naming `my-agent` and the `[clis.my-agent]` override
- **AND** the invocation argv SHALL be `my-agent --batch`
- **AND** the launch SHALL NOT fail

## ADDED Requirements

### Requirement: `[supervisor.escalation]` config section

The system SHALL accept an optional `[supervisor.escalation]` table with `enabled: bool` (default `true`) and `timeout_seconds: u64` (default `1800`), annotated `#[serde(default)]` so its absence yields the defaults and older configs load unchanged. `timeout_seconds = 0` SHALL be rejected at load with an error naming the key.

#### Scenario: Absent section yields defaults

- **GIVEN** a config with `[supervisor]` and no `[supervisor.escalation]`
- **WHEN** the config is loaded
- **THEN** `escalation.enabled` SHALL be `true` and `escalation.timeout_seconds` SHALL be `1800`

#### Scenario: Section round-trips

- **GIVEN** `[supervisor.escalation]` with `enabled = false` and `timeout_seconds = 600`
- **WHEN** the config is saved and loaded back
- **THEN** both values SHALL match

#### Scenario: Zero timeout is rejected

- **GIVEN** `[supervisor.escalation] timeout_seconds = 0`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `timeout_seconds`
