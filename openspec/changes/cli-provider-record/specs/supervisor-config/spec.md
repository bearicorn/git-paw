## MODIFIED Requirements

### Requirement: Permission flag mapping

The system SHALL provide a flag-resolution function that maps a CLI and approval level to the CLI-specific permission flags to append to the launch command. Resolution SHALL consult, in order:

1. **Per-CLI config override**: when `[clis.<name>]` defines an `approval_args` map for the requested level (keys are the kebab-case level names), its value SHALL be used verbatim. This remains the seam for a CLI whose family has no built-in row, or whose flags differ from its family's (e.g. a retained `gemini` re-added as `[clis.gemini]`).
2. **Built-in table, keyed by the CLI's provider family** (per `cli-provider`): a known CLI resolves its own family; a `[clis.<name>]` entry resolves the family it declares with `family`, else `generic`. The following mappings SHALL be supported:

| Family | Level | Flags |
|---|---|---|
| `claude` | `FullAuto` | `"--dangerously-skip-permissions"` |
| `claude` | `Auto` | `"--permission-mode acceptEdits"` |
| `claude` | `Manual` | `""` |
| `codex` | `FullAuto` | `"--dangerously-bypass-approvals-and-sandbox"` |
| `codex` | `Auto` | `"--sandbox workspace-write"` |
| `codex` | `Manual` | `""` |
| `agy` | `FullAuto` | `"--dangerously-skip-permissions"` |
| `qwen` | `FullAuto` | `"--yolo"` |
| any other | any | `""` |

3. **Fallback**: any family/level pair not covered above resolves to `""` (no flags).

The `Auto` level SHALL resolve to a deterministic per-family flag rather than the empty string wherever the CLI provides a low-friction "accept edits" mode distinct from `Manual` and from full bypass, so an `Auto` pane does not silently inherit whatever default the CLI's config directory happens to carry. For Claude this is `--permission-mode acceptEdits`: it auto-accepts file edits, and every other command prompt is left to the CLI and, under the drive loop, escalated for a human decision. Codex's `Auto` already resolves to `--sandbox workspace-write`. A family with no such mode keeps `""` and relies on the per-CLI `approval_args` override; its prompts reach the human.

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

- **GIVEN** a config with `[clis.claude-oss]` defining `command` and `approval_args = { "full-auto" = "--dangerously-skip-permissions" }` and no `family`
- **WHEN** flags are resolved for `("claude-oss", FullAuto)`
- **THEN** the result is `"--dangerously-skip-permissions"`

#### Scenario: A declared family supplies the built-in row

- **GIVEN** a config with `[clis.claude-oss]` defining `command` and `family = "claude"` and no `approval_args`
- **WHEN** flags are resolved for `("claude-oss", Auto)`
- **THEN** the result is `"--permission-mode acceptEdits"`

#### Scenario: Unknown CLI returns empty string

- **WHEN** flags are resolved for `("some-agent", FullAuto)` with no config override
- **THEN** the result is `""`

#### Scenario: Any CLI with manual returns empty string

- **WHEN** flags are resolved for `("claude", Manual)` with no config override
- **THEN** the result is `""`

#### Scenario: Flag mapping is deterministic

- **WHEN** flags are resolved twice for the same `(cli, level, config)` triple
- **THEN** both calls return the same value
