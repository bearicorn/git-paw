## ADDED Requirements

### Requirement: Add subcommand accepts --dry-run

The `add` subcommand SHALL accept an optional `--dry-run` flag, combinable with a branch or `--from-spec`, `--cli` or `--profile`, `--no-rebase` and `--json`. Its help SHALL say it previews the added pane's plan without changing anything and that the `runs:` line is the pane command.

#### Scenario: Add parses --dry-run

- **GIVEN** `add feat/x --dry-run` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Add` with `branch` `feat/x` and `dry_run` true

#### Scenario: Add defaults to no dry run

- **GIVEN** `add feat/x` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Add` with `dry_run` false

#### Scenario: --dry-run combines with --from-spec

- **GIVEN** `add --from-spec add-export --dry-run` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Add` with `from_spec` `add-export` and `dry_run` true
