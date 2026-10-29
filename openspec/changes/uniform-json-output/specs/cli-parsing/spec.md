## MODIFIED Requirements

### Requirement: Status subcommand

The `status` subcommand SHALL parse with no positional arguments and SHALL accept an optional `--json` flag selecting the machine-readable document defined by `cli-machine-output`.

#### Scenario: Status parses
- **GIVEN** `status` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Status` with `json` false

Test: `cli::tests::status_parses`

#### Scenario: Status accepts --json
- **GIVEN** `status --json` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::Status` with `json` true

Test: `cli::tests::status_json_flag_parses`

### Requirement: List-CLIs subcommand

The `list-clis` subcommand SHALL parse with no positional arguments and SHALL accept an optional `--json` flag. With `--json` it SHALL print a `cli-machine-output` document containing `schema_version` and `clis`, an array whose entries carry `name` (the binary name), `display_name`, `source` (`detected` or `custom`), and `path`. When no CLI is found, `clis` SHALL be an empty array and the command SHALL exit 0.

#### Scenario: List-CLIs parses
- **GIVEN** `list-clis` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::ListClis` with `json` false

Test: `cli::tests::list_clis_parses`

#### Scenario: List-CLIs accepts --json
- **GIVEN** `list-clis --json` is passed
- **WHEN** the CLI is parsed
- **THEN** the command SHALL be `Command::ListClis` with `json` true

#### Scenario: list-clis --json lists detected and custom CLIs
- **GIVEN** a custom CLI `my-agent` registered in config and resolvable on PATH
- **WHEN** `git paw list-clis --json` is run
- **THEN** `clis` SHALL contain an entry with `name` `"my-agent"`, `source` `"custom"`, and its resolved `path`

#### Scenario: An empty roster is an empty array
- **GIVEN** no detected or custom CLI resolves
- **WHEN** `git paw list-clis --json` is run
- **THEN** the document SHALL contain `"clis": []` and the command SHALL exit 0
