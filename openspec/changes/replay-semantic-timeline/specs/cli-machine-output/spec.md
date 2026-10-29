## MODIFIED Requirements

### Requirement: The JSON roster is fixed and guarded

The commands that provide `--json` SHALL be exactly: `status`, `doctor`, `approvals`, `add`, `remove`, `list-clis`, `config check`, `finish`, and `replay`. `replay` provides two JSON documents, the timeline and the `--list` listing, and each SHALL follow this capability's contract. A standing test SHALL enumerate every subcommand that defines a `--json` flag and SHALL fail if that set differs from this roster, or if the CLI reference does not document the JSON document of every roster command. Adding `--json` to another command SHALL add it to this roster in the same change.

#### Scenario: The flag set matches the roster

- **WHEN** the CLI definition is enumerated for subcommands defining `--json`
- **THEN** the set SHALL equal `status`, `doctor`, `approvals`, `add`, `remove`, `list-clis`, `config check`, `finish`, `replay`

#### Scenario: Every roster command is documented

- **WHEN** the CLI reference is inspected
- **THEN** it SHALL document the JSON document of each roster command, including `schema_version`
- **AND** for `replay` it SHALL document both the timeline document and the `--list` document

#### Scenario: Both replay documents carry the schema version

- **GIVEN** a repository with a session record
- **WHEN** `git paw replay --json` and `git paw replay --list --json` are run
- **THEN** each stdout SHALL parse as exactly one JSON object with an integer `schema_version` equal to 1
