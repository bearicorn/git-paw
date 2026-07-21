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

### Requirement: --specs-format accepts speckit value

The system SHALL accept `speckit` as a valid value for the `--specs-format` CLI flag, alongside `openspec` and `markdown`. The flag's value SHALL override the `[specs]` config (there is no filesystem auto-detection to override).

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

### Requirement: Spec-system selection is explicit (config or CLI only)

The spec system SHALL be resolved from EXPLICIT sources only, in this precedence (highest first):

1. the `--specs-format` CLI value;
2. the `[specs]` section in `.git-paw/config.toml`.

git-paw SHALL NOT probe the filesystem to infer the spec system. When neither an `[specs]` section nor `--specs-format` is provided, spec scanning SHALL fail with an actionable error naming both remedies (add a `[specs]` section, or pass `--specs-format`). When `--specs-format` names a format but no `dir` is configured, the format's conventional directory SHALL be supplied (`.specify/specs` for `speckit`, `docs/superpowers/plans` for `superpowers`).

#### Scenario: Unconfigured repo errors even when layouts exist on disk

- **GIVEN** a repo with `.specify/specs/` and `docs/superpowers/plans/*.md` present on disk, no `[specs]` section, and no `--specs-format`
- **WHEN** spec scanning runs
- **THEN** it SHALL fail with an error naming `[specs]` and `--specs-format`
- **AND** it SHALL NOT infer a spec system from the filesystem

#### Scenario: Config [specs] is used verbatim regardless of on-disk layout

- **GIVEN** `[specs] type = "markdown"`, `dir = "specs"` in config, and a `.specify/specs/` directory also present on disk
- **WHEN** spec scanning runs
- **THEN** the Markdown backend SHALL be used (the `.specify/` layout is ignored)

#### Scenario: --specs-format supplies the format's conventional dir

- **WHEN** `--specs-format speckit` is passed with no configured `dir`
- **THEN** `specs.dir` SHALL default to `.specify/specs`
- **AND** `--specs-format superpowers` SHALL likewise default `specs.dir` to `docs/superpowers/plans`

