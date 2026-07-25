# Tasks — code-standards

## 1. The skill
- [x] Author `.agents/skills/code-standards/SKILL.md` (agentskills.io; name↔folder; decoupling patterns, module domains, idiom rules, frozen do-not-touch zones, refactor rules, review-gate checklist)
- [x] Author `references/rust-api-guidelines.md` (condensed Rust API Guidelines checklist + git-paw applicability)
- [x] Author `references/cli-and-devtool-design.md` (condensed CLI/dev-tool guidelines + git-paw applicability)
- [x] Author `references/non-functional-requirements.md` (NFR set + conflict resolutions + precedence spine — the rationale the standards serve)
- [x] Verify conformance against the agentskills.io spec (`name = "code-standards"` matches folder; `description` ≤ 1024 says what + when; SKILL.md < 500 lines; references linked one level deep)
- [x] Mark clearly which standards are enforced-now vs aspirational (seams/splits pending the refactor)

## 2. Contributor guidance
- [x] Add an AGENTS.md code-style pointer naming the `code-standards` skill as the canonical reference for structuring/decoupling/refactoring
- [ ] Cross-reference from CONTRIBUTING.md if it carries code-style guidance

## 3. Verification (five gates)
- [ ] Gate 1/2 — validated by the `test-strategy` all-skills CI guard; full suite green vs merge-base
- [ ] Gate 3 — every `code-standards` scenario maps to a test (skill present+conformant, references present+linked, guidance points to skill)
- [ ] Gate 4 — AGENTS.md updated; no mdBook change needed (dev tooling)
- [ ] Gate 5 — security: dev-local, not exported; `assets/agent-skills/` untouched (agnosticism preserved)
- [ ] `just check` green; `cargo fmt` before commit
- [ ] `openspec validate code-standards --strict` passes

## Notes
- Repo-local DEV skill — do NOT add to `assets/agent-skills/` or ship via `git paw init`.
- Applied by `code-analysis-refactor`; consumed by `standards-skill-integration`. Keep the
  aspirational patterns in sync as the refactor introduces the seams/splits.
