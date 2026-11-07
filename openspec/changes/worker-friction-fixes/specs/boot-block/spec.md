## ADDED Requirements

### Requirement: Boot block asks for bare helper calls

Every rendered boot block (the standard and the MCP variant) SHALL instruct the agent to run each `.git-paw/scripts/broker.sh` call as a bare command, with no appended exit-status probe (such as `; echo "exit=$?"`) and no pipe, so a permission rule matching the helper's path prefix matches every call. The instruction SHALL describe the call's shape and SHALL NOT name any agent CLI or its permission system.

#### Scenario: Standard boot block asks for bare helper calls

- **WHEN** a standard boot block is rendered for any branch
- **THEN** it SHALL contain an instruction to run each `broker.sh` call bare, without an exit-status probe or a pipe
- **AND** it SHALL contain no `; echo` appended to any `broker.sh` example

#### Scenario: MCP boot block asks for bare helper calls

- **WHEN** an MCP-variant boot block is rendered
- **THEN** it SHALL contain the same bare-call instruction

#### Scenario: The instruction names no agent CLI

- **WHEN** either rendered boot block is audited for vendor and stack tokens
- **THEN** the bare-call instruction SHALL name no agent CLI product
