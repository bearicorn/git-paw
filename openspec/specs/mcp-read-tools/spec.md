# mcp-read-tools Specification

## Purpose
TBD - created by archiving change mcp-server. Update Purpose after archive.
## Requirements
### Requirement: Coordination tools

The system SHALL expose MCP tools for inspecting active agent
coordination state. The category SHALL include `get_intents`,
`get_intent`, and `get_conflicts`. Every tool in this category
SHALL return empty arrays (not errors) when no broker process is
running or no session is active.

#### Scenario: get_intents returns all active intents when broker is live

- **GIVEN** the broker is running and N agents have published
  `agent.intent` messages whose TTLs have not expired
- **WHEN** the MCP client calls `get_intents()` with no parameters
- **THEN** the response SHALL be a JSON object `{ "intents": [...] }`
  containing N entries, each entry carrying at minimum
  `branch_id`, `files` (array of paths), `summary`, `published_at`,
  and `valid_for_seconds`

#### Scenario: get_intent looks up a single agent's intent by branch_id

- **GIVEN** the broker is running and an agent with `branch_id =
  "feat/foo"` has an active intent
- **WHEN** the MCP client calls `get_intent({ "branch_id":
  "feat/foo" })`
- **THEN** the response SHALL be a JSON object containing that
  agent's intent fields, OR `null` if no matching intent exists

#### Scenario: get_conflicts returns all detected conflicts

- **GIVEN** the broker is running and the conflict-detection
  subsystem has registered conflict events (forward / in-flight /
  ownership-violation)
- **WHEN** the MCP client calls `get_conflicts()`
- **THEN** the response SHALL be a JSON object
  `{ "conflicts": [...] }` containing every active conflict, each
  entry carrying at minimum `shape`, `branches`, `files`, and
  `detected_at`

#### Scenario: Coordination tools degrade to empty arrays when broker is off

- **GIVEN** no broker process is running for the target repository
- **WHEN** the MCP client calls any coordination tool
- **THEN** the response SHALL be a successful JSON-RPC response
  with empty arrays for collection fields and `null` for
  single-record fields

### Requirement: Governance tools

The system SHALL expose MCP tools for reading the documents
configured under `[governance]` in `.git-paw/config.toml`. The
category SHALL include `get_adrs`, `get_adr`, `get_test_strategy`,
`get_security_checklist`, `get_dod`, `check_dod`, and
`get_constitution`. The system SHALL read files lazily — content
is only loaded when a tool is invoked, not at server startup.

#### Scenario: get_adrs lists ADR files under the configured directory

- **GIVEN** `[governance].adr = "docs/adr"` is set and the
  directory contains files matching `ADR-*.md`
- **WHEN** the MCP client calls `get_adrs()`
- **THEN** the response SHALL be a JSON object
  `{ "adrs": [...] }` where each entry carries `id`, `title`,
  `path`, and `status` parsed from the ADR file

#### Scenario: get_adr returns a single ADR matched by query

- **GIVEN** the same ADR directory and an ADR titled
  "ADR-0007: Choose tokio for async runtime"
- **WHEN** the MCP client calls `get_adr({ "query": "tokio" })`
- **THEN** the response SHALL include the ADR's full Markdown
  content along with its `id` and `path`

#### Scenario: get_constitution reads the Spec Kit constitution

- **GIVEN** `[governance].constitution = ".specify/memory/
  constitution.md"` is set (or auto-detected from `.specify/`
  presence)
- **WHEN** the MCP client calls `get_constitution()`
- **THEN** the response SHALL include the file's full Markdown
  content as the `content` field

#### Scenario: check_dod returns per-item completion against the configured DoD

- **GIVEN** `[governance].dod` points at a Markdown file
  containing a checklist of `- [ ]` / `- [x]` items
- **WHEN** the MCP client calls `check_dod({ "branch":
  "feat/foo" })`
- **THEN** the response SHALL include each DoD item with its
  current completion state derived from the branch's state
  (committed code, tests passing, docs updated, etc., to the
  extent the tool can determine without LLM judgment)

#### Scenario: Governance tools degrade to null when no paths configured

- **GIVEN** `.git-paw/config.toml` has no `[governance]` section
- **WHEN** the MCP client calls any governance tool
- **THEN** the response SHALL be a successful JSON-RPC response
  with `null` for single-document fields and empty arrays for
  collection fields

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
  (one of `openspec | markdown | speckit`), `title`, `status`,
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

### Requirement: Session state tools

The system SHALL expose MCP tools for reading the active or most
recent session's state. The category SHALL include
`get_session_status`, `get_session_summary`, and `get_learnings`.
The tools SHALL read from `.git-paw/sessions/*.json` and
`.git-paw/session-learnings.md` (the file produced by
[[learnings-mode]] in v0.5.0).

#### Scenario: get_session_status returns the active session summary

- **GIVEN** a session is running with N agent panes registered
- **WHEN** the MCP client calls `get_session_status()`
- **THEN** the response SHALL include the session name, agent
  count, broker URL (if broker is enabled), pause state, and
  per-agent last-seen / status data drawn from the session JSON
  and broker `/status` endpoint

#### Scenario: get_session_status returns null session when none is active

- **GIVEN** no session is active for the target repository
- **WHEN** the MCP client calls `get_session_status()`
- **THEN** the response SHALL have its `session` field set to null
  and no additional error

#### Scenario: get_learnings parses the session-learnings.md file

- **GIVEN** `.git-paw/session-learnings.md` exists with the
  four v0.5.0 sections (Conflict events, Where agents got stuck,
  Recovery cycles, Permission patterns)
- **WHEN** the MCP client calls `get_learnings()`
- **THEN** the response SHALL parse each section into a structured
  array of entries with `category` and `body` fields

#### Scenario: get_learnings returns empty sections when no learnings file exists

- **GIVEN** no `.git-paw/session-learnings.md` file exists
- **WHEN** the MCP client calls `get_learnings()`
- **THEN** the response SHALL return an object with each section
  present as an empty array

### Requirement: Git context tools

The system SHALL expose MCP tools that wrap read-only git
operations. The category SHALL include `get_branches`,
`get_recent_commits`, and `get_diff`. The tools SHALL invoke `git`
via `std::process::Command` against the resolved repository root.

#### Scenario: get_branches lists local branches

- **WHEN** the MCP client calls `get_branches()`
- **THEN** the response SHALL include every local branch with
  name, head commit SHA, whether it is the currently checked-out
  branch, and whether it is a git-paw-managed worktree branch

#### Scenario: get_recent_commits returns the last N commits on a branch

- **WHEN** the MCP client calls `get_recent_commits({ "branch":
  "main", "limit": 10 })`
- **THEN** the response SHALL include up to 10 commits in
  reverse-chronological order, each with SHA, author, ISO
  timestamp, and subject line

#### Scenario: get_diff returns the diff between a branch and its base

- **WHEN** the MCP client calls `get_diff({ "branch":
  "feat/foo" })`
- **THEN** the response SHALL include the diff against the branch's
  base (default `main`) as a string and a summary of
  files-changed / lines-added / lines-deleted

#### Scenario: Git tools work even with no git-paw session

- **GIVEN** the repository is a valid git repository but has no
  `.git-paw/` directory and no session has ever been started
- **WHEN** the MCP client calls any git context tool
- **THEN** the tools SHALL return successful responses populated
  from git state alone

### Requirement: All tools include a JSON Schema for parameters and return shape

The system SHALL include, for every tool advertised via MCP, a
JSON Schema describing both the input parameter shape and the
output result shape. The schema SHALL be reachable via the MCP
`tools/list` method and SHALL be precise enough that a client
LLM can validate its own invocations without runtime trial-and-error.

#### Scenario: tools/list advertises schemas for every tool

- **WHEN** the MCP client sends an MCP `tools/list` request
- **THEN** every entry in the response SHALL include
  `inputSchema` per the MCP specification, and the schema SHALL
  be a valid JSON Schema 2020-12 document

#### Scenario: Optional parameters are marked optional in the schema

- **WHEN** an MCP client inspects the input schema for a tool
  that accepts optional arguments (e.g. `get_recent_commits`'s
  `limit`)
- **THEN** the schema SHALL distinguish required from optional
  parameters per JSON Schema convention, and SHALL include
  default values where applicable

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

