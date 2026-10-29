## ADDED Requirements

### Requirement: `--profile` flag on start and add

`git paw start` and `git paw add` SHALL accept `--profile <NAME>` selecting a worker profile for every branch they launch, with a `help` string and an example in each subcommand's `long_about`. `--profile` SHALL conflict with `--cli` on both subcommands and with `--preset` on `start`; a conflicting combination SHALL be rejected at parse time naming both flags. `git paw add --from-spec <spec> --profile <NAME>` SHALL launch the spec's branch with the named profile.

#### Scenario: start parses --profile

- **WHEN** `git paw start --profile routine --branches feat/a` is parsed
- **THEN** the start command SHALL carry profile `routine` and branch `feat/a`

#### Scenario: add parses --profile

- **WHEN** `git paw add feat/a --profile routine` is parsed
- **THEN** the add command SHALL carry profile `routine`

#### Scenario: --profile conflicts with --cli and --preset

- **WHEN** `git paw start --profile routine --cli claude`, `git paw start --profile routine --preset dev` and `git paw add feat/a --profile routine --cli claude` are parsed
- **THEN** each SHALL be rejected naming both conflicting flags

### Requirement: Delegate subcommand

The system SHALL provide `git paw delegate [NAME] [--task <TEXT>] [--list]` with `about`, a `long_about` carrying examples, and a `help` string on every argument. `NAME` SHALL be required unless `--list` is given; `--list` SHALL conflict with `NAME` and `--task`. The subcommand SHALL be listed in the root help and in the CLI reference.

#### Scenario: delegate parses a name and a task

- **WHEN** `git paw delegate explorer --task "map the config loader"` is parsed
- **THEN** the delegate command SHALL carry name `explorer` and that task

#### Scenario: delegate --list needs no name

- **WHEN** `git paw delegate --list` is parsed
- **THEN** parsing SHALL succeed with the list mode selected

#### Scenario: delegate without a name or --list is rejected

- **WHEN** `git paw delegate` is parsed
- **THEN** parsing SHALL fail stating that a name or `--list` is required

#### Scenario: delegate appears in help

- **WHEN** `git paw --help` runs
- **THEN** its output SHALL list the `delegate` subcommand
