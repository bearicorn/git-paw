## Why

The suite has grown to **2449 tests** (1960 in `src/` unit modules, 489 across 80 files
under `tests/`) — **+411** since the v0.8.0 audit — and much of that growth is redundancy,
not coverage: one-test-per-{variant,field,flag} batteries (getters, slugify, config
defaults, clap flags), tautological `error.rs` Display-substring assertions, source-grep
introspection tests, brittle exact-substring prose pins on bundled assets, and unit tests
that only re-check a contract the integration/HTTP layer already guards. The re-audit
(`.git-paw/v0.13.0-test-consolidation-reaudit.md`) reconciles the original ~330-cut plan
against the current tree and identifies a **~330–345 net-reduction** opportunity with a
**~120–135-test safe first wave** that carries zero sole-guard risk.

This is the "apply" sibling of the already-authored `test-strategy` skill: the skill is the
decision procedure (delete / table-ify / collapse-to-integration / replace-with-e2e / keep /
protect-sole-guard); this change **applies it across the suite in coverage- and
mutation-gated waves**. v0.13.0 is a quality cycle before the v1.0.0 freeze — the right
moment to rebalance the pyramid (wide fast unit tables at the bottom, a thin behavioral cap)
without touching product behavior.

The consolidation is **behavioral-only** — it removes and restructures tests, never product
code. The risk it must never realize is a silent coverage drop, so every wave is gated on
coverage ≥ the pre-consolidation baseline and, for the risky broker cuts, a `cargo-mutants`
spot-check.

## What Changes

- **Rebalance, don't just trim.** Route every redundant cluster through the `test-strategy`
  decision procedure. One-per-{variant,field,flag} batteries become **table-driven** tests
  (one row per behavioral rule); tautologies are **deleted**; unit tests that only re-check a
  higher layer's contract **collapse to integration**; source-grep introspection tests are
  **replaced with behavioral tests**, never merely deleted.
- **Preserve every OpenSpec scenario's coverage.** No cut may leave a scenario with zero
  covering tests. The requirement→test map is run before and after each wave as the safety
  net; a removal that drops coverage on a real branch means a sole guard was cut — restore it
  (as a table row if needed).
- **Protect the sole guards explicitly:**
  - The `sweep_sh_*` parity suite (52 tests / 9 files) guards the **shipped bundled bash
    artifact** (`.git-paw/scripts/sweep.sh`) — a *different* artifact from the Rust
    classifier. It is kept; only intra-file row merges are allowed, each row preserving its
    spec §. It is never cross-cut against `permission_prompt.rs`/`auto_approve.rs`.
  - The `*_skill_content` / prose-pin tests (~21 new + `skills.rs` prose pins) are the sole
    guards of prose-only spec scenarios. They are **rewritten to stable-anchor / keyword-set
    assertions**, never deleted.
- **Reconcile with W1 (`cli-interaction-e2e`).** The ~10 prompt-surface tests that W1's PTY
  matrix subsumes are excluded from the safe first wave and sequenced **after W1 lands and is
  green**, so consolidation never removes a prompt guard before its PTY replacement exists.

This change adds no product behavior. It introduces one **new capability**,
`test-suite-hygiene`, capturing the enforceable invariants the consolidation must satisfy
(coverage-preserving, behavioral-only, table-driven clusters, coverage ≥ baseline).

## Capabilities

### New Capabilities
- `test-suite-hygiene`: the enforceable properties every consolidation wave must satisfy —
  scenario-coverage preservation (each OpenSpec scenario retains ≥1 covering test, verified
  against the requirement→test map), behavioral-only assertions, table-driven expression of
  one-per-{variant,field,flag} clusters, sole-guard protection for the `sweep_sh_*` parity
  suite and prose-pin tests, and coverage ≥ the pre-consolidation baseline at every wave gate.

### Modified Capabilities
_None._ No product behavior changes; no existing requirement is touched.

## Impact

- **Tests:** `src/` unit modules and `tests/*.rs` files across the clusters enumerated in
  `tasks.md` (getter/slugify/config/flag batteries → tables; `error.rs` Display deletions;
  unit→integration collapses; source-grep deletions + prose-pin rewrites). No product code
  changes.
- **No product code, no spec deltas to existing capabilities.** The delta is a single new
  `test-suite-hygiene` capability describing the hygiene invariants; no `openspec/specs/`
  product capability is modified.
- **Not enum-variant ripple:** no `BrokerMessage` / `SpecBackendKind` variant is added or
  removed. (F1's getter table-ify must still grep the `BrokerMessage` variant set before
  merging, per the CLAUDE.md enum-variant-ripple note — the cluster grew 14→16, suggesting a
  variant was added since the audit.)
- **Sequencing dependency on W1:** the final wave (subsumed prompt tests) MUST NOT start
  until the `cli-interaction-e2e` PTY matrix is merged and green.
- **Docs:** none user-facing. The `AGENTS.md` / CLAUDE.md testing conventions and the
  `test-strategy` skill already describe the target state; this change realizes it.
- **Safety net:** coverage ≥ baseline and the requirement→test map at every wave gate; a
  `cargo-mutants` spot-check on the risky broker `delivery.rs` / `messages.rs` cuts.
