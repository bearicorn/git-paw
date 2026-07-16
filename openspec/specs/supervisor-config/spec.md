# supervisor-config Specification

## Purpose
Defines the `[supervisor]` config schema — the `SupervisorConfig` struct, its `ApprovalLevel` and nested sub-tables (learnings, conflict, common dev allowlist, gate-command templates), the enabled-mode resolution chain, the `approval_flags` CLI-permission mapping, and the `git paw init` prompts and commented-block that generate it.
## Requirements
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

The system SHALL provide a flag-resolution function that maps a CLI name and approval level to the CLI-specific permission flags to append to the launch command. Resolution SHALL consult, in order:

1. **Per-CLI config override**: when `[clis.<name>]` defines an `approval_args` map for the requested level (keys are the kebab-case level names), its value SHALL be used verbatim. This is the seam for custom or variant CLIs (e.g. a claude-oss entry launched via `CLAUDE_CONFIG_DIR`) to get native flags without a built-in table row.
2. **Built-in table**: the following mappings SHALL be supported:

| CLI | Level | Flags |
|---|---|---|
| `"claude"` | `FullAuto` | `"--dangerously-skip-permissions"` |
| `"claude"` | `Auto` | `""` |
| `"claude"` | `Manual` | `""` |
| `"codex"` | `FullAuto` | `"--dangerously-bypass-approvals-and-sandbox"` |
| `"codex"` | `Auto` | `"--sandbox workspace-write"` |
| `"codex"` | `Manual` | `""` |
| `"gemini"` | `FullAuto` | `"--yolo"` |
| `"qwen"` | `FullAuto` | `"--yolo"` |
| any other | any | `""` |

3. **Fallback**: any CLI/level pair not covered above resolves to `""` (no flags).

The built-in rows SHALL be verified against each CLI's upstream documentation at implementation time; a row whose upstream flag has changed SHALL be corrected via spec amendment before the change lands.

> Verified against upstream docs 2026-07-15: gemini and qwen `--yolo` are current
> (gemini documents it as the shortcut for `--approval-mode=yolo`). The codex rows
> were amended here: the legacy TypeScript CLI's `--approval-mode=full-auto` /
> `--approval-mode=auto-edit` no longer exist in the current Rust CLI. Their
> current equivalents are `--dangerously-bypass-approvals-and-sandbox` (run every
> command without approvals or sandboxing) and `--sandbox workspace-write` (the
> documented low-friction sandboxed mode; upstream deprecated `--full-auto` in
> its favor and prints a warning when it is used).

#### Scenario: Claude with full-auto returns skip-permissions flag

- **WHEN** flags are resolved for `("claude", FullAuto)` with no config override
- **THEN** the result is `"--dangerously-skip-permissions"`

#### Scenario: Codex with auto returns workspace-write sandbox flag

- **WHEN** flags are resolved for `("codex", Auto)` with no config override
- **THEN** the result is `"--sandbox workspace-write"`

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

### Requirement: Learnings opt-in flag

The system SHALL extend `SupervisorConfig` with a `learnings: bool` field. The field SHALL default to `false` when the field or section is absent from `.git-paw/config.toml`.

When `[supervisor] learnings = true` is set AND supervisor mode is otherwise active for the session (per the existing supervisor-cli resolution chain, including respect for `--no-supervisor`), the broker SHALL start the learnings aggregator subsystem (per `learnings-mode`). When `learnings = false` (or absent), the aggregator SHALL NOT start.

#### Scenario: learnings defaults to false when absent

- **GIVEN** a config file with `[supervisor] enabled = true` and no `learnings` field
- **WHEN** the config is loaded
- **THEN** `supervisor.learnings` SHALL be `false`

#### Scenario: learnings = true is loaded

- **GIVEN** a config file with `[supervisor] enabled = true` and `learnings = true`
- **WHEN** the config is loaded
- **THEN** `supervisor.learnings` SHALL be `true`

#### Scenario: learnings round-trips through save and load

- **GIVEN** a `SupervisorConfig` with `learnings = true`
- **WHEN** saved to TOML and loaded back
- **THEN** the loaded value SHALL be `true`

#### Scenario: Pre-v0.5 configs load with learnings = false

- **GIVEN** a `.git-paw/config.toml` produced before v0.5.0 (no `learnings` field)
- **WHEN** the config is loaded with the v0.5.0 binary
- **THEN** loading SHALL succeed without error
- **AND** `supervisor.learnings` SHALL be `false`

### Requirement: LearningsConfig sub-table

The system SHALL extend `SupervisorConfig` with a nested `learnings_config: LearningsConfig` field (or equivalent name matching local conventions). `LearningsConfig` SHALL contain:

- `flush_interval_seconds: u64` — defaults to `60` when the field or section is absent. Used by the learnings aggregator's periodic flush timer.

The `[supervisor.learnings_config]` table (TOML key name follows local serde conventions; design suggests `[supervisor.learnings_config]` to avoid colliding with the boolean `learnings` field) SHALL be optional. A config file with `[supervisor] learnings = true` and no nested table SHALL load with `flush_interval_seconds = 60`.

#### Scenario: LearningsConfig defaults when section absent

- **GIVEN** a config file with `[supervisor] enabled = true`, `learnings = true`, and no `[supervisor.learnings_config]` section
- **WHEN** the config is loaded
- **THEN** `supervisor.learnings_config.flush_interval_seconds` SHALL be `60`

#### Scenario: Custom flush_interval_seconds is honoured

- **GIVEN** a config file with `[supervisor.learnings_config] flush_interval_seconds = 30`
- **WHEN** the config is loaded
- **THEN** the loaded `flush_interval_seconds` SHALL be `30`

#### Scenario: LearningsConfig round-trips through save and load

- **GIVEN** a `LearningsConfig { flush_interval_seconds: 90 }`
- **WHEN** saved to TOML and loaded back
- **THEN** the loaded value matches the original

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

### Requirement: Common dev allowlist sub-table

The system SHALL extend `SupervisorConfig` with a nested
`common_dev_allowlist: CommonDevAllowlistConfig` field (or equivalent
name matching local serde conventions).

`CommonDevAllowlistConfig` SHALL contain:

- `enabled: bool` — defaults to `true` when the field or section is
  absent from `.git-paw/config.toml`. Controls whether the
  dev-allowlist seeder (per `dev-command-allowlist`) runs on supervisor
  start.
- `extra: Vec<String>` — defaults to empty when the field or section
  is absent. User-supplied additional prefix patterns appended to the
  built-in preset by the seeder.

The `[supervisor.common_dev_allowlist]` TOML table SHALL be fully
optional. A config file with `[supervisor] enabled = true` and no
nested `common_dev_allowlist` table SHALL load with `enabled = true`
and `extra = []`.

The field SHALL use `#[serde(default)]` so missing fields parse to the
documented defaults rather than triggering parse errors.

#### Scenario: Defaults when sub-table is absent

- **GIVEN** a config file with `[supervisor] enabled = true` and no
  `[supervisor.common_dev_allowlist]` section
- **WHEN** the config is loaded
- **THEN** `supervisor.common_dev_allowlist.enabled` SHALL be `true`
- **AND** `supervisor.common_dev_allowlist.extra` SHALL be empty

#### Scenario: Enabled false opt-out is honoured

- **GIVEN** a config file with
  `[supervisor.common_dev_allowlist] enabled = false`
- **WHEN** the config is loaded
- **THEN** `supervisor.common_dev_allowlist.enabled` SHALL be `false`

#### Scenario: Extra patterns parsed

- **GIVEN** a config file with
  `[supervisor.common_dev_allowlist] extra = ["pnpm test", "deno fmt"]`
- **WHEN** the config is loaded
- **THEN** `supervisor.common_dev_allowlist.extra` SHALL equal
  `["pnpm test", "deno fmt"]`

#### Scenario: Sub-table round-trips through save and load

- **GIVEN** a `SupervisorConfig` with
  `common_dev_allowlist = CommonDevAllowlistConfig { enabled: false, extra: vec!["x".to_string()] }`
- **WHEN** the config is serialised to TOML and parsed back
- **THEN** the round-trip loaded value SHALL equal the original

#### Scenario: Pre-v0.5 configs load with defaults

- **GIVEN** a `.git-paw/config.toml` produced before v0.5.0
  (no `[supervisor.common_dev_allowlist]` table)
- **WHEN** the config is loaded with the v0.5.0 binary
- **THEN** loading SHALL succeed without error
- **AND** `supervisor.common_dev_allowlist.enabled` SHALL be `true`
- **AND** `supervisor.common_dev_allowlist.extra` SHALL be empty

### Requirement: SupervisorConfig SHALL carry six gate-command template fields

`SupervisorConfig` in `src/config.rs` SHALL expose the following six optional fields beyond the existing `test_command`:

- `lint_command: Option<String>` — pre-stage lint invocation.
- `build_command: Option<String>` — compile step when distinct from test.
- `doc_build_command: Option<String>` — documentation build (gate 4 input).
- `spec_validate_command: Option<String>` — spec validator (gate 3 input). MAY contain a `{{CHANGE_ID}}` placeholder that the supervisor agent expands at verification time with the change name.
- `fmt_check_command: Option<String>` — formatter check.
- `security_audit_command: Option<String>` — security audit tooling (gate 5 input).

Each field SHALL be annotated with `#[serde(default, skip_serializing_if = "Option::is_none")]` so older TOMLs without the fields deserialise as `None` and configs that set the fields to `None` omit them on round-trip.

#### Scenario: Fields default to None when absent from TOML

- **GIVEN** a `.git-paw/config.toml` containing `[supervisor]\nenabled = true\ncli = "claude"\n` (no gate-command keys)
- **WHEN** `load_config` reads the file
- **THEN** the resulting `SupervisorConfig` has `lint_command = None`, `build_command = None`, `doc_build_command = None`, `spec_validate_command = None`, `fmt_check_command = None`, `security_audit_command = None`
- **AND** the existing `test_command` field is also `None`

#### Scenario: Fields round-trip through serialize + deserialize

- **GIVEN** a `SupervisorConfig` with `test_command = Some("just check")`, `lint_command = Some("cargo clippy -- -D warnings")`, `build_command = None`, `doc_build_command = Some("mdbook build docs/")`, `spec_validate_command = Some("openspec validate {{CHANGE_ID}} --strict")`, `fmt_check_command = Some("cargo fmt --check")`, `security_audit_command = Some("cargo audit")`
- **WHEN** the value is serialised to TOML and deserialised back
- **THEN** the resulting struct SHALL equal the original

#### Scenario: None-valued fields omit from serialised TOML

- **GIVEN** a `SupervisorConfig` with all six gate-command fields set to `None`
- **WHEN** the value is serialised to TOML
- **THEN** the resulting TOML SHALL NOT contain the keys `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`

### Requirement: `git paw init` SHALL write a commented-out [supervisor] block enumerating gate keys

`src/init.rs::run_init` SHALL append (or include in the initial `config.toml` content it writes) a commented-out `[supervisor]` block listing the six new gate-command keys with example values illustrating common stacks. The block SHALL be entirely commented (every line prefixed with `#`) so TOML parsing ignores it.

The block SHALL include at minimum the six keys (`lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`) plus the existing `test_command`, `enabled`, and `cli` keys, each with one example value.

#### Scenario: `git paw init` writes the commented block

- **GIVEN** a fresh repository with no `.git-paw/config.toml`
- **WHEN** `git paw init` is invoked
- **THEN** `.git-paw/config.toml` SHALL exist
- **AND** the file content SHALL contain `# [supervisor]` (commented section header) on its own line
- **AND** the file content SHALL contain a commented line for each of the seven `[supervisor]` keys (`enabled`, `cli`, `test_command`, `lint_command`, `build_command`, `doc_build_command`, `spec_validate_command`, `fmt_check_command`, `security_audit_command`)

#### Scenario: The written commented block is valid TOML when uncommented

- **GIVEN** the `git paw init`-written commented block
- **WHEN** every line prefixed with `# ` has its prefix stripped (turning the block into uncommented TOML)
- **THEN** the resulting TOML SHALL parse without error
- **AND** the parsed `SupervisorConfig` SHALL have all seven listed keys populated with the example values

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

