## 1. Root-session detection

- [ ] 1.1 `RepoContext::is_main_worktree` via the shipped `cli_setup::shell::resolve_layout` + `RepoLayout::is_linked_worktree` (`src/cli_setup/shell.rs:41`) and `RepoContext::authority` (from the user-data `Session`, loaded only when the server resolved the main working tree — `for_root` finds the owner's `Session` from a worktree too, `src/mcp/mod.rs:88` — never logged); conditional router composition so root-only tools are registered only for the main working tree; by-name call from a worktree → MCP error naming the restriction; worker tools publish as `supervisor` in a root session
- [ ] 1.2 Tests: root lists root-only tools; worktree does not; by-name call errors and the server keeps running; a worktree server's `RepoContext` holds no credential although it resolves the owner's `Session`; a `--separate-git-dir` main checkout is a root session (the "Root-session detection" scenarios)
- [ ] 1.3 `src/mcp/server.rs`: update the `get_info` instructions (lines 64-73: "Supervisor authority verbs … are not exposed") to say the lifecycle and verdict tools exist for a root session only; add the six root-only names to `ALL_ALLOWED_TOOL_NAMES`; `registry_is_exactly_the_closed_allowlist` checks the root registry against the full list and a worktree registry against it minus the root-only names; `no_generic_publish_or_authority_verb_tool_is_advertised` becomes worktree-scoped

## 2. Lifecycle tools

- [ ] 2.1 `start_session` (cli, branches, specs selection, supervisor, dry_run; detached; no `unattended` param; returns session name, attach command, orchestrator-context paths), `pause_session`, `stop_session` (no parameters), `purge_session` (`confirm` required, `destructiveHint`, deleted/retained branches with tips) — in-process calls into `git_paw::commands` with a capturing sink; `no_session: true` when none; JSON Schemas
- [ ] 2.2 `tests/mcp_lifecycle_e2e.rs`: start → panes exist and server responsive; schema has no `unattended`; pause → Paused + port free; stop → Stopped; `stop_session` schema has no `confirm`/`force`; purge without `confirm` → error and nothing changed; purge with confirm keeps an unmerged branch and lists it; `purge_session` is annotated destructive; no-session → graceful

## 3. Verdict publish tools

- [ ] 3.1 `publish_verified {agent, message}` / `publish_feedback {agent, gate?, errors}`: publisher `supervisor`, target from params, `X-Paw-Authority` from `RepoContext::authority`; wire-identical to `sweep.sh verified` / `feedback-gate`; descriptions satisfy the v0.17 "do not invite peer cherry-picking" rule; schema test (no credential / publisher params)
- [ ] 3.2 E2E: root `publish_verified` → 202 + broadcast; worktree session cannot self-verify (not advertised; by-name error; nothing published); the `mcp-agent-publish` scenarios

## 4. Docs + gates

- [ ] 4.1 `docs/src/user-guide/mcp.md`: lifecycle + verdict categories, root-only note, `start_session` detached-only, purge semantics, a pointer to the spec-authoring tools documented by `mcp-spec-authoring-tools`; `architecture.md` MCP section; `mdbook build docs/` succeeds
- [ ] 4.2 Every scenario maps to a named test; `just check` + `just deny` green; full suite green vs merge-base; Gate-5 note: structural root-only gating, credential, `confirm` + `destructiveHint`, no arbitrary path or command parameter
