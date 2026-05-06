# supervisor-injection Specification

## Purpose
TBD - created by archiving change boot-prompt-standard. Update Purpose after archive.
## Requirements
### Requirement: Supervisor mode boot block prepending

In supervisor auto-start mode, the system SHALL prepend the boot instruction block to each agent's task prompt before injecting it into the tmux pane.

#### Scenario: Boot block prepended to agent prompts

- **GIVEN** agent task prompt "Implement error handling"
- **WHEN** `cmd_supervisor()` constructs the full prompt
- **THEN** the injected text SHALL be:
  ```
  <boot_block>\n\nImplement error handling
  ```

#### Scenario: Boot block comes before task content

- **GIVEN** an agent pane receiving its initial prompt
- **WHEN** the prompt is injected via `tmux send-keys`
- **THEN** the boot block SHALL appear first
- **AND** the actual task SHALL appear after two newlines

### Requirement: Supervisor boot block timing

The system SHALL inject boot blocks during the supervisor launch sequence, specifically after tmux session creation but before the supervisor CLI starts.

#### Scenario: Boot blocks injected before supervisor CLI starts

- **GIVEN** `cmd_supervisor()` is executing
- **WHEN** agent panes are created and initialized
- **THEN** boot blocks SHALL be injected before the supervisor CLI process begins
- **AND** before the 2-second boot delay completes

### Requirement: All agents receive boot blocks

In supervisor mode, the system SHALL ensure every coding agent pane receives the boot instruction block, regardless of whether it has a spec file or uses default prompt.

#### Scenario: Agents with specs receive boot blocks

- **GIVEN** agent with spec file content
- **WHEN** prompt is constructed
- **THEN** boot block SHALL be prepended to spec content

#### Scenario: Agents without specs receive boot blocks

- **GIVEN** agent with no spec file (default prompt)
- **WHEN** prompt is constructed
- **THEN** boot block SHALL be prepended to default prompt

