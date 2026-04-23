## ADDED Requirements

### Requirement: Manual mode boot block pre-fill

In manual broker mode (without supervisor), the system SHALL pre-fill the boot instruction block into each agent pane's input line without sending an Enter key. This allows users to paste their actual task after the boot instructions.

#### Scenario: Boot block pre-filled without Enter

- **GIVEN** broker-enabled session in manual mode
- **WHEN** agent panes are created
- **THEN** boot block SHALL be sent to input line
- **AND** no Enter key SHALL be sent
- **AND** cursor SHALL remain at end of boot block

#### Scenario: User can append task after boot block

- **GIVEN** boot block pre-filled in agent pane
- **WHEN** user pastes task instructions
- **THEN** task appears after boot block
- **AND** user can press Enter to submit combined content

### Requirement: Manual mode injection timing

The system SHALL inject boot blocks in manual mode immediately after tmux session creation, before returning control to the user.

#### Scenario: Boot blocks injected during session setup

- **GIVEN** `git paw start --from-specs --cli claude` (manual mode)
- **WHEN** tmux session is created
- **THEN** boot blocks SHALL be pre-filled before command returns

### Requirement: Manual mode configuration

The system SHALL respect the same boot block configuration in manual mode as in supervisor mode, ensuring consistent behavior across all usage patterns.

#### Scenario: Configuration applies to manual mode

- **GIVEN** boot block configuration enabled
- **WHEN** manual mode session starts
- **THEN** boot blocks SHALL be pre-filled using same template

#### Scenario: Disabled configuration affects both modes

- **GIVEN** boot block configuration disabled (if implemented)
- **WHEN** manual mode session starts
- **THEN** no boot blocks SHALL be pre-filled