## Why

W15-22a: the fn-granularity conflict detector can miss two agents editing the SAME region — the false-negative complement of the v0.9.0 additive-vs-true work. Region intersection compares opaque name strings for exact equality (`src/broker/conflict.rs:123-183`), so `function validate_token` vs `function Validate Token` (or `validate_token()`) never intersect, and a named `function foo` vs a named `block foo` fall through the differing-kinds arm to no-intersection. Under-declared or inconsistently-spelled regions turn a true same-region collision into an `Additive` downgrade — a silent bad merge in an unattended run.

## What Changes

- **Detector tweak (normalization)**: region-name comparison normalizes before equality — case-fold, trim, collapse separator differences (space/underscore/hyphen), strip a trailing `()` and a leading declaration keyword — so spelling variants of the same symbol intersect.
- **Detector tweak (named cross-kind)**: named-vs-named comparisons across different kinds (`function`/`class`/`block`) with matching normalized names intersect conservatively, with the same "conservative comparison" hint the named-vs-range rule carries.
- **Declaration audit (guidance)**: the coordination skill's region-declaration prose additionally requires the canonical symbol spelling as it appears in source, declaring ALL touched regions (including shared constant/import blocks), and re-publishing `agent.intent` when scope grows mid-task.
- The in-flight `Additive`/`True` classification inherits the fix automatically (it calls the same intersection).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `conflict-detector-fn-granularity`: MODIFY "Region-aware forward-conflict detection" (normalized name matching + named cross-kind conservative intersection) and "Coordination skill teaches region declaration" (canonical spelling, declare-all-touched, re-publish on scope growth).

## Impact

- `src/broker/conflict.rs`: `regions_intersect` normalization + the differing-named-kinds arm; `classify_in_flight` benefits transitively.
- `assets/agent-skills/coordination.md`: region-declaration prose — ⚠ pinned by skill-content tests; update alongside.
- Tests: normalization variants, named cross-kind, existing non-overlap scenarios stay green (distinct symbols still pass).
- No wire-format, config, or message changes; `Region`/`FileIntent` payloads unchanged.
