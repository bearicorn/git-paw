# Tasks — gemini-to-antigravity-cli

## 1. Detection roster (`src/detect.rs`)
- [x] Replace `gemini` with `agy` in the `KNOWN_CLIS` const (17 entries; agy in gemini's position)
- [x] Ensure `derive_display_name("agy")` yields an acceptable label; if "Agy" is undesirable, add an explicit display-name mapping for `agy` → "Antigravity" (added `known_display_name` helper used by `detect_known_clis`)
- [x] Update the inline `KNOWN_CLIS` mirror in `all_known_clis_detected_when_present` to the new roster and assert 17 entries
- [x] Update the single-CLI detection test that uses `fake_path_with_binaries(&["gemini"])` to use `agy`
- [x] Add a test asserting `agy` detects with `binary_name = "agy"` and that `gemini` is NOT auto-detected (`agy_detected_and_gemini_not_known`)

## 2. Approval flags (`src/config.rs`)
- [x] In `approval_flags`, drop `gemini` from the `--yolo` arm (leave `("qwen", FullAuto) => "--yolo"`)
- [x] Add `agy` to Claude's arm: `("claude" | "agy", FullAuto) => "--dangerously-skip-permissions"`
- [x] Update the `approval_flags` doc-comment example (now asserts the `agy` → skip-permissions pairing)
- [x] Split `approval_flags_gemini_and_qwen_full_auto_are_yolo` into qwen `--yolo`, `agy` → skip-permissions, and `gemini` → `""` assertions (`approval_flags_qwen_yolo_agy_skip_permissions_gemini_empty`)

## 3. Export-agnosticism guards
- [x] `src/skills.rs`: add `agy` to the memory-isolation forbidden-vendor-token list (keeps existing `gemini` negative)
- [x] `src/supervisor/auto_approve.rs`: add `.agents` to the never-built-in product-dir list (keeps existing `.gemini` negative)
- [x] Confirmed no exported asset (`assets/**`, init default config, `sweep.sh`) hard-codes `agy`/`gemini` as always-safe (grep clean)

## 4. Docs
- [x] `src/cli.rs`: update the `--cli` `--help` example (`e.g., claude, codex, gemini` → `agy`)
- [x] README supported-CLI table + detected-CLI picker mock: gemini → Antigravity (`agy`)
- [x] mdBook: `supported-clis.md` (roster row), `configuration/README.md` (flags table + preset example), `introduction.md` (detected-CLI list). Narrative/illustrative `gemini`-as-example fixtures left untouched (still valid as an explicit pass-through / `[clis.gemini]` entry)
- [x] `mdbook build docs/` succeeds

## 5. Specs
- [x] Delta `specs/cli-detection/spec.md` authored (MODIFIED roster requirement)
- [x] Delta `specs/supervisor-config/spec.md` authored (MODIFIED permission-flag table)
- [x] `openspec validate gemini-to-antigravity-cli --strict` passes
- [ ] Standing guard: a test asserting `KNOWN_CLIS` stays in sync with the `cli-detection` spec roster — DEFERRED to spec-traceability-audit (owns the robust spec↔code roster guard); the swap-lock guard below covers the immediate gemini-reopen regression
- [x] Standing guard: a test that fails if `gemini` reappears as a built-in known CLI (`gemini_is_not_a_known_cli`) plus the config test asserting `gemini` has no built-in flag row

## 6. Verification (five gates)
- [x] Gate 1 — `cargo test --lib` for the change's own tests (detect 76 passed, approval_flags 12 passed, guards + swap-lock 5 passed, doctest 2 passed)
- [ ] Gate 2 — full regression suite green vs merge-base (change-level DONE verify; run serialised, not concurrent with other E2E)
- [x] Gate 3 — spec audit: both MODIFIED scenarios map to tests; no orphaned gemini requirement remains
- [x] Gate 4 — doc audit: `--help`, README, mdBook, config reference consistent; `mdbook build docs/` passes
- [x] Gate 5 — security: no secrets; export-agnosticism preserved (guards extended, nothing vendor-hard-coded)
- [x] `cargo fmt` + `cargo clippy --all-targets` clean before commit (full `just check` regression is the change-level DONE gate)

## Notes / deferred
- Courtesy hint on stale `gemini` reference (D5) — deferred; do not implement unless requested.
- Antigravity global-skills path + settings.json allow/deny path — resolve against the official migration guide; needed for v1.0.0 per-CLI tables, not this change.
- Cosmetic `gemini`-as-generic-string fixtures (`cli-selection`, `configuration`, `markdown-integration`, `openspec-integration`, `mcp-server`, `session.rs`, `tmux.rs`, `specs/*`) — left untouched; handled by the spec-audit + consolidation workstream if at all.
