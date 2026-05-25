## Why

v0.4 `--from-specs` is a single boolean flag with one mode: launch a worktree for every discovered spec. There's no way to launch a *subset* without manually editing the `[specs]` config or pre-removing entries. v0.5.0's parallel-by-default thesis (forward-coordination + conflict-detection make multi-worktree sessions safer) makes the "pick a subset" gap matter more, because users will routinely want to launch 3 of 8 discovered specs.

Two separate concerns push the redesign:

1. **Naming.** `--from-specs` reads as "from specs" (mode flag) but its actual behaviour is "from *all* specs." Users who first encounter it expect either a picker or to be able to name specific ones. Renaming the canonical flag to `--from-all-specs` makes the "all" intent explicit.
2. **Picker UX.** The branch picker (multi-select, exists at `src/interactive.rs::select_branches`) is the established pattern for "user picks a subset interactively." A spec picker fits the same shape.

This change adds the new surface, deprecates the old name with a graceful alias, and lays out the v1.0.0 removal plan. Discovery (`scan_specs`) is unchanged — the new flags filter discovered results downstream.

## What Changes

- **Rename canonical.** `--from-all-specs` is the new name for "launch every discovered spec." Behaviour is byte-for-byte identical to v0.4 `--from-specs`.
- **Hidden alias.** `--from-specs` continues to work at runtime as an alias for `--from-all-specs`. The alias is hidden from `--help`, the README, the mdBook, and v0.5.0 release notes. Existing v0.4 scripts and CI invocations keep working without source changes.
- **`--from-specs` removal in v1.0.0.** The alias is deprecated for one release cycle and removed in v1.0.0 alongside the CLI contract freeze. After removal, passing `--from-specs` SHALL produce a clear migration error pointing at `--from-all-specs`.
- **New `--specs` flag.** Comma-separated values matching the existing `--branches` syntax (`--specs add-auth,fix-session`):
  - `--specs` (no values, TTY available) → opens a multi-select picker showing every discovered spec. The picker entry for each spec includes a worktree-count hint (e.g. `003-user-list — 3 worktrees: 2 [P] + 1 phase/`) so users see the scope before committing.
  - `--specs` (no values, no TTY) → exits with an actionable error pointing at `--specs NAME[,NAME...]` and `--from-all-specs`.
  - `--specs NAME` or `--specs NAME1,NAME2` → narrows the session to the named specs. Comma-separated, mirroring the v0.4 `--branches feat/a,feat/b` convention.
- **Mutual exclusion.** `--from-all-specs` and `--specs` cannot be combined on the same invocation. Passing both SHALL exit with a clear "specify one or the other" error. clap's `conflicts_with` enforces this at parse time.
- **Spec name matching.** `--specs NAME` matches against the unit identifier per format:
  - Spec Kit: feature directory name (e.g. `003-user-list`). Numeric prefix prefix matches are allowed (`003` matches `003-user-list`) only when unambiguous; ambiguous matches error with the candidate list.
  - OpenSpec: change subdirectory name.
  - Plain Markdown: filename stem, OR the `paw_branch` frontmatter value when set (matching whichever the user supplied).
  Unknown names error with "spec '<name>' not found in discovered set; discovered: …".
- **Filtering downstream.** The narrowing is applied *after* `scan_specs()` returns; `spec-scanning` and the format backends are unchanged. The filter step sits in the start command's spec-mode dispatcher (`src/main.rs::cmd_start_from_specs` or its renamed successor).
- **Picker integration.** `src/interactive.rs` gains a `select_specs(specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError>` analogous to the existing `select_branches`. The picker shows one entry per `SpecEntry` (which already includes Spec-Kit-decomposed entries from `spec-kit-format` if that change is active) — the worktree-count hint is computed by grouping entries by feature.
- **Documentation policy.** v0.5.0 docs and `--help` text reference only `--from-all-specs` and `--specs`. The alias `--from-specs` is intentionally undocumented in v0.5.0 to nudge migration.
- **Existing flag interactions preserved.** `--cli`, `--dry-run`, `--force`, `--supervisor`, `--no-supervisor` continue to work with both `--from-all-specs` and `--specs` exactly as v0.4 worked with `--from-specs`.

Not in scope:
- Changing v0.4 behaviour for `--from-specs` on its own (it remains an exact alias of `--from-all-specs`).
- Renaming Markdown's `paw_status` field or flipping its semantics. Discovery rules for all three formats are unchanged.
- Any change to `scan_specs()` or the `SpecBackend` trait. Backends still return the full discovered set; filtering is downstream.
- Auto-completion for spec names. Could be a v1.0.0 nice-to-have alongside shell completions.
- Persisting picker selections across invocations.

## Capabilities

### New Capabilities
*(none — all changes extend existing capabilities)*

### Modified Capabilities
- `cli-parsing`: add `--from-all-specs` flag; add `--specs` flag with optional comma-separated values (matching the existing `--branches` syntax); keep `--from-specs` as a hidden alias of `--from-all-specs`; mutual-exclusion rule between `--from-all-specs` and `--specs`.
- `interactive-selection`: add spec multi-select picker (`select_specs`) with worktree-count hint per entry; cancellation propagates as `PawError::UserCancelled` matching the existing branch picker.

## Impact

**Code**:
- `src/cli.rs` — new flag definitions, alias wiring (`alias = "from-specs"` or hidden flag with the same effect), mutual-exclusion via clap `conflicts_with`. Parse path collapses `--from-specs` → same internal state as `--from-all-specs`.
- `src/main.rs` — the `cmd_start_from_specs` entry-point either keeps its name (treating both flags as paths into it) or is split into two helpers; either way, after `scan_specs()` returns, the dispatcher applies `--specs`-driven filtering, falls into the picker on bare `--specs`, or proceeds with the full set on `--from-all-specs`.
- `src/interactive.rs` — `select_specs` mirroring `select_branches`. Shape: `MultiSelect` with one item per spec, label including spec id and worktree-count hint; cancellation → `PawError::UserCancelled`.
- `src/error.rs` — possibly add `PawError::UnknownSpec { name, candidates }` and `PawError::PickerRequiresTty` (or extend an existing variant) for the new error paths. Reuse existing error variants where the message is the only thing changing.
- `assets/agent-skills/` — no changes; this is purely a CLI surface change.
- `docs/src/user-guide/start.md` (or wherever `git paw start --from-specs` is documented) — replace `--from-specs` references with `--from-all-specs` plus a section introducing `--specs` with picker example. Do NOT mention the `--from-specs` alias in user-facing docs.

**Tests**:
- CLI parse:
  - `--from-all-specs` parses to the canonical state.
  - `--from-specs` parses to the *same* canonical state (alias).
  - `--specs` (no values) parses to the picker state.
  - `--specs name1,name2` (comma-separated) parses to the narrow state with both names.
  - `--specs name1` (single value) parses to the narrow state with one name.
  - `--from-all-specs --specs name` errors with mutual-exclusion message.
  - `--from-specs --specs name` (alias + new flag) also errors with mutual-exclusion (alias must enforce the same rule).
- Spec filtering:
  - `--specs name` narrows the discovered set to one entry; unknown name errors with candidate list.
  - `--specs 003` (Spec Kit prefix) matches `003-user-list` when unambiguous.
  - `--specs 003 --specs 004` matches both unambiguously.
  - Ambiguous prefix match errors with candidate list and does NOT proceed.
- Picker:
  - `select_specs` shows worktree-count hint for Spec Kit features (3 = 2 `[P]` + 1 `phase/`).
  - Empty selection from picker → `PawError::UserCancelled`.
  - Non-TTY environment with bare `--specs` → exits with the no-TTY error before invoking dialoguer.
- Backward compatibility:
  - Every v0.4 test that exercises `--from-specs` continues to pass unchanged with the alias in place.
  - `--from-specs --force --dry-run` (the `start-force-flag` integration) behaves identically to `--from-all-specs --force --dry-run`.

**Backward compatibility**: full preservation. Existing scripts using `--from-specs` continue to work without changes. The flag is hidden from help to nudge migration but is fully functional. `--specs` is purely additive.

**Mismatch follow-ups (none new — already tracked under MILESTONE.md item 16)**.
