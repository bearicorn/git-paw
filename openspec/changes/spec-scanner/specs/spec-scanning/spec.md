## ADDED Requirements

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
