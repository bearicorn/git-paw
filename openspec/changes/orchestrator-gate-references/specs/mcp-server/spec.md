## ADDED Requirements

### Requirement: get_skill returns a package's references with its skill

The `get_skill` tool SHALL return, as a named skill's rendered content, the rendered `SKILL.md` followed by each of that skill's rendered references, each reference introduced by a heading naming its package-relative path. A skill with no references SHALL return its rendered `SKILL.md` unchanged. The tool SHALL remain read-only.

#### Scenario: get_skill includes the orchestrator's gate references

- **WHEN** the MCP client calls `get_skill({ "name": "orchestrator" })`
- **THEN** the content SHALL contain the rendered orchestrator `SKILL.md`
- **AND** SHALL contain a heading naming `references/gates/spec-audit.md` followed by that reference's rendered content

#### Scenario: A skill without references is unchanged

- **WHEN** the MCP client calls `get_skill({ "name": "docs-fetch" })`
- **THEN** the content SHALL equal the rendered `docs-fetch` `SKILL.md`
- **AND** no file SHALL be written to disk
