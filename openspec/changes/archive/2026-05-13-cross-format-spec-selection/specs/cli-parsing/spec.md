## ADDED Requirements

### Requirement: --from-all-specs flag

The `start` subcommand SHALL accept a `--from-all-specs` flag (boolean, default `false`). When passed, the resulting `StartArgs` SHALL indicate the "launch every discovered spec" mode — the v0.4 behaviour previously gated by `--from-specs`.

The flag SHALL appear in `git paw start --help` output with a description naming it as the canonical name for this behaviour.

#### Scenario: --from-all-specs sets the launch-all mode

- **GIVEN** the user invokes `git paw start --from-all-specs`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs` SHALL indicate the launch-all-discovered-specs mode

#### Scenario: --from-all-specs combined with --supervisor

- **GIVEN** `start --from-all-specs --supervisor`
- **WHEN** the CLI is parsed
- **THEN** both `from_all_specs` and `supervisor` SHALL be `true`

#### Scenario: --from-all-specs appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output contains `--from-all-specs`
- **AND** the output describes the flag as launching every discovered spec

### Requirement: --from-specs is a hidden alias of --from-all-specs

The `start` subcommand SHALL accept `--from-specs` as a hidden alias of `--from-all-specs`. When the user passes `--from-specs`, the parsed `StartArgs` SHALL be byte-for-byte identical to the parse result for `--from-all-specs`. No stderr warning SHALL be emitted at runtime; the alias is silent.

The alias SHALL NOT appear in `git paw start --help` output. The alias SHALL be removed in v1.0.0; v0.5.0 keeps it for backward compatibility with v0.4 scripts.

#### Scenario: --from-specs parses identically to --from-all-specs

- **GIVEN** two CLI invocations: `start --from-specs` and `start --from-all-specs`
- **WHEN** both are parsed
- **THEN** the resulting `StartArgs` values SHALL be equal

#### Scenario: --from-specs does not appear in help

- **WHEN** `git paw start --help` is run
- **THEN** the output SHALL NOT contain the substring `--from-specs`

#### Scenario: --from-specs emits no stderr warning

- **GIVEN** the user runs a command containing `--from-specs`
- **WHEN** the CLI parses
- **THEN** no stderr warning SHALL be emitted regarding the flag's deprecation
- **AND** the command proceeds exactly as if `--from-all-specs` had been passed

### Requirement: --specs flag with comma-separated values

The `start` subcommand SHALL accept a `--specs` flag whose value is a comma-separated list of spec names (mirroring the existing `--branches feat/a,feat/b` syntax). The flag SHALL accept zero or more values:

- `--specs` (no values) — indicates the picker mode.
- `--specs NAME` — narrows to a single named spec.
- `--specs NAME1,NAME2,NAME3` — narrows to the listed specs.
- `--specs NAME1,NAME2 --specs NAME3` — equivalent to `--specs NAME1,NAME2,NAME3` if clap's value-accumulation across repetitions is enabled (implementation choice; tests assert behaviour for the comma-separated form).

The parsed value distinguishes three states:
- Flag absent → no spec mode requested.
- Flag present with zero values → picker mode.
- Flag present with one or more values → narrow mode with the listed names.

The flag SHALL appear in `git paw start --help` output.

#### Scenario: --specs with single value parses as narrow

- **GIVEN** `start --specs add-auth`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs` SHALL indicate narrow mode with `["add-auth"]`

#### Scenario: --specs with comma-separated values parses as narrow with multiple names

- **GIVEN** `start --specs add-auth,fix-session,add-logging`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs` SHALL indicate narrow mode with `["add-auth", "fix-session", "add-logging"]`

#### Scenario: --specs with no values parses as picker

- **GIVEN** `start --specs`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs` SHALL indicate picker mode

#### Scenario: --specs absent leaves spec mode unset

- **GIVEN** `start --supervisor` (no `--specs`, no `--from-all-specs`)
- **WHEN** the CLI is parsed
- **THEN** `StartArgs` SHALL indicate no spec mode (falls through to standard branch selection)

### Requirement: --from-all-specs and --specs are mutually exclusive

The system SHALL reject any invocation that combines `--from-all-specs` (or its alias `--from-specs`) with `--specs`. clap's parse step SHALL produce an error before the command runs. The error message SHALL clearly state that the two flags express opposing intents and SHALL list both flags.

#### Scenario: --from-all-specs and --specs together are rejected

- **GIVEN** `start --from-all-specs --specs add-auth`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both `--from-all-specs` and `--specs`

#### Scenario: --from-specs alias and --specs together are also rejected

- **GIVEN** `start --from-specs --specs add-auth`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both flags
- **AND** the alias SHALL enforce the same mutual-exclusion rule as the canonical name
