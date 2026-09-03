## ADDED Requirements

### Requirement: The server exposes a bounded write surface alongside read-only tools

The MCP server SHALL NOT be exclusively read-only. It SHALL advertise a bounded, agent-scoped publish category covering the four agent boot events, alongside its read-only tool categories. Every tool category other than that publish set SHALL remain read-only and deterministically sourced.

The write surface SHALL be bounded by construction: a fixed set of tools, each publishing as the calling agent only, with the supervisor authority verbs excluded. The server SHALL NOT expose file writes, git mutations, configuration writes, or any other state change beyond publishing those four broker events.

#### Scenario: Publish category coexists with read-only categories

- **WHEN** the tool registry is inspected
- **THEN** it SHALL contain the bounded publish category
- **AND** every other advertised category SHALL perform reads only

#### Scenario: No file or git mutation is exposed

- **WHEN** the tool registry is inspected
- **THEN** it SHALL NOT contain a tool that writes a file, mutates git state, or writes configuration

#### Scenario: Read-only guarantees of other categories are preserved

- **GIVEN** the coordination, governance, project-knowledge, session-state, git-context, documentation, and source/file categories
- **WHEN** any of their tools is invoked
- **THEN** it SHALL perform deterministic reads only
- **AND** SHALL NOT mutate repository or broker state
