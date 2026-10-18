## MODIFIED Requirements

### Requirement: The server exposes a bounded write surface alongside read-only tools

The MCP server SHALL NOT be exclusively read-only. It SHALL advertise, alongside its read-only tool categories: (a) the agent-scoped publish category — the four worker boot-event tools for every session, plus the two authority publish tools for a root session only; and (b) a root-session lifecycle category (`start_session`, `pause_session`, `stop_session`, `purge_session`) that performs exactly what the equivalent `git paw` command performs, by calling the library command surface in-process. Every other tool category SHALL remain read-only and deterministically sourced.

The write surface SHALL be bounded by construction: a fixed set of tools, each acting as the calling session's identity, with root-only tools advertised only to a root session. The server SHALL NOT expose a tool that writes an arbitrary file, runs an arbitrary git mutation, writes configuration, or executes an arbitrary command; the only repository mutations reachable are those the lifecycle tools perform on git-paw-managed worktrees and branches.

#### Scenario: Publish and lifecycle categories coexist with read-only categories

- **WHEN** the tool registry is inspected from a root session
- **THEN** it SHALL contain the publish category and the lifecycle category
- **AND** every other advertised category SHALL perform reads only

#### Scenario: No arbitrary file, git or config mutation is exposed

- **WHEN** the tool registry is inspected
- **THEN** it SHALL NOT contain a tool that writes a caller-named file, runs a caller-supplied git or shell command, or writes configuration

#### Scenario: Read-only guarantees of other categories are preserved

- **GIVEN** the coordination, governance, project-knowledge, session-state, git-context, documentation, and source/file categories
- **WHEN** any of their tools is invoked
- **THEN** it SHALL perform deterministic reads only
- **AND** SHALL NOT mutate repository or broker state

## ADDED Requirements

### Requirement: Root-session detection

The server SHALL determine at startup whether it runs in the repository's main working tree or in a linked worktree, as git reports it (the checkout's git directory differs from the common git directory for a linked worktree — which also holds under `--separate-git-dir`), and SHALL advertise root-only tools — the lifecycle category and the authority publish tools — only for the main working tree. The server SHALL load the session's authority credential only when it resolved the main working tree, even though a worktree server also resolves the owning repository's session. Invoking a root-only tool from a worktree session SHALL return an MCP-level error stating the tool is not available from a worktree session, without crashing the server.

#### Scenario: Root session sees root-only tools

- **GIVEN** an MCP server resolved to the repository's main working tree
- **WHEN** the client lists tools
- **THEN** the list SHALL include `start_session`, `pause_session`, `stop_session`, `purge_session`, `publish_verified` and `publish_feedback`

#### Scenario: Worktree session does not see root-only tools

- **GIVEN** an MCP server resolved to a `git worktree add`-created worktree
- **WHEN** the client lists tools
- **THEN** the list SHALL include the four worker publish tools
- **AND** SHALL NOT include any lifecycle tool or authority publish tool

#### Scenario: A worktree session holds no credential

- **GIVEN** a running session with an authority credential and an MCP server started in one of its worktrees
- **WHEN** the server resolves the owning repository's session
- **THEN** it SHALL hold no authority credential
- **AND** SHALL NOT advertise `publish_verified` or `publish_feedback`

#### Scenario: A separate-git-dir main checkout is a root session

- **GIVEN** a repository created with `git init --separate-git-dir` and an MCP server started in its main checkout
- **WHEN** the client lists tools
- **THEN** the list SHALL include the lifecycle tools

#### Scenario: Calling a root-only tool from a worktree errors cleanly

- **GIVEN** a worktree MCP session
- **WHEN** the client calls `pause_session` by name
- **THEN** the server SHALL return an MCP-level error mentioning the worktree restriction
- **AND** SHALL continue running

### Requirement: Session lifecycle tools for the root session

The server SHALL expose, to a root session, `start_session` (parameters: optional `cli`, `branches`, `specs` selection, `supervisor: bool`, `dry_run: bool`; launches the session detached and SHALL NOT run the unattended drive loop; returns the session name, the attach command and the orchestrator-context paths), `pause_session` (no parameters), `stop_session` (no parameters; behaves as `git paw stop`, which never prompts because stopping is recoverable) and `purge_session` (`confirm: true` required; behaves as a confirmed `git paw purge`, including keeping every branch that carries unmerged commits, and returns the deleted and retained branches with their tips). `purge_session` SHALL carry the MCP `destructiveHint` annotation. Each tool SHALL call the library command surface in-process and return structured JSON. When no session exists, `pause_session`, `stop_session` and `purge_session` SHALL return a successful response with a `no_session: true` field rather than an error. Each tool SHALL advertise a JSON Schema for its parameters and return shape.

#### Scenario: start_session launches detached

- **GIVEN** a root session in a repository with two branches and the broker enabled
- **WHEN** the client calls `start_session({ "branches": ["feat/a", "feat/b"], "supervisor": true })`
- **THEN** a tmux session `paw-<project>` SHALL exist with the dashboard and two agent panes
- **AND** the response SHALL contain the session name, the attach command and `.git-paw/AGENTS.local.md`
- **AND** the MCP server SHALL still be responsive (no blocking loop)

#### Scenario: start_session never runs the drive loop

- **WHEN** the `start_session` input schema is inspected
- **THEN** it SHALL contain no `unattended` parameter
- **AND** invoking it SHALL NOT start an in-process drive loop

#### Scenario: pause and stop round-trip

- **GIVEN** an active session
- **WHEN** the client calls `pause_session()` and then `stop_session()`
- **THEN** after the first call the session state SHALL be `Paused` and the broker port free
- **AND** after the second the tmux session SHALL be gone and the state `Stopped`

#### Scenario: stop_session takes no confirmation or force parameter

- **WHEN** the `stop_session` input schema is inspected
- **THEN** it SHALL contain no `confirm` and no `force` parameter

#### Scenario: Purge requires confirmation

- **WHEN** the client calls `purge_session({})` without `confirm: true`
- **THEN** the server SHALL return an MCP-level error naming `confirm`
- **AND** the session, worktrees and branches SHALL be unchanged

#### Scenario: Purge keeps unmerged branches

- **GIVEN** a stopped session whose branch `feat/x` carries a commit not in the default branch
- **WHEN** the client calls `purge_session({ "confirm": true })`
- **THEN** `feat/x` SHALL still exist
- **AND** the response SHALL list `feat/x` as retained with its tip

#### Scenario: Purge is annotated destructive

- **WHEN** the client lists tools from a root session
- **THEN** `purge_session` SHALL carry `destructiveHint: true`

#### Scenario: No session yields a graceful result

- **GIVEN** no session exists for the repository
- **WHEN** the client calls `pause_session()`
- **THEN** the response SHALL be successful with `no_session: true`
