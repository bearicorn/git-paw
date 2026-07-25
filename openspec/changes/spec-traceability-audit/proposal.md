## Why

The `openspec/specs/` set is the contract v1.0.0 freezes, but ~18 requirements have **drifted
behind intentional, tested code changes** — the spec lags the code. Auditing the whole merged
set for truth before the freeze is gate-3 of the five-gate framework applied across the entire
contract. It also produces the **requirement→test map** that proves `spec-consolidation`'s 97→46
merges drop no scenario's coverage — so it is the safety-net sibling that runs first.

## What Changes

- **Trace** every SHALL/MUST → implementing code and every WHEN/THEN scenario → a covering test
  across the audited capabilities; classify each `OK` / `no-impl` / `contradicts` / `no-test` /
  `orphan`.
- **Reconcile the ~18 contradictions** by amending specs to match the shipped, tested behavior
  (the code is the tested truth). Enumerated in `tasks.md`; **seeded here** with the
  `approval-configuration` `safe`-preset wording fix. One reconciliation is a genuine decision
  (the `cli-parsing` Stop prompt) — flagged, not silently chosen.
- **Fill the real scenario→test gaps** as test tasks (the ~40 `no-test` are mostly TUI/live-TTY
  coverage-exempt; a handful are genuine).
- **Add ~7 cheap standing guards** (modeled on `tests/spec_purpose_backfilled.rs`) so the closed
  gaps can't silently reopen: `KNOWN_CLIS`↔spec sync, `--specs-format` value completeness,
  a no-`.specify/`-auto-detection prose guard, a gemini-as-current-CLI guard, …
- **Produce and maintain the requirement→test map**, run before AND after each `spec-consolidation`
  merge wave.

## Capabilities

### New Capabilities
_None._ (The standing guards are tests, not a capability.)

### Modified Capabilities
- `approval-configuration`: fix the `safe`-preset scenario — the effective whitelist is the
  *composed* defaults (built-ins + resolved dev-allowlist), not "defaults only," matching the
  composition model its own tests assert. **(seed delta)**

The remaining reconciliations (`cli-parsing` Stop, `supervisor-launch` prompt injection,
`message-delivery` roster upsert ×3, `broker-endpoints` cursor ×2, the `.specify/` residues) land
as MODIFIED deltas during apply — enumerated in `tasks.md`. The `cli-detection` roster (gemini),
phantom `resume`, and `learnings-mode` negative are owned by `gemini-to-antigravity-cli` /
`spec-consolidation`; this change verifies they're closed, not duplicated.

## Impact

- **Specs:** amend the drifted requirements in `openspec/specs/*`; no product code change.
- **Tests:** new guard tests under `tests/` (like `spec_purpose_backfilled.rs`); test tasks for the
  genuine scenario→test gaps.
- **Pairs with `spec-consolidation`** (the map is its safety net; run this first/alongside) and
  **feeds `test-suite-consolidation`** (its gaps become test tasks). Sequenced first in the
  specs-before-tests-before-code order.
