---
name: doc-completeness
description: Verify everything built is documented across git-paw's four doc layers (the Gate-4 doc audit). Use when a change adds or alters any user-facing surface — a CLI command or flag, a config field, a capability, or a module. Maps each change type to the layers it must update, requires the layers stay consistent and `mdbook build` passes, and gives the doc-audit checklist.
license: MIT
compatibility: git-paw
---

# Documentation Completeness

Use when a change adds or alters any user-facing surface. The Gate-4 doc audit fails a change whose
docs lag its behavior.

## The four doc layers (all must stay consistent)

1. **`--help`** — every command/flag has `about` + `long_about` + examples (`src/cli.rs`).
2. **README** — the landing page (CLI table, feature list, quick starts).
3. **mdBook** — the user guide under `docs/src/`, one chapter per feature.
4. **Rustdoc + configuration reference** — every public item `///`-documented; every config field in
   `docs/src/configuration/`.

## What each change type must update

- **New/changed CLI command or flag** → `--help` + README CLI table + the relevant mdBook chapter +
  `cli-reference`.
- **New/changed config field** → the configuration reference + a commented example in the init default.
- **New capability/feature** → a mdBook chapter (+ cross-link from the user-guide index) + a README
  feature entry.
- **Module/architecture change** → `docs/src/architecture.md`.
- **Removed command/flag** → purge it from **all** layers (no phantom references — e.g. the
  `git paw resume` / deprecated `--from-specs` cases).

## Rules

- All four layers agree; no layer contradicts another or the code.
- `mdbook build docs/` MUST pass.
- Machine surfaces (`--json`, the agent-friendly docs site) are the stable contract — keep them in step.

## Gate-4 review checklist

- [ ] Does this change alter a user-facing surface? If so, which layers does it touch?
- [ ] `--help` / README / mdBook / config-reference all updated and consistent?
- [ ] `mdbook build docs/` passes?
- [ ] No phantom or removed command/flag left referenced anywhere?

---

Repo-local dev skill; a consumer of git-paw applies its own doc standards.
