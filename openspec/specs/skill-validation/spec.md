# skill-validation Specification

## Purpose
TBD - created by archiving change standardize-agent-skills. Update Purpose after archive.
## Requirements
### Requirement: Schema validation
The system SHALL validate that new format skills conform to the agentskills.io schema specification.

#### Scenario: Valid skill passes validation
- **WHEN** a standardized skill conforms to the schema
- **THEN** the system SHALL accept and load the skill

#### Scenario: Invalid skill fails validation
- **WHEN** a standardized skill has missing required fields
- **THEN** the system SHALL reject the skill with clear error message

### Requirement: Validation error reporting
The system SHALL provide clear and actionable error messages when skill validation fails.

#### Scenario: Detailed validation errors
- **WHEN** skill validation fails
- **THEN** the system SHALL report specific missing or invalid fields

