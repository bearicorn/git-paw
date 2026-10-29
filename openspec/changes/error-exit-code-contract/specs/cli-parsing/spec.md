## MODIFIED Requirements

### Requirement: --from-specs is a hidden alias of --from-all-specs

The `start` subcommand SHALL accept `--from-specs` as a hidden alias of `--from-all-specs`. When the user passes `--from-specs`, the parsed `StartArgs` SHALL be byte-for-byte identical to the parse result for `--from-all-specs`, and the command SHALL proceed exactly as if `--from-all-specs` had been passed. `--from-specs` SHALL be recorded in the deprecation registry (`core-deprecation`) with its removal in v1.0.0 and its replacement `--from-all-specs`, and each time the alias is used the binary SHALL write one deprecation warning to stderr through the registry's shared helper, stating that `--from-specs` is deprecated, that it will be removed in v1.0.0, and that `--from-all-specs` replaces it. The warning SHALL NOT change the exit status or stdout. `--from-all-specs` SHALL produce no such warning. An argument `--from-specs` appearing after a `--` terminator SHALL NOT trigger the warning.

The alias SHALL NOT appear in `git paw start --help` output. The alias SHALL be removed in v1.0.0; v0.5.0 keeps it for backward compatibility with v0.4 scripts.

#### Scenario: --from-specs parses identically to --from-all-specs

- **GIVEN** two CLI invocations: `start --from-specs` and `start --from-all-specs`
- **WHEN** both are parsed
- **THEN** the resulting `StartArgs` values SHALL be equal

Test: `cli::tests::start_with_from_specs_alias_parses_identically_to_from_all_specs`

#### Scenario: --from-specs does not appear in help

- **WHEN** `git paw start --help` is run
- **THEN** the output SHALL NOT contain the substring `--from-specs`

Test: `cli::tests::start_help_contains_from_all_specs_and_specs_but_not_alias`

#### Scenario: --from-specs emits a deprecation warning

- **GIVEN** the user runs `git paw start --from-specs --dry-run` in a repository with specs
- **WHEN** the binary runs
- **THEN** stderr SHALL contain one warning naming `--from-specs`, its removal in v1.0.0 and `--from-all-specs`
- **AND** stdout and the exit status SHALL equal those of the same command run with `--from-all-specs`

Test: `tests/cross_format_spec_selection.rs::from_specs_alias_emits_a_deprecation_warning`

#### Scenario: --from-all-specs emits no warning

- **GIVEN** the user runs `git paw start --from-all-specs --dry-run`
- **WHEN** the binary runs
- **THEN** stderr SHALL contain no deprecation warning

Test: `tests/cross_format_spec_selection.rs::from_specs_alias_emits_a_deprecation_warning`

#### Scenario: Arguments after a terminator are not inspected

- **GIVEN** a raw argument list containing `--` followed by `--from-specs`
- **WHEN** the deprecation scan runs
- **THEN** it SHALL return no warning

Test: `cli::tests::deprecation_scan_stops_at_terminator` (pure, table-driven)
