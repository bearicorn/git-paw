## 0. Preconditions

- [ ] 0.1 Confirm v0.21.0 `supervisor-vocabulary-split` and v0.24.0 `error-exit-code-contract` are archived (registry, coordinated-mode vocabulary, the `--from-specs` registry entry); rebase the MODIFIED / REMOVED blocks on the archived text if it moved
- [ ] 0.2 Confirm `remove-supervisor-vocabulary` is NOT yet archived: v1.0.0 archive order is this change first, then `remove-supervisor-vocabulary` (its sweep renames headers this delta targets)

## 1. Reject the removed flag

- [ ] 1.1 In `src/cli.rs`, drop `alias = "from-specs"` from `from_all_specs`; add a hidden `--from-specs` boolean arg on `Start` that exists only to be rejected (doc comment says so)
- [ ] 1.2 Add the removed-flag table (flag → replacement) with its one row `--from-specs` → `--from-all-specs`; `remove-supervisor-vocabulary` extends it later
- [ ] 1.3 In `src/main.rs`, immediately after `Cli::parse()` and before dispatch, turn a set `--from-specs` into a clap usage error (`Cli::command().error(ErrorKind::UnknownArgument, …)`) built from the table, naming `--from-specs`, the word "renamed", `--from-all-specs`, and the corrected invocation; exit through clap so the code is 2
- [ ] 1.4 Delete the `--from-specs` entry from `deprecation::REGISTRY` (`src/deprecation.rs`) and its rows in the deprecated-flag scan tests (`cli::tests::deprecation_scan_stops_at_terminator`); at crate version `1.0.0`, `deprecation::tests::no_entry_outlives_its_removal` no longer names it
- [ ] 1.5 Update the `SpecMode::All` doc and any comment that calls `--from-specs` a hidden or deprecated alias

## 2. Tests

- [ ] 2.1 Replace `start_with_from_specs_alias_parses_identically_to_from_all_specs`, `start_with_from_specs_alias_and_specs_is_rejected` and v0.24.0's `tests/cross_format_spec_selection.rs::from_specs_alias_emits_a_deprecation_warning` with rejection tests (unit: the parsed hidden flag is set; integration: real process exits 2, stderr contains `--from-specs`, `renamed`, `--from-all-specs`, and no worktree, session file or tmux session exists) — covers "--from-specs fails with a migration message" and "--from-specs combined with --specs reports the removal"
- [ ] 2.2 Keep the help test asserting `--from-specs` is absent from `start --help`; add the same assertion for `git paw completions bash|zsh|fish` output — covers "The removed flag is not advertised"
- [ ] 2.3 Registry test: no entry has the item `--from-specs`, and the CLI reference's Deprecations section does not contain `--from-specs` — covers "The deprecation registry no longer lists --from-specs"
- [ ] 2.4 Migrate the `--from-specs` rows in `src/cli.rs` parse tables and every invocation in `tests/e2e_tests.rs`, `tests/cross_format_spec_selection.rs`, `tests/session_logging_capture_integration.rs`, `tests/from_specs_launch_fixes_integration.rs`, `tests/start_force_integration.rs`, `tests/cli_supervisor_non_tty.rs` to `--from-all-specs` (and any remaining `--supervisor` / `--no-supervisor` there to `--coordinated` / `--bare`); their assertions stay (they cover the re-stated `cli-parsing`, `boot-block`, `cli-resolution` scenarios, including *Spec mode + coordinated mode dispatch* and "Non-TTY coordinated launch runs no foreground CLI" via `non_tty_supervisor_skips_cli_launch`)
- [ ] 2.5 Add a no-mention guard (model: `tests/cli_dx_docs.rs::no_doc_references_a_resume_command`): README, `docs/src/` except the changelog and the stability chapter's removal note, `assets/`, and the generated default config contain no `--from-specs`

## 3. Exported asset and comments

- [ ] 3.1 `config::generate_default_config`: reword the `default_spec_cli` comment to "for spec-driven launches (--from-all-specs / --specs)"; keep the template otherwise byte-identical
- [ ] 3.2 `PawConfig::default_spec_cli` doc comment and the `src/commands/start.rs` module doc name `--from-all-specs`

## 4. Docs

- [ ] 4.1 Confirm `docs/src/cli-reference.md`, `user-guide/spec-driven-launch.md` and README name only `--from-all-specs`; drop `--from-specs` from the CLI reference's Deprecations section
- [ ] 4.2 Add the removal to the stability chapter's "removed at 1.0.0" note (from `cli-contract-freeze`)
- [ ] 4.3 At archive, hand-edit the `## Purpose` of `openspec/specs/boot-block/spec.md` to drop "`--from-specs` a deprecated alias"

## 5. Gates

- [ ] 5.1 Commit as a breaking change: `!` after the type/scope and a `BREAKING CHANGE:` footer stating `--from-specs` is removed and to use `--from-all-specs`
- [ ] 5.2 Every scenario in the four delta specs maps to a test named above
- [ ] 5.3 `cargo fmt`, then `just lint`
- [ ] 5.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 5.5 `mdbook build docs/`
- [ ] 5.6 `openspec validate remove-from-specs-alias --strict`
- [ ] 5.7 Archive before `remove-supervisor-vocabulary` (task 0.2)
