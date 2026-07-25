# Tasks — test-suite-consolidation

Full cluster inventory + per-file counts + the top-13 reconciliation table + the
W1-subsumed set + the needs-coverage-check list: `.git-paw/v0.13.0-test-consolidation-reaudit.md`.
Decision procedure for every cluster: `.agents/skills/test-strategy/SKILL.md`.

**Every wave is gated (D1):** record the pre-consolidation coverage baseline once, then at
each wave's gate run the requirement→test map and `just coverage` and confirm coverage ≥
baseline with no scenario at zero tests before the wave's PR merges. Waves land in order;
Wave 5 MUST NOT start until W1 (`cli-interaction-e2e`) is merged and green.

## 0. Baseline
- [ ] Record the pre-consolidation `just coverage` line-coverage number as the baseline for every subsequent gate
- [ ] Snapshot the requirement→test map (scenario → covering test(s)) as the before-image for the traceability diff

## 1. Safe first wave (~120–135, risk none/low, zero sole-guard risk, zero W1 overlap)
- [ ] `src/broker/messages.rs`: getter table (`agent_id_*` + `status_label_*`) + slugify table (drop `_deterministic` tautology) + advanced_main missing/blank + StatusPayload → table-driven (−33). **Grep the `BrokerMessage` variant set first** (cluster grew 14→16; ensure the table covers every variant)
- [ ] `src/error.rs`: delete the 18 Display-substring tests; **keep** the exit-code + hint-mapping assertions (−14)
- [ ] `src/supervisor/layout.rs`: `layout_for_N` table + drop `constants_have_expected_values` tautology (−8)
- [ ] `src/dashboard.rs`: `status_symbol` / `format_age` / `format_status_line` → tables (−13)
- [ ] `src/supervisor/{permission_prompt,manual_approvals}.rs`: classifier per-cargo/per-git table + `suggest_target` table (−13). Keep `claude_y_n_..._classifies_each_class`
- [ ] `src/cli.rs`: Start/Status/Purge/Stop flag tables + help-grep merge — **arg-parse only** (F6; not a prompt, not subsumed by W1) (−14)
- [ ] `src/config.rs`: default/parse-twin merges + enum-variant tables + ONE legacy fixture (the low-risk subset only; the bulk lands in Wave 2) (~−26)
- [ ] Gate: `just check` + `just deny` + coverage ≥ baseline + requirement→test map shows no scenario at zero tests

## 2. Config backward-compat consolidation (its own PR)
- [ ] Collapse per-`default()` + per-"section absent" twins into table-driven default/parse tests (F9)
- [ ] Replace the per-field "still parses" legacy batteries (`pre_v0_11_*`, `v0_5_0_config_without_layout_parses`, `v050_dashboard_section_without_broker_log_still_parses`) with ONE rich legacy fixture + a table of section-absent cases (F3)
- [ ] **Coverage-check:** confirm every pre-v0.11 / v0.5.0 config shape that a deleted test loaded still loads under the retained fixture/table (back-compat is a real contract)
- [ ] **Keep** the sole guard `governance_config_rejects_gates_field`
- [ ] Gate: `just check` + coverage ≥ baseline + back-compat configs still load

## 3. Unit→integration collapses (needs-coverage-check items; E2E serial, cold-start env)
- [ ] `src/broker/conflict.rs`: table-ify `regions_intersect_*` (7→15) **one row per normalization rule** — case / separator / trim / trailing-paren / leading-declaration-keyword / spelling-variant / distinct-symbols-stay-distinct. **Do NOT collapse to arithmetic cases** (§5.1); verify each rule keeps a row (−N, coverage-neutral)
- [ ] `tests/terminal_status_integration.rs`: delete the whole file + fold `delivery.rs` dups, **only after** confirming `delivery.rs::all_terminal_states_are_protected` + `committed_reenters_working_within_ttl` + `committed_stays_terminal_when_ttl_zero` remain as the sole guards of `terminal-status-protection` (keep one committed→working TTL guard) (−8)
- [ ] `tests/broker.rs`: trim the ~10 raw-TCP status-code dups onto the `server.rs` `oneshot` layer **after** confirming `server.rs` covers each code; **keep** `phantom_agents_cannot_appear_in_status` (sole `/status` non-appearance guard) (~−10)
- [ ] `dev_allowlist`: collapse unit vs integration (−5) **after** preserving the unrelated-field-preservation assertion from the unit `merges_with_existing_user_entries`
- [ ] `src/broker/delivery.rs`: broadcast/targeted routing tables (−8) + drop private-state reads (−6) **after** confirming the HTTP E2E (`http_publish_and_poll_verified_and_feedback`, `full_orchestration`) + `recent_messages_includes_all_types` guard the routing/log contracts
- [ ] **`cargo-mutants` spot-check** on the touched `delivery.rs` / `messages.rs` functions: retained tests still kill the mutants the deleted tests killed
- [ ] Gate: `just check` + coverage ≥ baseline + requirement→test map (broker scenarios) + mutants spot-check green

## 4. Impl-detail deletions + prose-pin rewrites (labor, not deletion count)
- [ ] Replace source-grep / brace-walk introspection tests in `src/main.rs` (purge/from-specs internals) with behavioral assertions — **replace, do not merely delete** (§ test-strategy anti-pattern)
- [ ] Rewrite the `*_skill_content` / prose-pin files to keyword-set / stable-anchor assertions, **never delete**: `skill_stuck_shapes_prose.rs` (6), `verify_at_tip_skill_content.rs` (5), `docs_agent_surface.rs` (5), `broker_sh_conventions.rs` (4), `spec_purpose_backfilled.rs` (1), `supervisor_routing_skill_content.rs`, and the ~123 `skills.rs` prose pins
- [ ] **Coverage-check (§5.7):** confirm each prose-only spec scenario retains ≥1 asserting test after the rewrite
- [ ] **Protect (do NOT touch):** the `sweep_sh_*` parity suite (52 tests / 9 files) guards the shipped bash artifact; only intra-file row merges preserving each spec § are allowed, and it is never cross-cut against `permission_prompt.rs` / `auto_approve.rs`
- [ ] Gate: `just check` + coverage ≥ baseline + every prose-only scenario still has a covering test

## 5. Post-W1 subsumed prompt tests (do NOT start until W1 is merged and green)
- [ ] Confirm `cli-interaction-e2e`'s PTY matrix is merged and green (precondition for this wave)
- [ ] Fold `tests/stop_confirmation_test.rs` (`stop_force_skips_prompt`, `stop_non_tty_skips_prompt`) into W1's destructive-confirmation matrix
- [ ] Fold `tests/init_interactive_specs.rs` (`..._records_chosen_spec_system_in_config`, `..._supervisor_choice_in_config`) into W1's init matrix
- [ ] Delete the source-grep `tests/cli_specs_tty_proceeds_to_picker.rs` (`bare_specs_on_tty_invokes_picker`) — W1 supplies the real picker-on-TTY behavioral test
- [ ] Delete the source-grep `tests/cli_from_specs_boot_block_failure.rs` (`boot_block_failure_is_non_fatal`) — W1 covers the `--from-specs` path
- [ ] `src/main.rs` purge-prompt family: keep the unmerged-warning **behavioral** assertions; do not retain duplicate prompt-shows unit tests W1 now covers
- [ ] `src/interactive.rs`: keep the `TrackingPrompter` stub tests as the fast deterministic layer; apply only the small `select_cli`/`default_cli`/`unknown_cli_from_any_source` merges now that W1 stabilises the PTY layer
- [ ] `tests/cross_format_spec_selection.rs`: cut the `bare_specs_in_non_tty_*` picker cells (W1 non-TTY exit path); **keep** the clap mutual-exclusion + alias cells
- [ ] Gate: `just check` + coverage ≥ baseline + no prompt scenario left without a W1 or unit guard

## 6. Verification (five gates, at the final DONE state)
- [ ] Gate 1 (testing) — `cargo test --no-fail-fast` green (the change's own restructured tests pass; `--no-fail-fast` so the env-guard test can't mask failures)
- [ ] Gate 2 (regression) — full suite green diffed against the merge-base, not a stale branch tip
- [ ] Gate 3 (spec audit) — requirement→test map: every OpenSpec scenario retains ≥1 covering test vs the Task-0 before-image; no SHALL/MUST left unguarded
- [ ] Gate 4 (doc audit) — `AGENTS.md` / CLAUDE.md testing conventions + the `test-strategy` skill still describe the realized state; `mdbook build docs/` passes (no docs churn expected)
- [ ] Gate 5 (security) — no exported-asset / agnosticism regression; the `sweep_sh_*` parity suite still guards the shipped bash artifact
- [ ] Coverage ≥ the Task-0 baseline across the whole change; `cargo fmt` before every commit
- [ ] `openspec validate test-suite-consolidation --strict` passes
