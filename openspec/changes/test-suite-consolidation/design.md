# Design — test-suite-consolidation

## Context

2449 tests (1960 unit + 489 integration across 80 files), +411 since the v0.8.0 audit.
Growth is largely redundancy: per-variant/field/flag batteries, tautological Display
assertions, source-grep introspection, brittle prose pins, and unit tests duplicating a
higher layer's guard. The reconciled plan (`.git-paw/v0.13.0-test-consolidation-reaudit.md`)
targets a ~330–345 net reduction with a ~120–135-test safe first wave. This change is the
"apply" sibling of the `test-strategy` skill (`.agents/skills/test-strategy/SKILL.md`),
which is the decision procedure this change executes. Full cluster inventory, per-file
counts, the top-13 reconciliation table, the W1-subsumed set, and the needs-coverage-check
list live in the re-audit.

## Decisions

### D1 — Coverage-preserving by construction; the requirement→test map is the gate

"Reduce redundancy, never reduce coverage." Every wave runs the requirement→test map and
`just coverage` before and after. If a scenario loses its covering test, or coverage drops
below the pre-consolidation baseline, a sole guard was cut — restore it (as a table row if
needed). This is the same discipline the `spec-consolidation` change applies to specs,
transposed to tests: the invariant, not the diff, is what we verify.

### D2 — Route every cluster through the `test-strategy` decision procedure

Each redundant cluster maps to exactly one skill outcome:
- **delete** — tautologies (`derive` works, a getter round-trips, `error.rs` Display
  substrings, `constants_have_expected_values`).
- **table-ify** — one-per-{variant,field,flag} batteries → one table test, one row per
  behavioral rule. A new rule adds a *row*, not a *test*.
- **collapse to integration** — a unit test that only re-checks a contract the
  integration/HTTP layer already guards (e.g. `terminal_status_integration.rs`,
  broker raw-TCP status-code dups vs the `server.rs` `oneshot` layer).
- **replace with e2e** — source-grep / brace-walk introspection tests → a real behavioral
  test (W1's PTY matrix supplies these for the interactive surface).
- **keep as unit** — fast pure-logic tables; the right tool, leave them.
- **protect as sole guard** — `sweep_sh_*` parity, prose-only scenarios; rewrite to stable
  anchors if brittle, never delete.

### D3 — Table-ify preserves one row per behavioral rule (not per copy)

The trap the original audit fell into: `regions_intersect_*` (conflict.rs) grew 7→15, and
the 8 new tests are **distinct normalization rules** (case / separator / trim / trailing-paren
/ leading-declaration-keyword / spelling-variant folding, distinct-symbols-stay-distinct),
not copies. Table-ifying is correct; **collapsing to "2 arithmetic cases" would drop real
coverage**. Rule: after any table-ify, verify each pre-existing behavioral rule still has a
row. Same for the F1 getter cluster — grep the `BrokerMessage` variant set first (it grew
14→16; a variant was likely added) so the table covers every variant.

### D4 — Protect the two sole-guard classes by construction

- **`sweep_sh_*` (52 tests / 9 files)** guards the *shipped bash artifact*
  (`.git-paw/scripts/sweep.sh`), which is a **different artifact** from the Rust
  auto-approve classifier. `sweep_sh_classify.rs`'s own docstring says it "verifies parity
  with the Rust auto-approve classifier" — that parity is the point, not a duplication.
  Only **intra-file** row merges are allowed, and every row maps to a classifier /
  stuck-detection spec §; it is never cross-cut against `permission_prompt.rs` /
  `auto_approve.rs`.
- **`*_skill_content` / prose-pin tests** are the sole guards of prose-only spec scenarios
  (97 spec.md files, many prose-only). They are **rewritten to keyword-set / stable-anchor
  assertions** (a required key, a command name, a structural marker), never deleted. This is
  **labor, not deletion count** — it adds to Wave 4's work while keeping every guarded
  scenario covered.

### D5 — W1 (`cli-interaction-e2e`) reconciliation: defer the subsumed prompt tests to last

W1 adds a PTY matrix asserting which prompts show, in what order, and which are bypassed
across `init` / `start` / `start --from-specs` / `stop` / `purge`. The ~10 tests it subsumes
(`stop_confirmation_test.rs`, `init_interactive_specs.rs`, the two source-grep files
`cli_specs_tty_proceeds_to_picker.rs` / `cli_from_specs_boot_block_failure.rs`, the purge
prompt-shows half, the `interactive.rs` prompt-sequencing merges, the cross-format picker
cells) are **excluded from the safe first wave** and cut only **after W1's PTY matrix is
merged and green**. This guarantees neither workstream removes a prompt guard the other has
not yet replaced, and neither double-counts the same test. Argument *parsing* tests (F6's
clap flag/help tables) are NOT prompts — W1 does not touch them — so they proceed in the
safe first wave regardless.

### D6 — Gate the risky broker cuts with `cargo-mutants`

The `delivery.rs` broadcast/targeted routing tables (−8, medium risk) and `messages.rs`
cuts are the highest-risk broker cuts. Before finalizing them, run a `cargo-mutants`
spot-check on the touched functions to confirm the retained tests still kill the mutants the
deleted tests killed. Verify the HTTP E2E (`http_publish_and_poll_verified_and_feedback`,
`full_orchestration`) and `recent_messages_includes_all_types` guard the routing/log
contracts before cutting.

## Execution order (lowest-risk first — from the re-audit)

Waves are independent PRs; each is gated on `just check` + `just deny` + coverage ≥ baseline
+ the requirement→test map (D1). Ordered so risk climbs only after the safe base lands.

1. **Safe first wave (~120–135):** pure table merges + tautological Display deletions. No
   prompt tests, no prose pins, no unit→integration collapses, no W1 overlap. Every cut
   leaves ≥1 behavioral guard.
2. **Config backward-compat (Wave 2):** default/parse-twin merges, ONE rich legacy fixture
   vs per-field "still parses" batteries. Keep the sole-guard `governance_config_rejects_gates_field`.
3. **Unit→integration collapses (Wave 3):** the needs-coverage-check items — region
   table-ify, `terminal_status_integration.rs` delete (keep the TTL guard somewhere),
   broker raw-TCP trim (keep `phantom_agents_cannot_appear_in_status`), `dev_allowlist`
   unit-vs-integration, `delivery.rs` broadcast tables + private-state reads. Coverage open,
   E2E serial, cold-start env; `cargo-mutants` spot-check on the broker cuts.
4. **Impl-detail deletions + prose-pin rewrites (Wave 4):** source-grep `main.rs`
   introspection tests replaced with behavioral tests; all `*_skill_content` / prose-pin
   files rewritten to stable anchors (labor, not deletion). Confirm each guarded scenario
   keeps ≥1 asserting test.
5. **Post-W1 subsumed prompt tests (Wave 5):** the ~10 §4 prompt tests, cut only after
   `cli-interaction-e2e`'s PTY matrix is merged and green.

## Risks

- **Silent coverage drop on a cut** — the primary risk; mitigated by the before/after
  requirement→test map and coverage ≥ baseline gate (D1). A drop means a sole guard was cut;
  restore it.
- **Mis-flagging a sole guard as a duplicate** — the `sweep_sh_*` parity suite and prose
  pins look redundant but are not (D4); protected by construction and by the
  needs-coverage-check list.
- **Over-collapsing a real-rule table** (the `regions_intersect` trap, D3) — mitigated by
  the "one row per rule, verified after merge" rule.
- **Removing a prompt guard before W1 replaces it** — mitigated by deferring the entire
  subsumed set to Wave 5, post-W1-green (D5).
- **A broker routing regression a coarse coverage number misses** — mitigated by the
  `cargo-mutants` spot-check on the risky broker cuts (D6).
