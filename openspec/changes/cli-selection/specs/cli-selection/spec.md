## ADDED Requirements

### Requirement: CLI resolution chain for spec-driven launches

The system SHALL resolve which CLI to use for each spec-driven branch using a 5-level priority chain, from highest to lowest priority.

#### Scenario: --cli flag overrides everything
- **WHEN** `--cli claude` is passed and specs have various `paw_cli` values
- **THEN** all branches SHALL use `"claude"` regardless of spec or config values

#### Scenario: paw_cli in spec overrides config
- **WHEN** no `--cli` flag is passed and a spec has `paw_cli: gemini`
- **THEN** that branch SHALL use `"gemini"` regardless of `default_spec_cli` or `default_cli`

#### Scenario: default_spec_cli fills remaining without prompt
- **WHEN** no `--cli` flag, some specs have no `paw_cli`, and `default_spec_cli = "claude"` in config
- **THEN** specs without `paw_cli` SHALL use `"claude"` with no interactive prompt

#### Scenario: default_cli pre-selects in picker
- **WHEN** no `--cli` flag, no `paw_cli`, no `default_spec_cli`, and `default_cli = "claude"` in config
- **THEN** the CLI picker SHALL be shown with `"claude"` pre-selected

#### Scenario: No defaults — full picker
- **WHEN** no `--cli` flag, no `paw_cli`, no `default_spec_cli`, and no `default_cli`
- **THEN** the CLI picker SHALL be shown with no pre-selection

### Requirement: Mixed resolution across specs

The system SHALL handle specs where some have `paw_cli` and others don't in the same launch.

#### Scenario: Mix of paw_cli and default_spec_cli
- **WHEN** 3 specs are launched, 1 has `paw_cli: gemini`, and `default_spec_cli = "claude"`
- **THEN** the gemini spec SHALL use `"gemini"` and the other 2 SHALL use `"claude"`

#### Scenario: Mix of paw_cli and interactive
- **WHEN** 3 specs are launched, 1 has `paw_cli: gemini`, no `default_spec_cli`, and user picks `"claude"` in the prompt
- **THEN** the gemini spec SHALL use `"gemini"` and the other 2 SHALL use `"claude"`

### Requirement: Prompt at most once

The system SHALL prompt the user for CLI selection at most once during a `--from-specs` launch, applying the choice to all branches without a `paw_cli` or `default_spec_cli`.

#### Scenario: Single prompt for remaining branches
- **WHEN** 5 specs are launched, 2 have `paw_cli`, and the picker fires for the remaining 3
- **THEN** the picker SHALL fire once and the chosen CLI SHALL be applied to all 3

### Requirement: Validate resolved CLI names

The system SHALL validate that each resolved CLI name matches an available CLI.

#### Scenario: paw_cli references unknown CLI
- **WHEN** a spec has `paw_cli: nonexistent` and no CLI named `"nonexistent"` is available
- **THEN** the system SHALL return `PawError::CliNotFound("nonexistent")`

#### Scenario: default_spec_cli references unknown CLI
- **WHEN** `default_spec_cli = "nonexistent"` and no such CLI is available
- **THEN** the system SHALL return `PawError::CliNotFound("nonexistent")`

### Requirement: No prompt when fully resolved

The system SHALL not show any interactive prompt when all branches are resolved via `--cli`, `paw_cli`, or `default_spec_cli`.

#### Scenario: All resolved without prompt
- **WHEN** `--cli claude` is passed
- **THEN** no interactive prompt SHALL be shown

#### Scenario: All resolved via paw_cli and default_spec_cli
- **WHEN** every spec has `paw_cli` or `default_spec_cli` covers the rest
- **THEN** no interactive prompt SHALL be shown
