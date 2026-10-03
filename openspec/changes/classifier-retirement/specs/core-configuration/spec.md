## MODIFIED Requirements

### Requirement: Configuration reference documents the configuration tables

`docs/src/configuration/README.md` SHALL include subsections
documenting `[supervisor.conflict]`, `[supervisor.learnings_config]`,
and `[governance]`. Each subsection SHALL list every field, its type,
its default value (where applicable), and a one-line description.
It SHALL also document `[supervisor.auto_approve]` and
`[supervisor.common_dev_allowlist]` as deprecated: both are still
accepted, have no effect, print a deprecation warning when present,
and can be removed.

#### Scenario: Configuration reference documents supervisor.conflict

- **WHEN** `docs/src/configuration/README.md` is inspected
- **THEN** it contains a subsection or table for `[supervisor.conflict]`
- **AND** the subsection mentions `window_seconds`, `warn_on_intent_overlap`, and `escalate_on_violation`

#### Scenario: Configuration reference marks the auto-approval tables deprecated

- **WHEN** `docs/src/configuration/README.md` is inspected
- **THEN** it names `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]`
- **AND** states that both are deprecated and have no effect
- **AND** points to the agent CLI's own permission rules, or the `full-auto` approval level inside a sandbox, as the replacement

#### Scenario: Configuration reference documents supervisor.learnings_config

- **WHEN** `docs/src/configuration/README.md` is inspected
- **THEN** it contains a subsection or table for `[supervisor.learnings_config]`
- **AND** the subsection mentions `flush_interval_seconds`

#### Scenario: Configuration reference documents governance

- **WHEN** `docs/src/configuration/README.md` is inspected
- **THEN** it contains a subsection or table for `[governance]`
- **AND** the subsection lists all five fields: `adr`, `test_strategy`, `security`, `dod`, `constitution`
