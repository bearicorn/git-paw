## Context

`regions_intersect` (`src/broker/conflict.rs:123-183`) matches named regions by exact string equality per kind; differing named kinds fall to the `_ => {}` no-intersection arm. `classify_in_flight` (275-298) downgrades to `Additive` exactly when both sides declared regions and no pair intersects — so a same-symbol collision spelled two ways is downgraded to informational instead of escalating. The v0.9.0 dogfood hit this on shared skill assets (W15-22a).

## Goals / Non-Goals

**Goals:** spelling/kind variants of the same declared construct intersect; agents are taught to declare canonically and completely; the additive downgrade stays useful (genuinely disjoint functions still pass).
**Non-Goals:** source parsing (resolving names to line ranges), semantic overlap detection, W15-22b in-tool ordered merge-orchestration (stays v1.0.0), wire-format changes.

## Decisions

- **D1 — Normalize, don't fuzzy-match.** Deterministic normalization (case-fold, trim, separator collapse, strip trailing `()` and leading `fn`/`def`/`function`/`class`) equates spelling variants without similarity thresholds. Rationale: fuzzy scores create unexplainable verdicts; every normalization step maps to an observed declaration habit from the dogfood.
- **D2 — Named-vs-named across kinds intersects on normalized-name match.** `function foo` vs `block foo` is almost always the same construct declared through different lenses. Uses the existing conservative-hint pathway (`cross_kind = true`), so the warning is honest about certainty. Distinct-name cross-kind pairs still do NOT intersect — keeping the additive downgrade meaningful.
- **D3 — Guidance carries the rest.** Where two names are genuinely different strings for the same code, no string rule can catch it; the skill prose (canonical spelling, declare-all-touched, re-publish on scope growth) narrows declarations toward detector-comparable form. Same lever the fn-granularity change already uses.
- **D4 — Fix lands in `regions_intersect` itself** so forward-conflict AND in-flight `Additive`/`True` classification inherit it in one place.

## Risks / Trade-offs

- Slightly more conservative: normalization may equate genuinely distinct symbols differing only in case/separators (rare in one file); acceptable — cost is one informational-to-warning upgrade, vs a silent bad merge the other way.
- Skill-content pinning tests must be updated with the prose change (helper-migration ripple lesson).

## Migration Plan

None: no payload or config changes; stored intents re-evaluate under the new rules on the next detector pass.

## Open Questions

None.
