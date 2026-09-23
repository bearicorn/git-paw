## Why

A CLI's config KEY and the binary it launches are conflated. `[clis.<name>] command = "..."` is
resolved only for availability (`detect.rs` `resolve_custom_clis_in` resolves `def.command` but
stores `binary_name: def.name`); every pane-launch site then starts the pane with the KEY. The
documented example `[clis.claude-variant] command = "claude"` therefore launches
`claude-variant` — a binary that does not exist (dogfood GP-25; the v0.15.0 sandbox dogfood had to
work around it with a PATH symlink). Two related identity gaps ride along: the MCP-capable boot form
is keyed on a hard-coded name table with no opt-in, so a custom Claude variant (e.g.
`[clis.claude-oss]`) always gets the helper-script boot block (GP-29); and the known-CLI roster and
MCP table name `cursor`, which on most machines is the Cursor *editor* launcher — Cursor's agent CLI
binary is `cursor-agent` (DS-6).

## What Changes

- Every pane-launch path (start, spec-driven start, supervisor pane, `git paw add`, recovery)
  SHALL spawn the resolved `[clis.<name>].command` — as a single shell word, quoted when it
  contains shell-significant characters — and fall back to the CLI name when no `[clis.<name>]`
  entry exists. The config KEY stays the CLI's identity for approval flags, session-id/resume
  templates, prompt profile, settle delay, and `WorktreeEntry.cli`.
- The readiness gate resolves the prompt profile from the CLI identity, not from the (now
  possibly different) launch string it relaunches.
- New optional `[clis.<name>] mcp = true|false` field (serde default, skipped when unset) that
  overrides the built-in MCP-capable table when choosing the boot-block form.
- **BREAKING (auto-detection only)**: the known-CLI roster and the built-in MCP-capable table
  replace `cursor` with `cursor-agent`. A user relying on `cursor` auto-detection must install
  Cursor's agent CLI or add a `[clis.cursor]` entry; an explicit `--cli cursor`, a
  `default_cli = "cursor"`, a `[clis.cursor]` entry, or a saved session's `cli = "cursor"` still
  passes through unchanged.
- README and mdBook Supported-CLIs tables, the coordination/skill-templates MCP lists, and the
  configuration reference (`command` semantics + `mcp`, including the stranding risk of
  `mcp = true` on a CLI without git-paw's MCP server registered) are updated.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-resolution`: MODIFIED *Auto-detect known AI CLIs on PATH* (`cursor` → `cursor-agent`) and
  *README Supported AI CLIs table matches `src/detect.rs`* (roster-sync guard; stale count prose
  dropped); ADDED *Panes launch the configured command of a CLI* and *The CLI identity, not the
  launch command, keys per-CLI resolution*.
- `boot-block`: ADDED *Per-CLI MCP capability override* (the `mcp` field and the built-in
  MCP-capable set `claude`, `codex`, `cursor-agent`, `agy`).

## Impact

- `src/detect.rs` — `KNOWN_CLIS` (`cursor` → `cursor-agent`), `known_display_name`
  (`cursor-agent` → "Cursor Agent"), roster tests.
- `src/config/cli.rs` — `CustomCli` gains `mcp: Option<bool>`; new `launch_command(cli, &clis)`
  helper (leading-token lookup, single-shell-word quoting via `domain::shell_quote`).
- `src/skills.rs` — `MCP_CAPABLE_CLIS` (`cursor` → `cursor-agent`); `cli_speaks_mcp(cli, clis)`
  consults `[clis.<name>].mcp` first.
- `src/main.rs` — `attach_agent` (`cli_command` built from `launch_command`; `cli_speaks_mcp`
  gains `ctx.clis`); `gate_pane_or_fail_on_dialog` passes the identity.
- `src/commands/start.rs` — `cmd_start` / `cmd_start_with_specs` `PaneSpec.cli_command` and both
  `cli_speaks_mcp` calls.
- `src/commands/supervisor.rs` — `supervisor_cli_command`, `cli_speaks_mcp(&supervisor_cli, …)`,
  gate calls; `src/commands/add.rs` — gate call.
- `src/commands/recover.rs` — `cli_command_for_recovery` and `recover_supervisor_session`.
- `src/tmux/readiness.rs` — `gate_pane_for_injection` takes the CLI identity for
  `resolve_prompt_profile` separately from the `cli_command` it relaunches.
- `README.md`, `docs/src/supported-clis.md`, `docs/src/configuration/README.md`,
  `docs/src/user-guide/coordination.md`, `docs/src/user-guide/skill-templates.md`.
- No config struct derives `JsonSchema`, so no schema regeneration is needed.
- **Sequencing**: pending `start-spec-launch-parity` (approval flags on every launch path) and
  `broker-repo-identity` also edit `start.rs` / `supervisor.rs` / `main.rs`; this change lands
  after them and composes `launch_command` with their resolved flags. No requirement collision:
  neither touches `cli-resolution` nor the requirements modified here; `start-spec-launch-parity`'s
  *Empty resolved flags launch the bare command* ("the CLI alone") is read as "the launch command
  alone" after this change.
- **Security**: `command` is user config interpolated into a tmux/shell command line; it is emitted
  as one shell word (quoted), never word-split or expanded.
