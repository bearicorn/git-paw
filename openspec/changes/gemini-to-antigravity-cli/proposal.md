## Why

Google is retiring **Gemini CLI** and transitioning to **Antigravity CLI** (`agy`).
Gemini CLI stops serving free / AI Pro / AI Ultra users on **2026-06-18**, so for
almost every git-paw user there is no `gemini` binary left to auto-detect. v0.13.0
is the last cycle before the v1.0.0 CLI freeze, so the detected-CLI roster must be
correct *before* the freeze locks it in — we should not freeze a soon-dead entry.

This is not a frozen-contract break: `KNOWN_CLIS` only drives *auto-detection*. An
explicit `default_cli = "gemini"`, a `[clis.gemini]` custom entry, or a saved-session
`cli` string still passes through untouched, so the enterprise cohort (Gemini Code
Assist Standard/Enterprise, which keeps a working `gemini` past that date) loses
nothing — they re-add it as a custom CLI.

## What Changes

- **Swap the auto-detect roster** (`src/detect.rs::KNOWN_CLIS`): remove `gemini`,
  add `agy` (Antigravity CLI). Display name "Antigravity".
- **Retarget the full-auto flag mapping** (`src/config.rs::approval_flags`): Antigravity
  uses `--dangerously-skip-permissions` (the *same* flag as Claude), **not** `--yolo`.
  Fold `agy` into Claude's arm; drop `gemini` from the `--yolo` arm, leaving
  `("qwen", FullAuto) => "--yolo"`.
- **Correct the `cli-detection` roster requirement to match code.** The spec listed 7
  CLIs (`claude, codex, gemini, aider, vibe, qwen, amp`) while `KNOWN_CLIS` has drifted
  to 17. Because this change edits that exact requirement for the freeze, it also aligns
  the requirement (and its scenario's "8 known" count) with the real 17-entry roster.
  *(Scoped into this change deliberately — writing a faithful delta is impossible
  against a list that already contradicts code.)*
- **Extend the export-agnosticism guards** (`src/skills.rs`, `src/supervisor/auto_approve.rs`):
  add `agy` / `.agents` to the forbidden-vendor-token lists so the "git-paw hard-codes no
  vendor CLI" guards cover the new name too (alongside the existing `.gemini` negatives).
- **Doc accuracy:** `--help` example in `src/cli.rs`, README + mdBook supported-CLI
  tables, and illustrative `gemini` mentions in reference docs.
- **Not in scope (optional cosmetic follow-ups):** the many specs/tests that use
  `"gemini"` as a *generic explicit CLI string* fixture (`cli-selection`, `configuration`,
  `markdown-integration`, `openspec-integration`, `mcp-server` example lists,
  `session.rs`/`tmux.rs`/`specs/*` fixtures). These still work — an explicit `gemini`
  reference is a pass-through, not an auto-detected entry — so they are left untouched to
  keep the change surgical. Noted for the v0.13.0 spec-traceability workstream.

No **BREAKING** changes: auto-detection is not part of the frozen contract, and explicit
references pass through unchanged.

## Capabilities

### New Capabilities
_None._

### Modified Capabilities
- `cli-detection`: the "Auto-detect known AI CLIs on PATH" requirement changes its known
  roster (remove `gemini`, add `agy`) and is corrected to the true 17-entry list.
- `supervisor-config`: the "Permission flag mapping" requirement changes its built-in
  table (remove the `gemini → --yolo` row, add `agy → --dangerously-skip-permissions`)
  and the corresponding scenario.

## Impact

- **Code:** `src/detect.rs` (`KNOWN_CLIS` const + its inline mirror in
  `all_known_clis_detected_when_present` and the single-CLI detection test
  `fake_path_with_binaries(&["gemini"])`); `src/config.rs` (`approval_flags` match arm,
  its doc-comment example, and the `approval_flags_gemini_and_qwen_full_auto_are_yolo`
  test → retarget to qwen-only + a new `agy` case); `src/cli.rs` (`--help` example);
  `src/skills.rs` + `src/supervisor/auto_approve.rs` (forbidden-token guard lists).
- **Docs:** README supported-CLI table; mdBook `docs/src/supported-clis.md`,
  `configuration/`, `architecture.md`, quick-start pages; `mdbook build docs/` must pass.
- **Specs:** delta files for `cli-detection` and `supervisor-config` (this change).
- **Not enum-variant ripple:** `KNOWN_CLIS` is a plain `&[&str]` const, not `BrokerMessage`
  or `SpecBackendKind` — no exhaustive-match ripple.
- **Open items to resolve against the official migration guide when implementing:**
  Antigravity's global-skills path and its `settings.json` allow/deny path (the
  Claude-style trust surface) are not named in the current sources — needed for the
  v1.0.0 per-CLI feature tables, not for this roster swap. Confirm `agy`'s display name
  and that `--dangerously-skip-permissions` is the launch full-auto flag.
