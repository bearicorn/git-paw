# Tasks

## 1. Query layer

- [x] 1.1 `src/mcp/query/source.rs` — `list_files(repo_root, subpath: Option<&str>)` via `git ls-files --cached --others --exclude-standard [-- subpath]`; return `Vec<String>` (paths relative to repo root); empty when not a git repo
- [x] 1.2 `read_file(repo_root, path)` — resolve under repo root, canonicalize + `starts_with` confinement (reuse the `query::docs::read_doc` guard pattern); refuse gitignored paths (`git check-ignore` or working-tree-set membership); return on-disk content; `None`/refused with reason otherwise
- [x] 1.3 `search_code(repo_root, query, subpath: Option<&str>)` via `git grep -n -I --untracked -e <query> [-- subpath]`; return `Vec<Match { path, line_number, line }>`; cap to a bounded count and record if truncated; empty when no match / not a git repo
- [x] 1.4 Register `pub mod source;` in `src/mcp/query/mod.rs`
- [x] 1.5 Unit tests with `tempfile` git fixtures: list excludes gitignored + includes untracked-not-ignored; subpath scope; read happy path; read traversal (`../`, absolute) refused; read gitignored refused; search finds a known string; search no-match empty; non-git dir → empty

## 2. Tool layer

- [x] 2.1 `src/mcp/tools/source.rs` — `list_files(ListFilesParams { subpath: Option<String> })`, `read_file(ReadFileParams { path })`, `search_code(SearchCodeParams { query, subpath: Option<String> })` with `schemars::JsonSchema` params/responses, mapping onto the query layer; `#[tool]` + `#[tool_router]` → `source_router()`; mirror `tools/project.rs`
- [x] 2.2 Merge `Self::source_router()` into `GitPawMcpServer::new` in `src/mcp/server.rs`
- [x] 2.3 Module doc comment + `pub mod source;` in `src/mcp/tools/mod.rs`
- [x] 2.4 Unit tests per tool: happy path, empty/degraded path, `read_file` traversal + gitignored refused (not a transport error)

## 3. Docs

- [x] 3.1 `docs/src/user-guide/mcp.md` — add `list_files`/`read_file`/`search_code` to the tool reference (Source/Files category); note the gitignore + repo-confinement guards
- [x] 3.2 `git paw mcp --help` surface description mentions source browsing (if it enumerates categories)
- [x] 3.3 `mdbook build docs/` succeeds

## 4. Quality gates

- [ ] 4.1 `just check` (fmt + clippy + tests) passes
- [ ] 4.2 `just deny` passes
- [x] 4.3 No `unwrap()`/`expect()` in non-test code; public items documented; no `print!`/`println!` under `src/mcp/`
- [x] 4.4 Every scenario in `specs/mcp-read-tools/spec.md` maps to a test
