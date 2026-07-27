# Tasks — spec-traceability-audit

Full findings: `.git-paw/v0.13.0-spec-traceability-audit.md` (this IS the requirement→test map).
Run first in the specs→tests→code order; the map is `spec-consolidation`'s safety net.

## 1. Reconcile the spec-vs-code contradictions (amend spec to match shipped, tested code)
- [x] `approval-configuration` `safe` preset → composed defaults (built-ins + resolved dev-allowlist), no user extras (delta authored)
- [x] `cli-parsing` Stop subcommand — RESOLVED (A, owner 2026-07-25): `stop` is non-destructive → no prompt; `--force` kept accepted-but-inert + documented no-op. Delta authored; matches `cli-interaction-e2e`
- [x] `supervisor-launch` — amended the "Initial prompt injection" requirement + 3 scenarios to the shipped sidecar model (`build_task_prompt` points every arm at `.git-paw/AGENTS.local.md`; OpenSpec→`/opsx:apply {id}`, Markdown/SpecKit→`openspec/changes/{id}`, Superpowers→plan pointer, None→verbatim fallback). Scenarios map to `main::tests::task_prompt_*`
- [x] `message-delivery` — MODIFIED "Publish updates sender's agent record" (scoped to Status/Artifact/Blocked/Intent); REMOVED the two contradictory "Agent record updated for {new types,Question}" reqs; ADDED "Roster updates exclude identity and routing variants" (Verified/Feedback/Answer/Question never upsert). Scenarios map to the four `broker::delivery::tests::*_does_not_mutate/create_*` tests
- [x] `broker-endpoints` — MODIFIED "GET /messages/:agent_id" to hold-the-cursor-at-`since` on empty (not reset to 0), reconciling it with `message-delivery`'s monotonic-cursor requirement. `since==latest` scenario → `broker::delivery::tests::poll_since_latest_holds_cursor_at_since`
- [x] `cli-parsing` `--specs-format` help/doc: scrubbed the stale `.specify/` filesystem-auto-detection prose and added `superpowers` to the value list + `help=` string (gate-4 `--help` fix). **DEFERRED (product code, out of this change's "no product code change" Impact):** the `src/mcp/query/specs.rs::resolve_dir` `.specify/` probe removal → owned by `code-analysis-refactor`; the audit confirms it is near-inert (authoritative `scan_specs` returns empty when unconfigured)
- [ ] `--from-specs` deprecated alias spec reconciliation → **DEFERRED to `spec-consolidation`** (pure spec-text; the `--help` surface already does not present `--from-specs` as current, and `--from-all-specs` is the canonical flag in `cli-interaction-e2e`)
- [x] Verify (don't duplicate) sibling-owned fixes: `cli-detection` roster (gemini) is closed by `gemini-to-antigravity-cli` and locked by `detect::tests::gemini_is_not_a_known_cli`; phantom `git paw resume` + `learnings-mode` negative are verified at `spec-consolidation`

## 2. Fill genuine scenario→test gaps
- [ ] **DEFERRED to `test-suite-consolidation`**: the ~40 `no-test` scenarios are triaged in the findings file (mostly TUI-draw/live-TTY coverage-exempt). The genuine gaps (`mcp/query/{session,conflicts,intents}` outputs, get_constitution, SpecKit get_tasks) become test tasks there rather than duplicating the mcp harness here

## 3. Standing guards (model on `tests/spec_purpose_backfilled.rs`)
- [ ] `KNOWN_CLIS` ↔ `cli-detection` spec roster sync → **DEFERRED (archive-ordering)**: the main `cli-detection` spec is stale (7/"8") until `gemini-to-antigravity-cli`'s delta archives; a CI guard reading the main spec would fail pre-archive. Re-add post-archive (or in `spec-consolidation`, which touches the main roster). The code-side lock already exists (`gemini_is_not_a_known_cli`)
- [x] `--specs-format` value-list completeness → `cli::tests::specs_format_help_lists_every_variant` (iterates every `SpecsFormat` variant, asserts each appears in the `--specs-format --help`; caught `superpowers` missing)
- [ ] no-`.specify/`-auto-detection prose guard → **DEFERRED**: pairs with the `resolve_dir` code-path removal (still present), so a prose guard would flag a known-tracked surface; lands with `code-analysis-refactor`
- [x] gemini-as-current-CLI guard → `detect::tests::gemini_is_not_a_known_cli` (added under `gemini-to-antigravity-cli`; arms the agy swap)
- [x] `build_task_prompt` sidecar-invariant → `main::tests::every_build_task_prompt_arm_points_at_the_sidecar` (covers the previously-untested `Superpowers` arm; locks C3)
- [ ] `BrokerMessage` variant-count + `upserts_roster` set guards → **DEFERRED**: the "seven variants" envelope-count staleness is a `broker-messages` fix owned by `spec-consolidation`; the roster-decision set is already covered by the per-variant behavioral tests + the AGENTS.md enum-variant-ripple checklist

## 4. Requirement→test map + verification
- [x] Requirement→test map produced across the audited capabilities → `.git-paw/v0.13.0-spec-traceability-audit.md`
- [ ] Re-run the map before AND after each `spec-consolidation` merge wave → **process task carried to `spec-consolidation`** (no scenario loses its covering test)
- [x] Gate-3 self-check: every amended/added requirement carries a `Test:` pointer to a confirmed existing test (verified the four `delivery` negatives, `poll_since_latest_holds_cursor_at_since`, and the `task_prompt_*` tests all exist)
- [x] `openspec validate spec-traceability-audit --strict` passes; `cargo fmt` + `clippy --all-targets` clean (full `just check` regression is the change-level DONE gate)
