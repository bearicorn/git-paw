## MODIFIED Requirements

### Requirement: Hard errors only for malformed configuration

The system SHALL return JSON-RPC protocol errors (not empty
results) when a user's configuration points at resources that exist
but cannot be read or parsed. A configured `[specs].type` SHALL be
treated as invalid only when the system's spec discovery does not
support it; the set of valid types SHALL be derived from the same
source spec discovery uses, so a supported backend is never
rejected by the server. These cases reflect user error and
SHALL be visible to the client so the LLM can surface them.

#### Scenario: Governance path points at an unreadable file

- **GIVEN** `[governance].dod = "docs/dod.md"` is set but the file
  exists with permissions preventing read access
- **WHEN** the MCP client calls `get_dod()`
- **THEN** the tool SHALL return a JSON-RPC error identifying the
  path and the I/O failure reason, NOT an empty response

#### Scenario: Configured spec backend type is invalid

- **GIVEN** `[specs].type = "unrecognised"` is set in
  `.git-paw/config.toml`
- **WHEN** the server starts
- **THEN** the server SHALL exit with non-zero status and SHALL
  emit a human-readable error to stderr identifying the invalid
  value and the valid options

#### Scenario: Every supported spec backend type is accepted at startup

- **GIVEN** `[specs].type` is set to any backend type the system's spec discovery supports — `openspec`, `markdown`, `speckit` or `superpowers`
- **WHEN** the server starts
- **THEN** the server SHALL start and serve, and SHALL NOT report the type as invalid

#### Scenario: The accepted set cannot drift from spec discovery

- **WHEN** a spec backend type is supported by the system's spec discovery
- **THEN** the MCP server's startup validation SHALL accept it, because the accepted set is derived from the same source as discovery rather than maintained as a separate list

### Requirement: Project knowledge tools

The system SHALL expose MCP tools for indexing and reading the
repository's specifications and the agent skills git-paw would
inject. The category SHALL include `get_specs`, `get_spec`,
`get_tasks`, `get_task`, `get_dependency_graph`, and `get_skill`.
The spec tools SHALL handle all three supported backends —
OpenSpec, plain Markdown, and Spec Kit — using the same discovery
logic that `git paw start --from-all-specs` uses. `get_skill`
SHALL return the rendered content of a named skill using the same
resolution and `{{...}}` substitution pipeline that boot-time skill
injection uses (project `.agents/skills/` → user override →
embedded default); it performs read-only rendering and SHALL NOT
write any skill to disk, register a watcher, or expose a version /
hot-reload endpoint.

#### Scenario: get_specs lists discovered specs across all backends

- **GIVEN** the repository contains a mix of OpenSpec changes
  under `openspec/changes/`, Markdown specs with `paw_status:
  pending` frontmatter, and a Spec Kit `.specify/specs/` tree
- **WHEN** the MCP client calls `get_specs()`
- **THEN** the response SHALL be a JSON object
  `{ "specs": [...] }` where each entry carries `id`, `backend`
  (one of `openspec | markdown | speckit | superpowers`), `title`, `status`,
  and `path`

#### Scenario: get_spec returns the full content of a named spec

- **WHEN** the MCP client calls
  `get_spec({ "id": "mcp-server" })`
- **THEN** the response SHALL include the spec's discovered
  artifacts (proposal, design, specs, tasks for OpenSpec; spec
  + plan + tasks + checklists for Spec Kit; raw body for plain
  Markdown) with their content

#### Scenario: get_tasks returns Spec Kit task checkboxes with status

- **GIVEN** a Spec Kit feature with phased `tasks.md` containing
  a mix of `- [ ]` and `- [x]` items
- **WHEN** the MCP client calls `get_tasks({ "spec":
  "001-user-list" })`
- **THEN** the response SHALL list every task with its ID, phase,
  parallel marker `[P]` boolean, description, and completion state

#### Scenario: get_dependency_graph returns spec-level dependencies

- **WHEN** the MCP client calls `get_dependency_graph()`
- **THEN** the response SHALL describe inter-spec dependencies
  derived from cross-references in proposals (e.g. `[[other-spec]]`
  links), with `nodes` (specs) and `edges` (dependencies between
  them)

#### Scenario: get_skill returns a named skill's rendered content

- **GIVEN** the repository resolves a skill named `coordination`
  (from `.agents/skills/`, the user override directory, or the
  embedded default)
- **WHEN** the MCP client calls `get_skill({ "name":
  "coordination" })`
- **THEN** the response SHALL include the skill's rendered content
  (post `{{...}}` substitution) plus its `source` (one of
  `standard | user_override | embedded`)
- **AND** no skill file SHALL be written to disk and no watcher or
  version endpoint SHALL be created as a side effect

#### Scenario: get_skill reports an unknown skill without erroring the transport

- **WHEN** the MCP client calls `get_skill({ "name":
  "does-not-exist" })`
- **THEN** the response SHALL be a successful JSON-RPC response
  carrying a `null` (or empty) skill payload and a human-readable
  `error`/`message` field, not a transport-level failure

#### Scenario: Project knowledge tools return empty arrays when no specs exist

- **GIVEN** the repository has no OpenSpec changes, no
  pending-status Markdown specs, and no `.specify/` directory
- **WHEN** the MCP client calls `get_specs()` or `get_tasks()`
- **THEN** the response SHALL be a successful JSON-RPC response
  with empty arrays

#### Scenario: A Superpowers repository is served

- **GIVEN** a repository configured with `[specs].type = "superpowers"` and writing-plans documents under its configured directory
- **WHEN** the MCP client calls `get_specs()`
- **THEN** the response SHALL list those plans with `backend` `superpowers`
- **AND** `get_skill` SHALL render the spec-path doctrine for the Superpowers backend rather than the no-backend sentinel
