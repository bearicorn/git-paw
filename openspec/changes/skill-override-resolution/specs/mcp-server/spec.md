## ADDED Requirements

### Requirement: get_skill reports the source the production resolver chose

The `get_skill` tool SHALL resolve the named skill through the production entry point the launch paths use, rooted at the server's resolved repository rather than the process's current directory, and SHALL report as `source` the level that supplied the template: `standard` (repository `.agents/skills/`), `user_override` (user configuration directory) or `embedded`. That template and source SHALL be the ones a launch or export from that repository would use. The tool SHALL remain read-only.

#### Scenario: A user override is reported as user_override

- **GIVEN** a `git paw mcp` server whose `HOME` is a temporary directory holding a user override for `coordination`, with `XDG_CONFIG_HOME` unset
- **AND** no repository override
- **WHEN** the MCP client calls `get_skill({ "name": "coordination" })`
- **THEN** `source` SHALL be `user_override`
- **AND** the content SHALL be the override's rendered text

#### Scenario: --repo selects whose repository override applies

- **GIVEN** a repository with `.agents/skills/coordination/SKILL.md`
- **AND** a server started with `--repo <that repository>` from a current directory outside it
- **WHEN** the MCP client calls `get_skill({ "name": "coordination" })`
- **THEN** `source` SHALL be `standard`
- **AND** the content SHALL be that override's rendered text

#### Scenario: A server rooted in a worktree ignores the worktree's override

- **GIVEN** a server whose resolved repository root is a linked worktree whose checkout contains `.agents/skills/coordination/SKILL.md`
- **AND** the main working tree has no such override and no user override exists
- **WHEN** the MCP client calls `get_skill({ "name": "coordination" })`
- **THEN** `source` SHALL be `embedded`

#### Scenario: No override reports embedded

- **GIVEN** no override at either level
- **WHEN** the MCP client calls `get_skill({ "name": "coordination" })`
- **THEN** `source` SHALL be `embedded`
- **AND** no file SHALL be written to disk
