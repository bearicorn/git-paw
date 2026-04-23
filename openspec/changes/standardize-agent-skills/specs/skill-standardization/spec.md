## ADDED Requirements

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