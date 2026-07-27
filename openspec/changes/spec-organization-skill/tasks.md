# Tasks — spec-organization-skill

## 1. The skill
- [x] Author `.agents/skills/spec-organization/SKILL.md` (agentskills.io; capability granularity, contract-preserving reorg + count invariant, RFC-2119 + GIVEN/WHEN/THEN authoring incl. the SHALL-on-first-line trap and the Purpose guard, specs-as-docs + the `{{#include}}` coupling, the pre-freeze error catalog, the `--skip-specs` archive discipline, standing-guard patterns)
- [x] Verify it conforms via the shared `tests/agent_skills_conform.rs` guard (now 7 skills)

## 2. Wire-in
- [x] AGENTS.md pointer under Spec-Driven Development (drive spec work through this skill + the `opsx:*` skills)
- [x] Back-reference from `definition-of-done` (the spec done-dimension points at `spec-organization`)

## 3. Verification (five gates)
- [ ] Gate 1/2 — conformance guard passes with 7 skills; full suite green vs merge-base
- [x] Gate 3 — the change's scenario maps to the conformance guard (skill present + conformant)
- [x] Gate 4 — AGENTS.md pointer added; no mdBook change (dev tooling)
- [x] Gate 5 — dev-local (`.agents/`), NOT exported; `assets/agent-skills/` untouched
- [x] `openspec validate spec-organization-skill --strict` passes

## Notes
- Repo-local DEV skill — do NOT ship via `git paw init` or place in `assets/agent-skills/`.
- Future consolidation note: the several "<skill> provided" dev-tooling capabilities could group into one `standards-skills` domain capability (per this skill's own granularity rule) — a candidate for a later spec-consolidation pass, not now.
