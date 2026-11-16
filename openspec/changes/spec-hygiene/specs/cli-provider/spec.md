## MODIFIED Requirements

### Requirement: Usage is a provider facet with an unknown fallback

A provider SHALL be able to declare how its CLI reports a session's token usage — input, output,
cache-read and cache-write tokens, and a cost figure only when the CLI itself reports one. The
`claude` family SHALL declare a usage source that locates a worker's report through the session id
git-paw assigned at launch. Every other family, and any session without an assigned id, SHALL
resolve usage as unknown with a stated reason.

#### Scenario: A known report yields figures

- **GIVEN** a recorded Claude-family usage report for a session
- **WHEN** it is parsed
- **THEN** the input, output, cache-read and cache-write token totals SHALL equal the sums in the
  report

Test: `src/agent_cli/usage.rs::a_known_report_yields_the_sums_counting_each_response_once`

#### Scenario: A generic CLI's usage is unknown

- **GIVEN** a worker running a `generic`-family CLI
- **WHEN** its usage is resolved
- **THEN** the result SHALL be unknown with a reason naming the CLI

Test: `src/agent_cli/usage.rs::a_generic_cli_is_unknown_with_a_reason_naming_the_cli`

#### Scenario: A session without an assigned id is unknown

- **GIVEN** a Claude-family worker with no git-paw-assigned session id
- **WHEN** its usage is resolved
- **THEN** the result SHALL be unknown with a stated reason

Test: `src/agent_cli/usage.rs::a_session_without_an_assigned_id_is_unknown`

### Requirement: Usage reports are parsed without exposing their content

Turning a report's text into figures SHALL be a pure function. A report that cannot be parsed SHALL
yield unknown, never a partial or guessed figure. Only the usage figures SHALL be read into memory;
no other report content SHALL be logged, stored or shown in an unknown reason.

#### Scenario: A malformed report yields unknown

- **GIVEN** a Claude-family report whose usage records cannot be parsed
- **WHEN** it is parsed
- **THEN** the result SHALL be unknown, not a partial total

Test: `src/agent_cli/usage.rs::a_malformed_report_is_unknown_not_a_partial_total`, `src/agent_cli/usage.rs::a_line_that_is_not_json_is_unknown`

#### Scenario: An unknown reason carries no report content

- **GIVEN** a malformed report whose unparseable record contains transcript text
- **WHEN** it is parsed
- **THEN** the unknown reason SHALL NOT contain that text

Test: `src/agent_cli/usage.rs::a_malformed_report_is_unknown_not_a_partial_total`

### Requirement: A CLI entry's settings path locates its usage reports

For a `claude`-family entry, the parent directory of an absolute `[clis.<name>].settings_path`
(after `~` expansion) SHALL be the first directory searched for the session's usage reports, before
`CLAUDE_CONFIG_DIR` and `~/.claude`. The settings file itself SHALL NOT be read or written.

#### Scenario: A relocated variant's reports are found

- **GIVEN** `[clis.claude-oss]` with `family = "claude"` and `settings_path` inside a directory that
  holds the session's reports
- **WHEN** that worker's usage is resolved
- **THEN** the reports under that directory SHALL be read

Test: `src/agent_cli/usage.rs::the_adapter_reads_the_session_and_its_subagents_from_the_relocated_dir`, `src/agent_cli/usage.rs::config_dirs_are_searched_entry_then_env_then_home`
