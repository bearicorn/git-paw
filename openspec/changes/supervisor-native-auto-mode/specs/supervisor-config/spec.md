## MODIFIED Requirements

### Requirement: SupervisorConfig struct with defaults

The system SHALL define a `SupervisorConfig` struct with the following fields:

- `enabled: bool` — defaults to `false` when the field or section is absent
- `cli: Option<String>` — defaults to `None` when absent (resolved at runtime)
- `test_command: Option<String>` — defaults to `None` when absent
- `agent_approval: ApprovalLevel` — defaults to `Auto` when absent
- `approval: Option<ApprovalLevel>` — defaults to `None` when absent. The SUPERVISOR pane's own approval level. When `None`, the supervisor pane inherits `agent_approval` (the pre-v0.11.0 behavior, preserved exactly). The field SHALL be annotated `#[serde(default, skip_serializing_if = "Option::is_none")]` so older TOMLs parse unchanged and unset values omit on round-trip.

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

### Requirement: Permission flag mapping

The system SHALL provide a flag-resolution function that maps a CLI name and approval level to the CLI-specific permission flags to append to the launch command. Resolution SHALL consult, in order:

1. **Per-CLI config override**: when `[clis.<name>]` defines an `approval_args` map for the requested level (keys are the kebab-case level names), its value SHALL be used verbatim. This is the seam for custom or variant CLIs (e.g. a claude-oss entry launched via `CLAUDE_CONFIG_DIR`) to get native flags without a built-in table row.
2. **Built-in table**: the following mappings SHALL be supported:

| CLI | Level | Flags |
|---|---|---|
| `"claude"` | `FullAuto` | `"--dangerously-skip-permissions"` |
| `"claude"` | `Auto` | `""` |
| `"claude"` | `Manual` | `""` |
| `"codex"` | `FullAuto` | `"--approval-mode=full-auto"` |
| `"codex"` | `Auto` | `"--approval-mode=auto-edit"` |
| `"codex"` | `Manual` | `""` |
| `"gemini"` | `FullAuto` | `"--yolo"` |
| `"qwen"` | `FullAuto` | `"--yolo"` |
| any other | any | `""` |

3. **Fallback**: any CLI/level pair not covered above resolves to `""` (no flags).

The built-in rows SHALL be verified against each CLI's upstream documentation at implementation time; a row whose upstream flag has changed SHALL be corrected via spec amendment before the change lands.

#### Scenario: Claude with full-auto returns skip-permissions flag

- **WHEN** flags are resolved for `("claude", FullAuto)` with no config override
- **THEN** the result is `"--dangerously-skip-permissions"`

#### Scenario: Codex with auto returns auto-edit flag

- **WHEN** flags are resolved for `("codex", Auto)` with no config override
- **THEN** the result is `"--approval-mode=auto-edit"`

#### Scenario: Gemini and qwen full-auto return yolo

- **WHEN** flags are resolved for `("gemini", FullAuto)` or `("qwen", FullAuto)` with no config override
- **THEN** the result is `"--yolo"`

#### Scenario: Per-CLI override takes precedence over the built-in table

- **GIVEN** a config with `[clis.claude]` defining `approval_args = { "full-auto" = "--my-custom-flag" }`
- **WHEN** flags are resolved for `("claude", FullAuto)`
- **THEN** the result is `"--my-custom-flag"` (the override wins over the built-in row)

#### Scenario: Override enables a CLI with no built-in row

- **GIVEN** a config with `[clis.claude-oss]` defining `command` and `approval_args = { "full-auto" = "--dangerously-skip-permissions" }`
- **WHEN** flags are resolved for `("claude-oss", FullAuto)`
- **THEN** the result is `"--dangerously-skip-permissions"`

#### Scenario: Unknown CLI returns empty string

- **WHEN** flags are resolved for `("some-agent", FullAuto)` with no config override
- **THEN** the result is `""`

#### Scenario: Any CLI with manual returns empty string

- **WHEN** flags are resolved for `("claude", Manual)` with no config override
- **THEN** the result is `""`

#### Scenario: Flag mapping is deterministic

- **WHEN** flags are resolved twice for the same `(cli, level, config)` triple
- **THEN** both calls return the same value

## ADDED Requirements

### Requirement: Supervisor-specific approval level resolution

The system SHALL resolve the SUPERVISOR pane's effective approval level as `supervisor.approval` when the field is `Some(level)`, and as `supervisor.agent_approval` when the field is `None`. Coding-agent panes SHALL continue to resolve from `agent_approval` alone — setting `approval` SHALL NOT change any coding-agent pane's launch flags.

When the supervisor's effective level is `FullAuto` and flag resolution yields `""` (no built-in row and no `[clis.<name>].approval_args` override), the system SHALL print a warning naming the CLI and pointing at the `[clis.<name>]` override, and SHALL launch the supervisor pane without flags (behaving as `auto`). The launch SHALL NOT fail.

#### Scenario: approval set to full-auto relaxes only the supervisor pane

- **GIVEN** a config with `[supervisor]` containing `agent_approval = "auto"` and `approval = "full-auto"` and `cli = "claude"`
- **WHEN** the supervisor session launch commands are built
- **THEN** the supervisor pane command SHALL include `--dangerously-skip-permissions`
- **AND** every coding-agent pane command SHALL NOT include it

#### Scenario: Absent approval inherits agent_approval for the supervisor pane

- **GIVEN** a config with `[supervisor]` containing `agent_approval = "full-auto"` and no `approval` key and `cli = "claude"`
- **WHEN** the supervisor session launch commands are built
- **THEN** the supervisor pane command SHALL include `--dangerously-skip-permissions` (identical to pre-v0.11.0 behavior)

#### Scenario: Full-auto with an unmapped CLI warns and degrades

- **GIVEN** a config with `[supervisor]` containing `approval = "full-auto"` and `cli = "my-agent"` and no `[clis.my-agent].approval_args`
- **WHEN** the supervisor session is launched
- **THEN** a warning SHALL be printed naming `my-agent` and the `[clis.my-agent]` override
- **AND** the supervisor pane SHALL launch with no approval flags
- **AND** the launch SHALL NOT fail

### Requirement: CustomCli approval_args override field

The `CustomCli` struct (`[clis.<name>]`) SHALL gain an optional `approval_args` map from kebab-case approval-level names (`"manual"`, `"auto"`, `"full-auto"`) to flag strings. The field SHALL default to absent (`#[serde(default)]`, omitted on round-trip when empty), so existing configs parse unchanged. Unknown level keys SHALL be rejected at config load with an error naming the invalid key.

#### Scenario: approval_args parses and round-trips

- **GIVEN** a config with `[clis.mycli]` containing `command = "mycli"` and `approval_args = { "full-auto" = "--yolo" }`
- **WHEN** the config is loaded and saved back
- **THEN** the loaded map SHALL contain `"full-auto" → "--yolo"` and the round-trip SHALL preserve it

#### Scenario: Existing CustomCli entries parse unchanged

- **GIVEN** a pre-v0.11.0 config with `[clis.mycli]` containing only `command = "mycli"`
- **WHEN** the config is loaded
- **THEN** loading SHALL succeed and `approval_args` SHALL be empty/absent

#### Scenario: Invalid level key is rejected

- **GIVEN** a config with `[clis.mycli]` containing `approval_args = { "yolo-mode" = "--x" }`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error mentioning `yolo-mode`
