# Tasks

## 1. Config

- [x] 1.1 Add `McpConfig { name: Option<String> }` to `src/config.rs` with `#[serde(default)]`, and `pub mcp: McpConfig` on `PawConfig` (`#[serde(default)]`); doc-comment both
- [x] 1.2 Unit tests: `[mcp] name` parses; omitted → `None`; round-trip; pre-existing config without `[mcp]` still loads

## 2. Server identity

- [x] 2.1 In `src/mcp/server.rs` `get_info()`, set `server_info.name` to the effective name (configured `[mcp].name` or `"git-paw"`) and `server_info.version` to `env!("CARGO_PKG_VERSION")`
- [x] 2.2 Plumb the effective name to `get_info()` — resolve at construction (read `config.mcp.name`) and store on `RepoContext`/server; do not re-load config per call
- [x] 2.3 Unit/integration test: handshake `serverInfo.name == "git-paw"` by default; `== configured name` when `[mcp].name` set; `version == env!("CARGO_PKG_VERSION")`

## 3. Docs

- [x] 3.1 `docs/src/user-guide/mcp.md` — note the advertised identity + `[mcp].name` override, and clarify it's distinct from the client-side `mcpServers` key
- [x] 3.2 Configuration reference — document the `[mcp]` section / `name`
- [x] 3.3 `mdbook build docs/` succeeds

## 4. Quality gates

- [x] 4.1 `just check` (fmt + clippy + tests) passes
- [x] 4.2 `just deny` passes
- [x] 4.3 No `unwrap()`/`expect()` in non-test code; public items documented; no `print!`/`println!` under `src/mcp/`
- [x] 4.4 Every scenario in the two delta spec files maps to a test
