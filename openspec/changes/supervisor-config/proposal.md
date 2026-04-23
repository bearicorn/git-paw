## Why

v0.4.0 introduces a supervisor agent that orchestrates coding agents, runs tests, and manages merges. Before the supervisor can be implemented, the config system needs to know which CLI to use for supervision, what test command to run, and what permission level to grant coding agents. This change adds the `[supervisor]` config section that the supervisor-agent, supervisor-mode, and auto-start changes will consume.

## What Changes

- Add a `[supervisor]` section to `.git-paw/config.toml` with four fields:
  - `enabled: bool` — whether supervisor mode is active by default (defaults to `false`). The `--supervisor` CLI flag overrides this for a single session.
  - `cli: Option<String>` — which CLI binary to use for the supervisor agent (defaults to `default_cli` if set, otherwise requires explicit config or `--supervisor-cli` flag)
  - `test_command: Option<String>` — command the supervisor runs after an agent reports done (e.g. `"just check"`, `"cargo test"`). Defaults to `None` (supervisor skips testing if not set)
  - `agent_approval: ApprovalLevel` — permission level for coding agents: `"auto"` (default), `"manual"`, or `"full-auto"`
- Add `SupervisorConfig` struct to `src/config.rs` with `Default` impl and serde support
- Add a permission flag mapping function that translates `(cli_name, approval_level)` → CLI-specific flags:
  - `("claude", "full-auto")` → `"--dangerously-skip-permissions"`
  - `("claude", "auto")` → `""` (Claude's default is already interactive approval)
  - `("codex", "full-auto")` → `"--approval-mode=full-auto"`
  - `("codex", "auto")` → `"--approval-mode=auto-edit"`
  - Unknown CLI + any level → `""` (no flags, CLI's default behavior)
- Update `generate_default_config()` to include a commented-out `[supervisor]` section
- Config merges with repo-wins semantics (same as `[broker]`, `[specs]`, `[logging]`)
- Existing configs without `[supervisor]` load as `None` (unconfigured)
- `git paw start` prompts "Start in supervisor mode?" when unconfigured (no section in config, no CLI flag)
- `git paw init` prompts "Enable supervisor mode by default?" and writes the section to prevent future prompts

## Capabilities

### New Capabilities

- `supervisor-config`: The `[supervisor]` config section, `SupervisorConfig` struct, permission flag mapping function, and default generation. Covers config parsing, defaults, validation, merge semantics, and the CLI-to-flags translation.

### Modified Capabilities

- `configuration`: Add `[supervisor]` section to the config schema. Existing fields and merge rules unchanged.

## Impact

- **Modified file:** `src/config.rs` — add `SupervisorConfig` struct, `supervisor` field on `PawConfig`, `approval_flags()` function, update `generate_default_config()`
- **No new files, no new modules, no new dependencies.**
- **Backward compatible:** `serde(default)` on the `supervisor` field; v0.3.0 configs load without error.
- **Dependents:** `supervisor-agent` (reads cli + test_command), `supervisor-mode` (reads agent_approval + calls approval_flags), `per-cli-skills` (may reference approval_flags for CLI detection).
- **No CLI surface changes.** No new commands or flags in this change.
