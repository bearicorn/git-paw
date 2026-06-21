## Context

The v0.7.0 `mcp-server` ships five tool categories but no way to read the repo's README or mdBook docs. `git paw mcp` is meant to run standalone, so a client may have no other way to read those files. The data layer (`src/mcp/query/`) and tool layer (`src/mcp/tools/`) are cleanly separated; `query::resolve_under_root` + `read_optional_doc` already implement BYO-path reads + repo-root resolution used by the governance tools. Tool categories attach via `<cat>_router()` merged in `GitPawMcpServer::new`.

## Goals / Non-Goals

**Goals:**
- Read-only access to the README and the documentation tree over MCP, via BYO config paths.
- Reuse existing patterns: `GovernanceConfig` for the paths, `query::*` for reads, a `docs_router()` for tools.
- Path-traversal safety on `get_doc` (confine to the configured docs dir).

**Non-Goals:**
- No new config section (paths live under `[governance]`); no rename.
- No hardcoded README/docs locations; unset → empty/null (graceful degradation).
- No write tools, no rendering/transform of docs (raw content only).

## Decisions

- **Config home:** extend `GovernanceConfig` with `readme` + `docs` (`Option<PathBuf>`) rather than a new `[docs]`/`[mcp]` section — keeps one BYO config home, non-breaking.
- **`get_doc` confinement:** resolve the requested path under the configured docs dir, then canonicalize and verify the result is still within the docs dir (reject `..`/absolute escapes) — mirroring the path guards the security review confirmed for `get_spec`/`get_tasks`.
- **Degradation:** `readme`/`docs` unset → `get_readme` null, `list_docs` empty, `get_doc` empty/refused — never a transport error. Configured-but-unreadable surfaces as a tool-level error/message.
- **`list_docs`:** non-recursive vs recursive — walk the docs dir recursively for `*.md`, returning paths relative to the docs dir (so they feed directly back into `get_doc`).

## Risks / Trade-offs

- **Semantics of "docs under [governance]":** README/user-docs aren't strictly governance, but `[governance]` is already the repo-knowledge path-pointer section; folding docs in avoids a second section. Accepted.
- **Path-traversal:** the single real risk; mitigated by reusing the confirmed confinement pattern + an explicit traversal-rejection test.
