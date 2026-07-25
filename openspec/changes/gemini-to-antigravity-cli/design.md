# Design — gemini-to-antigravity-cli

## Context

Google is retiring Gemini CLI (`gemini`) in favour of Antigravity CLI (`agy`);
Gemini CLI stops serving free / AI Pro / AI Ultra users on 2026-06-18. git-paw's
auto-detect roster (`KNOWN_CLIS`) and its full-auto flag table both name `gemini`.
This is the last cycle before the v1.0.0 CLI freeze, so the roster must be correct
before it is frozen.

## Decisions

### D1 — Clean swap, not a deprecation alias

Remove `gemini` from `KNOWN_CLIS`; add `agy`. We do **not** keep `gemini` as a
hidden/deprecated auto-detect entry. Rationale: `KNOWN_CLIS` drives *auto-detection
only*. For the ~all-users cohort there is no `gemini` binary to detect after
2026-06-18, so keeping it detects nothing. The enterprise cohort (Gemini Code Assist
Standard/Enterprise) that retains a working `gemini` re-adds it as a `[clis.gemini]`
custom entry — a one-line config, and their explicit `default_cli`/session strings
already pass through untouched. Dropping it from auto-detection costs them nothing.

### D2 — Antigravity full-auto flag is `--dangerously-skip-permissions`, folded into Claude's arm

Antigravity's no-confirmation launch flag is `--dangerously-skip-permissions` (the
*same* flag Claude uses), **not** the Gemini `--yolo`. So this is not a `--yolo` key
rename. In `approval_flags`:
- drop `gemini` from the `("gemini" | "qwen", FullAuto) => "--yolo"` arm, leaving
  `("qwen", FullAuto) => "--yolo"`;
- add `agy` to Claude's arm: `("claude" | "agy", FullAuto) => "--dangerously-skip-permissions"`.

The retarget of `approval_flags_gemini_and_qwen_full_auto_are_yolo` splits into a
qwen-only `--yolo` assertion plus a new `agy` → skip-permissions assertion.

### D3 — Correct the `cli-detection` roster drift in this change

The `cli-detection` spec listed 7 CLIs; `KNOWN_CLIS` has drifted to 17. Since this
change edits that exact requirement for the freeze, it also aligns the requirement
and its scenario count with the real 17-entry roster (agy replacing gemini). This is
scoped in deliberately — a faithful delta cannot be written against a list that
already contradicts code, and a frozen v1.0.0 requirement must not be knowingly
wrong. Broader spec-drift cleanup remains the v0.13.0 spec-audit + consolidation
workstream's job.

### D4 — Extend the export-agnosticism guards, don't just rename

`src/skills.rs` and `src/supervisor/auto_approve.rs` use `.gemini` / `gemini` as
*negative* examples proving git-paw hard-codes no vendor CLI. Add `agy` / `.agents`
to those forbidden-token lists (keep the legacy `.gemini` negatives too) so the guard
covers the new name. Per `AGENTS.md`, exported assets stay project-agnostic.

### D5 — Courtesy hint (deferred, optional)

A "Gemini CLI is retiring (2026-06-18) — Antigravity CLI is `agy`" hint on detecting a
stale `gemini` reference (mirroring the existing `--from-specs` → `--from-all-specs`
guidance) is a nice-to-have. Deferred out of this change to keep it surgical; it would
add user-facing behavior needing its own scenario. Revisit if requested.

## Open items (out of scope here; needed for v1.0.0 per-CLI tables)

- Antigravity's **global-skills path** and its **`settings.json` allow/deny path** (the
  Claude-style trust/allowlist surface) are not named in the current sources.
- Confirm the display name and that `--dangerously-skip-permissions` is the correct
  full-auto launch flag against the official migration guide
  (<https://antigravity.google/docs/cli/gcli-migration>).

These block the v1.0.0 Hook-Provider / Trust-Folder / native-auto-mode / Broker-Curl
tables (each carries a Gemini row keyed on `.gemini/settings.json`), not this swap.

## Risks

- **Low.** Behaviour-preserving for explicit references; only auto-detection and the
  built-in flag table change. Existing configs/sessions naming `gemini` still resolve.
- The `KNOWN_CLIS` const is mirrored inline in two `detect.rs` tests — const and both
  tests must stay in sync (covered in tasks).
