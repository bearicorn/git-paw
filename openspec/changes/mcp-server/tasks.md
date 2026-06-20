## 1. Dependency review

- [x] 1.1 Survey the Rust MCP SDK landscape; identify the canonical
      crate (likely `rmcp`), verify its license is MIT or Apache-2.0
      compatible, and check its current stability/version
- [x] 1.2 If `rmcp` (or equivalent) is FOSS-compatible, add it to
      `Cargo.toml` and append it to the approved-dependency table in
      AGENTS.md with a one-line justification; otherwise document the
      hand-rolled JSON-RPC fallback decision in design.md's Open
      Questions and add `serde_json` usage notes
- [x] 1.3 Run `just deny` to confirm license + advisory checks pass
      with the new dependency

## 2. CLI surface

- [x] 2.1 Add `Command::Mcp { repo: Option<PathBuf>, log_file:
      Option<PathBuf> }` variant to `src/cli.rs`
- [x] 2.2 Write `about` + `long_about` + per-flag `help` strings on
      the new variant per the AGENTS.md CLI convention
- [x] 2.3 Include a copy-pasteable Claude Desktop config snippet in
      the `long_about` so users see it in `--help`
- [x] 2.4 Wire the variant in `src/main.rs` to a new `cmd_mcp`
      function

## 3. Repository resolution

- [x] 3.1 Implement `src/mcp/repo.rs` (or inline in `src/mcp/mod.rs`)
      with the resolution algorithm from design D3: `--repo` flag
      wins; otherwise CWD walk; worktree resolves to worktree root;
      bare repos rejected; non-git path under `--repo` rejected
- [x] 3.2 Build a `RepoContext { root, git_paw_dir, broker_url }`
      struct constructed during startup and passed to every tool
- [x] 3.3 Add unit tests covering: --repo with valid path, --repo
      with non-git path errors, CWD finds enclosing repo, worktree
      resolves to own root, no-git-ancestor errors clearly

## 4. Module skeleton

- [x] 4.1 Create `src/mcp/mod.rs`, `src/mcp/server.rs`,
      `src/mcp/tools/mod.rs`, `src/mcp/query/mod.rs` per design D2
- [x] 4.2 Register the new module tree in `src/lib.rs` and
      `src/main.rs`
- [x] 4.3 Add module-level doc comments (`//!`) per the AGENTS.md
      convention; declare the dependency rule (`query` → no MCP;
      `tools` → MCP + query; `server` → wires only) in `mod.rs`
- [x] 4.4 Add a CI lint test that asserts no `print!` / `println!`
      invocations exist under `src/mcp/`

## 5. Data layer (query)

- [x] 5.1 `src/mcp/query/intents.rs` — wrap `broker::intents`
      access; return `Vec<Intent>` or empty when broker is off
- [x] 5.2 `src/mcp/query/conflicts.rs` — wrap conflict registry
      access; return `Vec<Conflict>` or empty
- [x] 5.3 `src/mcp/query/specs.rs` — reuse the existing spec
      discovery used by `--from-all-specs` to enumerate OpenSpec /
      Markdown / Spec Kit specs
- [x] 5.4 `src/mcp/query/session.rs` — read
      `<git_paw_dir>/sessions/*.json` and the broker `/status`
      endpoint if a session is active
- [x] 5.5 `src/mcp/query/learnings.rs` — parse
      `<git_paw_dir>/session-learnings.md` into structured sections;
      return empty sections when the file does not exist
- [x] 5.6 `src/mcp/query/governance.rs` — read files at
      `[governance]` paths; return null for unset; return
      structured error for unreadable
- [x] 5.7 `src/mcp/query/git.rs` — wrap `std::process::Command`
      invocations for `git branch`, `git log`, `git diff`
- [x] 5.8 Unit tests per query function using `tempfile` fixtures

## 6. Tool implementations

- [x] 6.1 `src/mcp/tools/coordination.rs` — register
      `get_intents`, `get_intent`, `get_conflicts` with input
      schemas; map onto query layer
- [x] 6.2 `src/mcp/tools/governance.rs` — register `get_adrs`,
      `get_adr`, `get_test_strategy`, `get_security_checklist`,
      `get_dod`, `check_dod`, `get_constitution`; include the
      governance-file-unreadable error path
- [x] 6.3 `src/mcp/tools/project.rs` — register `get_specs`,
      `get_spec`, `get_tasks`, `get_task`, `get_dependency_graph`
- [x] 6.3a `get_skill(name)` in `src/mcp/tools/project.rs` — call
      the existing `skills::render` resolution pipeline (project
      `.agents/skills/` → user override → embedded default) and
      return rendered content + `source`; read-only, no disk write,
      no watcher, no version endpoint
- [x] 6.3b Unit tests for `get_skill`: rendered-content happy path
      across all three sources, and unknown-skill → null payload +
      message (not a transport error)
- [x] 6.4 `src/mcp/tools/session.rs` — register
      `get_session_status`, `get_session_summary`, `get_learnings`
- [x] 6.5 `src/mcp/tools/git.rs` — register `get_branches`,
      `get_recent_commits`, `get_diff`
- [x] 6.6 Confirm every tool advertises a precise `inputSchema`
      (JSON Schema 2020-12) on the MCP `tools/list` response
- [x] 6.7 Unit tests per tool: schema correctness, happy path,
      empty/null degradation path

## 7. Server lifecycle

- [x] 7.1 `src/mcp/server.rs` — implement stdio loop (initialize
      handshake → tools/list → tools/call → notifications →
      shutdown) using the chosen SDK or hand-rolled framing
- [x] 7.2 Exit cleanly with status 0 when stdin EOF is received
- [x] 7.3 Return JSON-RPC `tool not found` on `tools/call` for
      unknown tool names without crashing
- [x] 7.4 Wire `cmd_mcp` to resolve the repo, build `RepoContext`,
      register tools, and enter the server loop

## 8. Logging

- [x] 8.1 Initialize `tracing-subscriber` at server startup with
      stderr writer; default level `warn`; respects `RUST_LOG`
- [x] 8.2 When `--log-file <path>` is set, additionally tee
      tracing output to that file
- [x] 8.3 Add an E2E test that asserts stdout contains only valid
      JSON-RPC frames across a full lifecycle

## 9. Documentation

- [x] 9.1 Create `docs/src/user-guide/mcp.md` mdBook chapter
- [x] 9.2 Per-client subsections with config snippets + restart
      steps + verification steps: Claude Desktop, ChatGPT Desktop,
      Cursor, VS Code MCP, Windsurf
- [x] 9.3 Known-limitations subsection: ChatGPT Web unsupported,
      per-repo config required, Claude Desktop needs `--repo` (each
      with a brief why)
- [x] 9.4 Tool-reference subsection enumerating every tool with
      input/output shapes (auto-generated from the JSON Schemas if
      feasible; otherwise hand-maintained alongside the schemas)
- [x] 9.5 Update README.md with a MCP quick-start section pointing
      at the mdBook chapter
- [x] 9.6 Add `git paw mcp` to the README CLI table
- [x] 9.7 Confirm `mdbook build docs/` succeeds with the new
      chapter

## 10. Integration tests

- [x] 10.1 `tests/mcp_e2e.rs` — spawn `git paw mcp` as a subprocess,
      drive an initialize → tools/list → tools/call → shutdown
      lifecycle, verify JSON-RPC framing and response shapes
- [x] 10.2 E2E test for cold-repo case (`--repo` pointing at a
      fresh git repo with no `.git-paw/`): every category returns
      well-formed empty/null responses
- [x] 10.3 E2E test for active-session case (build a fixture
      session with a broker, intents, learnings, governance docs;
      verify each tool returns populated data)
- [x] 10.4 E2E test for non-git `--repo` value: server exits with
      non-zero status and clear stderr message
- [x] 10.5 Audit test that asserts no agent CLI binary
      (`claude`, `gemini`, `codex`, `aider`, `opencode`, `vibe`,
      `amp`, `qwen`) appears in the server's child-process tree
      across the full tool surface

## 11. Spec audit + quality gates

- [x] 11.1 For every requirement in
      `openspec/changes/mcp-server/specs/mcp-server/spec.md` and
      `mcp-read-tools/spec.md`, confirm at least one test asserts
      the corresponding behaviour
- [x] 11.2 Run `just check` (fmt + clippy + tests) — must pass
- [x] 11.3 Run `just deny` (license + advisory) — must pass
- [x] 11.4 Run `cargo audit` — must pass
- [x] 11.5 Verify coverage ≥ 80% on `src/mcp/`
- [ ] 11.6 Manual dogfood pass: configure Claude Desktop against a
      real repo, exercise tools from each of the five categories,
      record findings in MILESTONE drift list if anything surfaces
