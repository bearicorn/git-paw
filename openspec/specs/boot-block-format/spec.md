# boot-block-format Specification

## Purpose
TBD - created by archiving change boot-prompt-standard. Update Purpose after archive.
## Requirements
### Requirement: Standard boot block format

The system SHALL provide a standardized boot instruction block that contains exactly four essential runtime events: register, done, blocked, and question. The boot block SHALL use a consistent format with clear section headers and pre-expanded curl commands.

#### Scenario: Boot block contains all four essential events

- **WHEN** the boot block is generated
- **THEN** it SHALL contain sections for:
  1. REGISTER - Initial status publication
  2. DONE - Task completion reporting
  3. BLOCKED - Dependency waiting notification
  4. QUESTION - Uncertainty escalation

#### Scenario: Boot block uses consistent formatting

- **WHEN** the boot block is generated
- **THEN** it SHALL use the format:
  ```
  ## BOOT INSTRUCTIONS - DO NOT REMOVE
  
  1. REGISTER: <instructions>
     <pre-expanded curl command>
  
  2. DONE: <instructions>
     <pre-expanded curl command>
  
  3. BLOCKED: <instructions>
     <pre-expanded curl command>
  
  4. QUESTION: <instructions>
     <pre-expanded curl command>
  ```

### Requirement: Boot block content requirements

The boot block SHALL include specific instructions for each event type:

1. **REGISTER**: Instruct agent to immediately publish working status with "booting" message
2. **DONE**: Instruct agent to publish agent.artifact with done status on completion
3. **BLOCKED**: Instruct agent to publish agent.blocked with dependency information
4. **QUESTION**: Instruct agent to publish agent.question and WAIT for answer

#### Scenario: Each event has clear instructions

- **WHEN** the boot block is examined
- **THEN** each of the four events SHALL have:
  - Clear one-line description of when to use it
  - Complete curl command with all required fields
  - Appropriate message content for the event type

#### Scenario: QUESTION event emphasizes waiting

- **WHEN** the QUESTION section is examined
- **THEN** it SHALL contain the phrase "DO NOT continue until you receive an answer!"
- **AND** the instruction SHALL be in bold or uppercase for emphasis

### Requirement: Paste handling instructions

The boot block SHALL include specific instructions for handling paste operations, particularly the requirement to send a second Enter key after pasted content.

#### Scenario: Paste handling instruction included

- **WHEN** the boot block is examined
- **THEN** it SHALL contain instructions about paste detection
- **AND** it SHALL mention the need for a second Enter key

#### Scenario: Paste instruction format

- **WHEN** the paste handling section is examined
- **THEN** it SHALL explain that Claude collapses pasted text into `[Pasted text #N]`
- **AND** it SHALL instruct agents to send an additional Enter after paste operations

