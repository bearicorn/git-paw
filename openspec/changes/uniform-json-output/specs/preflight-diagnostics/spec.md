## ADDED Requirements

### Requirement: Doctor JSON is a versioned document with stable check ids

`git paw doctor --json` SHALL follow the `cli-machine-output` contract. The document SHALL contain `schema_version`, the overall `status` (`ok`, `warn`, or `fail`), and `checks`. Every check entry SHALL carry `id`, `group`, `name`, `status`, `detail`, and `remedy` (`null` for a passing check). `id` SHALL be a stable machine identifier of the form `<group>.<check>` built from lowercase letters, digits, and hyphens, SHALL be unique within the report, and SHALL NOT change when the check's display `name`, `detail`, or `remedy` text is reworded.

#### Scenario: Every check carries a well-formed unique id

- **WHEN** `git paw doctor --json` is run
- **THEN** every entry in `checks` SHALL carry an `id` matching `^[a-z0-9-]+\.[a-z0-9-]+$`
- **AND** no two entries SHALL share an `id`

#### Scenario: The document carries the schema version and existing fields

- **WHEN** `git paw doctor --json` is run
- **THEN** the document SHALL contain `"schema_version": 1`
- **AND** every check SHALL still carry `group`, `name`, `status`, `detail`, and `remedy`

#### Scenario: Ids are stable across runs

- **GIVEN** the same environment
- **WHEN** `git paw doctor --json` is run twice
- **THEN** both documents SHALL list the same set of `id` values

### Requirement: Doctor JSON reports a check that was not evaluated as skipped

In the `git paw doctor --json` document, a check's `status` SHALL be one of `ok`, `warn`, `fail`, or `skipped`. `skipped` SHALL mark a check that was not evaluated. The overall `status` SHALL NOT be `skipped`. It SHALL be the worst status among the evaluated checks, so a skipped check SHALL NOT change it.

#### Scenario: A skipped check is reported as skipped

- **GIVEN** an environment in which doctor does not evaluate at least one check
- **WHEN** `git paw doctor --json` is run
- **THEN** each such check entry SHALL have `"status": "skipped"`
- **AND** the overall `status` SHALL be `ok`, `warn`, or `fail`, never `skipped`

#### Scenario: Skipped checks do not change the overall status

- **GIVEN** a report whose evaluated checks are all `ok` or `warn` and at least one of which is `warn`, plus one `skipped` check
- **WHEN** the overall `status` is computed
- **THEN** it SHALL be `warn`, the value the evaluated checks alone produce
