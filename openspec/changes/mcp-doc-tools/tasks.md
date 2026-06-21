# Tasks

## 1. Config

- [x] 1.1 Add `readme: Option<PathBuf>` and `docs: Option<PathBuf>` to `GovernanceConfig` in `src/config.rs`, each `#[serde(default, skip_serializing_if = "Option::is_none")]`
- [x] 1.2 Unit tests: `[governance]` with `readme`+`docs` parses both; omitted → both `None`; round-trip serialization preserves them; pre-existing `[governance]` (no new fields) still loads

## 2. Query layer

- [x] 2.1 `src/mcp/query/docs.rs` — `read_readme(repo_root, &GovernanceConfig)` via `read_optional_doc` on the configured `readme`
- [x] 2.2 `list_docs(repo_root, &GovernanceConfig)` — recursively enumerate `*.md` under the configured `docs` dir; return `Vec<DocEntry { path }>` with paths relative to the docs dir; empty when `docs` unset
- [x] 2.3 `read_doc(repo_root, &GovernanceConfig, rel_path)` — resolve under the configured docs dir, canonicalize, and verify the result stays within the docs dir; refuse (empty/null + message) on traversal/escape; empty when `docs` unset
- [x] 2.4 Register `pub mod docs;` in `src/mcp/query/mod.rs`
- [x] 2.5 Unit tests with `tempfile`: readme present/absent/unconfigured; list across nested dirs; read happy path; traversal (`../`, absolute) rejected

## 3. Tool layer

- [x] 3.1 `src/mcp/tools/docs.rs` — `get_readme()`, `list_docs()`, `get_doc(GetDocParams { path })` with `schemars::JsonSchema` params/responses, mapping onto the query layer; mirror `tools/project.rs` structure (`#[tool]` + `#[tool_router]` → `docs_router()`)
- [x] 3.2 Merge `Self::docs_router()` into `GitPawMcpServer::new` in `src/mcp/server.rs`
- [x] 3.3 Module doc comment + register `pub mod docs;` in `src/mcp/tools/mod.rs`
- [x] 3.4 Unit tests per tool: happy path, unconfigured → empty/null, `get_doc` traversal → refused (not a transport error)

## 4. Docs

- [x] 4.1 `docs/src/user-guide/mcp.md` — add `get_readme`/`list_docs`/`get_doc` to the tool reference and document the `[governance].readme`/`[governance].docs` config
- [x] 4.2 Configuration reference — document the two new `[governance]` fields
- [x] 4.3 `git paw mcp --help` tool list includes the three docs tools (if the help enumerates tools)
- [x] 4.4 `mdbook build docs/` succeeds

## 5. Quality gates

- [x] 5.1 `just check` (fmt + clippy + tests) passes
- [x] 5.2 `just deny` passes
- [x] 5.3 No `unwrap()`/`expect()` in non-test code; all public items documented; no `print!`/`println!` under `src/mcp/`
- [x] 5.4 Every scenario in `specs/mcp-read-tools/spec.md` + `specs/governance-config/spec.md` maps to a test
