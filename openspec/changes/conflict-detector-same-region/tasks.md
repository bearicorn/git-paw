## 1. Detector

- [ ] 1.1 Add region-name normalization (case-fold, trim, separator collapse, strip trailing `()`, strip leading `fn`/`def`/`function`/`class`) and apply it in `regions_intersect`'s same-kind arm (`src/broker/conflict.rs`); unit tests: each variant class equates, distinct symbols do not
- [ ] 1.2 Replace the differing-named-kinds no-intersection arm: named-vs-named across kinds intersects when normalized names match, setting the conservative hint (`cross_kind`); tests: `function foo` vs `block foo` conflicts with hint, `function foo` vs `block bar` does not
- [ ] 1.3 Regression tests: existing non-overlap scenarios (distinct functions, disjoint ranges) still pass; `classify_in_flight` upgrades a normalized-match pair from `Additive` to `True`

## 2. Skill guidance

- [ ] 2.1 Extend coordination.md's region-declaration prose: canonical source spelling, declare ALL touched regions (shared constants/imports/assets), re-publish `agent.intent` on scope growth
- [ ] 2.2 Update pinned skill-content tests for the prose change

## 3. Docs

- [ ] 3.1 Coordination mdBook chapter: normalized matching + declaration rules
- [ ] 3.2 `mdbook build docs/` passes
