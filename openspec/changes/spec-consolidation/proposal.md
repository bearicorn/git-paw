## Why

`openspec/specs/` has grown to **97 capabilities**, roughly a third of which are
single-requirement residue of individual archived changes ("one change → one permanent
capability"), and several tight clusters (broker, supervisor, approval/allowlist, spec
backends, boot-block injection) are split far finer than any consumer reads them. The set
is *truthful* (the old `Purpose: TBD` crisis is fully resolved — 0/97) but **fragmented**,
and it carries a few frozen-contract errors. These specs ARE the contract v1.0.0 freezes,
so the set should be lean, coherent, and error-free first.

Critically, the docs are **mechanically coupled** to the spec directory layout: the
Specifications page (`docs/src/specifications/README.md`) `{{#include}}`s spec files by
path and links the rest by directory name. Any merge/rename breaks those includes and fails
`mdbook build`. So a docs restructure is **forced** by the consolidation — which makes this
the right moment to also fix the docs' own problems (a flat A–Z list of 89 GitHub links, an
arbitrary "8 foundational specs embedded" v0.1-era split, and a hand-maintained index that
drifts). Consolidating the specs and restructuring the Specifications page are one job.

## What Changes

- **Consolidate 97 → ~46 capabilities**, grouped by domain, **contract-preserving**: every
  SHALL/MUST requirement and WHEN/THEN scenario moves *verbatim* as a section under its
  merged capability. No requirement is dropped, weakened, or reworded except the two flagged
  reconciliations below. The traceability audit's requirement→test map is the safety net,
  run before and after each merge to prove no scenario lost its test.
- **Fix the frozen-contract errors:**
  - Rewrite the phantom `git paw resume` requirement/scenario in `add-branch` to `git paw
    start` (there is no `resume` subcommand — `start` resumes a paused session). **BREAKING
    for the contract text only**, not behavior.
  - Remove the superseded, self-contradictory negative requirement `learnings-mode` "No
    `agent.learning` broker variant in v0.5.0" (contradicted by `agent-learning-variant`).
  - Delete `test-coverage-v0-5-0` — a meta/bookkeeping spec with no product SHALL (the
    traceability audit owns scenario→test mapping now); also the only version-stamped name.
  - Editorial pass on dated "preserve v0.5.0 behaviour" framing (behavior unchanged).
- **Restructure the docs Specifications page** to a **domain-grouped, Purpose-led index**
  aligned 1:1 with the consolidated capabilities, replacing the flat A–Z list and the
  "foundational 8" split; keep `mdbook build` green as `{{#include}}` paths change; cross-link
  to the matching `user-guide/` chapters. Prefer a generated index (pairs with the
  `agent-friendly-docs-site` machine-readable surface) so it cannot drift again.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `add-branch`: rewrite the "Paused-session interplay" requirement + scenarios — `git paw
  resume` → `git paw start`.
- `learnings-mode`: remove the obsolete negative requirement "No `agent.learning` broker
  variant in v0.5.0" (reconciles the contradiction with `agent-learning-variant`).

The 97→~46 domain merges move requirements verbatim (capability renames/consolidations, not
requirement changes); they are enumerated in `tasks.md` and the full before→after map lives
in `.git-paw/v0.13.0-spec-consolidation-audit.md`. `test-coverage-v0-5-0` is removed entirely
(no product requirement).

## Impact

- **Specs:** `openspec/specs/` reorganised to ~46 capability directories (per the audit's
  31-row merge table + execution order). No code touched.
- **Docs:** `docs/src/specifications/README.md` (include paths + structure) and
  `docs/src/SUMMARY.md`; `mdbook build docs/` must pass (gate-4). Pairs with the `agent-docs`
  capability (a generated, domain-grouped index feeds `llms.txt`/sitemap).
- **Safety net:** the `spec-traceability-audit` requirement→test map is run before/after to
  prove no scenario lost coverage. This change SHOULD sequence alongside/after that audit.
- **Not enum-variant ripple / no code:** structural doc reorganisation only. The
  `learnings-mode` negative removal concerns a variant that already exists in code; no
  `BrokerMessage` set changes here.
