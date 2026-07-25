# Design — test-strategy

## Context

The test suite drifted from the behavioral-only standard. A one-off consolidation would drift
again; a codified, discoverable, enforced decision procedure will not. That procedure ships as
an agent skill, and the consolidation (`test-suite-consolidation`) applies it.

## Decisions

### D1 — agentskills.io format, at `.agents/skills/`, repo-local (not `.claude/`, not exported)

The skill is authored per the agentskills.io spec: a directory with `SKILL.md`, `name` matching
the folder, required `name`/`description` frontmatter (verified against
<https://agentskills.io/specification>). It lives at `.agents/skills/test-strategy/` — the
portable standard location git-paw's **own** resolver (`src/skills.rs`) already loads and the
one Antigravity/Codex/Claude Code all discover — **not** `.claude/skills/` (Claude-specific).

It is **repo-local dev tooling**, not an exported asset: the skill is git-paw-specific (cargo,
the PTY harness, OpenSpec scenarios), so it must **not** go in `assets/agent-skills/`, which
`AGENTS.md` requires to stay project-agnostic. `.agents/skills/` in git-paw's own repo is
git-paw's own tooling and may be git-paw-specific.

### D2 — Enforce by dogfooding git-paw's own `skill-validation`

The CI guard validates every `.agents/skills/*` skill by loading it through git-paw's own
skill loader/validator (`skill-standardization` + `skill-validation` capabilities) and asserting
it parses with required fields and a name matching its folder. This needs **no external
dependency** and dogfoods a shipped capability. `skills-ref validate` (the agentskills.io
reference validator) is the equivalent external option if a language-neutral check is preferred.

### D3 — Position in the testing workstream

`test-strategy` (this change) authors the standard; `test-suite-consolidation` applies it across
the suite (routing every cluster via the skill's decision table); `cli-interaction-e2e` supplies
the PTY e2e layer the skill routes interactive behaviors to. So: **W1 net → this skill → apply
(consolidate) → domain refactors (W4)**.

### D4 — Skill content (already authored)

`SKILL.md` sections: prime directive (behavioral-only) · Step 1 pick-the-layer decision table ·
Step 2 write-it-robustly per layer · Step 3 classify-an-existing-test (consolidation routing) ·
anti-pattern catalog · robustness checklist. Kept < 500 lines per the spec's progressive-disclosure
guidance; no `references/` split needed yet.

### D5 — Anti-pattern lint (design note, not required here)

A deeper guard that greps the suite for source-grep introspection (`include_str!("../src/…")`
brace-walks) and impl-detail smells is valuable but belongs to `test-suite-consolidation`'s
enforcement (it acts on the existing suite). This change's CI guard is scoped to *skill
conformance*; the anti-pattern lint is noted for the consolidation change.

## Non-goals

- Not an exported/bundled skill; `git paw init` does not ship it.
- Not a change to any product behavior, CLI, config, or wire surface.
- Not the consolidation itself (that is `test-suite-consolidation`).

## Risks

- **Low.** Docs/dev-tooling + a CI check. The only failure mode is a false CI failure from an
  over-strict validator — mitigated by reusing git-paw's own tested loader (D2).
