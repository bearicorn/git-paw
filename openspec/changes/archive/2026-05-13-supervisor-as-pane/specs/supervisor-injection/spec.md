## MODIFIED Requirements

### Requirement: Supervisor mode boot block prepending

In supervisor auto-start mode, the system SHALL prepend the boot instruction block to each agent's task prompt before injecting it into the tmux pane. This SHALL apply to ALL pane-bound agents — the supervisor pane (pane 0), the dashboard pane (pane 1, where applicable; the dashboard is a TUI process and does not receive a `send-keys` boot block, but the requirement is unchanged for clarity), and the coding agent panes (panes 2..N+1).

#### Scenario: Boot block prepended to agent prompts

- **GIVEN** agent task prompt "Implement error handling"
- **WHEN** `cmd_supervisor()` constructs the full prompt for the coding agent pane
- **THEN** the injected text SHALL be:
  ```
  <boot_block>\n\nImplement error handling
  ```

#### Scenario: Boot block prepended to supervisor pane prompt

- **GIVEN** the supervisor pane (index 0) is being initialised with a "Begin observing" framing message
- **WHEN** `cmd_supervisor()` constructs the supervisor pane's prompt
- **THEN** the injected text SHALL be:
  ```
  <boot_block (with BRANCH_ID = supervisor)>\n\nBegin observing ...
  ```

#### Scenario: Boot block comes before task content

- **GIVEN** any agent or supervisor pane receiving its initial prompt
- **WHEN** the prompt is injected via `tmux send-keys`
- **THEN** the boot block SHALL appear first
- **AND** the actual task content SHALL appear after two newlines

### Requirement: Supervisor boot block timing

The system SHALL inject boot blocks during the supervisor launch sequence, specifically after tmux session creation but before `cmd_supervisor()` returns. The 2-second sleep between session creation and `tmux send-keys` invocations is preserved (panes need to reach an interactive state before key injection).

#### Scenario: Boot blocks injected before cmd_supervisor returns

- **GIVEN** `cmd_supervisor()` is executing
- **WHEN** agent panes are created and initialized
- **THEN** boot blocks SHALL be injected for all pane-bound agents
- **AND** the 2-second boot delay SHALL elapse between session creation and the first `send-keys` call
- **AND** all `send-keys` calls SHALL complete before `cmd_supervisor()` returns

### Requirement: All agents receive boot blocks

In supervisor mode, the system SHALL ensure every coding agent pane AND the supervisor pane receive the boot instruction block, regardless of whether the agent has a spec file or uses a default prompt. The dashboard pane is excluded (it runs a TUI process, not a chat-style agent).

#### Scenario: Coding agents with specs receive boot blocks

- **GIVEN** a coding agent pane with spec file content
- **WHEN** the prompt is constructed
- **THEN** the boot block SHALL be prepended to the spec content

#### Scenario: Coding agents without specs receive boot blocks

- **GIVEN** a coding agent pane with no spec file (default prompt)
- **WHEN** the prompt is constructed
- **THEN** the boot block SHALL be prepended to the default prompt

#### Scenario: Supervisor pane receives a boot block

- **GIVEN** the supervisor pane (index 0)
- **WHEN** the prompt is constructed
- **THEN** the boot block (with `BRANCH_ID = supervisor`) SHALL be prepended to the "Begin observing" framing message
