# boot-block-format Specification

## Purpose
Defines the standardized boot-instruction block injected into each agent, covering exactly four runtime events — register, done, blocked, question — expressed as `broker.sh` helper invocations rather than raw curl. It establishes commit-first task completion (the post-commit hook auto-publishes the artifact) with a manual done fallback scoped to code-less tasks, and includes paste-handling instructions for paste-collapsing CLIs.

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

