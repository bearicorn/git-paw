## MODIFIED Requirements

### Requirement: SupervisorConfig struct with defaults

The system SHALL define a `SupervisorConfig` struct with the following fields:

- `enabled: bool` — defaults to `false` when the field or section is absent
- `cli: Option<String>` — defaults to `None` when absent. Names the orchestrator CLI used for headless escalation invocations and for seeding the roster's `supervisor` row; not required to launch a session.
- `test_command: Option<String>` — defaults to `None` when absent
- `agent_approval: ApprovalLevel` — defaults to `Auto` when absent, whatever the configuration's version and when no configuration file exists. `Auto` launches the CLI's own auto mode (*Permission flag mapping*). An explicit version-1 value `"auto"` is a written value, not an absent one, and loads as `AcceptEdits` per `core-configuration`'s *Version-1 `auto` approval values load as `accept-edits`*.
- `approval: Option<ApprovalLevel>` — defaults to `None` when absent. The orchestrator's own approval level, applied to git-paw-initiated headless orchestrator invocations. When `None`, those invocations inherit `agent_approval`. The field SHALL be annotated `#[serde(default, skip_serializing_if = "Option::is_none")]` so older TOMLs parse unchanged and unset values omit on round-trip.

The `ApprovalLevel` enum SHALL have four variants, ordered from narrowest to widest: `Manual`, `AcceptEdits`, `Auto`, `FullAuto`, serialized as kebab-case (`"manual"`, `"accept-edits"`, `"auto"`, `"full-auto"`). An invalid value SHALL fail loading with an error naming the value and listing all four names. The meaning of each level is defined by *Permission flag mapping*; how a version-1 file's `"auto"` is read is defined by `core-configuration`'s *Version-1 `auto` approval values load as `accept-edits`*.

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
- **AND** `agent_approval` SHALL be `Auto`, the default
- **AND** `approval` SHALL be `None`

#### Scenario: The default is auto in every version

- **GIVEN** a `claude` worker, no global config, and in turn: a version-1 config that sets no worker approval, a `version = 2` config that sets none, and no config file
- **WHEN** each configuration is loaded and the pane command is built
- **THEN** the worker approval SHALL be `Auto` each time
- **AND** each pane command SHALL contain `--permission-mode auto` and SHALL NOT contain `--permission-mode acceptEdits`
- **AND** the orchestrator approval, unset, SHALL resolve to `Auto`

#### Scenario: Invalid agent_approval value is rejected

- **GIVEN** a config file with `[supervisor]` containing `agent_approval = "yolo"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error mentioning the invalid value
- **AND** the error SHALL list `manual`, `accept-edits`, `auto` and `full-auto`

#### Scenario: supervisor approval field parses all three levels

- **GIVEN** a version-1 config file with `[supervisor]` containing `approval = "full-auto"`
- **WHEN** the config is loaded
- **THEN** `supervisor.approval` SHALL be `Some(ApprovalLevel::FullAuto)`
- **AND** the equivalent holds for `"manual"`
- **AND** `approval = "auto"` in that version-1 file SHALL load as `Some(ApprovalLevel::AcceptEdits)` (per `core-configuration`)

#### Scenario: accept-edits and a version-2 auto parse

- **GIVEN** a version-2 config file with `[orchestrator] approval = "accept-edits"`, and another with `[orchestrator] approval = "auto"`
- **WHEN** each is loaded
- **THEN** the orchestrator approval SHALL be `Some(ApprovalLevel::AcceptEdits)` and `Some(ApprovalLevel::Auto)` respectively

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

The system SHALL provide a flag-resolution function that maps a CLI and approval level to the CLI-specific permission flags to append to the launch command. Resolution SHALL consult, in order:

1. **Per-CLI config override**: when `[clis.<name>]` defines an `approval_args` map for the requested level (keys are the kebab-case level names), its value SHALL be used verbatim. This remains the seam for a CLI whose family has no built-in row, or whose flags differ from its family's (e.g. a retained `gemini` re-added as `[clis.gemini]`).
2. **Built-in table, keyed by the CLI's provider family** (per `cli-provider`): a known CLI resolves its own family; a `[clis.<name>]` entry resolves the family it declares with `family`, else `generic`. The following mappings SHALL be supported:

| Family | Level | Flags |
|---|---|---|
| `claude` | `FullAuto` | `"--dangerously-skip-permissions"` |
| `claude` | `Auto` | `"--permission-mode auto"` |
| `claude` | `AcceptEdits` | `"--permission-mode acceptEdits"` |
| `claude` | `Manual` | `""` |
| `codex` | `FullAuto` | `"--dangerously-bypass-approvals-and-sandbox"` |
| `codex` | `Auto` | `"--sandbox workspace-write"` |
| `codex` | `AcceptEdits` | `"--sandbox workspace-write"` |
| `codex` | `Manual` | `""` |
| `agy` | `FullAuto` | `"--dangerously-skip-permissions"` |
| `qwen` | `FullAuto` | `"--yolo"` |
| any other | any | `""` |

3. **Fallback**: any family/level pair not covered above resolves to `""` (no flags). A level SHALL NOT borrow another level's row.

The levels SHALL mean:

- `Manual` — no flags: the CLI's own default permission mode.
- `AcceptEdits` — the CLI's mode that applies file edits without a prompt and leaves every other action the CLI's own rules do not allow to a prompt. For Claude this is `--permission-mode acceptEdits`. Codex has no verified edits-only mode distinct from its workspace-write sandbox, so its row equals its `Auto` row. This level launches exactly what `Auto` launched before this change, for every family.
- `Auto` — the CLI's own automatic permission mode, in which the CLI itself decides each action. For Claude this is `--permission-mode auto` (Claude Code's auto mode). Codex keeps `--sandbox workspace-write`. A family with no such mode keeps `""` and relies on the per-CLI `approval_args` override; its prompts reach the human.
- `FullAuto` — the CLI's no-prompt mode, which belongs inside a sandbox.

Each non-empty row SHALL resolve to a deterministic mode rather than the empty string wherever the CLI provides one, so a pane at that level does not silently inherit whatever default the CLI's config directory happens to carry.

The resolved approval flags are git-paw's only influence on whether an agent's command runs without a prompt. git-paw SHALL NOT approve a prompt the resolved mode leaves in place, and SHALL NOT judge any command itself at any level.

The built-in rows SHALL be verified against each CLI's upstream documentation at implementation time; a row whose upstream flag has changed SHALL be corrected via spec amendment before the change lands. In particular, the Claude `Auto` and `AcceptEdits` values SHALL be confirmed against the live Claude Code `--permission-mode` enumeration (on Claude Code 2.1.325, 2026-11-16: `acceptEdits` / `auto` / `bypassPermissions` / `manual` / `dontAsk` / `plan`), and the Claude `Auto` row SHALL additionally pass a recorded behaviour check of a worker launched as git-paw launches it before it lands. The verifying CLI version and date SHALL be recorded beside the row.

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

#### Scenario: Claude with auto returns the CLI's own auto mode

- **WHEN** flags are resolved for `("claude", Auto)` with no config override
- **THEN** the result is `"--permission-mode auto"`

#### Scenario: Claude with accept-edits returns an explicit accept-edits permission mode

- **WHEN** flags are resolved for `("claude", AcceptEdits)` with no config override
- **THEN** the result is `"--permission-mode acceptEdits"` (a deterministic mode, not the empty string)

#### Scenario: Claude with auto returns an explicit accept-edits permission mode

- **GIVEN** a version-1 config whose worker approval is `"auto"`, which loads as `AcceptEdits`
- **WHEN** flags are resolved for a `claude` worker with no config override
- **THEN** the result is `"--permission-mode acceptEdits"`, as it was before this change

#### Scenario: Codex with auto returns workspace-write sandbox flag

- **WHEN** flags are resolved for `("codex", Auto)` with no config override
- **THEN** the result is `"--sandbox workspace-write"`

#### Scenario: Codex with accept-edits keeps the workspace-write sandbox

- **WHEN** flags are resolved for `("codex", AcceptEdits)` with no config override
- **THEN** the result is `"--sandbox workspace-write"`

#### Scenario: Accept-edits launches what auto launched before for every family

- **GIVEN** the built-in table as it stood before this change, frozen in a test
- **WHEN** flags are resolved at `AcceptEdits` for every detected CLI and for an unknown CLI
- **THEN** each result SHALL equal that CLI's former `Auto` row

#### Scenario: A level never borrows another level's row

- **WHEN** flags are resolved for `("qwen", AcceptEdits)` and `("qwen", Auto)` with no config override
- **THEN** both results are `""`, although qwen's `FullAuto` row is `"--yolo"`

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

#### Scenario: An override for one level does not apply to another

- **GIVEN** a version-2 config with `[clis.claude]` defining `approval_args = { "auto" = "--some-auto-flag" }`
- **WHEN** flags are resolved for `("claude", AcceptEdits)`
- **THEN** the result is `"--permission-mode acceptEdits"`

#### Scenario: Override enables a CLI with no built-in row

- **GIVEN** a config with `[clis.claude-oss]` defining `command` and `approval_args = { "full-auto" = "--dangerously-skip-permissions" }` and no `family`
- **WHEN** flags are resolved for `("claude-oss", FullAuto)`
- **THEN** the result is `"--dangerously-skip-permissions"`

#### Scenario: A declared family supplies the built-in row

- **GIVEN** a config with `[clis.claude-oss]` defining `command` and `family = "claude"` and no `approval_args`
- **WHEN** flags are resolved for `("claude-oss", AcceptEdits)` and for `("claude-oss", Auto)`
- **THEN** the results are `"--permission-mode acceptEdits"` and `"--permission-mode auto"`

#### Scenario: Unknown CLI returns empty string

- **WHEN** flags are resolved for `("some-agent", FullAuto)` with no config override
- **THEN** the result is `""`

#### Scenario: Any CLI with manual returns empty string

- **WHEN** flags are resolved for `("claude", Manual)` with no config override
- **THEN** the result is `""`

#### Scenario: Flag mapping is deterministic

- **WHEN** flags are resolved twice for the same `(cli, level, config)` triple
- **THEN** both calls return the same value

### Requirement: CustomCli approval_args override field

The `CustomCli` struct (`[clis.<name>]`) SHALL gain an optional `approval_args` map from kebab-case approval-level names (`"manual"`, `"accept-edits"`, `"auto"`, `"full-auto"`) to flag strings. The field SHALL default to absent (`#[serde(default)]`, omitted on round-trip when empty), so existing configs parse unchanged. Unknown level keys SHALL be rejected at config load with an error naming the invalid key and listing the four accepted keys. In a version-1 file the key `auto` SHALL be read as `accept-edits` (per `core-configuration`), so an existing override keeps applying to the same workers.

#### Scenario: approval_args parses and round-trips

- **GIVEN** a config with `[clis.mycli]` containing `command = "mycli"` and `approval_args = { "full-auto" = "--yolo" }`
- **WHEN** the config is loaded and saved back
- **THEN** the loaded map SHALL contain `"full-auto" → "--yolo"` and the round-trip SHALL preserve it

#### Scenario: The accept-edits key is accepted

- **GIVEN** a config with `[clis.mycli]` containing `command = "mycli"` and `approval_args = { "accept-edits" = "--edits-only" }`
- **WHEN** the config is loaded and a worker using `mycli` is launched at `accept-edits`
- **THEN** loading SHALL succeed and the pane command SHALL contain `--edits-only`

#### Scenario: Existing CustomCli entries parse unchanged

- **GIVEN** a pre-v0.11.0 config with `[clis.mycli]` containing only `command = "mycli"`
- **WHEN** the config is loaded
- **THEN** loading SHALL succeed and `approval_args` SHALL be empty/absent

#### Scenario: Invalid level key is rejected

- **GIVEN** a config with `[clis.mycli]` containing `approval_args = { "yolo-mode" = "--x" }`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error mentioning `yolo-mode`
- **AND** the error SHALL list `manual`, `accept-edits`, `auto` and `full-auto`
