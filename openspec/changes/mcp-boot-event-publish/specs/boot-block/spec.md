## MODIFIED Requirements

### Requirement: Standard boot block format

The system SHALL provide a standardized boot instruction block that contains exactly four essential runtime events: register, done, blocked, and question. The boot block SHALL use a consistent format with clear section headers and a pre-expanded invocation for each event.

Each of the four events SHALL be expressed either as an invocation of the bundled broker helper script or, when the target CLI speaks MCP, as an invocation of the corresponding MCP publish tool. The set of events, their names, and their meanings SHALL be identical in both forms; only the invocation mechanism differs. When the target CLI's MCP support is unknown or absent, the helper-script form SHALL be used.

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
     <pre-expanded invocation>

  2. DONE: <instructions>
     <pre-expanded invocation>

  3. BLOCKED: <instructions>
     <pre-expanded invocation>

  4. QUESTION: <instructions>
     <pre-expanded invocation>
  ```
- **AND** each `<pre-expanded invocation>` SHALL be either a bundled broker helper invocation or an MCP publish tool invocation, per the target CLI

#### Scenario: MCP-capable CLI receives the tool form

- **GIVEN** a target CLI that speaks MCP
- **WHEN** the boot block is rendered for that CLI
- **THEN** the four events SHALL be expressed as MCP publish tool invocations

#### Scenario: Non-MCP CLI receives the helper form

- **GIVEN** a target CLI that does not speak MCP
- **WHEN** the boot block is rendered for that CLI
- **THEN** the four events SHALL be expressed as bundled broker helper invocations

#### Scenario: Unknown MCP support falls back to the helper form

- **GIVEN** a target CLI whose MCP support is not known
- **WHEN** the boot block is rendered for that CLI
- **THEN** the helper-script form SHALL be used

#### Scenario: Both forms carry the same four events

- **WHEN** the MCP form and the helper form of the boot block are compared
- **THEN** both SHALL cover exactly register, done, blocked, and question
- **AND** neither SHALL add or omit an event relative to the other
