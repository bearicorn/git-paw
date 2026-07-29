# skill-standardized Specification

## Purpose
Supports the agentskills.io standardized skill format — a directory containing a `SKILL.md` plus optional resource subdirectories — auto-detecting the directory format and loading/parsing such skills, and validates them against the agentskills.io schema, accepting conforming skills and rejecting those with missing or invalid fields via clear, actionable error messages that name the specific offending fields.

## Requirements
### Requirement: Standardized skill format
The system SHALL support the agentskills.io standardized format for agent skills, which includes a directory structure with SKILL.md as the main file plus optional subdirectories for resources.

#### Scenario: Load standardized skill
- **WHEN** a skill directory contains SKILL.md file
- **THEN** the system SHALL load and parse the skill using the standardized format

#### Scenario: Validate standardized skill structure
- **WHEN** loading a standardized skill
- **THEN** the system SHALL validate that required fields are present

### Requirement: Format detection
The system SHALL automatically detect standardized skill format.

#### Scenario: Format detection for directory-based skills
- **WHEN** a skill is in directory format with SKILL.md
- **THEN** the system SHALL identify it as standardized format

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
