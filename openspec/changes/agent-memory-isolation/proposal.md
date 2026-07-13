## Why

A confirmed v0.9.0 dogfood incident (Wave 5): a coding agent wrote to the OPERATOR's `MEMORY.md`, outside its worktree. Today an out-of-worktree write prompt merely fails the safe-by-pattern check and falls to Unknown — it still sits as an approvable prompt a busy operator can rubber-stamp, and CLI-internal memory persistence may not raise a bash prompt at all. `supervisor-native-auto-mode` (#1) raises the blast radius of exactly this class, so the guard lands in the same release.

## What Changes

- **Classifier**: writes targeting operator configuration/memory territory (a config-driven **protected-path set**: home-level CLI config dirs like `~/.claude`, every configured `[clis.<name>].settings_path` parent, `CLAUDE_CONFIG_DIR`, their `projects/**/memory` subtrees, and the host repo's `.claude/` + `.git-paw/` when outside the agent's worktree) SHALL classify as danger — terminal escalation, never auto-approved, same precedence as the curated danger-list. Reads are unaffected. Mirrored in `sweep.sh classify`.
- **Guidance**: the bundled coordination skill gains a memory-isolation section (all persistent agent artifacts live inside the worktree; operator config dirs are off-limits; publish `agent.question` when a task seems to require an outside write). The supervisor skill treats observed out-of-worktree write attempts as violations to feed back on.
- Launch-level filesystem scoping (per-CLI env redirection) is explicitly deferred to v1.0.0's per-CLI providers (operator decision 2026-07-14).

## Capabilities

### New Capabilities

- `agent-memory-isolation`: the protected-path definition (config-driven, CLI-agnostic) and the worktree-scoped memory guidance in the bundled coordination + supervisor skills.

### Modified Capabilities

- `safe-command-classification`: ADDED requirement — operator config-path writes escalate as danger (evaluated like the curated danger-list; canonicalized, fail-closed).

## Impact

- `src/supervisor/auto_approve.rs`: protected-path derivation + danger wiring for file-prompt paths and command-slice write targets; `assets/scripts/sweep.sh` classify mirror (lockstep).
- `assets/agent-skills/coordination.md` + `assets/agent-skills/supervisor.md`: new guidance prose — ⚠ skill content is pinned by `skills.rs` / `*_skill_content.rs` tests; update them alongside.
- Tests: protected-path canonicalization (symlink/`..` escape), danger precedence, sweep.sh parity fixture, skill-content assertions.
- Docs: coordination + supervisor chapters, configuration reference note. No config surface change (derivation uses existing fields).
- Export-agnosticism: no hardcoded CLI names — the set derives from config and well-known defaults.
