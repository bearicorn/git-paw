# MCP Server

`git paw mcp` runs a [Model Context Protocol](https://modelcontextprotocol.io)
(MCP) server over **stdio**, so any MCP-aware client — Claude Desktop, Cursor,
ChatGPT Desktop, Windsurf, VS Code MCP extensions — can query a repository's
state without an active git-paw session.

It exposes the same information git-paw already tracks as deterministic,
read-only tools, beside a bounded write surface (publish tools, and session
lifecycle and verdict tools for a root session only — see below). The
read-only tools cover:

- **Coordination** — active agent intents and detected conflicts
- **Governance** — your configured ADRs, test strategy, security checklist,
  Definition of Done, and constitution
- **Project knowledge** — discovered specs and tasks (OpenSpec, Markdown, Spec
  Kit, Superpowers), the spec dependency graph, and rendered agent skills
- **Session state** — the active session's status/summary and the
  session-learnings file
- **Git context** — branches, recent commits, and branch diffs
- **Source/Files** — browse the local working tree, read a file, and search
  code contents (gitignored paths excluded; reads confined to the repo root)

Alongside those, the server exposes a bounded **write** surface: the
[Publish tools](#publish-tools) for every session, and — for a **root session**
only (a server started in the repository's main working tree) — the
[Session lifecycle](#session-lifecycle-root-session-only) tools and the two
verdict publish tools, plus — only once the repository opts in with
`[mcp] allow_repo_writes` — the
[Spec authoring](#spec-authoring-repository-writes-root-session-only) write
tools. `validate_spec` is read-only and every session gets it.

The server is **standalone**: it does not need a tmux session, broker, or
supervisor for the read-only categories (the publish tools do need a reachable
broker — see below). When a data source is unavailable (no broker, no
session, no governance config) read tools return well-formed empty/null
results rather than errors — so the client always gets an unambiguous answer.

> **Read/write boundary.** Every tool category except **Publish tools**,
> **Session lifecycle** and the two **Spec authoring** write tools is read-only, deterministically sourced from files, git,
> and broker state. The write surface is a fixed set of tools: the four worker
> publish tools (the same four events the bundled `broker.sh` helper covers),
> each publishing only as the calling session; and, for a root session only,
> the session lifecycle tools (exactly what `git paw start` / `pause` / `stop` /
> `purge` do) and the two verdict tools (`publish_verified`,
> `publish_feedback`, exactly what `sweep.sh verified` / `feedback-gate` do).
> The spec-authoring write tools, for a consenting repository's root session,
> write only inside one spec change directory. There is no tool that writes a
> file outside a spec change directory, runs a caller-supplied git or shell
> command, or writes configuration, and no generic "publish anything"
> tool. A server in an agent's worktree never lists or runs the root-only tools,
> so a coding agent cannot verify its own work through MCP — see
> [Publish tools](#publish-tools). The server also **never invokes an agent
> CLI** (`claude`, `codex`, …) as an inference backend; every result comes from
> files, git, and broker state.

## How it works

The MCP client spawns `git paw mcp` as a child process and talks to it over
the process's stdin/stdout using newline-delimited JSON-RPC 2.0. The client
owns the lifecycle: the server starts when the client launches it and exits
cleanly when the client closes its stdin.

```text
git paw mcp [--repo <PATH>] [--log-file <PATH>]
```

| Flag | Purpose |
|------|---------|
| `--repo <PATH>` | Operate against a specific repository instead of the current directory. **Required for Claude Desktop** (see below). |
| `--log-file <PATH>` | Also write diagnostics to a file. stderr is always used; stdout is reserved for the JSON-RPC stream. |

Verbosity follows the standard `RUST_LOG` convention (default `warn`):

```bash
RUST_LOG=debug git paw mcp --repo /path/to/repo
```

### Repository resolution

1. If `--repo <PATH>` is given, the server uses that path (it must be inside a
   git repository).
2. Otherwise the server walks up from the current directory to the nearest
   enclosing git repository. Inside a `git worktree`, it resolves to the
   worktree's **own** root.

If no repository can be found, the server prints a clear error to stderr and
exits non-zero — it never silently serves nothing.

**Finding the session and its broker from inside a worktree.** File-serving
tools keep the resolution above: inside a worktree, the repository IS that
worktree. The *session* lookup is deliberately wider — a worktree root never
equals the repository root, so matching it directly would leave a server
started by an agent's CLI (its working directory is the agent's worktree, and
it is given no `--repo`) with no session and no broker URL. The server
therefore resolves a worktree to the repository that **owns** it before looking
the session up, trying the exact path first and falling back to the owner only
on a miss. `get_session_status` and the broker URL in the server's context
come from that lookup.

This widening is confined to the MCP server's read-only view. Every `git paw`
command that mutates session state — `stop`, `purge`, `add`, `remove`,
`pause`, and `start`'s recover/resume dispatch — keeps the exact match, so
running one inside an agent worktree never acts on the parent session.

### Server identity

In the MCP `initialize` handshake the server advertises `serverInfo.name =
"git-paw"` and its real crate version. To distinguish several repositories that
each run `git paw mcp`, set a custom name in the repo's `.git-paw/config.toml`:

```toml
[mcp]
name = "my-project"   # advertised as serverInfo.name; defaults to "git-paw"
```

This is the **server's own** advertised identity. It is distinct from the
display label some clients show, which comes from the `mcpServers` **key** in
the client config (e.g. `"git-paw"` in the snippet below) — that key is yours
to rename per client entry regardless of `[mcp].name`.

## Per-client setup

Each client needs its own entry pointing at this server. The command is `git`
with args `paw mcp …` (git-paw installs as a `git` subcommand). If you prefer,
`git-paw mcp …` works too.

### Claude Desktop

Claude Desktop spawns MCP servers from its own application-support directory,
**not** your project — so current-directory discovery cannot work. You **must**
pass `--repo` with an absolute path.

1. Open the config file:
   - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
   - Windows: `%APPDATA%\Claude\claude_desktop_config.json`
2. Add a server entry:

   ```json
   {
     "mcpServers": {
       "git-paw": {
         "command": "git",
         "args": ["paw", "mcp", "--repo", "/absolute/path/to/your/repo"]
       }
     }
   }
   ```

3. Restart Claude Desktop completely (quit and reopen).
4. **Verify:** open a new chat, click the tools/🔌 icon, and confirm `git-paw`
   is listed with its tools. Ask "what specs are pending in this repo?" and
   confirm a tool call runs.

### Cursor

Cursor is workspace-aware and spawns from the workspace root, so bare
`git paw mcp` works (or pass `--repo` to be explicit).

1. Create `.cursor/mcp.json` in your project (or edit the global
   `~/.cursor/mcp.json`):

   ```json
   {
     "mcpServers": {
       "git-paw": {
         "command": "git",
         "args": ["paw", "mcp"]
       }
     }
   }
   ```

2. Reload Cursor (or toggle the server in **Settings → MCP**).
3. **Verify:** open **Settings → MCP** and confirm `git-paw` shows a green
   status with its tools listed.

### ChatGPT Desktop (macOS)

ChatGPT Desktop supports local MCP servers in Developer Mode. (ChatGPT **Web**
in a browser is **not** supported — see Known limitations.)

1. Enable **Settings → Connectors → Advanced → Developer mode**.
2. Add a connector with command `git` and args `paw mcp --repo /absolute/path/to/your/repo`
   (ChatGPT Desktop, like Claude Desktop, does not run from your project
   directory, so pass `--repo`).
3. Restart ChatGPT Desktop.
4. **Verify:** start a new chat, open the connector/tools menu, and confirm
   `git-paw` and its tools are available.

### Windsurf

1. Open **Settings → Cascade → MCP Servers → Add Server** (or edit
   `~/.codeium/windsurf/mcp_config.json`):

   ```json
   {
     "mcpServers": {
       "git-paw": {
         "command": "git",
         "args": ["paw", "mcp", "--repo", "/absolute/path/to/your/repo"]
       }
     }
   }
   ```

2. Refresh the MCP server list (or restart Windsurf).
3. **Verify:** the MCP panel shows `git-paw` connected with its tools.

### VS Code (MCP)

VS Code (1.102+) supports MCP servers. It is workspace-aware, so bare
`git paw mcp` works from a workspace folder.

1. Create `.vscode/mcp.json` in your workspace:

   ```json
   {
     "servers": {
       "git-paw": {
         "type": "stdio",
         "command": "git",
         "args": ["paw", "mcp"]
       }
     }
   }
   ```

2. Run **MCP: List Servers** from the Command Palette and start `git-paw`
   (or reload the window).
3. **Verify:** in Agent mode, open the tools picker and confirm the `git-paw`
   tools appear.

## Known limitations

- **ChatGPT Web is not supported.** The browser version of ChatGPT cannot
  spawn local processes, which stdio MCP requires. ChatGPT **Desktop** on macOS
  works (with Developer Mode). The server speaks stdio only — there is no HTTP
  transport.
- **Per-repo configuration is required.** Each repository needs its own entry
  in the client config (with its own `--repo` path where the client spawns from
  a fixed directory). For many repos this is repetitive; one server instance
  serves exactly one repository.
- **Claude Desktop (and ChatGPT Desktop) need `--repo`.** They spawn MCP
  servers from their own app-support directory, not your project, so
  current-directory discovery cannot find your repo. Workspace-aware clients
  (Cursor, VS Code MCP, Windsurf) work with a bare `git paw mcp`.

## Tool reference

Every category below is read-only except **Publish tools**, **Session
lifecycle** and the two write tools of **Spec authoring**. For the
read-only categories, collection results are empty (`[]`) and single-record
results are `null` when the underlying data is unavailable. Governance tools
return a protocol error only when a **configured** document path exists but
cannot be read.

### Coordination

| Tool | Input | Result |
|------|-------|--------|
| `get_intents` | — | `{ intents: [{ branch_id, files, regions, summary, published_at, valid_for_seconds }] }` |
| `get_intent` | `{ branch_id }` | `{ intent: <intent> \| null }` |
| `get_conflicts` | — | `{ conflicts: [{ shape, branches, files, detected_at }] }` |
| `wait_for_events` | `{ since?, timeout_seconds? }` | `{ entries: [<broker message>], last_seq, epoch, wave }` |

`wait_for_events` is the MCP form of the broker's
[long-pollable event log](coordination.md#waiting-for-events). `since` is a
broker log sequence number (default `0`) and `timeout_seconds` defaults to
`30`, capped at `60`. The tool long-polls `GET /log`, so it returns as soon as
an entry newer than `since` is appended, or when the timeout elapses —
whichever comes first. The `wave` object reports
[wave staleness](coordination.md#wave-staleness), so a call that times out
with no entries still tells the caller whether the wave has stalled.

`epoch` identifies the broker instance that answered. Sequence numbers
restart at zero whenever a broker does — a `pause` + `start` cycle, for
instance — so a cursor is only meaningful against the epoch it came from.
**Record the epoch with your cursor and pass `since: 0` on the next call
whenever it changes**, or you will hold a cursor above everything the new
broker appends and time out on every call. See
[The broker epoch](coordination.md#the-broker-epoch--reset-your-cursor-when-it-changes).

With no reachable session broker it returns at once with empty `entries`,
`last_seq` equal to the `since` it was given, and `no_session: true`. It
mutates nothing: like every tool outside [Publish tools](#publish-tools), it
only reads broker state.

### Governance

| Tool | Input | Result |
|------|-------|--------|
| `get_adrs` | — | `{ adrs: [{ id, title, path, status }] }` |
| `get_adr` | `{ query }` | `{ adr: { id, path, content } \| null }` |
| `get_test_strategy` | — | `{ content: <string> \| null }` |
| `get_security_checklist` | — | `{ content: <string> \| null }` |
| `get_dod` | — | `{ content: <string> \| null }` |
| `check_dod` | `{ branch }` | `{ branch, items: [{ text, complete }] \| null }` |
| `get_constitution` | — | `{ content: <string> \| null }` |

### Project knowledge

| Tool | Input | Result |
|------|-------|--------|
| `get_specs` | — | `{ specs: [{ id, backend, title, status, path }] }` |
| `get_spec` | `{ id }` | `{ spec: { id, backend, path, artifacts: [{ name, content }] } \| null }` |
| `get_tasks` | `{ spec }` | `{ tasks: [{ id, phase, parallel, description, complete }] }` |
| `get_task` | `{ spec, id }` | `{ task: <task> \| null }` |
| `get_dependency_graph` | — | `{ nodes: [{ id, backend }], edges: [{ from, to }] }` |
| `get_skill` | `{ name }` | `{ skill: { name, content, source } \| null, message? }` |

`source` is one of `standard` (`.agents/skills/`), `user_override`, or
`embedded`. An unknown skill returns `skill: null` with a `message` — not an
error. The orchestrator skill is served under the name `orchestrator`
(`supervisor` is an alias returning the same content) — the same procedure
`git paw init` exports to `.git-paw/skills/orchestrator/SKILL.md`.

### Session state

| Tool | Input | Result |
|------|-------|--------|
| `get_session_status` | — | `{ session: { name, mode, status, paused, agent_count, broker_url, agents:[…], wave? } \| null }` |
| `get_session_summary` | — | `{ summary: { name, status, agent_count, agents_by_status } \| null }` |
| `get_learnings` | — | `{ sections: [{ category, entries:[…] }] }` |

`get_session_status` includes the broker's `wave` object — the session's
[wave-staleness](coordination.md#wave-staleness) reading
(`last_progress_unix_secs`, `idle_seconds`, `stall_threshold_seconds`,
`stalled`) — whenever a broker is reachable, and omits the field entirely
otherwise.

### Git context

| Tool | Input | Result |
|------|-------|--------|
| `get_branches` | — | `{ branches: [{ name, head, current, worktree }] }` |
| `get_recent_commits` | `{ branch, limit? }` | `{ commits: [{ sha, author, timestamp, subject }] }` (`limit` defaults to 20) |
| `get_diff` | `{ branch, base? }` | `{ base, branch, diff, files_changed, insertions, deletions }` (`base` defaults to the repo's default branch) |

### Source/Files

Browse and read the repository's local working tree — tracked files **plus**
untracked-but-not-ignored files. Gitignored paths (build artifacts, secrets)
are excluded throughout, and `read_file` is **confined to the repository root**
(canonicalise + `starts_with` guard, the same as `get_doc`): a path escaping
the root (`../`, an absolute path, a symlink target outside the root) is
refused, and a gitignored path is refused, both with `null` content and a
`message` rather than a read. Reads return the on-disk working-tree content, so
uncommitted/branch state shows. Implemented over `git ls-files` / `git grep`,
so a non-git directory or a no-match search degrades to an empty result.

| Tool | Input | Result |
|------|-------|--------|
| `list_files` | `{ subpath? }` | `{ files: [<path>] }` (tracked + untracked-not-ignored, relative to the repo root, optionally scoped to `subpath`; empty when not a git repo) |
| `read_file` | `{ path }` | `{ content: <string> \| null, message? }` (`path` relative to the repo root; confined to the root and gitignore-respecting — a path escaping the root or naming a gitignored file is refused with `null` content and a `message`; `null` with a message when absent) |
| `search_code` | `{ query, subpath? }` | `{ matches: [{ path, line_number, line }], truncated }` (matches across the working tree, binaries skipped, optionally scoped to `subpath`; capped with a `truncated` flag; empty when no matches or not a git repo) |

### Documentation

Read-only access to the repository's own documentation, driven by the
bring-your-own `[governance].readme` and `[governance].docs` config paths
(see [below](#documentation-config)). Locations are configured, never
hardcoded — unset paths degrade to `null`/empty results.

| Tool | Input | Result |
|------|-------|--------|
| `get_readme` | — | `{ content: <string> \| null }` (null when `[governance].readme` is unset or the file is absent) |
| `list_docs` | — | `{ docs: [{ path }] }` (Markdown files under `[governance].docs`, paths relative to that dir; empty when unset) |
| `get_doc` | `{ path }` | `{ content: <string> \| null, message? }` (`path` relative to `[governance].docs`; confined to that dir — a path escaping it, e.g. `../`, is refused with `null` content and a `message`, not a file read outside the directory) |

<a id="documentation-config"></a>
The documentation tools read two optional `[governance]` paths in
`.git-paw/config.toml`:

```toml
[governance]
readme = "README.md"   # path to the repository README
docs   = "docs/src"     # path to the documentation root directory
```

Both default to unset, in which case `get_readme` returns `null`, `list_docs`
returns an empty list, and `get_doc` returns `null` — identical to the
pre-v0.7.0 surface. `list_docs` walks the `docs` directory recursively for
`*.md` files; the relative paths it returns feed directly back into `get_doc`.

### Publish tools

Four worker tools, one per agent boot event, each publishing the same message
the bundled [`broker.sh` helper](coordination.md#broker-helper) does — this is
the [MCP tool form of the boot block](coordination.md#mcp-tool-form) for a CLI
that speaks MCP — plus two verdict tools for a root session only, each
publishing the same message `sweep.sh verified` / `feedback-gate` does.

| Tool | Input | Result |
|------|-------|--------|
| `publish_status` | `{ message }` | `{ published: true }` — `agent.status` (`status:"working"`, the message, `modified_files:[]`) |
| `publish_artifact` | `{ exports?, modified_files? }` | `{ published: true }` — `agent.artifact` (`status:"done"`) — the code-less DONE fallback; a task that produces a commit reports via the post-commit hook instead |
| `publish_blocked` | `{ needs, from }` | `{ published: true }` — `agent.blocked` with dependency info |
| `publish_question` | `{ question }` | `{ published: true }` — `agent.question`, routed to the supervisor inbox |
| `publish_verified` *(root session only)* | `{ agent, message }` | `{ published: true }` — `agent.verified` for `agent`, `verified_by:"supervisor"` |
| `publish_feedback` *(root session only)* | `{ agent, gate?, errors }` | `{ published: true }` — `agent.feedback` to `agent` from `supervisor`; with `gate`, each error is tagged `[<gate>] ` |

There is no inbox or poll tool: an agent reads its broker inbox with
`.git-paw/scripts/broker.sh poll`, whichever form its boot block uses.

Three properties are structural, not incidental:

- **Bounded, not generic.** These are distinct tool definitions, not one
  `publish(type, payload)` tool. A message type is never an accepted
  parameter, so the set of things a client can publish cannot grow by passing
  a new string — it can only grow by this server shipping a new tool.
- **Session-scoped identity.** No publish tool accepts an `agent_id` or
  publisher parameter. In an agent's worktree, each derives the publishing id
  from the worktree's branch (the same conversion the `broker.sh poll`/`status`
  path uses), so a session can only ever publish as itself. In the repository's
  **main working tree** — the orchestrator's — every publish tool publishes as
  the root identity `supervisor`, never as an agent named after the default
  branch. The verdict tools take the *target* agent as `agent`, as the helpers
  do, and set the verifier / sender to `supervisor` themselves.
- **Verdicts belong to the root session only.** `agent.verified` is what
  authorises a branch to be merged; `agent.feedback` from the supervisor is its
  corrective channel. The broker accepts either only with the session's
  authority credential, which lives outside the repository. `publish_verified`
  and `publish_feedback` are registered only for a server started in the main
  working tree, which attaches the credential itself (it is never a tool
  parameter and never appears in a result or an error). A server in an agent's
  worktree does not list them, and a by-name call returns an MCP-level error
  naming the restriction, so a coding agent cannot self-verify through MCP.

Publish tools need a **reachable broker** — unlike the read-only categories,
there is no meaningful "empty" result for a publish that did not happen. When
no session is active, or the broker is unreachable, or it rejects the
message, the tool returns an MCP-level error and the server keeps running;
nothing is ever written to stdout outside the JSON-RPC protocol stream. The
verdict tools read the session's broker and credential when they run, so a root
server that started the session itself (`start_session`) can publish verdicts
without restarting.

### Session lifecycle (root session only)

Offered only to a server started in the repository's main working tree (as git
reports it, so a `--separate-git-dir` checkout counts); a server in an agent's
worktree does not list them, and a by-name call returns an MCP-level error.
Each does exactly what the equivalent `git paw` command does, in-process, and
returns structured JSON with the command's output lines and warnings.

| Tool | Input | Result |
|------|-------|--------|
| `start_session` | `{ cli?, branches?, specs?, from_all_specs?, supervisor?, dry_run? }` | `{ outcome, session_name, attach_command, orchestrator_context, output, warnings }` |
| `pause_session` | `{}` | `{ no_session, session_name, outcome, output, warnings }` |
| `stop_session` | `{}` | `{ no_session, session_name, outcome, output, warnings }` |
| `purge_session` | `{ confirm }` | `{ no_session, session_name, deleted, retained, output, warnings }` |

- **`start_session` is detached and never prompts.** It launches (or revives —
  restarts from pause, recovers a stopped session) the repository's session
  without attaching a terminal, and returns once the launch's readiness gates
  have run, with the session name, the command a person runs to attach, and the
  orchestrator-context file (`.git-paw/AGENTS.local.md`). It has **no
  `unattended` parameter**: run `git paw start --unattended` yourself for an
  unattended wave. Nothing is asked interactively — pass `branches` or a spec
  selection, and `cli` unless the repository sets `default_cli`; a choice the
  command would otherwise put to a picker is refused with the parameter to
  pass. `supervisor` absent means the repository's `[supervisor]` config
  decides, as a non-interactive `git paw start` does. `dry_run` reports the plan
  and creates nothing.
- **`stop_session` takes no confirmation.** Stopping is recoverable (the
  worktrees, branches and state stay; `start_session` recovers it), as with
  `git paw stop`.
- **`purge_session` requires `confirm: true`** and is annotated destructive
  (`destructiveHint`). It removes the tmux session, every worktree (including
  uncommitted changes in them) and the session state, as a confirmed
  `git paw purge` does. Branches git-paw created are deleted only when merged;
  a branch with unmerged commits is kept and listed in `retained` with its tip.
- With no session, `pause_session`, `stop_session` and `purge_session` return
  `{ "no_session": true }` rather than an error.

Authoring specs over MCP is a separate surface, gated by the repository's own
consent; see [Spec authoring](#spec-authoring-repository-writes-root-session-only).

### Spec authoring (repository writes, root session only)

Three tools let an orchestrator author and check an OpenSpec change without its
own file tools. `create_spec` and `write_spec_artifact` write files into the
repository working tree, so they are **off by default**. `validate_spec` only
reads, and every session gets it.

| Tool | Input | Result |
|------|-------|--------|
| `create_spec` | `{ name }` | `{ written }` |
| `write_spec_artifact` | `{ change, path, content }` | `{ written, replaced }` |
| `validate_spec` | `{ change }` | `{ valid, findings: [{ file, message }] }` |

**Consent.** The two write tools are listed and run only when both of these
hold:

- The repository's own `.git-paw/config.toml` sets
  [`[mcp] allow_repo_writes = true`](../configuration/README.md#mcp). A value in
  the user-level config has no effect, and no environment variable or tool
  argument turns it on.
- The server runs in the repository's main working tree (a root session). A
  server in an agent's worktree never gets them, even with the switch on.

The server reads the switch once, at startup, and logs whether repository
writes are enabled. Changing it in either direction takes effect only after
you restart the MCP server. When a condition fails, the tools are absent from
`tools/list`. A by-name call returns an MCP error and writes nothing. The
error names `allow_repo_writes` and `.git-paw/config.toml` when the switch is
off, or the worktree restriction in a worktree session.

**The tools:**

- **`create_spec`** creates `<[specs] dir>/<name>/.openspec.yaml` with
  `schema: spec-driven` and today's date as `created: YYYY-MM-DD`. That is the
  scaffold `openspec new change <name>` creates, so the consumer needs no
  OpenSpec CLI installed. `name` must be lowercase letters, digits and hyphens,
  and an existing change is refused.
- **`write_spec_artifact`** writes `content` (at most 1 MiB) verbatim to
  `path` inside the existing change directory `<[specs] dir>/<change>/`. It
  refuses `.openspec.yaml`, which `create_spec` owns. It creates missing
  parent directories inside the change and replaces an existing regular file.
  `replaced` says whether it did. A change that does not exist is an error.
- **`validate_spec`** runs an in-process structural check on a change. It
  reports a missing `proposal.md`, no `specs/<capability>/spec.md` with an
  `## ADDED`, `## MODIFIED`, `## REMOVED` or `## RENAMED Requirements` header,
  an added or modified requirement whose first body line has neither SHALL nor
  MUST, and an added or modified requirement without a `#### Scenario:`.
  `valid` is `true` exactly when `findings` is empty. It spawns no process and
  checks only this subset, so your project's own spec validator stays
  authoritative.

```json
{ "name": "write_spec_artifact",
  "arguments": { "change": "add-export",
                 "path": "specs/data-export/spec.md",
                 "content": "## ADDED Requirements\n..." } }
```

```json
{ "written": ["openspec/changes/add-export/specs/data-export/spec.md"], "replaced": false }
```

Returned paths are repository-relative. Each write logs one line on the server
log with the path, byte count and whether it replaced a file. The tools never
stage or commit, so `git status` and `git diff` show exactly what was authored.

**Confinement.** Before writing, the server refuses the call with an MCP error
naming the offending input, and writes nothing, when:

- `path` is absolute or has a `..` or `.git` component;
- `change` is not a single kebab-case change name (lowercase letters, digits
  and hyphens);
- the spec directory resolves outside the repository, inside its `.git`, to
  the repository root itself, or inside `.git-paw`;
- a directory between the change directory and the target is a symbolic link
  resolving outside the change directory;
- the target itself is a symbolic link.

Content goes to a temporary file in the target directory and is renamed into
place.

**OpenSpec only.** The spec directory and format come from the `[specs]`
section of the repository's own `.git-paw/config.toml`; a user-level `[specs]`
section is ignored here, as it is for consent. Under any other `[specs] type`
the three tools return "not supported for format `<type>`". With no `[specs]`
section in the repository config they return an error naming `[specs]`.

The switch does not govern the lifecycle and verdict tools above. Those follow
the root-session rule alone.
