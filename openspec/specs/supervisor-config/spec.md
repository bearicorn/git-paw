# supervisor-config Specification

## Purpose
TBD - created by archiving change supervisor-config. Update Purpose after archive.
## Requirements
### Requirement: SupervisorConfig struct with defaults

The system SHALL define a `SupervisorConfig` struct with the following fields:

- `enabled: bool` — defaults to `false` when the field or section is absent
- `cli: Option<String>` — defaults to `None` when absent (resolved at runtime)
- `test_command: Option<String>` — defaults to `None` when absent
- `agent_approval: ApprovalLevel` — defaults to `Auto` when absent

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

#### Scenario: Invalid agent_approval value is rejected

- **GIVEN** a config file with `[supervisor]` containing `agent_approval = "yolo"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error mentioning the invalid value

#### Scenario: SupervisorConfig round-trips through save and load

- **GIVEN** a `SupervisorConfig` with all fields populated
- **WHEN** saved to TOML and loaded back
- **THEN** all fields SHALL match the original

### Requirement: Supervisor enabled resolution chain

The system SHALL determine whether supervisor mode is active using this resolution chain:

1. `--supervisor` CLI flag → enables for this session (no prompt)
2. `[supervisor] enabled = true` in config → enables by default (no prompt)
3. `[supervisor] enabled = false` in config → disabled (no prompt, explicit opt-out)
4. No `[supervisor]` section at all → prompt the user: "Start in supervisor mode? (y/n)"

The distinction between `None` (unconfigured) and `Some(enabled = false)` (explicitly disabled) SHALL be preserved.

#### Scenario: CLI flag enables supervisor regardless of config

- **GIVEN** a config with `[supervisor] enabled = false`
- **WHEN** `--supervisor` flag is passed
- **THEN** supervisor mode SHALL be active without prompting

#### Scenario: Config enables supervisor without flag

- **GIVEN** a config with `[supervisor] enabled = true`
- **WHEN** no `--supervisor` flag is passed
- **THEN** supervisor mode SHALL be active without prompting

#### Scenario: Config explicitly disables supervisor without prompt

- **GIVEN** a config with `[supervisor] enabled = false`
- **WHEN** no `--supervisor` flag is passed
- **THEN** supervisor mode SHALL NOT be active
- **AND** the user SHALL NOT be prompted

#### Scenario: No supervisor section prompts the user

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** no `--supervisor` flag is passed
- **THEN** the user SHALL be prompted "Start in supervisor mode? (y/n)"

#### Scenario: Dry run skips the prompt

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** `--dry-run` is passed without `--supervisor`
- **THEN** the dry run SHALL assume supervisor mode is off (no prompt)

### Requirement: Init prompts for supervisor configuration

During `git paw init`, the system SHALL prompt the user to configure supervisor mode:

1. "Enable supervisor mode by default? (y/n)"
2. If yes: "Test command to run after each agent completes (e.g. 'just check', leave empty to skip):"

The answers SHALL be written to the generated `.git-paw/config.toml` as a `[supervisor]` section. If the user answers no, `[supervisor] enabled = false` SHALL be written (explicit opt-out, preventing future prompts during `start`).

#### Scenario: Init with supervisor enabled

- **WHEN** the user answers "yes" to supervisor mode and enters "just check" as test command
- **THEN** the generated config SHALL contain `[supervisor]` with `enabled = true` and `test_command = "just check"`

#### Scenario: Init with supervisor disabled

- **WHEN** the user answers "no" to supervisor mode
- **THEN** the generated config SHALL contain `[supervisor]` with `enabled = false`
- **AND** future `git paw start` calls SHALL NOT prompt about supervisor mode

### Requirement: Permission flag mapping

The system SHALL provide a function `pub fn approval_flags(cli: &str, level: &ApprovalLevel) -> &'static str` that maps a CLI name and approval level to the CLI-specific permission flags to append to the launch command.

The following mappings SHALL be supported:

| CLI | Level | Flags |
|---|---|---|
| `"claude"` | `FullAuto` | `"--dangerously-skip-permissions"` |
| `"claude"` | `Auto` | `""` |
| `"claude"` | `Manual` | `""` |
| `"codex"` | `FullAuto` | `"--approval-mode=full-auto"` |
| `"codex"` | `Auto` | `"--approval-mode=auto-edit"` |
| `"codex"` | `Manual` | `""` |
| any other | any | `""` |

#### Scenario: Claude with full-auto returns skip-permissions flag

- **WHEN** `approval_flags("claude", &ApprovalLevel::FullAuto)` is called
- **THEN** the result is `"--dangerously-skip-permissions"`

#### Scenario: Codex with auto returns auto-edit flag

- **WHEN** `approval_flags("codex", &ApprovalLevel::Auto)` is called
- **THEN** the result is `"--approval-mode=auto-edit"`

#### Scenario: Unknown CLI returns empty string

- **WHEN** `approval_flags("some-agent", &ApprovalLevel::FullAuto)` is called
- **THEN** the result is `""`

#### Scenario: Any CLI with manual returns empty string

- **WHEN** `approval_flags("claude", &ApprovalLevel::Manual)` is called
- **THEN** the result is `""`

#### Scenario: Flag mapping is deterministic

- **WHEN** `approval_flags("claude", &ApprovalLevel::FullAuto)` is called twice
- **THEN** both calls return the same value

