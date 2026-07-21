## Purpose

Discover and represent pending specs using a pluggable backend system, providing a unified `SpecEntry` type, backend dispatch by config, branch name derivation, and actionable error reporting for scanning failures.
## Requirements
### Requirement: SpecEntry represents a discovered spec

The system SHALL represent each discovered spec as a `SpecEntry` with an id, derived branch name, optional CLI override, prompt content, and optional file ownership list.

#### Scenario: SpecEntry has all fields populated
- **WHEN** a `SpecEntry` is constructed with id, branch, cli, prompt, and owned_files
- **THEN** all fields SHALL be accessible

#### Scenario: SpecEntry with optional fields absent
- **WHEN** a `SpecEntry` is constructed without cli or owned_files
- **THEN** `cli` SHALL be `None` and `owned_files` SHALL be `None`

### Requirement: SpecBackend trait for format-specific scanning

The system SHALL define a `SpecBackend` trait with a `scan` method that takes a directory path and returns a list of `SpecEntry` results.

#### Scenario: Backend returns discovered specs
- **WHEN** a `SpecBackend` implementation scans a directory with pending specs
- **THEN** it SHALL return a `Vec<SpecEntry>` with one entry per pending spec

#### Scenario: Backend returns empty list when no pending specs
- **WHEN** a `SpecBackend` implementation scans a directory with no pending specs
- **THEN** it SHALL return an empty `Vec`

### Requirement: Scan specs from config

The system SHALL provide a `scan_specs` function that reads the `[specs]` config section, selects the correct backend, and returns discovered specs.

#### Scenario: Scan with valid config and pending specs
- **WHEN** `scan_specs()` is called with a config that has `specs.dir` and `specs.type` set, and the directory contains pending specs
- **THEN** it SHALL return the specs discovered by the matching backend

#### Scenario: Scan with no specs config
- **WHEN** `scan_specs()` is called with a config that has no `[specs]` section
- **THEN** it SHALL return `PawError::SpecError` indicating specs are not configured

#### Scenario: Scan with nonexistent specs directory
- **WHEN** `scan_specs()` is called and the configured `specs.dir` does not exist
- **THEN** it SHALL return `PawError::SpecError` mentioning the missing directory

#### Scenario: Scan with specs directory that is a file
- **WHEN** `scan_specs()` is called and `specs.dir` points to a file, not a directory
- **THEN** it SHALL return `PawError::SpecError`

#### Scenario: Scan with unknown spec type
- **WHEN** `scan_specs()` is called with `specs.type = "unknown"`
- **THEN** it SHALL return `PawError::SpecError` mentioning the unknown type

### Requirement: Branch name derivation

The system SHALL derive branch names by concatenating the configured `branch_prefix` with the spec's `id`.

#### Scenario: Default branch prefix
- **WHEN** `branch_prefix` is not set in config and a spec has `id = "add-auth"`
- **THEN** the derived branch SHALL be `"spec/add-auth"`

#### Scenario: Custom branch prefix
- **WHEN** `branch_prefix = "feat/"` and a spec has `id = "add-auth"`
- **THEN** the derived branch SHALL be `"feat/add-auth"`

#### Scenario: Branch prefix with no trailing slash
- **WHEN** `branch_prefix = "spec"` (no trailing slash) and a spec has `id = "add-auth"`
- **THEN** the derived branch SHALL be `"spec/add-auth"` (slash inserted automatically)

### Requirement: Backend dispatch by spec type

The system SHALL select the correct `SpecBackend` implementation based on the `specs.type` config field.

#### Scenario: Type "openspec" selects OpenSpec backend
- **WHEN** `specs.type = "openspec"`
- **THEN** the OpenSpec backend SHALL be used for scanning

#### Scenario: Type "markdown" selects Markdown backend
- **WHEN** `specs.type = "markdown"`
- **THEN** the Markdown backend SHALL be used for scanning

### Requirement: SpecError for scanning failures

The system SHALL use `PawError::SpecError` for all spec scanning failures with actionable messages.

#### Scenario: SpecError includes directory path
- **WHEN** a spec directory is missing
- **THEN** the error message SHALL include the path that was not found

#### Scenario: SpecError includes spec type
- **WHEN** an unknown spec type is configured
- **THEN** the error message SHALL include the unknown type name

### Requirement: Backend dispatch for Spec Kit type

The system SHALL select the `SpecKitBackend` implementation when `specs.type = "speckit"` is configured. The dispatch SHALL be additive to the existing dispatch table — `"openspec"` and `"markdown"` dispatch SHALL continue to work unchanged.

#### Scenario: Type "speckit" selects SpecKit backend

- **WHEN** `specs.type = "speckit"` is configured
- **THEN** the SpecKit backend SHALL be used for scanning

#### Scenario: Existing types continue to dispatch correctly

- **WHEN** `specs.type = "openspec"` or `"markdown"` is configured
- **THEN** the corresponding existing backend SHALL be used for scanning
- **AND** the SpecKit backend SHALL NOT be invoked

#### Scenario: Unknown type still produces a SpecError

- **WHEN** `specs.type = "unrecognised"` is configured
- **THEN** the system SHALL return a `PawError::SpecError` mentioning the unknown type
- **AND** the error message SHALL list the known types including `"speckit"`

### Requirement: Auto-detection of Spec Kit projects

The system SHALL probe for a `.specify/` directory at the repository root when both of the following are true:

- The user has not set `[specs]` in `.git-paw/config.toml` (no `type` and no `dir`).
- The user has not passed `--specs-format` on the CLI.

When auto-detection runs and finds `.specify/` (a directory containing a `specs/` subdirectory), the system SHALL behave as if the user had configured `specs.type = "speckit"` and `specs.dir = ".specify/specs"`. Explicit configuration (either via TOML or `--specs-format`) SHALL always take precedence over auto-detection.

If `.specify/` exists but is not a directory, or its `specs/` subdirectory does not exist, auto-detection SHALL NOT activate the SpecKit backend.

#### Scenario: Auto-detection activates SpecKit backend in unconfigured project

- **GIVEN** a repository containing `.specify/specs/` and no `[specs]` section in `.git-paw/config.toml`
- **AND** `--specs-format` is not passed
- **WHEN** spec scanning runs
- **THEN** the SpecKit backend SHALL be used
- **AND** `specs.dir` SHALL be `.specify/specs`

#### Scenario: Explicit config in TOML wins over auto-detection

- **GIVEN** a repository containing `.specify/specs/` and `.git-paw/config.toml` with `[specs] type = "markdown"`
- **WHEN** spec scanning runs
- **THEN** the Markdown backend SHALL be used
- **AND** auto-detection SHALL NOT activate the SpecKit backend

#### Scenario: --specs-format CLI flag wins over auto-detection

- **GIVEN** a repository containing `.specify/specs/` and no `[specs]` section in config
- **WHEN** the user passes `--specs-format openspec` on the CLI
- **THEN** the OpenSpec backend SHALL be used

#### Scenario: Missing .specify/ does not activate SpecKit backend

- **GIVEN** a repository with no `.specify/` directory and no `[specs]` config
- **WHEN** spec scanning runs
- **THEN** the SpecKit backend SHALL NOT be activated
- **AND** the system SHALL fall through to its existing "specs not configured" behaviour

#### Scenario: Auto-detection skipped when .specify/specs/ is missing

- **GIVEN** a repository with `.specify/memory/constitution.md` but no `.specify/specs/` directory
- **WHEN** spec scanning runs without explicit config
- **THEN** SpecKit auto-detection SHALL NOT activate

### Requirement: --specs-format accepts speckit value

The system SHALL accept `speckit` as a valid value for the `--specs-format` CLI flag, alongside `openspec` and `markdown`. The flag's value SHALL override both auto-detection and TOML config.

#### Scenario: --specs-format speckit selects SpecKit backend

- **WHEN** `--specs-format speckit` is passed
- **THEN** the SpecKit backend SHALL be used regardless of any `[specs] type` set in config

#### Scenario: --specs-format with unknown value is rejected

- **WHEN** `--specs-format unknown-value` is passed
- **THEN** the CLI SHALL reject the invocation with an error listing valid values: `openspec`, `markdown`, `speckit`

### Requirement: Backend dispatch for Superpowers type

The system SHALL select the `SuperpowersBackend` implementation when `specs.type = "superpowers"` is configured. The dispatch SHALL be additive to the existing dispatch table — `"openspec"`, `"markdown"`, and `"speckit"` dispatch SHALL continue to work unchanged.

#### Scenario: Type "superpowers" selects Superpowers backend

- **WHEN** `specs.type = "superpowers"` is configured
- **THEN** the Superpowers backend SHALL be used for scanning

#### Scenario: Existing types continue to dispatch correctly

- **WHEN** `specs.type = "openspec"`, `"markdown"`, or `"speckit"` is configured
- **THEN** the corresponding existing backend SHALL be used for scanning
- **AND** the Superpowers backend SHALL NOT be invoked

#### Scenario: Unknown type error lists superpowers among known types

- **WHEN** `specs.type = "unrecognised"` is configured
- **THEN** the system SHALL return a `PawError::SpecError` mentioning the unknown type
- **AND** the error message SHALL list the known types including `"superpowers"`

### Requirement: --specs-format accepts superpowers value

The system SHALL accept `superpowers` as a valid value for the `--specs-format` CLI flag, alongside `openspec`, `markdown`, and `speckit`. The flag's value SHALL override the `[specs]` config.

#### Scenario: --specs-format superpowers selects Superpowers backend

- **WHEN** `--specs-format superpowers` is passed
- **THEN** the Superpowers backend SHALL be used regardless of any `[specs] type` set in config

#### Scenario: --specs-format value list includes superpowers

- **WHEN** `--specs-format unknown-value` is passed
- **THEN** the CLI SHALL reject the invocation with an error listing valid values: `openspec`, `markdown`, `speckit`, `superpowers`

