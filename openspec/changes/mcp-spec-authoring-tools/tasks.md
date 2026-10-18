## 1. Consent switch

- [ ] 1.1 `McpConfig::allow_repo_writes: bool` (`serde(default)`, skipped when `false`, doc comment); `PawConfig::merged_with` takes the repository (overlay) value only; commented `# allow_repo_writes = false` line with a one-line explanation in the default config template's `[mcp]` stanza (`src/config/mod.rs`)
- [ ] 1.2 Tests in `src/config/tests.rs` for the `core-configuration` *MCP configuration section* scenarios: field parses; absent → `false`; user-level `true` + repository unset → merged `false`; round-trip with `true`; `false` not serialized; existing `[mcp] name` tests unchanged

## 2. Gate

- [ ] 2.1 `src/mcp/repo_write.rs`: pure `repo_write_access(is_main_worktree, allow_repo_writes) -> Result<(), RepoWriteRefusal>` with messages naming `allow_repo_writes` + `.git-paw/config.toml` (and the restart), or the worktree restriction; `RepoContext` carries the startup-resolved switch; server logs at startup whether repository writes are enabled
- [ ] 2.2 Apply the gate to the tool list and as each write tool's first step, using the root-only mechanism from `mcp-orchestrator-write-tools` (`RepoContext::is_main_worktree`, the shipped `RepoLayout::is_linked_worktree` check — no new root test); unit table over the four (root × switch) cases

## 3. OpenSpec scaffold, check and confinement

- [ ] 3.1 Pure functions in `src/specs/openspec.rs`: change-name validation (lowercase letters, digits, hyphens), `.openspec.yaml` content (`schema: spec-driven`, `created: YYYY-MM-DD`), and the structural check over a change's files returning findings (missing `proposal.md`; no delta header; SHALL/MUST not on a requirement's first body line; no `#### Scenario:`); table-driven unit tests per rule
- [ ] 3.2 Confinement in `src/mcp/repo_write.rs`: lexical checks (single-segment `change`; relative `path` without `..` / `.git`), canonical spec dir inside the root and outside `.git`, per-component `symlink_metadata` check against the canonical change dir, one-level directory creation, symlinked target refused, temp file + rename; reuse the canonicalize idiom of `src/mcp/query/source.rs`

## 4. Tools

- [ ] 4.1 `src/mcp/tools/spec_authoring.rs`: `create_spec {name}`, `write_spec_artifact {change, path, content}` (returns written paths + `replaced`; one server-log line per write; never stages or commits), `validate_spec {change}` (read-only, every session); "not supported for format <type>" and "no spec system configured" errors; JSON Schemas; descriptions free of stack tokens and commit-format prose
- [ ] 4.2 `src/mcp/server.rs`: compose the router; add the three names to `ALL_ALLOWED_TOOL_NAMES` and mark the two write tools as gated; update the `get_info` instructions text (repository writes need `[mcp] allow_repo_writes` and a root session)
- [ ] 4.3 `tests/mcp_spec_authoring.rs` (real repo in `tempfile`, MCP over stdio) for the `mcp-server` scenarios: off by default (write tools absent, `validate_spec` present); by-name call while off names the switch and creates nothing; consent lists the tools; user-level config cannot consent; worktree session neither lists nor executes them; no schema has a consent parameter; `create_spec` scaffold / existing change / invalid names; `write_spec_artifact` delta write, replace, missing change; log file contains the path and HEAD/index unchanged; markdown format refused
- [ ] 4.4 Confinement tests: traversal, absolute path, `.git/hooks/pre-commit`, symlinked `specs/` dir leading out, symlinked target file, spec dir outside the repository — each refused with nothing written
- [ ] 4.5 `validate_spec` tests: well-formed change valid; missing scenario and second-line SHALL reported on the delta file; scaffold-only change reports missing proposal and delta; available from a worktree session with the switch off and modifies nothing
- [ ] 4.6 Registry tests: closed allowlist; *bounded write surface* scenarios (no tool writes outside a spec change directory, runs a caller-supplied command or writes config, in any session/switch combination; read-only categories plus `validate_spec` stay read-only); agnosticism guard on the three tool descriptions using the forbidden-token list of `tests/lang_agnostic_skill_audit.rs`

## 5. Docs

- [ ] 5.1 `docs/src/user-guide/mcp.md`: repository-write consent (key, repository config only, restart to change, root session only), the three tools with examples, confinement rules, OpenSpec-only note; extend the pointer to the spec-authoring tools that `mcp-orchestrator-write-tools` adds to this chapter
- [ ] 5.2 `docs/src/configuration/README.md`: `[mcp] allow_repo_writes` (default, repository-only, what it governs and what it does not — lifecycle/verdict tools)
- [ ] 5.3 `docs/src/quick-start-orchestrator.md` (from `orchestrator-skill-export`): authoring a change from your own agent — enable the switch, `create_spec` → `write_spec_artifact` → `validate_spec` → run the project's validator → review `git diff` → commit yourself
- [ ] 5.4 `docs/src/user-guide/security-posture.md`: the consent switch and the confinement guarantees, and their limit (an unsandboxed worker that can edit the root's config)
- [ ] 5.5 `mdbook build docs/` succeeds

## 6. Gates

- [ ] 6.1 Every scenario maps to a named test; `just check` and `just deny` green; full suite green against the merge-base
- [ ] 6.2 Gate 5 note: off by default; consent only from the repository file; root-only; lexical + canonical confinement; no process spawned; no stage/commit
