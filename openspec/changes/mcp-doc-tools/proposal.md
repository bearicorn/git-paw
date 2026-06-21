## Why

The v0.7.0 MCP server (`git paw mcp`) exposes specs, intents, governance docs, session state, and git context — but **not the repository's own documentation**: the README and the mdBook user guide. For a server designed to run **standalone** as the universal "developer-MCP-for-this-repo" (a client may have only `git paw mcp` connected, with no filesystem MCP), this is a real gap: an agent can ask "what specs are pending?" but cannot read the README or the MCP setup guide the repo ships. Adding read-only documentation tools closes that gap and completes the MCP read surface.

Consistent with git-paw's bring-your-own philosophy ([[governance-config]]), doc locations are **configured, not hardcoded** — git-paw stays unopinionated about where a repo keeps its README and docs. README + user docs are part of the same repo-knowledge path-pointer surface as ADR/security/DoD, so they live under the existing `[governance]` section (two new fields) rather than a new config section — one home, no rename, no breakage.

## What Changes

- **Two new `[governance]` fields** (bring-your-own paths, alongside `adr`/`security`/`test_strategy`/`dod`/`constitution`): optional `readme` (path to the README) and `docs` (path to the documentation root, e.g. `docs/src`). Both default to `None`; unset paths degrade to empty results, exactly like the existing governance fields.
- **Three new read-only MCP tools** in the `mcp-read-tools` surface:
  - `get_readme()` — returns the configured README's content (null when `[governance].readme` is unset or the file is absent).
  - `list_docs()` — enumerates Markdown documents under the configured `[governance].docs` directory (empty when unset).
  - `get_doc(path)` — returns the content of one document, **confined to the configured docs dir** via the existing repo-root path-traversal guards (reuses `query::resolve_under_root` confinement).
- All three are read-only, deterministic file reads (no agent CLI invocation), JSON-schema'd like every other MCP tool.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `mcp-read-tools`: add a **Documentation tools** requirement (`get_readme`, `list_docs`, `get_doc`) to the read-only tool surface.
- `governance-config`: add `readme` and `docs` optional path fields to `GovernanceConfig` (BYO pointers, `None` by default, backward compatible).

## Impact

- Affected code: `src/config.rs` (`GovernanceConfig` gains `readme` + `docs`, both `#[serde(default, skip_serializing_if = "Option::is_none")]`); `src/mcp/query/docs.rs` (new — read README, list docs, read one doc confined under the configured dir); `src/mcp/query/mod.rs` (`pub mod docs`); `src/mcp/tools/docs.rs` (new — three tools + `docs_router`); `src/mcp/server.rs` (merge `docs_router` into `GitPawMcpServer::new`).
- Docs: `docs/src/user-guide/mcp.md` (tool reference + the new `[governance]` doc fields), configuration reference, `git paw mcp --help` tool list.
- Backward compatible: `[governance]` sections without `readme`/`docs` load with both `None`; the tools return empty/null, identical to the pre-change MCP surface. No new dependencies.
