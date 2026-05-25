## ADDED Requirements

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
