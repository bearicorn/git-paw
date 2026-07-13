## Why

The v0.9.0 unattended dogfood showed the dominant residual babysitting cost is the SUPERVISOR pane's own permission prompts: its compound/expansion verify commands (`cd "$V" && bash -n …`, `git -C … diff "$MB...$TIP"`, `$(mktemp)`, subshells) cannot be prefix-matched by allowlists, so a human clears every one. The supervisor pane's launch flags today are derived from the SAME `agent_approval` knob that governs every coding agent (`approval_flags(&supervisor_cli, &supervisor_cfg.agent_approval)` in `cmd_supervisor` and recovery) — an operator cannot run the supervisor at its CLI's native skip-permissions level without also dropping every coding agent's guardrails. Splitting the knob is the last step to genuinely unattended operation; the supervisor is the trusted orchestrator pane and the natural place to relax first.

## What Changes

- Add `[supervisor] approval` (reusing the existing `ApprovalLevel` enum: `"manual"` / `"auto"` / `"full-auto"`) — a SUPERVISOR-pane-specific approval level, decoupled from `agent_approval`. When absent, the supervisor pane inherits `agent_approval` exactly as today (backward compatible).
- Supervisor pane launch (fresh start AND session recovery) resolves its flags from the new field; coding agents keep resolving from `agent_approval`. Dry-run output reports both levels.
- Extend the `approval_flags` known-CLI table beyond `claude`/`codex` to the other major CLIs known to run unattended (`opencode`, `gemini`, `qwen`), each mapped to its documented native skip-permissions/auto flags (verified against upstream docs at implementation time).
- Add a per-CLI override seam: `[clis.<name>]` gains an optional approval-args map consulted BEFORE the built-in table, so custom/variant CLIs (e.g. claude-oss via `CLAUDE_CONFIG_DIR`) get native flags without waiting for v1.0.0's per-CLI provider registry.
- Warn (do not fail) when `full-auto` is requested for a CLI with no known/configured mapping — the pane launches without flags and behaves as `auto`.
- `git paw init`'s commented `[supervisor]` block documents the new key; configuration reference and unattended-operation docs updated.

## Capabilities

### New Capabilities

<!-- none — the behavior extends existing capabilities -->

### Modified Capabilities

- `supervisor-config`: add the `approval: Option<ApprovalLevel>` field (inherit-from-`agent_approval` default) to the `SupervisorConfig` requirement; MODIFY the "Permission flag mapping" requirement — resolution order becomes per-CLI config override → extended built-in table → empty-with-warning at `full-auto`; extend the init commented-block requirement with the new key.
- `supervisor-launch`: supervisor pane command construction (auto-start flow and recovery) resolves flags from the supervisor-specific level; dry-run plan output prints both approval levels.

## Impact

- `src/config.rs`: `SupervisorConfig` gains `approval` (serde default, `skip_serializing_if`); `CustomCli` gains the approval-args override; `approval_flags` becomes a resolution that consults config (signature change from `&'static str`); commented init template block.
- `src/main.rs`: the three `approval_flags` call sites (`cmd_supervisor` start, recovery, `add`) split supervisor vs agent resolution.
- Tests: config round-trip/back-compat (absent field inherits `agent_approval`), flag-resolution table + override precedence, dry-run output.
- Docs: configuration reference `[supervisor]`/`[clis.<name>]` sections, unattended-operation chapter, `--help` unchanged (config-only surface).
- Backward compatibility: existing configs (no `approval` key) produce byte-identical launch commands to v0.10.0.
