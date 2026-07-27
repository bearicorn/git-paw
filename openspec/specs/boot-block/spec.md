# boot-block Specification

## Purpose

This capability defines the standardized boot-instruction block injected into each agent and the shared machinery that renders it. It covers the block's format and content (exactly four runtime events — register, done, blocked, question — expressed as `broker.sh` helper invocations, commit-first task completion with a code-less manual-done fallback, and paste-handling guidance), `{{VARIABLE_NAME}}` template substitution with branch-ID slugification and full pre-expansion at render time, and the shared pure `build_boot_block(branch_id, broker_url) -> String` helper in `src/skills.rs` that every launch path uses to assemble the identical block.

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

1. **REGISTER**: Instruct agent to immediately publish working status with "booting" message.
2. **DONE**: Instruct agent that the primary task-completion path is `git commit` — the post-commit hook installed by git-paw auto-publishes `agent.artifact { status: "committed" }` with the committed files attached, and the agent SHALL NOT publish anything manually for tasks that produce code changes. The section SHALL retain a manual `agent.artifact { status: "done" }` fallback for code-less tasks (docs-only updates handled outside the worktree, planning notes, exploration tasks where the artifact is information reported to the broker), and SHALL include a clear warning against publishing manual `done` when the worktree has uncommitted changes.
3. **BLOCKED**: Instruct agent to publish agent.blocked with dependency information.
4. **QUESTION**: Instruct agent to publish agent.question and WAIT for answer.

Each event's broker interaction SHALL be expressed as an invocation of
the bundled agent helper `.git-paw/scripts/broker.sh` (the
`agent-broker-helper` capability) rather than a raw `curl` command. The
boot block SHALL NOT inline a raw `curl` to the broker URL for any of
the four events; the broker URL and JSON shaping live inside the helper.

#### Scenario: Each event invokes the broker helper

- **WHEN** the boot block is examined
- **THEN** each of the four events SHALL have:
  - Clear one-line description of when to use it
  - A `.git-paw/scripts/broker.sh` invocation (REGISTER, BLOCKED,
    QUESTION sections; the DONE section's manual fallback) with the
    arguments appropriate to the event
  - Appropriate message content for the event type

#### Scenario: Boot block contains no raw broker curl

- **WHEN** the rendered boot block is examined
- **THEN** it SHALL NOT contain a raw `curl` command targeting the
  broker URL for the REGISTER, DONE-fallback, BLOCKED, or QUESTION
  events
- **AND** those events SHALL be expressed as `.git-paw/scripts/broker.sh`
  invocations instead

#### Scenario: QUESTION event emphasizes waiting

- **WHEN** the QUESTION section is examined
- **THEN** it SHALL contain the phrase "DO NOT continue until you receive an answer!"
- **AND** the instruction SHALL be in bold or uppercase for emphasis

#### Scenario: DONE section leads with commit-first instruction

- **GIVEN** the rendered boot block produced by the boot-block builder for any branch
- **WHEN** the DONE section body is examined
- **THEN** it SHALL contain an instruction directing the agent to commit its work via `git commit` as the primary task-completion path
- **AND** the commit-first instruction SHALL appear before the manual `agent.artifact { status: "done" }` helper invocation in the section body
- **AND** the section SHALL state that the post-commit hook auto-publishes `agent.artifact { status: "committed" }` on each commit, so the agent does not need to publish manually for tasks that produce code changes

#### Scenario: DONE section scopes manual done to code-less tasks

- **GIVEN** the rendered boot block produced by the boot-block builder for any branch
- **WHEN** the DONE section body is examined
- **THEN** it SHALL describe the manual `agent.artifact { status: "done" }` fallback as intended for tasks that produce no code changes
- **AND** it SHALL enumerate representative code-less task types (for example: docs-only updates handled outside this worktree, planning notes, exploration tasks)
- **AND** it SHALL contain an emphasised (bold or uppercase) warning that the agent SHALL NOT publish manual `done` when the worktree has uncommitted changes, and SHALL commit instead

#### Scenario: DONE section retains the manual done fallback for code-less tasks

- **GIVEN** the rendered boot block produced by the boot-block builder for any branch
- **WHEN** the DONE section body is examined
- **THEN** it SHALL include a complete, copy-pasteable `.git-paw/scripts/broker.sh artifact` invocation publishing `agent.artifact` with `status: "done"`
- **AND** the published message SHALL use the same JSON shape as in prior boot-block versions (type `agent.artifact`, payload fields `status`, `exports`, `modified_files`) so code-less agents have an unchanged fallback path

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

### Requirement: Shared boot block helper function

The system SHALL provide a shared `build_boot_block()` function in `src/skills.rs` that can be called from both supervisor and manual mode code paths.

#### Scenario: Function is accessible from multiple modules

- **GIVEN** `build_boot_block()` defined in `src/skills.rs`
- **WHEN** called from `src/main.rs` (supervisor mode)
- **THEN** it SHALL return the boot block string

#### Scenario: Same function used in manual mode

- **GIVEN** `build_boot_block()` defined in `src/skills.rs`
- **WHEN** called from `src/tmux.rs` (manual mode)
- **THEN** it SHALL return the same boot block string

### Requirement: Helper function signature

The `build_boot_block()` function SHALL have the following signature:
```rust
pub fn build_boot_block(branch_id: &str, broker_url: &str) -> String
```

#### Scenario: Function accepts required parameters

- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** it SHALL accept both parameters without error

#### Scenario: Function returns boot block string

- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** it SHALL return a `String` containing the boot instructions

### Requirement: Helper function reusability

The `build_boot_block()` function SHALL be designed for maximum reusability with no dependencies on calling context or global state.

#### Scenario: Function is pure (no side effects)

- **GIVEN** same input parameters
- **WHEN** `build_boot_block()` is called multiple times
- **THEN** it SHALL return identical output each time

#### Scenario: Function requires no external state

- **WHEN** `build_boot_block()` is called
- **THEN** it SHALL not access any global variables, configuration, or external services
- **AND** it SHALL only use its input parameters

### Requirement: Helper function testing

The `build_boot_block()` function SHALL be fully testable with comprehensive unit test coverage.

#### Scenario: Function can be tested in isolation

- **WHEN** unit tests call `build_boot_block()` with various inputs
- **THEN** the function SHALL produce expected output without requiring tmux or broker

#### Scenario: Edge cases are testable

- **WHEN** tests provide edge case inputs (empty strings, special characters)
- **THEN** the function SHALL handle them gracefully
