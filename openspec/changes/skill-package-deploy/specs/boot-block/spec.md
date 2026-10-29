## MODIFIED Requirements

### Requirement: Boot block content requirements

The boot block SHALL include specific instructions for each event type:

1. **REGISTER**: Instruct agent to immediately publish working status with "booting" message.
2. **DONE**: Instruct agent that the primary task-completion path is `git commit` — the post-commit hook installed by git-paw auto-publishes `agent.artifact { status: "committed" }` with the committed files attached, and the agent SHALL NOT publish anything manually for tasks that produce code changes. The section SHALL retain a manual `agent.artifact { status: "done" }` fallback for code-less tasks (docs-only updates handled outside the worktree, planning notes, exploration tasks where the artifact is information reported to the broker), and SHALL include a clear warning against publishing manual `done` when the worktree has uncommitted changes.
3. **BLOCKED**: Instruct agent to publish agent.blocked with dependency information.
4. **QUESTION**: Instruct agent to publish agent.question and WAIT for answer.

In the helper form, each event's broker interaction SHALL be expressed as an
invocation of the coordination package's bundled broker helper at its package
path, `.git-paw/skills/coordination/scripts/broker.sh` (the
`agent-broker-helper` capability), rendered from the package registry rather
than written as a literal in the template, and rather than a raw `curl`
command. In the MCP form, each event SHALL be expressed as the corresponding
MCP publish tool invocation. In neither form SHALL the boot block inline a raw
`curl` to the broker URL for any of the four events; the broker URL and JSON
shaping live inside the helper or the MCP server.

#### Scenario: Each event invokes the broker helper

- **GIVEN** the helper form of the boot block
- **WHEN** the boot block is examined
- **THEN** each of the four events SHALL have:
  - Clear one-line description of when to use it
  - A `.git-paw/skills/coordination/scripts/broker.sh` invocation (REGISTER,
    BLOCKED, QUESTION sections; the DONE section's manual fallback) with the
    arguments appropriate to the event
  - Appropriate message content for the event type

#### Scenario: Boot block contains no raw broker curl

- **WHEN** the rendered boot block is examined in either form
- **THEN** it SHALL NOT contain a raw `curl` command targeting the
  broker URL for the REGISTER, DONE-fallback, BLOCKED, or QUESTION
  events
- **AND** in the helper form those events SHALL be expressed as
  `.git-paw/skills/coordination/scripts/broker.sh` invocations instead

#### Scenario: The boot-block template carries no literal helper path

- **WHEN** the bundled boot-block templates are inspected before rendering
- **THEN** they SHALL name the broker helper only through its registry placeholder

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

- **GIVEN** the helper form of the rendered boot block for any branch
- **WHEN** the DONE section body is examined
- **THEN** it SHALL include a complete, copy-pasteable `.git-paw/skills/coordination/scripts/broker.sh artifact` invocation publishing `agent.artifact` with `status: "done"`
- **AND** the published message SHALL use the same JSON shape as in prior boot-block versions (type `agent.artifact`, payload fields `status`, `exports`, `modified_files`) so code-less agents have an unchanged fallback path
