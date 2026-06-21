## Why

The MCP server exposes specs, docs, coordination, governance, session, and git diffs/log — but no way to **browse the source tree or read source files**. A client connected only to `git paw mcp` cannot explore `src/`, so it falls back to cloning the public repo (observed in testing). Exploring the actual code is the most common need for a "developer-MCP-for-this-repo", and reading the *local* tree gives the true branch/uncommitted state a public clone can't. Adding source tools completes the read surface.

## What Changes

Three new read-only MCP tools in the `mcp-read-tools` surface (a Source/Files category), each deterministic file/git access — no agent CLI invoked, JSON-schema'd:

- **`list_files(subpath?)`** — list the repository's working-tree files via `git ls-files --cached --others --exclude-standard` (tracked **plus** untracked-but-not-ignored), optionally scoped to a subpath. Gitignored paths (`target/`, build artifacts, secrets) are **excluded**. Empty list when not a git repo.
- **`read_file(path)`** — return one file's content from the local working tree, **confined to the repository root** (canonicalize + `starts_with` guard, reusing `get_doc`'s confinement) and **gitignore-respecting** (a gitignored path is refused). Null/empty with a message when refused or absent.
- **`search_code(query, subpath?)`** — search file contents across the repository's tracked + non-ignored files, returning matches as `{ path, line_number, line }`. This is what lets a client trace logic across files (find a symbol's definition, then its call sites). Empty when no matches or not a git repo.

Together (`list_files` → `search_code` → `read_file`) these give a client the standard explore-and-trace toolkit over the local repo.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `mcp-read-tools`: add a **Source/Files tools** requirement (`list_files`, `read_file`, `search_code`) to the read-only tool surface.

## Impact

- Affected code: `src/mcp/query/source.rs` (new — `list_files`, `read_file`, `search_code` over `git ls-files`/`git grep` or equivalent, with repo-root confinement + gitignore respect); `src/mcp/query/mod.rs` (`pub mod source`); `src/mcp/tools/source.rs` (new — three tools + `source_router`); `src/mcp/tools/mod.rs` (`pub mod source`); `src/mcp/server.rs` (merge `source_router` into `GitPawMcpServer::new`).
- Implementation prefers git plumbing (`git ls-files`, `git grep`) so gitignore handling + tracked-set semantics come for free and stay consistent with the rest of the git-context tools; falls back to empty results when git is unavailable.
- Docs: `docs/src/user-guide/mcp.md` (tool reference), `git paw mcp --help` surface note.
- Security: exposes committed + working source on the user's own machine to a client they spawned; the gitignore exclusion + repo-root confinement keep secrets/build artifacts out. No write surface. No new dependencies (uses `git` via `std::process::Command`, as the existing git-context tools do).
