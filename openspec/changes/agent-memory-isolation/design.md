## Context

The classifier already has a worktree boundary: `is_path_inside_worktree` (`src/supervisor/auto_approve.rs:184+`, canonicalize + `starts_with`, fail-closed) gates `is_worktree_file_op` / `is_worktree_git_op`, so out-of-worktree writes are never safe-by-pattern — but they land as **Unknown**, i.e. an ordinary approvable prompt. The Wave-5 incident (agent wrote the operator's `MEMORY.md`) showed that is not enough, and #1's full-auto supervisor raises the stakes. CLI-internal memory persistence may never surface a bash prompt, so the classifier alone cannot fully close the hole.

## Goals / Non-Goals

**Goals:** prompted writes into operator config/memory territory become terminal danger escalations; agents are told (CLI-agnostically) to keep persistent artifacts inside the worktree; the supervisor has a violation procedure.
**Non-Goals:** launch-level filesystem scoping / env redirection (v1.0.0 per-CLI providers — operator decision 2026-07-14); guarding reads; sandboxing the supervisor pane itself.

## Decisions

- **D1 — Danger class, not Unknown.** Unknown still gets presented for human approval and is invisible in an unattended run's alert flow; danger is terminal, logged, and escalated. Precedence mirrors the curated danger-list (evaluated first).
- **D2 — Config-driven protected set.** Sources: `~/.claude` (the documented claude-format default), `CLAUDE_CONFIG_DIR`, `[clis.<name>].settings_path` parents, `projects/**/memory` subtrees, and repo-root `.claude/` + `.git-paw/` for embedded worktrees. No CLI product names hardcoded — same export-agnosticism rule that governs the allowlist presets.
- **D3 — Guidance closes the unprompted-write gap.** The coordination skill is the only CLI-agnostic lever for writes that never prompt. Worded as policy ("persistent artifacts live in your worktree") rather than tool-specific mechanics.
- **D4 — Reads unaffected.** Read escalation would flood the operator (read-mostly verbs legitimately touch config for debugging) with no comparable risk.

## Risks / Trade-offs

- **Skill-content pinning tests**: `skills.rs` / `*_skill_content.rs` pin rendered prose — must be updated in the same commit (the helper-migration ripple lesson).
- **sweep.sh lockstep**: the classify mirror needs the same rule where file-prompts flow through `sweep.sh classify`; keep the fixture-level parity test green (`tests/sweep_sh_classify.rs`).
- **False positives**: a legitimate workflow that edits repo-root `.git-paw/config.toml` from an agent pane now escalates — acceptable; that IS a supervisor/operator decision.

## Migration Plan

None: no config surface changes; new classification is strictly more conservative.

## Open Questions

None — enforcement level resolved with the operator 2026-07-14 (classifier + guidance; launch-level deferred).
