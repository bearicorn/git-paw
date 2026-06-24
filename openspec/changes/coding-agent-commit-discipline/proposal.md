## Why

Three coding-agent commit behaviours surfaced as friction across the v0.6.0 + v0.7.0 dogfoods, all addressable as bundled-skill prose:

1. **Self-archive reflex (F7).** Both finished agents typed `/opsx:archive` after their final commit. `opsx-role-gating` blocks *execution*, and the supervisor nudges each time — but reactively; the agent wastes a turn and the supervisor must intervene.
2. **Messy commit history.** Agents produce many `fix typo` / `address feedback` micro-commits, forcing a manual squash at release (v0.6.0: 148 commits hand-squashed to 10; v0.7.0: a 4-commit feature squashed to 1). This bloats the changelog and the reviewer's view.
3. **Over-opinionated commit-format prose.** `coordination.md` prescribes Conventional Commits (`feat(scope):` …). Commit-message format is a *per-project* convention (and the recurring AI-trailer problem lives in the same space) — it belongs in the project's injected `AGENTS.md`, not git-paw's generic bundled skill.

## What Changes

- **Stand-by-after-commit protocol (F7).** Bundled coding-agent skill states: after your final commit, **STAND BY** — publish `committed`/`done` and wait; do NOT run `/opsx:verify` or `/opsx:archive` (supervisor-only). Makes the role boundary proactive.
- **Releasable-unit commit discipline (#1).** Skill "Commit cadence" gains: each commit MUST build + pass its gates on its own; a small follow-up to the commit you *just* made (not yet verified / not yet moved past) SHOULD be `git commit --amend`ed rather than added as a micro-commit. (Do NOT amend an already-verified or earlier-group commit.) Generic orchestration discipline tied to per-commit verification + changelog hygiene — correctly in git-paw's bundled skill.
- **De-opinionate commit-format prose (#3).** Soften the bundled skill from prescribing Conventional Commits to "follow the project's commit-message conventions (see the project's `AGENTS.md`)." Project-specific format (and no-AI-trailer rules) live in the injected `AGENTS.md`, not the generic skill.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `agent-skills` (coordination skill): add stand-by-after-commit + releasable-unit/amend-fixups discipline; soften the commit-format prescription to defer to the project's `AGENTS.md`.
- `supervisor-skill-discipline`: cross-reference — the supervisor relies on agents standing by post-commit (it verifies + archives).

## Impact

- Affected code: `assets/agent-skills/coordination.md` (+ possibly `supervisor.md`) — bundled-skill prose only.
- Tests: skill-content tests assert the stand-by guidance, the releasable-unit/amend guidance, and that the skill no longer hardcodes a specific commit-message format (defers to project AGENTS.md).
- Docs: skill docs note the post-commit protocol + commit cadence.
- Backward compatible: skill-prose-only; no code-path or config change. Complements `opsx-role-gating` (execution guard) and the v0.9.0 approval work.
