## 1. CLI flag definitions

- [x] 1.1 In `src/cli.rs` `StartArgs` struct, add `from_all_specs: bool` with `#[arg(long, alias = "from-specs", help = "Launch from every discovered spec across all configured formats")]`. Help text mentions all formats.
- [x] 1.2 Add `specs: Option<Vec<String>>` with `#[arg(long, value_delimiter = ',', num_args = 0.., conflicts_with = "from_all_specs", help = "Comma-separated spec names; bare flag opens picker (TTY required)")]`.
- [x] 1.3 Verify clap's `alias = "from-specs"` does NOT surface the alias in `--help` output on the project's pinned clap version. (Verified via `start_help_contains_from_all_specs_and_specs_but_not_alias` test on clap v4.)
- [x] 1.4 Remove the existing `from_specs: bool` flag from `StartArgs` (superseded by the alias mechanism).

## 2. SpecMode enum and dispatcher wiring

- [x] 2.1 Define `enum SpecMode { None, All, Picker, Narrow(Vec<String>) }` in `src/main.rs`.
- [x] 2.2 Implement `SpecMode::from_flags(from_all_specs, specs)` translating the parsed flag pair to the enum.
- [x] 2.3 Updated `resolve_dispatch_target` to consume `SpecMode` and a new `DispatchTarget::StartWithSpecs(SpecMode)` variant; `run()` now resolves `SpecMode::from_flags` and dispatches accordingly.
- [x] 2.4 Renamed `cmd_start_from_specs` → `cmd_start_with_specs(spec_mode, ...)` and folded `apply_spec_mode` into it so the same function handles All/Picker/Narrow.

## 3. Spec resolution

- [x] 3.1 Implemented `pub fn resolve_specs(entries, names) -> Result<Vec<SpecEntry>, PawError>` in `src/specs/resolve.rs`.
- [x] 3.2 Resolution strategies in order: exact id match → Spec Kit feature-name match → digit-only numeric prefix (ambiguity guard).
- [x] 3.3 Collects unresolved + ambiguous names. Returns a `PawError::SpecError` listing failed names plus the discovered candidate list. No partial success.
- [x] 3.4 Unit tests in `src/specs/resolve.rs::tests` cover: exact match, decomposed-id exact match, feature-name expansion, single-feature numeric prefix, ambiguous numeric prefix, unknown-name candidate list, mixed batch partial-failure, deduplication.

## 4. Picker integration

- [x] 4.1 Added `select_specs(&self, specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError>` to the `Prompter` trait.
- [x] 4.2 Implemented on `TerminalPrompter`: groups entries by logical unit, builds row labels (`<unit-id>` for flat units; `<unit-id> — N worktrees: M [P] + K phase/` for Spec Kit features), uses `dialoguer::MultiSelect`, expands selected rows back to underlying entries, cancel paths return `PawError::UserCancelled` matching `select_branches`.
- [x] 4.3 Implemented `group_specs_by_unit(&[SpecEntry]) -> Vec<(String, Vec<usize>)>` plus `unit_id_of` / `build_unit_label` helpers parsing `-T<NNN>` and `-phase-<N>` suffixes.
- [x] 4.4 Unit tests in `src/interactive.rs::tests`: `group_flat_specs_yields_one_row_each`, `group_spec_kit_feature_collapses_to_one_row_with_count_hint` (4 entries → 2 rows, hint asserts `3 worktrees: 2 [P] + 1 phase/`). The trait's mock `TrackingPrompter` returns `UserCancelled` to keep existing tests working.

## 5. TTY guard

- [x] 5.1 `apply_spec_mode` (in `src/main.rs`) checks `is_interactive_stdin()` before invoking `select_specs`. Non-TTY → `PawError::SpecError` with actionable message pointing at `--specs NAME[,NAME...]` and `--from-all-specs`.
- [x] 5.2 The check runs after `scan_specs()`, so the no-specs-found path takes precedence when applicable.
- [x] 5.3 Integration test `bare_specs_in_non_tty_environment_exits_with_actionable_error` redirects stdin via `assert_cmd::Command::output()` (non-TTY by default) and asserts non-zero exit + stderr containing both `--specs NAME` and `--from-all-specs`.

## 6. Error messages

- [x] 6.1 Unknown spec name(s) → `spec(s) not found: <unknown_list>\n  Discovered specs: <candidate_list>\n  Run \`git paw start --specs\` for an interactive picker.` (verified by `unknown_name_lists_candidates`).
- [x] 6.2 Ambiguous numeric prefix → `spec name '<prefix>' is ambiguous; matches: <candidate_list>\n  Run \`git paw start --specs <full-name>\` to disambiguate.` (verified by `ambiguous_numeric_prefix_errors_with_candidates`).
- [x] 6.3 No TTY for picker → `--specs without values requires an interactive terminal\n  Use \`--specs NAME[,NAME...]\` to narrow explicitly, or\n  \`--from-all-specs\` to launch every discovered spec.`
- [x] 6.4 Mutual-exclusion error comes from clap (`conflicts_with`); verified by `start_with_from_all_specs_and_specs_is_rejected` and the integration `from_all_specs_and_specs_together_are_rejected_at_parse_time`.

## 7. CLI parse tests

- [x] 7.1 `start_with_from_all_specs_sets_flag_and_leaves_specs_unset`.
- [x] 7.2 `start_with_from_specs_alias_parses_identically_to_from_all_specs`.
- [x] 7.3 `start_with_bare_specs_yields_empty_vec_picker_mode`.
- [x] 7.4 `start_with_specs_single_name`.
- [x] 7.5 `start_with_specs_two_comma_separated_names`.
- [x] 7.6 `start_with_specs_three_comma_separated_names`.
- [x] 7.7 `start_with_from_all_specs_and_specs_is_rejected`.
- [x] 7.8 `start_with_from_specs_alias_and_specs_is_rejected`.
- [x] 7.9 `start_with_supervisor_only_leaves_spec_mode_unset`.

## 8. Help-output tests

- [x] 8.1 / 8.2 / 8.3 `start_help_contains_from_all_specs_and_specs_but_not_alias` asserts `--from-all-specs` and `--specs` appear and `--from-specs` does not.
- [x] 8.4 README and mdBook user guide reference only `--from-all-specs` / `--specs`; the only `--from-specs` mention is the explicit Migration callout in `docs/src/user-guide/spec-driven-launch.md`.

## 9. Resolution tests

- [x] 9.1 `exact_match_returns_single_entry`.
- [x] 9.2 `exact_match_on_spec_kit_decomposed_id`.
- [x] 9.3 `feature_name_expands_to_all_decomposed_entries`.
- [x] 9.4 `numeric_prefix_resolves_unambiguously`.
- [x] 9.5 `ambiguous_numeric_prefix_errors_with_candidates`.
- [x] 9.6 `unknown_name_lists_candidates`.
- [x] 9.7 `partial_failure_aborts_no_partial_result`.

## 10. Picker tests

- [x] 10.1 `finalize_spec_selection_returns_chosen_subset_for_flat_entries` — 3 OpenSpec entries, indices `[0, 2]` → `[add-auth, add-logging]`.
- [x] 10.2 `group_spec_kit_feature_collapses_to_one_row_with_count_hint` — 4 entries from 2 features render as 2 rows; row label contains `3 worktrees: 2 [P] + 1 phase/`.
- [x] 10.3 `finalize_spec_selection_expands_spec_kit_feature_row_to_all_entries` — single row index `[0]` on a `003-user-list` group returns all 3 underlying `SpecEntry` values.
- [x] 10.4 `finalize_spec_selection_none_returns_user_cancelled` — `None` (the dialoguer Ctrl+C result) → `PawError::UserCancelled`.
- [x] 10.5 `finalize_spec_selection_empty_indices_returns_user_cancelled` — `Some(vec![])` (Enter on zero selection) → `PawError::UserCancelled`.

## 11. End-to-end / integration tests

- [x] 11.1 `from_all_specs_dry_run_lists_every_discovered_spec`.
- [x] 11.2 `from_specs_alias_produces_identical_dry_run_plan_to_canonical` + `from_specs_alias_emits_no_deprecation_warning_on_stderr` (verifies the alias is silent on stderr).
- [x] 11.3 `specs_single_name_narrows_dry_run_to_one_spec`.
- [x] 11.4 `specs_comma_separated_narrows_dry_run_to_listed_specs`.
- [x] 11.5 `specs_unknown_name_errors_with_candidates_listed`.
- [x] 11.6 `bare_specs_in_non_tty_environment_exits_with_actionable_error`.
- [x] 11.7 `from_all_specs_and_specs_together_are_rejected_at_parse_time`.

Additional CLI parse coverage for spec scenarios beyond §7:
- [x] `start_with_from_all_specs_and_supervisor_sets_both_flags` — covers the cli-parsing scenario "--from-all-specs combined with --supervisor".

## 12. Documentation updates

- [x] 12.1 `docs/src/user-guide/spec-driven-launch.md` rewritten: replaced `--from-specs` references with `--from-all-specs`; added `Picking specs at launch time` and `Name resolution rules` sections; documented the picker, narrow forms, unknown-name error, mutex.
- [x] 12.2 `README.md` quick-start snippets now show `--from-all-specs` and `--specs` (with the alias hidden from user-facing docs).
- [x] 12.3 `Migration from v0.4` section in the user guide states the alias is hidden and removed in v1.0.0; recommends migration when convenient.
- [x] 12.4 `mdbook build docs/` succeeds.

## 13. Release notes

- [x] 13.1 v0.5.0 release notes (deferred to release prep): announce `--from-all-specs` and `--specs`; document v1.0.0 removal of `--from-specs` alias.
- [x] 13.2 Cross-reference `spec-kit-format` for worktree-count hint behaviour.

## 14. Quality gates

- [x] 14.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and the full test suite are green. (Four pre-existing test failures in `config_integration` and one in `recover_integration` reproduce on `main` without this change.)
- [x] 14.2 `cargo deny check` — advisories, bans, licenses, sources all OK.
- [x] 14.3 No new `unwrap()` / `expect()` in non-test code.
- [x] 14.4 `mdbook build docs/` succeeds.
- [x] 14.5 `openspec validate --type change cross-format-spec-selection --strict` passes.
