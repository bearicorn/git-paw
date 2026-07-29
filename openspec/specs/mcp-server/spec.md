# mcp-server Specification

## Purpose
A `git paw mcp` subcommand that runs a stdio JSON-RPC MCP server exposing read-only, deterministically-sourced tools (never invoking an agent CLI as an inference backend) over a resolved repository/worktree root — advertising a schema-carrying tool registry and git-paw server identity, keeping stdout reserved for protocol frames with logging on stderr, and distinguishing graceful empty/null degradation from hard errors on malformed configuration — together with the read-only tool set it advertises: coordination (intents/conflicts), governance docs, project knowledge (specs/tasks/skills), session state, git context, documentation, and source/file tools, each with a JSON Schema and deterministic, path-confined reads that degrade gracefully to empty arrays or null when their data source is absent and refuse path traversal or gitignored reads outside the repository/docs roots.

## Requirements
### Requirement: MCP server subcommand

The system SHALL provide a `git paw mcp` subcommand that runs a Model
Context Protocol (MCP) server over stdio. The subcommand SHALL accept
JSON-RPC 2.0 messages on stdin and SHALL emit JSON-RPC 2.0 responses
on stdout per the MCP specification. The subcommand SHALL exit cleanly
when the client closes the stdin stream.

#### Scenario: Client spawns the server and exchanges the MCP initialize handshake

- **WHEN** an MCP client spawns `git paw mcp` and sends an MCP
  `initialize` request on stdin
- **THEN** the server SHALL respond on stdout with an MCP
  `initialize` response advertising the implemented protocol version
  and the set of tools available

#### Scenario: Stdin EOF terminates the server

- **WHEN** the parent MCP client closes the server's stdin
- **THEN** `git paw mcp` SHALL terminate with exit status 0 within
  one second

### Requirement: Repository resolution

The system SHALL resolve a target repository on startup using the
following precedence: (1) the value of the `--repo <path>` flag if
provided; (2) the nearest ancestor of `std::env::current_dir()`
containing a `.git/` directory or `.git` file (worktree). If a
worktree is detected, the system SHALL resolve to the worktree's own
root, NOT the main repository root.

#### Scenario: --repo flag wins over CWD

- **WHEN** the user invokes `git paw mcp --repo /path/to/repo` from
  any working directory
- **THEN** the server SHALL operate against `/path/to/repo`
  regardless of where it was invoked from

#### Scenario: CWD walk finds the enclosing git repository

- **WHEN** the user invokes `git paw mcp` (no `--repo`) from a
  subdirectory of a git repository
- **THEN** the server SHALL operate against the nearest ancestor
  directory containing `.git/`

#### Scenario: Worktree resolves to worktree root

- **WHEN** the user invokes `git paw mcp` (no `--repo`) from inside
  a `git worktree add`-created worktree
- **THEN** the server SHALL operate against the worktree's own root,
  not the main repository's root

#### Scenario: Invocation outside any git repository fails clearly

- **WHEN** the user invokes `git paw mcp` (no `--repo`) from a
  directory with no ancestor containing `.git/`
- **THEN** the server SHALL exit with non-zero status and SHALL
  emit a human-readable error to stderr explaining that no git
  repository was found and how to use `--repo`

#### Scenario: --repo pointing at a non-git path fails clearly

- **WHEN** the user invokes `git paw mcp --repo /tmp/not-a-repo`
  where `/tmp/not-a-repo` exists but is not a git repository
- **THEN** the server SHALL exit with non-zero status and SHALL
  emit a human-readable error to stderr identifying the path and
  reason

### Requirement: Tool registry

The system SHALL expose every implemented tool via the MCP
`tools/list` method. Each advertised tool SHALL include a name,
description, and a JSON Schema for its input parameters. Calls to
the MCP `tools/call` method with an unknown tool name SHALL return
an MCP protocol-level error (not crash the server).

#### Scenario: Client lists available tools

- **WHEN** the MCP client sends a `tools/list` request after
  initialization
- **THEN** the server SHALL respond with the full list of
  implemented tools, each carrying name, description, and input
  schema

#### Scenario: Unknown tool name returns a protocol error

- **WHEN** the MCP client sends `tools/call` with a tool name not
  present in the registry
- **THEN** the server SHALL respond with a JSON-RPC error indicating
  "tool not found" and SHALL continue running

### Requirement: Graceful degradation when state is unavailable

The system SHALL return well-formed empty / null result documents
when a tool's underlying data source is unavailable. The system
SHALL NOT return a JSON-RPC error for these expected-empty cases.
"Unavailable" includes: no active broker process, no active tmux
session, no `[governance]` paths configured, no specs found in the
repository.

#### Scenario: Coordination tool returns empty arrays when no broker is running

- **GIVEN** the repository has no active tmux session and no
  broker process running
- **WHEN** the MCP client calls `get_intents()` or `get_conflicts()`
- **THEN** the tool SHALL return a successful response containing
  empty arrays for the respective collections

#### Scenario: Governance tool returns nulls when no governance paths are configured

- **GIVEN** the repository's `.git-paw/config.toml` has no
  `[governance]` section, or the section is empty
- **WHEN** the MCP client calls `get_dod()` or `get_constitution()`
- **THEN** the tool SHALL return a successful response with a
  null value for the requested document and an empty array for
  collection-shaped responses

#### Scenario: Session-state tool returns null session when no session is active

- **GIVEN** the repository has no `.git-paw/sessions/*.json` files
  or all session files describe stopped sessions
- **WHEN** the MCP client calls `get_session_status()`
- **THEN** the tool SHALL return a successful response with the
  `session` field set to null

### Requirement: Hard errors only for malformed configuration

The system SHALL return JSON-RPC protocol errors (not empty
results) when a user's configuration points at resources that exist
but cannot be read or parsed. These cases reflect user error and
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

### Requirement: Stdout reserved for MCP protocol

The system SHALL emit only MCP-protocol JSON-RPC frames on stdout.
The system SHALL route all logging, diagnostic output, and error
messages to stderr. The codebase SHALL contain no `print!` or
`println!` invocations within `src/mcp/` (only `eprint!`,
`eprintln!`, and `tracing` macros routed to stderr).

#### Scenario: Stdout contains only JSON-RPC frames after startup

- **WHEN** the server runs to completion through an
  initialize → tools/list → tools/call → shutdown lifecycle
- **THEN** every byte written to stdout SHALL be part of a
  well-formed JSON-RPC 2.0 frame per the MCP specification

#### Scenario: Tracing output appears on stderr at configurable verbosity

- **GIVEN** the user invokes `git paw mcp` with `RUST_LOG=debug`
  set in the environment
- **WHEN** the server processes any request
- **THEN** debug-level diagnostic messages SHALL appear on stderr,
  and stdout SHALL remain a clean JSON-RPC stream

### Requirement: No agent CLI invocation as inference backend

The system SHALL NOT invoke any agent CLI (`claude`, `gemini`,
`codex`, `aider`, etc.) as a programmatic inference backend.
Every tool result SHALL be derived from deterministic data sources:
files on disk, git process output, broker in-process state, or
parsed configuration. This guardrail SHALL be enforced by both
specification and code review.

#### Scenario: No agent CLI process is spawned by any tool

- **GIVEN** the full set of MCP tools implemented in this change
- **WHEN** any tool is invoked
- **THEN** the resulting process tree SHALL contain no child
  process whose argv[0] resolves to `claude`, `claude-oss`,
  `gemini`, `codex`, `aider`, `opencode`, `vibe`, `amp`, `qwen`,
  or any other agent CLI binary

### Requirement: Documentation deliverable

The system SHALL ship with detailed per-client setup documentation
in the mdBook user guide. The documentation SHALL cover at minimum:
Claude Desktop, ChatGPT Desktop, Cursor, VS Code MCP extensions,
and Windsurf. For each client the documentation SHALL include the
exact configuration file path, a copy-pasteable JSON snippet
showing the server entry, restart instructions, and a verification
step the user can run to confirm the connection. The documentation
SHALL also document the known limitations: ChatGPT Web is
unsupported in v0.7.0, per-repo configuration is required, and
Claude Desktop requires `--repo` because it spawns servers from
its own app-support directory.

#### Scenario: mdBook chapter exists with per-client walkthroughs

- **WHEN** `mdbook build docs/` runs successfully
- **THEN** the output SHALL contain a chapter titled "MCP" (or
  equivalent) under the user guide, and that chapter SHALL contain
  one subsection per supported client with at minimum a config
  snippet and a verification step

#### Scenario: Known limitations are documented prominently

- **WHEN** a user reads the MCP chapter
- **THEN** the limitations section SHALL clearly state that
  ChatGPT Web is unsupported, that per-repo configuration is
  required, and that Claude Desktop needs the `--repo` flag —
  each with a brief explanation of why

### Requirement: Subcommand flag surface

The system SHALL support exactly the following flags on
`git paw mcp` in v0.7.0:

- `--repo <PATH>`: override the repository resolution
- `--log-file <PATH>`: write tracing output to a file in addition
  to stderr (optional; off by default)

The system SHALL NOT advertise or implement `--port`, `--host`,
`--daemon`, `start`, `stop`, or `status` in v0.7.0. These are
reserved for the v2.0.0 HTTP-transport addition.

#### Scenario: --help text describes only the supported flags

- **WHEN** the user runs `git paw mcp --help`
- **THEN** the output SHALL describe `--repo` and `--log-file`
  with examples, and SHALL NOT advertise any daemon-style or
  HTTP-transport flags

### Requirement: Server identity

The MCP server SHALL advertise its own identity in the `initialize` handshake's `serverInfo`: `name` SHALL be `"git-paw"` (or the configured `[mcp].name` when set) and `version` SHALL be the git-paw crate version (`env!("CARGO_PKG_VERSION")`). The server SHALL NOT advertise the underlying MCP SDK's default identity.

#### Scenario: Default identity is git-paw

- **GIVEN** a repository with no `[mcp].name` configured
- **WHEN** an MCP client completes the `initialize` handshake
- **THEN** the response `serverInfo.name` SHALL be `"git-paw"`
- **AND** `serverInfo.version` SHALL be the git-paw crate version

#### Scenario: Configured name overrides the advertised identity

- **GIVEN** a repository with `[mcp] name = "my-project"` configured
- **WHEN** an MCP client completes the `initialize` handshake
- **THEN** the response `serverInfo.name` SHALL be `"my-project"`
- **AND** `serverInfo.version` SHALL still be the git-paw crate version

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
[[supervisor-learnings]] in v0.5.0).

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
