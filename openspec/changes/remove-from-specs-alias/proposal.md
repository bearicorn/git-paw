## Why

`--from-specs` was renamed to `--from-all-specs` in v0.5.0 and kept as a hidden clap alias
(`alias = "from-specs"` on `from_all_specs` in `src/cli.rs`) so v0.4 scripts kept working.
`cli-parsing` has said since then that "the alias SHALL be removed in v1.0.0", and since v0.24.0
(`error-exit-code-contract`) the alias warns through the `core-deprecation` registry with removal
release 1.0.0 — so the registry's guard fails the 1.0.0 build until the alias and its entry are
gone. Freezing it into the 1.x contract would carry a second spelling of the same flag for the
whole major version; removing it after the freeze would be a breaking change in a minor.

## What Changes

- **BREAKING:** `git paw start --from-specs` no longer launches anything. It fails before any
  side effect with exit code `2` (an argument rejected by parsing, per the frozen exit-code
  table) and a migration message on stderr naming `--from-specs`, stating it was renamed to
  `--from-all-specs`, and showing the corrected invocation. Scripts replace the flag with
  `--from-all-specs`; nothing else changes.
- The flag stays out of `--help` and out of generated shell completions.
- The `--from-specs` entry is deleted from the deprecation registry (`src/deprecation.rs`), so the
  removal guard passes at 1.0.0 and the CLI reference's Deprecations section drops it. The
  rejection goes through a removed-flag table (flag → replacement) introduced here with one row;
  `remove-supervisor-vocabulary` adds `--supervisor` and `--no-supervisor` to it.
- Specs, docs, the generated default config, code comments and tests stop naming
  `--from-specs` as a usable flag; the only remaining mentions are the removal message, the
  removal requirement, the stability chapter's removal note and the changelog.
- The requirement copies this change re-states are written in the coordinated-mode vocabulary
  (`--coordinated` / `--bare`, `default_mode`, the coordinated launch path), since their text
  outlives 1.0.0; *Spec mode + supervisor mode dispatch* is renamed *Spec mode + coordinated mode
  dispatch*.

**Archive order within v1.0.0:** `remove-from-specs-alias` archives first, then
`remove-supervisor-vocabulary`. Both modify `cli-parsing` (on disjoint requirements), and
`remove-supervisor-vocabulary` ends with a direct, verbatim sweep and capability rename of
`openspec/specs/`; deltas are folded in before that reorganization (spec-organization §6), so its
sweep runs on this change's archived text and never reintroduces `--from-specs`.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-parsing`: *--from-specs is a hidden alias of --from-all-specs* removed; new
  *Removed --from-specs flag is rejected with a migration message* (including the registry entry's
  deletion); *Spec mode + supervisor mode dispatch* RENAMED to *Spec mode + coordinated mode
  dispatch*; it, *Non-TTY launch handling*, *--from-all-specs flag* and *--from-all-specs and
  --specs are mutually exclusive* name the canonical flag and the coordinated-mode vocabulary
  (behaviour unchanged, except the flagged *Non-TTY launch handling* reconciliation in design D3).
  *Start subcommand with optional flags* is no longer modified here: v0.21.0's
  `supervisor-vocabulary-split` already restates it with `--from-all-specs` and `--coordinated`.
- `boot-block`: *Manual mode injection timing* and *Boot-block injection in
  cmd_start_from_specs* name `--from-all-specs` and bare mode (behaviour unchanged).
- `cli-resolution`: *Prompt at most once* names the spec-driven launch (behaviour unchanged).
- `core-configuration`: *The system SHALL support a default_spec_cli config field* names the
  spec-driven launch (behaviour unchanged).

## Impact

- **Code:** `src/cli.rs` — drop `alias = "from-specs"`; add a hidden `--from-specs` arg that
  exists only to be rejected; rewrite the alias tests (`start_with_from_specs_alias_*`, the help
  test) and migrate the `--from-specs` rows in the parse tables. `src/main.rs` — reject the flag
  after parse with a clap usage error before dispatch, through the removed-flag table; drop "or its
  hidden `--from-specs` alias" from `SpecMode` docs. `src/deprecation.rs` — delete the
  `--from-specs` registry entry and its rows in the deprecated-flag scan tests.
  `src/config/mod.rs` — `default_spec_cli` doc comment and the generated default config's
  comment ("…for --from-specs mode"), which is an exported asset.
  `src/commands/start.rs` module doc.
- **Tests:** about 25 invocations of `--from-specs` move to `--from-all-specs` in
  `tests/e2e_tests.rs`, `cross_format_spec_selection.rs`, `session_logging_capture_integration.rs`,
  `from_specs_launch_fixes_integration.rs`, `start_force_integration.rs`,
  `cli_supervisor_non_tty.rs`; v0.24.0's
  `cross_format_spec_selection.rs::from_specs_alias_emits_a_deprecation_warning` gives way to the
  rejection test; new rejection test and a no-mention guard.
- **Docs:** `docs/src/cli-reference.md` (Deprecations section drops `--from-specs`; otherwise only
  the canonical flag — confirm), the stability chapter's removal note, CHANGELOG via the
  `BREAKING CHANGE:` footer.
- **Enum-variant ripple:** none.
- **Delta coordination:** *Boot-block injection in cmd_start_from_specs* is also modified by
  v0.18.0 `launch-prompt-entry-point` (on the archived v0.17.0 `start-spec-launch-parity` text);
  this delta is written on top of that version and must be rebased if it changes before archive.
  No other v0.18.0 draft touches the requirements modified here; any later cycle that edits them
  must carry the rename.
- **Cross-version dependencies:**
  - v0.21.0 `supervisor-vocabulary-split` (archived first): the `core-deprecation` registry, the
    coordinated-mode vocabulary these copies use, and its version of *Start subcommand with
    optional flags*.
  - v0.24.0 `error-exit-code-contract` (archived first): registers `--from-specs` (removal 1.0.0)
    and MODIFIES *--from-specs is a hidden alias of --from-all-specs*, which this change REMOVES.
  - v1.0.0 `remove-supervisor-vocabulary` archives after this change (order above) and extends the
    removed-flag table; `cli-contract-freeze` supplies the frozen exit-code meaning of `2` and the
    stability chapter that records the removal (v0.24.0's exit-code pass is the source of truth if
    it changes that meaning).
