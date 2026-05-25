## Context

`git paw start` has two existing skip-picker flags:

- `--branches feat/a,feat/b` — comma-separated branch names; bypasses the branch picker.
- `--from-specs` — boolean; runs `cmd_start_from_specs`, which calls `scan_specs()` and proceeds with the full discovered set, no picker.

The branch picker (`src/interactive.rs::select_branches`) uses `dialoguer::MultiSelect`. The spec picker doesn't exist yet — `cmd_start_from_specs` has no "pick a subset" path.

This change adds:
1. A new spec multi-select picker that mirrors `select_branches`.
2. A new `--specs` flag with three modes (no values + TTY → picker; no values + no TTY → error; with values → narrow).
3. A canonical name `--from-all-specs` for the v0.4 "launch all" behaviour.
4. A hidden alias `--from-specs` preserving v0.4 invocations through the v0.5.0 cycle, removed in v1.0.0.
5. Mutual exclusion between `--from-all-specs` and `--specs`.
6. A spec-name resolver that maps each `--specs` value to a `SpecEntry` produced by `scan_specs()`, with format-aware matching rules.

The big design decisions are: how to spell the alias in clap, how the picker presents `SpecEntry` values that may be Spec-Kit-decomposed, and how to handle TTY vs. non-TTY at the bare-`--specs` boundary.

## Goals / Non-Goals

**Goals:**
- Mirror `--branches`'s syntactic shape (comma-separated, single flag, single internal `Vec<String>`) for `--specs`.
- Mirror `--branches`'s behavioural shape (skip picker, proceed directly) for `--specs NAME[,NAME...]`.
- Keep `scan_specs` and the format backends untouched. Filtering happens in the start-command dispatcher between `scan_specs` and worktree creation.
- Keep the deprecated `--from-specs` alias *invisible* — no warning to stderr, no help-text mention. The user's existing scripts behave identically; the migration nudge is via release notes only.
- Resolve unknown `--specs` names with a candidate list so the user can correct quickly. No silent fallthrough to branch mode.
- Detect non-TTY before invoking the picker so we never hang dialoguer on a script.
- Keep all v0.4 flag interactions (`--cli`, `--dry-run`, `--force`, `--supervisor`, `--no-supervisor`) working with both `--from-all-specs` and `--specs`.

**Non-Goals:**
- Changing `--branches` behaviour or syntax.
- Auto-creating spec scaffolds when a name doesn't exist (out per v0.5.0 non-goal: "git-paw doesn't run /speckit.specify").
- Persisting picker selections across runs.
- Shell completions for spec names. v1.0.0 owns shell completions; this change just makes the names exist.
- Format-mixing in a single `--specs` invocation (e.g. `--specs add-auth,003-user-list` where `add-auth` is OpenSpec and `003-user-list` is Spec Kit). The active backend is determined by config / auto-detection; all `--specs` values are matched against entries from that one backend.

## Decisions

### D1. Two flags, one internal mode enum

Internally, the start dispatcher converts the flag combination into a single `SpecMode` enum:

```rust
enum SpecMode {
    None,                     // neither flag passed; falls through to branch picker
    All,                      // --from-all-specs (or --from-specs alias)
    Picker,                   // --specs (no values)
    Narrow(Vec<String>),      // --specs NAME[,NAME...]
}
```

The CLI parse produces this enum directly (or a struct that maps to it). The dispatcher then has one match statement against the four cases. Mutual exclusion is enforced at the clap layer (parse-time error) so the dispatcher never sees an invalid combination.

### D2. clap surface

Three flags on `StartArgs`:

```rust
/// Launch worktrees for every discovered spec.
#[arg(long, alias = "from-specs", help = "Launch from every discovered spec (all formats)")]
from_all_specs: bool,

/// Narrow to named specs, or open a multi-select picker if no values are given.
#[arg(
    long,
    value_delimiter = ',',
    num_args = 0..,
    conflicts_with = "from_all_specs",
    help = "Comma-separated spec names; bare flag opens picker"
)]
specs: Option<Vec<String>>,
```

- `alias = "from-specs"` makes `--from-specs` parse identically to `--from-all-specs`. Hidden from help if `clap` supports `hide = true` on aliases (it does as of clap 4); otherwise the alias appears in help but with no separate description.
- `num_args = 0..` allows `--specs` with zero values (the picker case). When zero values are given, `Option<Vec<String>>` becomes `Some(vec![])`. The dispatcher distinguishes `Some(empty)` (picker) from `None` (flag absent) from `Some(non-empty)` (narrow).
- `conflicts_with = "from_all_specs"` enforces mutual exclusion at parse time. `clap` produces the standard "cannot be used together" error.

### D3. Hidden alias mechanics

clap 4's `alias` attribute creates a parse-time alias. When the user runs `--from-specs`, the parsed `from_all_specs` field becomes `true` exactly as if they'd run `--from-all-specs`. From the dispatcher's perspective, the two are indistinguishable.

For visibility:
- The canonical flag's help text appears in `git paw start --help`.
- The alias is suppressed via `hide_long_help` or by simply not documenting it. clap's behaviour around aliases in help text varies by version; the implementation will validate that `--help` output does NOT mention `--from-specs` (test-asserted in tasks 11.x).

If clap's aliasing surfaces the alias in help on the project's pinned clap version, the fallback is to define `--from-specs` as a separate hidden boolean (`#[arg(long, hide = true)]`) and OR-merge it into the canonical field manually in a parse-time hook. This is a small implementation detail; the spec just requires "hidden alias" semantics, not a specific clap idiom.

### D4. Spec-name resolution algorithm

Given `--specs NAME1,NAME2,...`, the dispatcher must map each name to a `SpecEntry` (or to an error). The resolver:

1. Calls `scan_specs(&config, &repo_root)` to get the full discovered list.
2. For each requested name, attempts these match strategies in order, taking the first that succeeds:
   - **Exact match** on `SpecEntry.id` (case-sensitive).
   - **Numeric prefix match** on Spec-Kit feature ids only (e.g. `003` matches `003-user-list-T009` if there's exactly one feature whose directory name starts with `003-`). Ambiguous prefixes (matches multiple features) are rejected with the candidate list.
   - **Filename-stem fallback** for Markdown specs: the user-supplied name compared against the spec file's stem.
3. If a name doesn't match anything, the resolver collects it into an "unknown" list. After processing all names, if any are unknown, the dispatcher exits with an error containing the unknown names AND the discovered-set names. No partial start.
4. Spec Kit feature names that resolve to multiple `SpecEntry` (e.g. `003-user-list` resolves to both the `[P]` task entries and the consolidated entry) → all matching entries are launched. The user picks a feature; git-paw expands to all worktrees within that feature's current phase.

Resolution is deterministic; the same names against the same scan produce the same selection.

### D5. Picker behaviour

`select_specs(specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError>` lives in `src/interactive.rs`. Implementation mirrors `select_branches`:

```rust
fn select_specs(&self, specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError> {
    let labels: Vec<String> = group_by_feature(specs)
        .iter()
        .map(format_picker_label)
        .collect();
    let selection = MultiSelect::new()
        .with_prompt("Select specs (space to toggle, enter to confirm)")
        .items(&labels)
        .interact_opt()
        .map_err(|e| map_dialoguer_error(&e))?;
    /* Empty selection or None → UserCancelled. Otherwise return chosen entries. */
}
```

Key difference from `select_branches`: the picker groups by feature for Spec Kit. Spec Kit produces multiple `SpecEntry` per feature; presenting them flat would mean the user has to tick three boxes (`task/T009-...`, `task/T010-...`, `phase/003-...`) for one logical feature. Instead, the picker shows one row per feature with a worktree-count hint, and selecting that row selects all the underlying entries.

For OpenSpec and Markdown, one `SpecEntry` = one feature, so grouping is a no-op.

`group_by_feature` strategy: rely on the `SpecEntry.id` shape. Spec Kit ids are `<feature>-<task-id>` or `<feature>-phase-<N>`; we can extract the feature prefix. OpenSpec / Markdown ids are flat, one entry per feature. The grouping function returns `Vec<(FeatureLabel, Vec<&SpecEntry>)>`.

### D6. TTY detection

Before invoking the picker, the dispatcher checks `std::io::IsTerminal::is_terminal(&std::io::stdin())`. If false:

```
error: --specs without values requires an interactive terminal
       to open the multi-select picker.
       Either:
         - run `git paw start --specs NAME[,NAME...]` to narrow explicitly, or
         - run `git paw start --from-all-specs` to launch every discovered spec.
```

Exit code matches the existing `PawError::*` exit-code mapping. The error message format follows the project's existing style (action-oriented, lists alternatives).

### D7. Filtering location

The filter step lives in `cmd_start_from_specs` (or whatever the canonical entry point is renamed to) — not in `scan_specs`, not in the backends. The flow:

```
cmd_start_from_specs(args)
  -> scan_specs(config, repo) -> Vec<SpecEntry>
  -> match args.spec_mode {
       All       => entries,
       Picker    => prompter.select_specs(&entries)?,
       Narrow(n) => resolve_names(&entries, &n)?,
     }
  -> proceed with the chosen subset (CLI assignment, dry-run check, etc.)
```

Keeping the backends pure (discovery only) means the filter logic is testable in isolation against any `Vec<SpecEntry>` and the backends can grow independently.

### D8. Error variants

Two new error paths:

- **Unknown spec name(s).** Reuse a generic error (e.g. extend `PawError::SpecError(String)` with a constructor `unknown_specs(unknown: Vec<String>, candidates: Vec<String>) -> Self` that produces a structured message). Avoid a brand-new variant unless the existing error machinery makes it awkward.
- **Picker requires TTY.** Same approach — produce a `SpecError` (or similar) with a fixed message. The error indicates the action; structured fields are optional.

Both errors set the appropriate exit code so CI / scripts can distinguish them from runtime errors.

### D9. `--from-specs` parse equivalence

When `--from-specs` is used, the parsed args struct SHALL be byte-for-byte identical to `--from-all-specs`. This implies:

- `args.from_all_specs == true`
- No extra "alias used" flag in the parsed struct (the dispatcher is alias-agnostic).
- No stderr warning. The migration nudge is via release notes and the absent help text, not a runtime nag.

Test surface confirms this: a parse test feeds `--from-specs` and asserts the `StartArgs` value exactly matches the parse of `--from-all-specs`.

### D10. Documentation policy

The `--help` output for `start` SHALL describe `--from-all-specs` and `--specs` only. The mdBook chapter (`docs/src/user-guide/start.md` or wherever the existing flag is documented) SHALL replace `--from-specs` references with `--from-all-specs` and add a section documenting `--specs` with picker example, narrow example, and the unknown-name error.

The README's quick-start snippets that show `--from-specs` SHALL be updated to `--from-all-specs`.

The v0.5.0 release notes SHALL document the new flags AND announce the v1.0.0 removal of the `--from-specs` alias.

The v0.5.0 user guide SHALL NOT mention `--from-specs` except possibly in a single line in a "Migration from v0.4" appendix (optional, at the implementer's discretion).

## Risks / Trade-offs

- **[Risk] clap version pinning affects alias hide behaviour.** If the project's clap is older than 4.x or pins a version where `alias` doesn't compose with help-hiding, the alias may surface in `--help`. → **Mitigation:** the implementation can fall back to a separate hidden flag (D3 fallback). The spec just requires "hidden alias" semantics; the test asserts `--help` output does not mention the alias.
- **[Risk] Numeric prefix matching for Spec Kit ambiguity.** A user runs `--specs 003` expecting `003-user-list` but the project also has `003-room-setup`. → **Mitigation:** ambiguous prefix produces an error listing both candidates. User retries with the unambiguous name.
- **[Risk] Picker UX with very long lists.** A project with 30 OpenSpec changes shows a 30-row picker. → **Mitigation:** dialoguer's MultiSelect supports paging for long lists by default. If dogfood shows pain, switch to fuzzy filtering (e.g. `dialoguer::FuzzySelect`) — out of scope here.
- **[Risk] User-forked scripts using `--from-specs --no-supervisor` (or similar combos).** They keep working in v0.5.0 because the alias is exact. They break in v1.0.0. → **Mitigation:** the v1.0.0 removal error message is loud and points at the rename. Release notes for v0.5.0 explicitly call out the v1.0.0 break, giving users a full release cycle to migrate.
- **[Risk] Spec Kit feature with mixed `[P]` + non-`[P]` produces multiple `SpecEntry` from one feature; picker entry shows count.** A user selects "003-user-list" expecting one worktree and gets three. → **Mitigation:** the picker label includes the count hint (`003-user-list — 3 worktrees: 2 [P] + 1 phase/`). The user sees what they're committing to before pressing enter.
- **[Trade-off] Comma-separated over repeatable.** Comma-separated matches `--branches`. The downside: spec names with commas in them break the parser. Spec names today are kebab-case file/dir names (no commas), so this isn't a real concern.
- **[Trade-off] Hidden alias vs. visible-with-deprecation-warning.** Visible-with-warning would be louder for migration but adds noise to every v0.4-script invocation. We prioritised silent compatibility and migration via docs. Re-evaluate if v0.6 / v0.7 dogfood shows users not noticing the rename.

## Migration Plan

1. Land the change. Existing `--from-specs` invocations continue to work unchanged (same parsed state, same behaviour).
2. v0.5.0 release notes announce: "the canonical name is now `--from-all-specs`. `--from-specs` is a hidden alias that will be removed in v1.0.0. Migrate when convenient."
3. v0.5.0 user-guide chapters use `--from-all-specs` exclusively.
4. v1.0.0 (later release): drop the alias. Passing `--from-specs` produces a hard error: `"--from-specs was renamed to --from-all-specs in v0.5.0 and removed in v1.0.0. Update your script."`

Rollback: revert the change. All v0.4 invocations work exactly as they did pre-v0.5.0. The `--specs` flag disappears.

## Open Questions

- **Should `--specs` also accept a single name without commas (`--specs add-auth`)?** Decision: yes. Comma-separation is a delimiter, not a requirement. `--specs add-auth` → `Vec<String>` of length 1. (Already encoded in clap's `value_delimiter` semantics.)
- **Should the picker allow zero selection (i.e. user cancels by selecting nothing and pressing enter)?** Decision: zero selection → `PawError::UserCancelled`, matching `select_branches`. The user's intent of "I don't want to launch anything" is honored as a cancel, not a no-op start.
- **Should auto-detect of Spec Kit (`spec-kit-format` change) interact with `--specs` differently?** Decision: no. Auto-detection determines the backend; `--specs` filters the backend's output. The two are orthogonal layers.
- **Does the `--specs` flag in `git paw start --dry-run` skip the picker?** Decision: no. Dry-run still goes through the same selection logic — `--specs` (no values) opens the picker, the user selects, dry-run prints the plan. This matches the "dry-run is a final-step preview" model. The non-TTY error path applies the same way.
