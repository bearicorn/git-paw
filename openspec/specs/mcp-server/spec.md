# mcp-server Specification

## Purpose
A `git paw mcp` subcommand that runs a stdio JSON-RPC MCP server exposing read-only, deterministically-sourced tools (never invoking an agent CLI as an inference backend) over a resolved repository/worktree root. It advertises a schema-carrying tool registry and git-paw server identity, keeps stdout reserved for protocol frames with logging on stderr, and distinguishes graceful empty/null degradation from hard errors on malformed configuration.
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

