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
