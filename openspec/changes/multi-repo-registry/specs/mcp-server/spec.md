## MODIFIED Requirements

### Requirement: Repository resolution

The system SHALL resolve a home repository on startup using the
following precedence: (1) the value of the `--repo <path>` flag if
provided; (2) the nearest ancestor of `std::env::current_dir()`
containing a `.git/` directory or `.git` file (worktree). If a
worktree is detected, the system SHALL resolve to the worktree's own
root, NOT the main repository root.

Without `--all-repos`, a home repository is required, and every tool
operates against it. With `--all-repos`, the home repository is
optional. A current directory outside any git repository SHALL NOT be
an error in that mode, but a `--repo` value that cannot be resolved
SHALL still be one. Each tool call then targets either the home
repository or the registered repository its `repo` parameter names, as
*Per-call repository targeting* specifies.

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

- **WHEN** the user invokes `git paw mcp` (no `--repo`, no
  `--all-repos`) from a directory with no ancestor containing `.git/`
- **THEN** the server SHALL exit with non-zero status and SHALL
  emit a human-readable error to stderr explaining that no git
  repository was found and how to use `--repo`

#### Scenario: --repo pointing at a non-git path fails clearly

- **WHEN** the user invokes `git paw mcp --repo /tmp/not-a-repo`
  where `/tmp/not-a-repo` exists but is not a git repository
- **THEN** the server SHALL exit with non-zero status and SHALL
  emit a human-readable error to stderr identifying the path and
  reason

#### Scenario: --all-repos starts without a home repository

- **WHEN** the user invokes `git paw mcp --all-repos` (no `--repo`)
  from a directory with no ancestor containing `.git/`
- **THEN** the server SHALL complete the MCP `initialize` handshake
  and serve requests with no home repository

#### Scenario: --all-repos with an invalid --repo still fails clearly

- **WHEN** the user invokes `git paw mcp --all-repos --repo
  /tmp/not-a-repo` where `/tmp/not-a-repo` is not a git repository
- **THEN** the server SHALL exit with non-zero status and SHALL
  emit a human-readable error to stderr identifying the path

### Requirement: Subcommand flag surface

The system SHALL support exactly the following flags on
`git paw mcp`:

- `--repo <PATH>`: override the repository resolution
- `--log-file <PATH>`: write tracing output to a file in addition
  to stderr (optional; off by default)
- `--all-repos`: serve every repository in the repository registry
  from this one server (optional; off by default)

The system SHALL NOT advertise or implement `--port`, `--host`,
`--daemon`, `start`, `stop`, or `status`. These are
reserved for a future HTTP-transport addition.

#### Scenario: --help text describes only the supported flags

- **WHEN** the user runs `git paw mcp --help`
- **THEN** the output SHALL describe `--repo`, `--log-file` and
  `--all-repos` with examples, and SHALL NOT advertise any
  daemon-style or HTTP-transport flags

### Requirement: Documentation deliverable

The system SHALL ship with detailed per-client setup documentation
in the mdBook user guide. The documentation SHALL cover at minimum:
Claude Desktop, ChatGPT Desktop, Cursor, VS Code MCP extensions,
and Windsurf. For each client the documentation SHALL include the
exact configuration file path, a copy-pasteable JSON snippet
showing the server entry, restart instructions, and a verification
step the user can run to confirm the connection. The documentation
SHALL also document the known limitations: ChatGPT Web is
unsupported in v0.7.0, a single-repository entry serves one
repository (one entry per repository unless the entry uses
`--all-repos`), and Claude Desktop requires `--repo` or `--all-repos`
because it spawns servers from its own app-support directory.

The documentation SHALL describe all-repos serving: how repositories
enter the registry (`git paw init`, re-run after upgrading), the
`list_repos` tool and the `repo` parameter, the access policy
(read-only across repositories, no access from worktree sessions,
per-repository opt-out with `[mcp] register = false`), and a warning
not to put `--all-repos` or a `--repo` naming a main working tree in
an MCP configuration that agent CLIs running in worktrees inherit.

#### Scenario: mdBook chapter exists with per-client walkthroughs

- **WHEN** `mdbook build docs/` runs successfully
- **THEN** the output SHALL contain a chapter titled "MCP" (or
  equivalent) under the user guide, and that chapter SHALL contain
  one subsection per supported client with at minimum a config
  snippet and a verification step

#### Scenario: Known limitations are documented prominently

- **WHEN** a user reads the MCP chapter
- **THEN** the limitations section SHALL clearly state that
  ChatGPT Web is unsupported, that a single-repository entry serves
  one repository unless it uses `--all-repos`, and that Claude
  Desktop needs `--repo` or `--all-repos` — each with a brief
  explanation of why

#### Scenario: All-repos serving and its access policy are documented

- **WHEN** a user reads the MCP chapter
- **THEN** it SHALL contain an all-repos section with a client
  snippet using `--all-repos`, the registration step, the
  `list_repos` and `repo` usage, the read-only cross-repository
  policy, the worktree restriction, the `[mcp] register = false`
  opt-out, and the inherited-configuration warning

## ADDED Requirements

### Requirement: All-repos serving mode

`git paw mcp --all-repos` SHALL serve every repository in the repository registry from one server process. In this mode it SHALL advertise a `list_repos` tool and an optional `repo` parameter on every other tool.

Without `--all-repos` the server SHALL NOT advertise `list_repos`, and no tool's input schema SHALL declare a `repo` property. The advertised tool set and every other tool parameter SHALL stay as they were before this mode existed. A call that supplies a `repo` argument anyway SHALL be refused with an MCP `invalid_params` error that names `--all-repos`.

When the home repository resolves to a linked worktree, the server SHALL ignore `--all-repos`, SHALL write a warning to stderr, and SHALL behave as if the flag were absent.

In all-repos mode with no home repository, the server SHALL NOT advertise any tool that publishes to a broker or changes repository or session state.

The advertised `serverInfo.name` SHALL come from the home repository's configuration when there is a home repository. Otherwise it SHALL come from the user configuration, and it SHALL default to `git-paw`.

#### Scenario: Single-repository mode advertises no repository surface

- **GIVEN** `git paw mcp --repo <repo>` without `--all-repos`
- **WHEN** the client lists tools
- **THEN** the list SHALL NOT contain `list_repos`
- **AND** no tool's input schema SHALL declare a `repo` property
- **AND** the set of tool names SHALL equal the set advertised before this mode existed

#### Scenario: Single-repository mode refuses a supplied repo argument

- **GIVEN** a server started without `--all-repos`
- **WHEN** the client calls `get_branches({ "repo": "other" })`
- **THEN** the server SHALL return an MCP `invalid_params` error naming `--all-repos`
- **AND** SHALL continue running

#### Scenario: All-repos mode advertises the repository surface

- **GIVEN** `git paw mcp --all-repos --repo <main working tree>`
- **WHEN** the client lists tools
- **THEN** the list SHALL contain `list_repos`
- **AND** every other tool's input schema SHALL declare an optional `repo` string property

#### Scenario: A worktree home disables all-repos mode

- **GIVEN** `git paw mcp --all-repos` started from inside a linked worktree
- **WHEN** the client lists tools
- **THEN** the list SHALL NOT contain `list_repos`, and no input schema SHALL declare `repo`
- **AND** stderr SHALL carry a warning that `--all-repos` is ignored for a worktree session

#### Scenario: No home repository means no write tools

- **GIVEN** `git paw mcp --all-repos` started outside any git repository
- **WHEN** the client lists tools
- **THEN** the list SHALL contain `list_repos` and the read-only tools
- **AND** SHALL NOT contain `publish_status`, `publish_artifact`, `publish_blocked`, `publish_question`, or any session-lifecycle or verdict tool

#### Scenario: Identity without a home repository defaults to git-paw

- **GIVEN** `git paw mcp --all-repos` started outside any git repository with no `[mcp].name` in the user configuration
- **WHEN** the client completes the `initialize` handshake
- **THEN** `serverInfo.name` SHALL be `"git-paw"`

### Requirement: Repository listing tool

In all-repos mode the server SHALL expose `list_repos()`. The tool SHALL take no parameters and SHALL return `{ "repos": [...] }`, with one entry per registry entry, sorted by `id`. Each entry SHALL carry `id`, `path`, `available` (a boolean), and `home` (true when the entry is the home repository). An entry whose `available` is false SHALL also carry a human-readable `reason`. Entries for repositories excluded by `[mcp] register = false` SHALL be omitted. A missing registry SHALL yield an empty list. A registry the server refuses to read (unparseable, a newer version, or unsafe permissions) SHALL yield an MCP-level error that names the registry file. The listing SHALL reflect the registry's contents at call time.

#### Scenario: Registered repositories are listed with the home flag

- **GIVEN** repositories `a` and `b` are registered and the server was started with `--all-repos --repo <a>`
- **WHEN** the client calls `list_repos()`
- **THEN** the response SHALL list `a` and `b` in that order, both with `available: true`
- **AND** only `a` SHALL have `home: true`

#### Scenario: Unavailable entry carries a reason

- **GIVEN** a registered repository whose directory has been deleted
- **WHEN** the client calls `list_repos()`
- **THEN** its entry SHALL have `available: false` and a `reason` stating the path no longer exists

#### Scenario: Excluded repository is omitted

- **GIVEN** a registry entry for a repository whose configuration sets `[mcp] register = false`
- **WHEN** the client calls `list_repos()`
- **THEN** the response SHALL NOT contain that repository

#### Scenario: Missing registry yields an empty list

- **GIVEN** no registry file exists
- **WHEN** the client calls `list_repos()`
- **THEN** the response SHALL be successful with an empty `repos` array

#### Scenario: Newly initialized repository appears without a restart

- **GIVEN** a running all-repos server
- **WHEN** `git paw init` registers a new repository and the client then calls `list_repos()`
- **THEN** the response SHALL contain the new repository

#### Scenario: Unreadable registry is reported, not hidden

- **GIVEN** a registry file containing invalid JSON
- **WHEN** the client calls `list_repos()`
- **THEN** the server SHALL return an MCP-level error naming the registry file
- **AND** SHALL continue running

### Requirement: Per-call repository targeting

In all-repos mode every tool other than `list_repos` SHALL accept an optional string parameter `repo`. When `repo` is omitted, or equals the home repository's registry id, the call SHALL target the home repository exactly as a single-repository server would.

Any other value SHALL be matched against registry ids by exact string equality. The server SHALL NOT interpret it as a filesystem path.

A value that names no registry entry, an unavailable entry, or an excluded repository SHALL be refused with an MCP `invalid_params` error that names `list_repos`. For an unavailable entry the error SHALL also carry the reason. With no home repository, a call that omits `repo` SHALL be refused with an MCP `invalid_params` error.

A call that targets a registered repository SHALL keep the tool's existing behavior relative to that repository: its root, its configuration and its active session's broker. That behavior includes graceful degradation, path confinement and gitignore refusal.

#### Scenario: A read tool targets another registered repository

- **GIVEN** an all-repos server whose home is `a`, with `b` registered and containing `README.md`
- **WHEN** the client calls `read_file({ "repo": "b", "path": "README.md" })`
- **THEN** the response SHALL contain the content of `b`'s `README.md`

#### Scenario: A path-shaped repo value is refused without touching the filesystem

- **WHEN** the client calls `list_files({ "repo": "/etc" })` or `list_files({ "repo": "../b" })`
- **THEN** the server SHALL return an MCP `invalid_params` error naming `list_repos`
- **AND** SHALL NOT list or read any file

#### Scenario: Unknown repository id is refused

- **WHEN** the client calls `get_branches({ "repo": "no-such-repo" })`
- **THEN** the server SHALL return an MCP `invalid_params` error naming `list_repos`

#### Scenario: Unavailable repository is refused with its reason

- **GIVEN** a registered repository whose directory has been deleted
- **WHEN** the client calls `get_branches` with that repository's id
- **THEN** the server SHALL return an MCP `invalid_params` error carrying the unavailability reason

#### Scenario: Omitting repo without a home repository is refused

- **GIVEN** an all-repos server started with no home repository
- **WHEN** the client calls `get_branches({})`
- **THEN** the server SHALL return an MCP `invalid_params` error stating that `repo` is required

#### Scenario: Path confinement applies relative to the target repository

- **GIVEN** registered repositories `a` and `b` that are sibling directories
- **WHEN** the client calls `read_file({ "repo": "b", "path": "../a/README.md" })`
- **THEN** the read SHALL be refused, with null content and a message
- **AND** no file outside `b`'s root SHALL be read

#### Scenario: A target with no session degrades gracefully

- **GIVEN** a registered repository `b` with no active session
- **WHEN** the client calls `get_intents({ "repo": "b" })`
- **THEN** the response SHALL be successful with an empty `intents` array

#### Scenario: The home repository's own id behaves as omission

- **GIVEN** an all-repos server whose home is registered as `a`
- **WHEN** the client calls `get_branches({ "repo": "a" })` and `get_branches({})`
- **THEN** both responses SHALL be identical

### Requirement: Cross-repository access is read-only

The server SHALL NOT act for any repository other than the home repository through a tool that publishes to a broker or changes repository or session state. Such a tool (the agent publish tools and every root-session tool) SHALL refuse a `repo` that names another repository with an MCP `invalid_params` error. It SHALL do so without contacting that repository's broker or changing its state. Write tools SHALL keep their existing identity rules for the home repository.

#### Scenario: Cross-repository publish is refused

- **GIVEN** an all-repos server whose home is `a`, and a registered repository `b` with a running broker
- **WHEN** the client calls `publish_status({ "repo": "b", "message": "booting" })`
- **THEN** the server SHALL return an MCP `invalid_params` error stating that cross-repository writes are not supported
- **AND** `b`'s broker log SHALL contain no new message

#### Scenario: Cross-repository session lifecycle is refused

- **GIVEN** an all-repos server whose home is the main working tree of `a`, with root-session lifecycle tools advertised, and a registered repository `b` with an active session
- **WHEN** the client calls `stop_session({ "repo": "b", "confirm": true })`
- **THEN** the server SHALL return an MCP `invalid_params` error
- **AND** `b`'s session SHALL be unchanged

#### Scenario: A home write through the home id is allowed

- **GIVEN** an all-repos server whose home is registered as `a`, with an active session and broker
- **WHEN** the client calls `publish_status({ "repo": "a", "message": "booting" })`
- **THEN** the message SHALL be published exactly as `publish_status({ "message": "booting" })` would publish it

### Requirement: Per-target configuration errors are isolated

The server SHALL validate its startup configuration against the home repository only. A configuration error in a targeted non-home repository SHALL be returned as an MCP-level error for calls that target that repository. Examples are an invalid `[specs].type`, or a configured governance path that exists but cannot be read. Such an error SHALL NOT stop the server, and SHALL NOT affect calls that target any other repository.

#### Scenario: Malformed target configuration errors only that target

- **GIVEN** an all-repos server with registered repositories `a` and `b`, where `b`'s configuration sets an invalid `[specs].type`
- **WHEN** the client calls `get_specs({ "repo": "b" })` and then `get_specs({ "repo": "a" })`
- **THEN** the first call SHALL return an MCP-level error identifying `b` and the invalid value
- **AND** the second call SHALL succeed

#### Scenario: Startup ignores a malformed non-home configuration

- **GIVEN** a registered repository whose configuration is malformed
- **WHEN** `git paw mcp --all-repos` starts outside any git repository
- **THEN** the server SHALL complete the `initialize` handshake
