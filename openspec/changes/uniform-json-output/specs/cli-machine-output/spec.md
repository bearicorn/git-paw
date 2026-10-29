## Purpose

The contract every git-paw command's `--json` output follows, so scripts, CI, and external orchestrators can consume git-paw programmatically across releases. A command in JSON mode prints exactly one JSON object on stdout carrying a top-level `schema_version`, keeps snake_case keys and `null` for absent values, emits no terminal escape sequences, preserves its human-mode exit code, and prints nothing on stdout when it fails. Within a schema version the documents evolve additively only. The capability also fixes the roster of commands that provide `--json` and the standing guard that keeps the roster, the code, and the CLI reference in step.

## ADDED Requirements

### Requirement: JSON mode prints exactly one versioned JSON object on stdout

When a command that supports `--json` is run with it, the command SHALL write exactly one JSON object to stdout, followed by a single newline, and nothing else. The object SHALL carry a top-level integer key `schema_version`, equal to 1 for this contract. Keys SHALL be snake_case, a value that is absent SHALL be `null` rather than omitted where the command's schema names the key, and the output SHALL contain no ANSI or other terminal escape sequence regardless of the terminal, `NO_COLOR`, or whether stdout is a TTY. Human-oriented progress, hints, and warnings SHALL go to stderr or be suppressed. Insignificant whitespace in the document SHALL NOT be part of the contract.

#### Scenario: Output parses as a single object with a version

- **GIVEN** any command on the JSON roster
- **WHEN** it is run with `--json` and succeeds
- **THEN** stdout SHALL parse as exactly one JSON object with no trailing data
- **AND** the object SHALL contain an integer `schema_version` equal to 1

#### Scenario: No escape sequences in JSON mode

- **GIVEN** a terminal that supports color
- **WHEN** any roster command is run with `--json`
- **THEN** stdout SHALL contain no `ESC` (0x1B) byte

### Requirement: JSON mode keeps the human-mode exit code and keeps errors off stdout

A command run with `--json` SHALL exit with the same status it would exit with in human mode for the same inputs and state. When the command fails, it SHALL write nothing to stdout and SHALL report the error on stderr exactly as in human mode. A command whose result itself reports failures (as `doctor` reports failing checks) SHALL still print its complete document before exiting non-zero.

#### Scenario: A failing command leaves stdout empty

- **GIVEN** a directory that is not inside a git repository
- **WHEN** `git paw status --json` is run there
- **THEN** stdout SHALL be empty
- **AND** stderr SHALL carry the same error message the human mode prints
- **AND** the exit code SHALL equal the human mode's exit code

#### Scenario: Doctor prints its document and then exits non-zero

- **GIVEN** an environment with at least one failing doctor check
- **WHEN** `git paw doctor --json` is run
- **THEN** stdout SHALL be the complete doctor document
- **AND** the process SHALL exit non-zero

### Requirement: JSON documents evolve additively within a schema version

Within a given `schema_version`, a JSON document SHALL only gain keys: an existing key SHALL NOT be removed, renamed, or change its type or meaning. Any such incompatible change SHALL increment `schema_version`. Consumers are expected to ignore unknown keys, and the CLI reference SHALL state this rule.

#### Scenario: Each roster document's key set is pinned

- **GIVEN** the golden key set recorded for each roster command's document
- **WHEN** the command's JSON is produced
- **THEN** every recorded key SHALL be present with its recorded type

### Requirement: The JSON roster is fixed and guarded

The commands that provide `--json` SHALL be exactly: `status`, `doctor`, `approvals`, `add`, `remove`, `list-clis`, and `config check`. A standing test SHALL enumerate every subcommand that defines a `--json` flag and SHALL fail if that set differs from this roster, or if the CLI reference does not document the JSON document of every roster command. Adding `--json` to another command SHALL add it to this roster in the same change.

#### Scenario: The flag set matches the roster

- **WHEN** the CLI definition is enumerated for subcommands defining `--json`
- **THEN** the set SHALL equal `status`, `doctor`, `approvals`, `add`, `remove`, `list-clis`, `config check`

#### Scenario: Every roster command is documented

- **WHEN** the CLI reference is inspected
- **THEN** it SHALL document the JSON document of each roster command, including `schema_version`
