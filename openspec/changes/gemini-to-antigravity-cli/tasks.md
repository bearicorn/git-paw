# Tasks — gemini-to-antigravity-cli

## 1. Detection roster (`src/detect.rs`)
- [ ] Replace `gemini` with `agy` in the `KNOWN_CLIS` const (17 entries; agy in gemini's position)
- [ ] Ensure `derive_display_name("agy")` yields an acceptable label; if "Agy" is undesirable, add an explicit display-name mapping for `agy` → "Antigravity" (match how other CLIs derive/override display names)
- [ ] Update the inline `KNOWN_CLIS` mirror in `all_known_clis_detected_when_present` to the new roster and assert 17 entries
- [ ] Update the single-CLI detection test that uses `fake_path_with_binaries(&["gemini"])` to use `agy`
- [ ] Add a test asserting `agy` detects with `binary_name = "agy"` and that `gemini` is NOT auto-detected (covers the new cli-detection scenario)

## 2. Approval flags (`src/config.rs`)
- [ ] In `approval_flags`, drop `gemini` from the `--yolo` arm (leave `("qwen", FullAuto) => "--yolo"`)
- [ ] Add `agy` to Claude's arm: `("claude" | "agy", FullAuto) => "--dangerously-skip-permissions"`
- [ ] Update the `approval_flags` doc-comment example (currently asserts the gemini `--yolo` pairing)
- [ ] Split `approval_flags_gemini_and_qwen_full_auto_are_yolo` into: a qwen-only `--yolo` assertion, a new `agy` → `--dangerously-skip-permissions` assertion, and a `gemini` → `""` (no built-in row) assertion

## 3. Export-agnosticism guards
- [ ] `src/skills.rs`: add `agy` / `.agents` to the forbidden-vendor-token list (keep existing `.gemini` negatives)
- [ ] `src/supervisor/auto_approve.rs`: same — extend the negative-example token list to cover `agy` / `.agents`
- [ ] Confirm no exported asset (`assets/**`, init default config, `sweep.sh`) hard-codes `agy` as always-safe (must stay project-agnostic per `AGENTS.md`)

## 4. Docs
- [ ] `src/cli.rs`: update the `--cli` `--help` example (`e.g., claude, codex, gemini` → `agy`)
- [ ] README supported-CLI table: gemini → Antigravity (`agy`)
- [ ] mdBook: `docs/src/supported-clis.md`, `configuration/`, `architecture.md`, quick-start pages — supported-CLI tables and any `.gemini` examples
- [ ] `mdbook build docs/` succeeds

## 5. Specs
- [ ] Delta `specs/cli-detection/spec.md` authored (MODIFIED roster requirement) — done
- [ ] Delta `specs/supervisor-config/spec.md` authored (MODIFIED permission-flag table) — done
- [ ] `openspec validate gemini-to-antigravity-cli --strict` passes
- [ ] Standing guard: a test asserting `KNOWN_CLIS` stays in sync with the `cli-detection` spec roster (per the spec-traceability audit) so the 7-vs-17 drift cannot silently reopen
- [ ] Standing guard: a test that fails if `gemini` reappears as a built-in known CLI or a built-in flag-table row (locks the swap)

## 6. Verification (five gates)
- [ ] Gate 1 — `cargo test --no-fail-fast` for the change's own tests (detect, config)
- [ ] Gate 2 — full regression suite green vs merge-base
- [ ] Gate 3 — spec audit: both MODIFIED scenarios map to tests; no orphaned gemini requirement remains
- [ ] Gate 4 — doc audit: `--help`, README, mdBook, config reference consistent; `mdbook build docs/` passes
- [ ] Gate 5 — security: no secrets, export-agnosticism preserved (guards extended, nothing vendor-hard-coded)
- [ ] `just check` green (fmt + clippy + tests); `cargo fmt` run before commit

## Notes / deferred
- Courtesy hint on stale `gemini` reference (D5) — deferred; do not implement unless requested.
- Antigravity global-skills path + settings.json allow/deny path — resolve against the official migration guide; needed for v1.0.0 per-CLI tables, not this change.
- Cosmetic `gemini`-as-generic-string fixtures (`cli-selection`, `configuration`, `markdown-integration`, `openspec-integration`, `mcp-server`, `session.rs`, `tmux.rs`, `specs/*`) — left untouched; handled by the spec-audit + consolidation workstream if at all.
