## ADDED Requirements

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

## MODIFIED Requirements

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

## REMOVED Requirements

### Requirement: Common dev allowlist sub-table

**Reason**: The dev-command allowlist fed only the retired classifier and the skill prose that taught approving it.
**Migration**: The table still parses with no effect and warns (*Deprecated auto-approval tables are parsed but have no effect*). Put routine commands in the agent CLI's own allow rules.

### Requirement: `[supervisor.auto_approve]` config section

**Reason**: There is no auto-approval feature for the table to control.
**Migration**: The table still parses with no effect and warns (*Deprecated auto-approval tables are parsed but have no effect*).

### Requirement: Configurable enable flag

**Reason**: Nothing is left to enable.
**Migration**: `enabled` still parses with no effect.

### Requirement: Configurable safe-command list

**Reason**: There is no whitelist to append to.
**Migration**: `safe_commands` still parses with no effect. Move its entries into the agent CLI's own allow rules.

### Requirement: Configurable stall threshold

**Reason**: The stall threshold paced the deleted dashboard auto-approve thread.
**Migration**: `stall_threshold_seconds` still parses with no effect and is no longer clamped. Stuck detection keeps its own thresholds (`broker-agent-helper`).

### Requirement: Approval level coarse switch

**Reason**: The `off` / `conservative` / `safe` presets selected whitelist shapes that no longer exist.
**Migration**: `approval_level` still parses with no effect. The CLI permission mode is chosen by `[supervisor] agent_approval` / `approval` (*Permission flag mapping*).
