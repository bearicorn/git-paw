## ADDED Requirements

### Requirement: Approvals JSON is a versioned document

`git paw approvals --json` SHALL follow the `cli-machine-output` contract. The document SHALL contain `schema_version`, `session`, and `approvals`, and each `approvals` entry SHALL keep `pattern`, `count`, `suggested_target`, `first_seen`, and `last_seen` with their current meanings. An empty or absent log SHALL yield an empty `approvals` array.

#### Scenario: The approvals document is versioned

- **GIVEN** a session whose manual-approvals log has entries
- **WHEN** `git paw approvals --json` is run
- **THEN** the document SHALL contain `"schema_version": 1`
- **AND** each `approvals` entry SHALL carry `pattern`, `count`, `suggested_target`, `first_seen`, and `last_seen`

#### Scenario: An empty log yields an empty array

- **GIVEN** a session with no manual-approvals log
- **WHEN** `git paw approvals --json` is run
- **THEN** the document SHALL contain `"approvals": []` and the command SHALL exit 0
