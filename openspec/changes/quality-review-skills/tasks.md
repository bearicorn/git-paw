# Tasks — quality-review-skills

## 1. The skills
- [x] Author `.agents/skills/doc-completeness/SKILL.md` (four doc layers, per-change mapping, Gate-4 checklist)
- [x] Author `.agents/skills/security-review/SKILL.md` (external bad actors: least-privilege, injection, secrets, deps, unsafe)
- [x] Author `.agents/skills/safety-review/SKILL.md` (rogue-agent blast radius: out-of-worktree, irreversible git, persistence, exfiltration; the containment + the planned sandbox)
- [x] Author `.agents/skills/definition-of-done/SKILL.md` (the completeness meta; references every dimension skill)
- [x] Verify all four conform via the shared `tests/agent_skills_conform.rs` guard (now 7 skills)

## 2. Cross-link
- [x] Cross-link from `.agents/skills/code-standards/SKILL.md` to `definition-of-done` + the review skills

## 3. Verification (five gates)
- [ ] Gate 1/2 — conformance guard passes with all 7 skills; full suite green vs merge-base
- [ ] Gate 3 — every `quality-review-skills` scenario maps to a check (each skill present + conformant; security≠safety; DoD references every dimension)
- [ ] Gate 4 — AGENTS.md already documents the five gates + Change Checklist; no mdBook change (dev tooling)
- [ ] Gate 5 — dev-local (`.agents/`), NOT exported; `assets/agent-skills/` untouched
- [x] `openspec validate quality-review-skills --strict` passes

## Notes
- Repo-local DEV skills — do NOT ship via `git paw init` or place in `assets/agent-skills/`.
- `safety-review` is the interim guard; once the roadmap v0.15.0 FS-scoped sandbox lands it becomes
  defence-in-depth rather than the primary containment.
