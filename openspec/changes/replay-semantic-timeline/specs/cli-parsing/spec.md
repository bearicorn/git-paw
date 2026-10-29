## MODIFIED Requirements

### Requirement: Replay subcommand

The `replay` subcommand SHALL accept an optional `<agent>` positional argument and the flags
`--list`, `--session <name>`, `--json` and `--raw`. `--raw` SHALL require `<agent>` and SHALL conflict
with `--json` and `--list`. With no argument and no flag, `replay` SHALL parse successfully and select
the whole-session timeline. The `--color` flag SHALL NOT be accepted.

#### Scenario: Replay with agent
- **WHEN** `replay feat/add-auth` is passed
- **THEN** `agent` SHALL be `Some("feat/add-auth")`, and `list`, `json` and `raw` SHALL be `false`

#### Scenario: Replay with no arguments
- **WHEN** `replay` is passed with no arguments
- **THEN** parsing SHALL succeed with `agent` `None` and `list` `false`

#### Scenario: Replay with --list
- **WHEN** `replay --list` is passed
- **THEN** `list` SHALL be `true` and `agent` SHALL be `None`

#### Scenario: Replay with --session
- **WHEN** `replay feat/add-auth --session paw-myproject` is passed
- **THEN** `session` SHALL be `Some("paw-myproject")`

#### Scenario: Replay with --json
- **WHEN** `replay --json` is passed
- **THEN** `json` SHALL be `true`

#### Scenario: Replay with --raw
- **WHEN** `replay feat/add-auth --raw` is passed
- **THEN** `raw` SHALL be `true` and `agent` SHALL be `Some("feat/add-auth")`

#### Scenario: --raw without an agent is rejected
- **WHEN** `replay --raw` is passed
- **THEN** parsing SHALL fail with an error indicating that an agent is required

#### Scenario: --raw with --json is rejected
- **WHEN** `replay feat/add-auth --raw --json` is passed
- **THEN** parsing SHALL fail with a conflict error

#### Scenario: --color is no longer accepted
- **WHEN** `replay feat/add-auth --color` is passed
- **THEN** parsing SHALL fail with an unexpected-argument error

#### Scenario: Replay help text
- **WHEN** `replay --help` is passed
- **THEN** stdout SHALL describe `<agent>`, `--list`, `--session`, `--json` and `--raw`, with examples
- **AND** it SHALL state that `--raw` output is written unmodified, escape sequences included
