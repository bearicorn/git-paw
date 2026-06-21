## ADDED Requirements

### Requirement: Documentation tools

The system SHALL expose read-only MCP tools for reading the repository's own documentation, driven by the bring-your-own `[governance].readme` and `[governance].docs` configuration (paths are configured, never hardcoded). The category SHALL include `get_readme`, `list_docs`, and `get_doc`. All three perform deterministic file reads only — no agent CLI is invoked — and each advertises a JSON Schema for its parameters and return shape. Reads are confined to the repository root and, for `get_doc`/`list_docs`, to the configured documentation directory.

#### Scenario: get_readme returns the configured README content

- **GIVEN** `[governance].readme = "README.md"` is configured and the file exists
- **WHEN** the MCP client calls `get_readme()`
- **THEN** the response SHALL contain the README's full text content

#### Scenario: get_readme degrades to null when unconfigured or absent

- **GIVEN** `[governance].readme` is unset, or is set to a path that does not exist
- **WHEN** the MCP client calls `get_readme()`
- **THEN** the response SHALL have a null/empty content field and SHALL NOT be a transport-level error (unset → graceful; configured-but-absent → null content)

#### Scenario: list_docs enumerates Markdown docs under the configured dir

- **GIVEN** `[governance].docs = "docs/src"` is configured and contains Markdown files
- **WHEN** the MCP client calls `list_docs()`
- **THEN** the response SHALL list each document with its path relative to the docs dir
- **AND** when `[governance].docs` is unset, the response SHALL be an empty list (graceful degradation)

#### Scenario: get_doc returns one document confined to the docs dir

- **GIVEN** `[governance].docs = "docs/src"` is configured
- **WHEN** the MCP client calls `get_doc({ "path": "user-guide/mcp.md" })`
- **THEN** the response SHALL contain that document's content

#### Scenario: get_doc rejects path traversal outside the docs dir

- **GIVEN** `[governance].docs = "docs/src"` is configured
- **WHEN** the MCP client calls `get_doc({ "path": "../../etc/passwd" })` (or any path resolving outside the configured docs dir)
- **THEN** the response SHALL refuse the read (null/empty content with an error/message field) and SHALL NOT read any file outside the configured docs dir
