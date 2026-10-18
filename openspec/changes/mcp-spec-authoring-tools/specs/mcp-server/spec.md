## MODIFIED Requirements

### Requirement: The server exposes a bounded write surface alongside read-only tools

The MCP server SHALL NOT be exclusively read-only. It SHALL advertise, alongside its read-only tool categories: (a) the agent-scoped publish category — the four worker boot-event tools for every session, plus the two authority publish tools for a root session only; (b) a root-session lifecycle category (`start_session`, `pause_session`, `stop_session`, `purge_session`) that performs exactly what the equivalent `git paw` command performs, by calling the library command surface in-process; and (c) a spec-authoring write category (`create_spec`, `write_spec_artifact`), advertised only to a root session of a repository whose own configuration sets `[mcp] allow_repo_writes = true`. Every other tool category, including `validate_spec`, SHALL remain read-only and deterministically sourced.

The write surface SHALL be bounded by construction: a fixed set of tools, each acting as the calling session's identity, with root-only tools advertised only to a root session. The server SHALL NOT expose a tool that writes a file outside a spec change directory, runs an arbitrary git mutation, writes configuration, or executes an arbitrary command; the only repository mutations reachable are those the lifecycle tools perform on git-paw-managed worktrees and branches, and those the spec-authoring tools perform inside a spec change directory of a consenting repository.

#### Scenario: Publish and lifecycle categories coexist with read-only categories

- **WHEN** the tool registry is inspected from a root session
- **THEN** it SHALL contain the publish category and the lifecycle category
- **AND** every other advertised category SHALL perform reads only, except the spec-authoring write category when the repository consents

#### Scenario: No arbitrary file, git or config mutation is exposed

- **WHEN** the tool registry is inspected, whatever the session kind and the value of `allow_repo_writes`
- **THEN** it SHALL NOT contain a tool that writes a file outside a spec change directory, runs a caller-supplied git or shell command, or writes configuration

#### Scenario: Read-only guarantees of other categories are preserved

- **GIVEN** the coordination, governance, project-knowledge, session-state, git-context, documentation, and source/file categories, and `validate_spec`
- **WHEN** any of their tools is invoked
- **THEN** it SHALL perform deterministic reads only
- **AND** SHALL NOT mutate repository or broker state

## ADDED Requirements

### Requirement: Repository write tools require repository consent and a root session

The server SHALL advertise and execute an MCP tool that writes file content into the repository working tree only when (a) the resolved repository's own `.git-paw/config.toml` sets `[mcp] allow_repo_writes = true` and (b) the session is a root session — the resolved repository is the main working tree, as for every root-only tool. The switch SHALL be read once when the server starts; a value in the user-level config, an environment variable or a tool argument SHALL NOT enable it, and no tool SHALL accept a parameter that grants it. When either condition fails, those tools SHALL NOT be advertised, and a call by name SHALL return an MCP error and SHALL write nothing: naming `allow_repo_writes` and the repository config file when the switch is off, and stating the tool is not available from a worktree session when the session is a worktree session. Read tools SHALL be advertised and behave identically whatever the switch's value.

#### Scenario: Writes are off by default

- **GIVEN** a root session in a repository whose config has no `allow_repo_writes` key
- **WHEN** the client lists tools
- **THEN** the list SHALL NOT include `create_spec` or `write_spec_artifact`
- **AND** SHALL include `validate_spec` and every read tool

#### Scenario: A by-name call while writes are off names the switch

- **GIVEN** a root session with `allow_repo_writes` off
- **WHEN** the client calls `create_spec({ "name": "add-x" })`
- **THEN** the server SHALL return an MCP error whose message contains `allow_repo_writes` and `.git-paw/config.toml`
- **AND** no directory SHALL be created

#### Scenario: Consent enables the write tools for the root session

- **GIVEN** a root session in a repository whose `.git-paw/config.toml` sets `[mcp] allow_repo_writes = true`
- **WHEN** the client lists tools
- **THEN** the list SHALL include `create_spec` and `write_spec_artifact`

#### Scenario: The user-level config cannot consent

- **GIVEN** the user-level config sets `[mcp] allow_repo_writes = true`
- **AND** the repository config does not set it
- **WHEN** the server starts at the main working tree and the client lists tools
- **THEN** the list SHALL NOT include `create_spec` or `write_spec_artifact`

#### Scenario: A worktree session never gets the write tools

- **GIVEN** an MCP server resolved to a `git worktree add`-created worktree whose config sets `allow_repo_writes = true`
- **WHEN** the client lists tools and then calls `write_spec_artifact` by name
- **THEN** the list SHALL NOT include `create_spec` or `write_spec_artifact`
- **AND** the call SHALL return an MCP error mentioning the worktree restriction
- **AND** no file SHALL be written

#### Scenario: No tool parameter grants consent

- **WHEN** the input schema of every advertised tool is inspected
- **THEN** none SHALL contain a parameter named `allow_repo_writes` or any other parameter that enables repository writes

### Requirement: Spec-authoring write tools

When repository writes are permitted, the server SHALL expose `create_spec` (parameter `name`) and `write_spec_artifact` (parameters `change`, `path`, `content`). Both SHALL act on the configured spec directory — the directory spec discovery scans, resolved from `[specs]` — when the configured spec format is OpenSpec; under any other format they SHALL return an MCP error stating the tool is not supported for that format, and with no spec system configured an MCP error naming `[specs]`, writing nothing in either case. `create_spec` SHALL accept only a name made of lowercase letters, digits and hyphens, SHALL refuse a name whose change directory already exists, and SHALL create `<spec dir>/<name>/` containing a `.openspec.yaml` with `schema: spec-driven` and the current date as `created` — what `openspec new change <name>` creates. `write_spec_artifact` SHALL write `content` verbatim to `path` inside the existing change directory `<spec dir>/<change>/`, creating missing parent directories inside it and replacing an existing regular file, and SHALL report whether it replaced one. Both tools SHALL return the repository-relative paths they wrote, SHALL log each written path on the server log, and SHALL NOT stage or commit anything.

#### Scenario: create_spec scaffolds an OpenSpec change

- **GIVEN** a consenting root session with `[specs] type = "openspec"` and `dir = "openspec/changes"`
- **WHEN** the client calls `create_spec({ "name": "add-export" })`
- **THEN** `openspec/changes/add-export/.openspec.yaml` SHALL exist containing `schema: spec-driven` and a `created:` date
- **AND** the response SHALL list that path

#### Scenario: create_spec refuses an existing change

- **GIVEN** `openspec/changes/add-export/` already exists
- **WHEN** the client calls `create_spec({ "name": "add-export" })`
- **THEN** the server SHALL return an MCP error stating the change exists
- **AND** the existing directory's contents SHALL be unchanged

#### Scenario: create_spec refuses an invalid name

- **WHEN** the client calls `create_spec` with `name` set to `"Add_Export"` or `"../x"`
- **THEN** the server SHALL return an MCP error describing the allowed name form
- **AND** nothing SHALL be created

#### Scenario: write_spec_artifact writes a delta spec

- **GIVEN** an existing change `add-export`
- **WHEN** the client calls `write_spec_artifact({ "change": "add-export", "path": "specs/data-export/spec.md", "content": "## ADDED Requirements\n" })`
- **THEN** `openspec/changes/add-export/specs/data-export/spec.md` SHALL contain exactly that content
- **AND** the response SHALL report `replaced: false`

#### Scenario: write_spec_artifact replaces an existing artifact

- **GIVEN** `openspec/changes/add-export/tasks.md` exists
- **WHEN** the client writes new content to `tasks.md` in that change
- **THEN** the file SHALL hold the new content
- **AND** the response SHALL report `replaced: true`

#### Scenario: write_spec_artifact needs an existing change

- **WHEN** the client calls `write_spec_artifact` for a change whose directory does not exist
- **THEN** the server SHALL return an MCP error naming the change
- **AND** no directory SHALL be created

#### Scenario: Writes are logged and never committed

- **GIVEN** a consenting root session started with `--log-file <file>`
- **WHEN** the client writes `proposal.md` into change `add-export`
- **THEN** the log file SHALL contain `openspec/changes/add-export/proposal.md`
- **AND** `HEAD` and the index SHALL be unchanged, with the file shown by `git status` as untracked

#### Scenario: Another spec format is not supported

- **GIVEN** a consenting root session with `[specs] type = "markdown"`
- **WHEN** the client calls `create_spec({ "name": "add-export" })`
- **THEN** the server SHALL return an MCP error naming the `markdown` format
- **AND** nothing SHALL be written

### Requirement: Repository writes are confined to the change directory

Before writing, the server SHALL resolve the target and SHALL refuse with an MCP error naming the offending input, writing nothing, when: `path` is absolute, or contains a `..` component or a `.git` component; `change` is not a single path segment; the canonical spec directory lies outside the canonical repository root or inside its `.git` directory; a directory between the change directory and the target resolves, after following symbolic links, outside the change directory; or the target itself is a symbolic link.

#### Scenario: Path traversal is refused

- **WHEN** the client calls `write_spec_artifact({ "change": "add-export", "path": "../../../src/main.rs", "content": "x" })`
- **THEN** the server SHALL return an MCP error
- **AND** no file outside `openspec/changes/add-export/` SHALL be created or modified

#### Scenario: Absolute paths and .git components are refused

- **WHEN** the client calls `write_spec_artifact` with `path` set to `/tmp/x.md`, or to `.git/hooks/pre-commit`
- **THEN** the server SHALL return an MCP error
- **AND** nothing SHALL be written

#### Scenario: A symlinked directory leading out is refused

- **GIVEN** `openspec/changes/add-export/specs` is a symbolic link to a directory outside the repository
- **WHEN** the client writes `specs/data-export/spec.md` into that change
- **THEN** the server SHALL return an MCP error
- **AND** nothing SHALL be written outside the repository

#### Scenario: A symlinked target file is refused

- **GIVEN** `openspec/changes/add-export/tasks.md` is a symbolic link
- **WHEN** the client writes `tasks.md` into that change
- **THEN** the server SHALL return an MCP error
- **AND** the link's target SHALL be unchanged

#### Scenario: A spec directory outside the repository is refused

- **GIVEN** `[specs] dir` resolves outside the repository root
- **WHEN** the client calls `create_spec({ "name": "add-export" })`
- **THEN** the server SHALL return an MCP error
- **AND** nothing SHALL be created

### Requirement: Spec validation tool

The server SHALL expose `validate_spec` (parameter `change`) as a read-only tool advertised to every session, whatever the value of `allow_repo_writes`. For the OpenSpec format it SHALL check the named change directory under the configured spec directory in-process, without spawning a process or writing a file, and SHALL return `{ "valid": bool, "findings": [{ "file", "message" }] }`, reporting: a missing `proposal.md`; no `specs/<capability>/spec.md` carrying a `## ADDED`, `## MODIFIED`, `## REMOVED` or `## RENAMED Requirements` header; an added or modified requirement whose first non-blank body line contains neither SHALL nor MUST; and an added or modified requirement without a `#### Scenario:` block. `valid` SHALL be true exactly when there are no findings. An unknown change SHALL return an MCP error naming it, and any other spec format an MCP error naming the format.

#### Scenario: A well-formed change is valid

- **GIVEN** a change with `proposal.md` and a delta spec whose requirements each state SHALL on their first line and carry a scenario
- **WHEN** the client calls `validate_spec({ "change": "add-export" })`
- **THEN** the response SHALL be `valid: true` with no findings

#### Scenario: Structural defects are reported

- **GIVEN** a change whose delta spec has one requirement without a scenario and another whose SHALL appears only on its second line
- **WHEN** the client calls `validate_spec` for it
- **THEN** the response SHALL be `valid: false`
- **AND** the findings SHALL name the delta file for both defects

#### Scenario: A change without deltas or proposal is reported

- **GIVEN** a change directory holding only `.openspec.yaml`
- **WHEN** the client calls `validate_spec` for it
- **THEN** the findings SHALL report the missing `proposal.md` and the missing delta spec

#### Scenario: Validation is available without consent and from a worktree

- **GIVEN** a worktree session in a repository with `allow_repo_writes` off
- **WHEN** the client lists tools and calls `validate_spec` for an existing change
- **THEN** the list SHALL include `validate_spec`
- **AND** the call SHALL return findings without modifying any file
