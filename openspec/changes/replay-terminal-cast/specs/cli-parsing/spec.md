## ADDED Requirements

### Requirement: Replay --cast flag

The `replay` subcommand SHALL accept a `--cast` flag that requires the `<agent>` argument and
conflicts with `--raw`, `--json` and `--list`. The `replay --help` text SHALL describe `--cast` as
animated playback through `asciinema`, with an example.

#### Scenario: Replay with --cast
- **WHEN** `replay feat/add-auth --cast` is passed
- **THEN** `cast` SHALL be `true` and `agent` SHALL be `Some("feat/add-auth")`

#### Scenario: --cast without an agent is rejected
- **WHEN** `replay --cast` is passed
- **THEN** parsing SHALL fail with an error indicating that an agent is required

#### Scenario: --cast with --raw is rejected
- **WHEN** `replay feat/add-auth --cast --raw` is passed
- **THEN** parsing SHALL fail with a conflict error

#### Scenario: Help describes --cast
- **WHEN** `replay --help` is passed
- **THEN** stdout SHALL describe `--cast` and mention `asciinema`
