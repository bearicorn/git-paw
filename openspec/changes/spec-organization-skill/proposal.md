## Why

The standards-as-skills suite now has `test-strategy` (Gate 1), `code-standards` (code),
`export-agnosticism` (cross-cutting), and the `quality-review-skills` trio (`doc-completeness` Gate 4,
`security-and-safety-review` Gate 5, `definition-of-done`). But **Gate 3 — the spec audit — has no
codifying skill.** The v0.13.0 `spec-traceability-audit` + `spec-consolidation` work proved there is a
repeatable doctrine for keeping the OpenSpec capability set clean, domain-grouped, traceable, and
readable as documentation. Capture it so future spec authoring, archiving, and consolidation follow one
standard and the supervisor gate can enforce it — the same way the other skills govern their
dimensions. This matters most heading into the v1.0.0 freeze, where the spec set IS the frozen contract
and doubles as the Specifications docs.

## What Changes

One new repo-local dev skill at `.agents/skills/spec-organization/`, covering: capability granularity
(one change ≠ one permanent capability; when to merge vs keep separate), contract-preserving verbatim
reorganization (the requirement-count invariant; the requirement→test map as the safety net),
RFC-2119 + GIVEN/WHEN/THEN authoring rules (including the SHALL-on-first-line validator trap and the
`## Purpose` placeholder guard), specs-as-documentation (the docs `{{#include}}` coupling; a
domain-grouped Purpose-led index), the errors a pre-freeze audit must catch (drift, phantom commands,
self-contradictory negatives, dated framing, meta-specs, enumeration drift), the frozen-contract +
`--skip-specs` archive discipline, and the standing-guard patterns.

Referenced by `definition-of-done` (spec is one done-dimension) and cross-linked with the sibling
standards skills. Repo-local dev tooling — **not** an export; conformance is auto-covered by
`tests/agent_skills_conform.rs`.

## Capabilities

### New Capabilities
- `spec-organization-skill`: the repository provides the `spec-organization` agent skill codifying the
  Gate-3 spec-audit doctrine.

### Modified Capabilities
_None._

## Impact

- **Skills:** new `.agents/skills/spec-organization/SKILL.md`. No product code.
- **Tests:** covered by the existing `tests/agent_skills_conform.rs` guard (now 7 skills) — no new test file.
- **Docs:** AGENTS.md gains a pointer under Spec-Driven Development. No mdBook change (dev tooling).
- **Consumed by** the worker at spec-author/archive time and the supervisor at the Gate-3 review (see
  `standards-skill-integration`). Pairs with `spec-traceability-audit` (produces the map this skill
  says to run) and `spec-consolidation` (executes the reorganization this skill governs).
- Not enum-variant ripple; no code touched.
