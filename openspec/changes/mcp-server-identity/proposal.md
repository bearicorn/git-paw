## Why

The MCP server advertises the wrong identity. `GitPawMcpServer::get_info()` builds `ServerInfo::default()` and overrides only `capabilities` + `instructions`, never `server_info.name`/`version` — so the `initialize` handshake returns `serverInfo: { name: "rmcp", version: "1.7.0" }` (the rmcp SDK defaults), not git-paw. The server misrepresents itself to every client and in logs. Separately, when a user runs `git paw mcp` against several repos, the instances are indistinguishable by server identity; the advertised name should be configurable per repo.

## What Changes

- **Fix server identity:** `get_info()` SHALL set `server_info.name = "git-paw"` and `server_info.version = env!("CARGO_PKG_VERSION")` so the handshake reports git-paw and its real version (not `rmcp`).
- **New `[mcp]` config section** with an optional `name: Option<String>` field (per-repo). When set, the MCP server SHALL advertise that name as `serverInfo.name` instead of the default `git-paw`; when unset, it SHALL advertise `git-paw`. This lets multi-repo setups distinguish instances via the server's own identity (independent of the client-side `mcpServers` key the user already controls).

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `mcp-server`: add a **Server identity** requirement — the server advertises `name = "git-paw"` (or the configured `[mcp].name`) and the crate version, never the SDK default.
- `configuration`: add an `[mcp]` section parsed into an `McpConfig` with an optional `name` field (`None` by default, backward compatible).

## Impact

- Affected code: `src/config.rs` (new `McpConfig { name: Option<String> }` + `PawConfig.mcp` field, `#[serde(default)]`); `src/mcp/server.rs` (`get_info()` sets name/version, reading the configured name from the `RepoContext`/config); `src/mcp/mod.rs` (carry the configured server name on `RepoContext` if not already available).
- Docs: `docs/src/user-guide/mcp.md` (note the advertised identity + the `[mcp].name` override), configuration reference (`[mcp]` section).
- Backward compatible: configs with no `[mcp]` section load with `McpConfig::default()` (`name: None`) → the server advertises `git-paw`. No new dependencies.
