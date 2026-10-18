## MODIFIED Requirements

### Requirement: Publish tools act as the calling agent only

Each publish tool SHALL derive the publishing `agent_id` from the resolved worktree branch of the calling session, using the established branch-to-`agent_id` conversion. A publish tool SHALL NOT accept an `agent_id` parameter from the caller. An agent SHALL NOT be able to publish a message attributed to a different agent. When the server runs in the repository's main working tree rather than a linked worktree (as git reports it: the checkout's git directory equals the common git directory, the check `RepoLayout::is_linked_worktree` makes), a publish tool SHALL NOT derive an `agent_id` from that tree's branch: it SHALL return an MCP-level error stating that publishing needs an agent's worktree session, SHALL publish nothing, and the server SHALL keep running.

#### Scenario: Agent id is derived, not supplied

- **WHEN** a publish tool's input schema is inspected
- **THEN** it SHALL NOT contain an `agent_id` parameter

#### Scenario: Published message carries the caller's own id

- **GIVEN** an MCP session resolved to a worktree on branch `feat/x`
- **WHEN** that session publishes a status event via the MCP publish tool
- **THEN** the published message's `agent_id` SHALL be the slugified form of `feat/x`

#### Scenario: Impersonation is not possible

- **GIVEN** an MCP session resolved to one worktree
- **WHEN** it attempts to publish an event attributed to a different agent
- **THEN** no message attributed to that other agent SHALL be published

#### Scenario: A publish from the main working tree is refused

- **GIVEN** an MCP server resolved to the repository's main working tree on branch `main` and a running broker
- **WHEN** the client calls `publish_status`
- **THEN** the server SHALL return an MCP-level error and keep running
- **AND** no message SHALL be published and the broker's `/status` SHALL NOT list `agent-main`
