# supervisor-config Specification

## Purpose
Defines the `[supervisor]` config schema — the `SupervisorConfig` struct, its `ApprovalLevel` and nested sub-tables (learnings, conflict, common dev allowlist, gate-command templates), the enabled-mode resolution chain, the `approval_flags` CLI-permission mapping, and the `git paw init` prompts and commented-block that generate it. It also defines the `[supervisor.auto_approve]` config section that controls the supervisor's auto-approval behaviour: an enable flag, an extensible safe-command list appended to composed built-in defaults, a clamped stall threshold, and a coarse `approval_level` preset (`safe`/`conservative`/`off`), with existing configs lacking the section parsing unchanged under sensible defaults.
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

1. **Per-CLI config override**: when `[clis.<name>]` defines an `approval_args` map for the requested level (keys are the kebab-case level names), its value SHALL be used verbatim. This is the seam for custom or variant CLIs (e.g. a claude-oss entry launched via `CLAUDE_CONFIG_DIR`, or a retained `gemini` re-added as `[clis.gemini]`) to get native flags without a built-in table row.
2. **Built-in table**: the following mappings SHALL be supported:

| CLI | Level | Flags |
|---|---|---|
| `"claude"` | `FullAuto` | `"--dangerously-skip-permissions"` |
| `"claude"` | `Auto` | `"--permission-mode acceptEdits"` |
| `"claude"` | `Manual` | `""` |
| `"codex"` | `FullAuto` | `"--dangerously-bypass-approvals-and-sandbox"` |
| `"codex"` | `Auto` | `"--sandbox workspace-write"` |
| `"codex"` | `Manual` | `""` |
| `"agy"` | `FullAuto` | `"--dangerously-skip-permissions"` |
| `"qwen"` | `FullAuto` | `"--yolo"` |
| any other | any | `""` |

3. **Fallback**: any CLI/level pair not covered above resolves to `""` (no flags).

The `Auto` level SHALL resolve to a deterministic per-CLI flag rather than the empty string wherever the CLI provides a low-friction "accept edits" mode distinct from `Manual` and from full bypass, so an `Auto` pane does not silently inherit whatever default the CLI's config directory happens to carry. For Claude this is `--permission-mode acceptEdits`: it auto-accepts file edits, and every other command prompt is left to the CLI and, under the drive loop, escalated for a human decision. Codex's `Auto` already resolves to `--sandbox workspace-write`. A CLI with no such mode keeps `""` and relies on the per-CLI `approval_args` override; its prompts reach the human.

The resolved approval flags are git-paw's only influence on whether an agent's command runs without a prompt. git-paw SHALL NOT approve a prompt the resolved mode leaves in place.

The built-in rows SHALL be verified against each CLI's upstream documentation at implementation time; a row whose upstream flag has changed SHALL be corrected via spec amendment before the change lands. In particular, the Claude `Auto` value SHALL be confirmed against the live Claude Code `--permission-mode` enumeration (`default` / `acceptEdits` / `plan` / `bypassPermissions`) so that `Auto` maps to a real, deterministic mode rather than an unrecognised token.

> `agy` is the Antigravity CLI, which replaces the retired Gemini CLI. Antigravity's
> full-auto / no-confirmation mode uses `--dangerously-skip-permissions` (the same flag
> as Claude, launched at startup), NOT the Gemini `--yolo` flag — so the former `gemini`
> row is removed and `agy` shares Claude's flag. `qwen` retains `--yolo`. Confirm the
> `agy` full-auto flag against the official Antigravity migration guide at implementation
> time; a retained Gemini install re-added via `[clis.gemini]` can still map `full-auto`
> to `--yolo` through the per-CLI `approval_args` override (path 1).

#### Scenario: Claude with full-auto returns skip-permissions flag

- **WHEN** flags are resolved for `("claude", FullAuto)` with no config override
- **THEN** the result is `"--dangerously-skip-permissions"`

#### Scenario: Claude with auto returns an explicit accept-edits permission mode

- **WHEN** flags are resolved for `("claude", Auto)` with no config override
- **THEN** the result is `"--permission-mode acceptEdits"` (a deterministic mode, not the empty string)

#### Scenario: Codex with auto returns workspace-write sandbox flag

- **WHEN** flags are resolved for `("codex", Auto)` with no config override
- **THEN** the result is `"--sandbox workspace-write"`

#### Scenario: Antigravity with full-auto returns skip-permissions flag

- **WHEN** flags are resolved for `("agy", FullAuto)` with no config override
- **THEN** the result is `"--dangerously-skip-permissions"`

#### Scenario: Qwen with full-auto returns yolo

- **WHEN** flags are resolved for `("qwen", FullAuto)` with no config override
- **THEN** the result is `"--yolo"`

#### Scenario: Retired Gemini has no built-in row

- **WHEN** flags are resolved for `("gemini", FullAuto)` with no config override
- **THEN** the result is `""` (the built-in `gemini` row was removed; a retained install maps its flag via a `[clis.gemini] approval_args` override)

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

### Requirement: Resolved approval flags apply on every worker-launch path

Every launch path that spawns a worker CLI SHALL build that pane's command from the resolved approval flags, so `agent_approval` and any `[clis.<name>].approval_args` override take effect regardless of which command started the session. The resolution chain itself is unchanged (per "Permission flag mapping"); what this requires is that no launch path bypass it.

A path that spawns a worker with the bare CLI name SHALL be treated as a defect: the worker then inherits whatever permission mode its CLI's own configuration happens to carry, which silently contradicts the configured `agent_approval` and leaves `approval_args` inert. This applies to the supervisor launch, the bare launch, the spec-driven launch, and `git paw add` alike.

#### Scenario: Spec-driven launch without supervisor applies approval flags

- **GIVEN** a config with `agent_approval = "auto"` and a CLI whose resolved `Auto` flags are non-empty
- **WHEN** a session is launched with `--specs` and no supervisor mode
- **THEN** each worker pane's command SHALL include the resolved flags

#### Scenario: Bare launch applies approval flags

- **GIVEN** a config with `agent_approval = "auto"` and a CLI whose resolved `Auto` flags are non-empty
- **WHEN** a session is launched with no spec selection and no supervisor mode
- **THEN** each worker pane's command SHALL include the resolved flags

#### Scenario: A per-CLI override reaches a non-supervisor launch

- **GIVEN** a config defining `[clis.<name>].approval_args` for the configured approval level
- **WHEN** a session is launched without supervisor mode
- **THEN** the override's value SHALL appear verbatim in the worker pane's command

#### Scenario: Empty resolved flags launch the bare command

- **GIVEN** a CLI and approval level whose resolution yields an empty flag string
- **WHEN** any launch path spawns that worker
- **THEN** the pane command SHALL be the CLI alone, with no trailing separator

### Requirement: Deprecated auto-approval tables are parsed but have no effect

The system SHALL continue to accept the `[supervisor.auto_approve]` table (`enabled`, `safe_commands`, `stall_threshold_seconds`, `approval_level` with the values `off` / `conservative` / `safe`, and `approve_worktree_writes`) and the `[supervisor.common_dev_allowlist]` table (`enabled`, `stacks`, `extra`) with their existing field names and types, so configurations written for earlier versions load without error. Neither table nor any of its fields SHALL have any behavioural effect. No prompt is approved, no skill text is rendered and no threshold is applied because of them, and a value that previously triggered a clamp warning (such as `stall_threshold_seconds = 0`) SHALL NOT trigger one.

A loaded table SHALL round-trip. Saving a configuration that declared either table SHALL write it back with its values. Saving a configuration that did not declare a table SHALL NOT add it.

When a configuration file declares either table, the system SHALL print exactly one warning line to stderr for that file per git-paw process. The line SHALL name every deprecated table the file declares and state that it has no effect and can be removed. A file that declares neither table SHALL produce no such warning.

#### Scenario: An old configuration with both tables loads with one warning

- **GIVEN** a `.git-paw/config.toml` declaring `[supervisor.auto_approve]` with every field set and `[supervisor.common_dev_allowlist]` with `stacks = ["rust"]` and `extra = ["just lint"]`
- **WHEN** a git-paw subcommand that loads the repository configuration runs
- **THEN** loading SHALL succeed
- **AND** stderr SHALL contain exactly one deprecation warning line, naming both `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]`

#### Scenario: The warning names only the declared table

- **GIVEN** a configuration file that declares `[supervisor.auto_approve]` and not `[supervisor.common_dev_allowlist]`
- **WHEN** it is loaded
- **THEN** the deprecation warning SHALL name `[supervisor.auto_approve]`
- **AND** SHALL NOT name `[supervisor.common_dev_allowlist]`

#### Scenario: A configuration without the tables produces no warning

- **GIVEN** a configuration file with a `[supervisor]` table and neither deprecated table
- **WHEN** it is loaded
- **THEN** loading SHALL succeed
- **AND** no deprecation warning SHALL be printed

#### Scenario: Declared tables round-trip

- **GIVEN** a configuration declaring both tables with non-default values
- **WHEN** it is loaded and saved back (for example by `git paw add-cli`)
- **THEN** the saved file SHALL contain both tables with the same values

#### Scenario: An undeclared table is not written on save

- **GIVEN** a configuration with a `[supervisor]` table and no `[supervisor.common_dev_allowlist]` table
- **WHEN** it is loaded and saved back
- **THEN** the saved file SHALL NOT contain a `[supervisor.common_dev_allowlist]` table

#### Scenario: The tables have no effect on a run

- **GIVEN** a configuration declaring `[supervisor.auto_approve] safe_commands = ["ls"]` and `[supervisor.common_dev_allowlist] stacks = ["rust"]`
- **WHEN** an `--unattended` run's agent pane shows a live permission prompt for `ls -la`
- **THEN** the prompt SHALL be escalated
- **AND** no keystroke SHALL be sent to that pane

