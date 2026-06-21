## Context

`get_info()` in src/mcp/server.rs returns `ServerInfo::default()` with only `capabilities` + `instructions` overridden, so `serverInfo.name`/`version` fall back to the rmcp SDK defaults (`rmcp` / `1.7.0`). The server already holds a `RepoContext`; the resolved `PawConfig` (hence a configurable name) is available at startup in `run()`/`new()`.

## Goals / Non-Goals

**Goals:**
- Advertise `name = "git-paw"` + the real crate version in the handshake.
- Allow a per-repo `[mcp].name` override for multi-repo disambiguation.

**Non-Goals:**
- No change to the client-side display name (the `mcpServers` key — already user-controlled).
- No `[mcp]` fields beyond `name` in this change.

## Decisions

- **Config home:** a new `[mcp]` section (`McpConfig { name }`) — MCP-server-specific settings, distinct from the repo-knowledge path pointers under `[governance]`. (Doc paths intentionally stayed under `[governance]`; the server *name* is not a path pointer, so `[mcp]` is its natural home.)
- **Plumbing:** resolve the effective name (`config.mcp.name` unwrap_or "git-paw") at server construction and store it on `RepoContext` (or the server struct), so `get_info()` reads it without re-loading config.
- **Version:** `env!("CARGO_PKG_VERSION")` (compile-time, always the real version).

## Risks / Trade-offs

- **Client display vs server identity:** some clients show the `mcpServers` key, others `serverInfo.name`; this change fixes the latter only. Documented so users know which knob does what.
- Minimal surface — single field, default-preserving.
