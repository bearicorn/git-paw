# template-substitution Specification

## Purpose
TBD - created by archiving change boot-prompt-standard. Update Purpose after archive.
## Requirements
### Requirement: Template variable substitution

The system SHALL support template variable substitution in boot blocks using the syntax `{{VARIABLE_NAME}}`. The system SHALL replace these variables with actual values at render time.

#### Scenario: Branch ID substitution

- **GIVEN** boot block template containing `{{BRANCH_ID}}`
- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** all occurrences of `{{BRANCH_ID}}` SHALL be replaced with `"feat-errors"`

#### Scenario: Broker URL substitution

- **GIVEN** boot block template containing `{{GIT_PAW_BROKER_URL}}`
- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** all occurrences of `{{GIT_PAW_BROKER_URL}}` SHALL be replaced with `"http://localhost:9119"`

### Requirement: Branch ID slugification

The system SHALL apply slugification to branch IDs during substitution to ensure valid agent IDs. Slugification SHALL replace `/` with `-` and remove any special characters.

#### Scenario: Branch slugification

- **GIVEN** branch name `"feat/errors"`
- **WHEN** substituted into boot block
- **THEN** it SHALL become `"feat-errors"`

#### Scenario: Complex branch name slugification

- **GIVEN** branch name `"fix/topological-cycle-fallback"`
- **WHEN** substituted into boot block
- **THEN** it SHALL become `"fix-topological-cycle-fallback"`

### Requirement: Pre-expansion at render time

The system SHALL expand all template variables before the boot block is injected into agent panes. This SHALL prevent shell expansion permission prompts in agent CLIs.

#### Scenario: All templates expanded before injection

- **GIVEN** boot block template with multiple `{{VARIABLE}}` placeholders
- **WHEN** `build_boot_block()` returns
- **THEN** the returned string SHALL contain no `{{` or `}}` characters
- **AND** all variables SHALL be replaced with actual values

#### Scenario: Invalid template variables handled gracefully

- **GIVEN** boot block template with unknown variable `{{UNKNOWN_VAR}}`
- **WHEN** `build_boot_block()` is called
- **THEN** the unknown variable SHALL be left as-is (no crash)
- **AND** a warning SHALL be logged

