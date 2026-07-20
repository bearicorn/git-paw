## ADDED Requirements

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
