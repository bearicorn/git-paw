# Tasks — export-agnosticism-skill

## 1. The skill
- [x] Author `.agents/skills/export-agnosticism/SKILL.md` (agentskills.io; name↔folder) covering the full export-surface inventory (skills, scripts/mirrored-code, init config, presets/allowlists, injected artifacts, templates), the rule, per-surface how-to, the enforcing audits + standing "new surface needs a guard" rule, and the review-gate checklist
- [x] Verify conformance via the shared `tests/agent_skills_conform.rs` guard (loads + validates every `.agents/skills/*`)

## 2. Cross-link
- [x] Cross-link from `.agents/skills/code-standards/SKILL.md` so an agent editing an export is routed here

## 3. Verification (five gates)
- [ ] Gate 1/2 — the conformance guard passes with the new skill (checks ≥3 skills now); full suite green vs merge-base
- [ ] Gate 3 — every `export-agnosticism-skill` scenario maps to a check (skill present+conformant; inventory spans >skills; code-standards cross-link)
- [ ] Gate 4 — AGENTS.md export-agnosticism principle already documents the rule; no mdBook change (dev tooling)
- [ ] Gate 5 — security/agnosticism: the skill itself is dev-local (`.agents/`), NOT added to `assets/agent-skills/`; nothing exported changed
- [x] `openspec validate export-agnosticism-skill --strict` passes

## Notes
- Repo-local DEV skill — do NOT ship via `git paw init` or place in `assets/agent-skills/`.
- Enforcement already exists (lang-agnostic audit, classifier guard, config-default test); this change adds the skill + cross-link, not new enforcement code.
