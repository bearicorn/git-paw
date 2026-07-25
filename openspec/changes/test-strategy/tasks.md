# Tasks — test-strategy

## 1. The skill
- [x] Author `.agents/skills/test-strategy/SKILL.md` in agentskills.io format (name↔folder, `name`/`description` frontmatter, layer-decision table, robustness rules, anti-pattern catalog, consolidation routing)
- [x] Verify conformance against the agentskills.io spec (frontmatter constraints; `name = "test-strategy"` matches folder; `description` ≤ 1024, says what + when)

## 2. CI validation guard
- [x] Add an integration test / CI step that loads every `.agents/skills/*/SKILL.md` via git-paw's own skill loader (`skill-standardization`/`skill-validation`) and asserts each parses with required fields + a name matching its folder; fails non-zero on a malformed skill
- [x] Wire it into the CI matrix (fmt/clippy/deny/audit-adjacent) so a non-conformant skill blocks merge
- [ ] (Optional) document `skills-ref validate ./.agents/skills/<name>` as the external equivalent

## 3. Contributor guidance
- [x] Add an AGENTS.md testing-section pointer naming the `test-strategy` skill as the canonical procedure for choosing a test's type and writing it behaviorally
- [ ] Cross-reference from CONTRIBUTING.md if it carries testing guidance

## 4. Verification (five gates)
- [ ] Gate 1/2 — the skill-validation test passes; full suite green vs merge-base
- [ ] Gate 3 — every `test-strategy` spec scenario maps to a test (skill-present, malformed-fails, conformant-passes, guidance-points-to-skill)
- [ ] Gate 4 — AGENTS.md/CONTRIBUTING updated; no mdBook change needed (dev tooling)
- [ ] Gate 5 — security: no secrets; the skill is dev-local, not exported; `assets/agent-skills/` untouched (agnosticism preserved)
- [ ] `just check` green; `cargo fmt` before commit
- [ ] `openspec validate test-strategy --strict` passes

## Notes
- Repo-local DEV skill — do NOT add to `assets/agent-skills/` or ship via `git paw init`.
- Precedes `test-suite-consolidation` (applies the skill). The anti-pattern lint over the
  existing suite belongs to that change, not this one.
