## ADDED Requirements

### Requirement: Source and file tools

The system SHALL expose read-only MCP tools for browsing and reading the repository's source tree: `list_files`, `read_file`, and `search_code`. All three perform deterministic file/git reads only — no agent CLI is invoked — and each advertises a JSON Schema for its parameters and return shape. The repository's working tree is defined as tracked files plus untracked-but-not-ignored files (gitignored paths are excluded). All reads are confined to the repository root, and `read_file` additionally refuses gitignored paths.

#### Scenario: list_files returns the working tree excluding gitignored paths

- **GIVEN** a git repository containing tracked files, an untracked-but-not-ignored file, and a gitignored path (e.g. `target/`)
- **WHEN** the MCP client calls `list_files()`
- **THEN** the response SHALL include the tracked and untracked-not-ignored files
- **AND** SHALL NOT include any gitignored path

#### Scenario: list_files scopes to a subpath

- **WHEN** the MCP client calls `list_files({ "subpath": "src" })`
- **THEN** the response SHALL include only files under `src`

#### Scenario: list_files degrades to empty when not a git repository

- **GIVEN** a directory that is not a git repository
- **WHEN** the MCP client calls `list_files()`
- **THEN** the response SHALL be an empty list (not a transport error)

#### Scenario: read_file returns a file's content from the local working tree

- **WHEN** the MCP client calls `read_file({ "path": "src/main.rs" })`
- **THEN** the response SHALL contain that file's content as it exists in the local working tree

#### Scenario: read_file refuses path traversal outside the repository root

- **WHEN** the MCP client calls `read_file({ "path": "../../etc/passwd" })` (or any path resolving outside the repository root)
- **THEN** the response SHALL refuse the read (null/empty content with a message) and SHALL NOT read any file outside the repository root

#### Scenario: read_file refuses a gitignored path

- **GIVEN** a path that is gitignored (e.g. `target/debug/foo`)
- **WHEN** the MCP client calls `read_file({ "path": "target/debug/foo" })`
- **THEN** the response SHALL refuse the read (null/empty content with a message) and SHALL NOT return the file's content

#### Scenario: search_code returns matches across the working tree

- **GIVEN** a repository whose source contains the string `register_watch_target_http`
- **WHEN** the MCP client calls `search_code({ "query": "register_watch_target_http" })`
- **THEN** the response SHALL list matches, each with `path`, `line_number`, and the matching `line`
- **AND** matches SHALL come only from tracked / non-ignored files

#### Scenario: search_code degrades to empty when there are no matches

- **WHEN** the MCP client calls `search_code({ "query": "a-string-that-appears-nowhere" })`
- **THEN** the response SHALL be an empty match list (not a transport error)
