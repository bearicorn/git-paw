# MCP Server

`git paw mcp` runs a read-only [Model Context Protocol](https://modelcontextprotocol.io)
(MCP) server over **stdio**, so any MCP-aware client — Claude Desktop, Cursor,
ChatGPT Desktop, Windsurf, VS Code MCP extensions — can query a repository's
state without an active git-paw session.

It exposes the same information git-paw already tracks, as deterministic,
read-only tools:

- **Coordination** — active agent intents and detected conflicts
- **Governance** — your configured ADRs, test strategy, security checklist,
  Definition of Done, and constitution
- **Project knowledge** — discovered specs and tasks (OpenSpec, Markdown, Spec
  Kit), the spec dependency graph, and rendered agent skills
- **Session state** — the active session's status/summary and the
  session-learnings file
- **Git context** — branches, recent commits, and branch diffs

The server is **standalone**: it does not need a tmux session, broker, or
supervisor. When a data source is unavailable (no broker, no session, no
governance config) tools return well-formed empty/null results rather than
errors — so the client always gets an unambiguous answer.

> **Read-only in v0.7.0.** There are no write tools (no creating specs,
> controlling sessions, or delivering feedback) — those are planned for a later
> release. The server also **never invokes an agent CLI** (`claude`, `gemini`,
> …) as an inference backend; every result comes from files, git, and broker
> state.

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
  works (with Developer Mode). ChatGPT Web waits for a future HTTP transport.
- **Per-repo configuration is required.** Each repository needs its own entry
  in the client config (with its own `--repo` path where the client spawns from
  a fixed directory). For many repos this is repetitive; a single-server,
  multi-repo registry is planned for a later release.
- **Claude Desktop (and ChatGPT Desktop) need `--repo`.** They spawn MCP
  servers from their own app-support directory, not your project, so
  current-directory discovery cannot find your repo. Workspace-aware clients
  (Cursor, VS Code MCP, Windsurf) work with a bare `git paw mcp`.

## Tool reference

All tools are read-only. Collection results are empty (`[]`) and single-record
results are `null` when the underlying data is unavailable. Governance tools
return a protocol error only when a **configured** document path exists but
cannot be read.

### Coordination

| Tool | Input | Result |
|------|-------|--------|
| `get_intents` | — | `{ intents: [{ branch_id, files, regions, summary, published_at, valid_for_seconds }] }` |
| `get_intent` | `{ branch_id }` | `{ intent: <intent> \| null }` |
| `get_conflicts` | — | `{ conflicts: [{ shape, branches, files, detected_at }] }` |

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
error.

### Session state

| Tool | Input | Result |
|------|-------|--------|
| `get_session_status` | — | `{ session: { name, mode, status, paused, agent_count, broker_url, agents:[…] } \| null }` |
| `get_session_summary` | — | `{ summary: { name, status, agent_count, agents_by_status } \| null }` |
| `get_learnings` | — | `{ sections: [{ category, entries:[…] }] }` |

### Git context

| Tool | Input | Result |
|------|-------|--------|
| `get_branches` | — | `{ branches: [{ name, head, current, worktree }] }` |
| `get_recent_commits` | `{ branch, limit? }` | `{ commits: [{ sha, author, timestamp, subject }] }` (`limit` defaults to 20) |
| `get_diff` | `{ branch, base? }` | `{ base, branch, diff, files_changed, insertions, deletions }` (`base` defaults to the repo's default branch) |

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
