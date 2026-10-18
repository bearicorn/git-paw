## Context

At the tip the MCP server (`src/mcp/`) writes no files: `mcp-server` *The server exposes a bounded write
surface…* forbids it, and the closed tool allowlist in `src/mcp/server.rs`
(`ALL_ALLOWED_TOOL_NAMES`) guards it. `mcp-orchestrator-write-tools` adds root-session detection
(`RepoContext::is_main_worktree`, through the shipped `cli_setup::shell::RepoLayout::is_linked_worktree`
check), the root-only lifecycle and verdict tools,
and deferred spec authoring (its D5). `[mcp]` today holds only `name` (`McpConfig`,
`src/config/mod.rs`); `PawConfig::merged_with` lets the repository config override the user-level
one field by field, and `RepoContext::for_root` resolves config once at startup. Spec discovery
resolves the spec directory from `[specs]` (`src/specs/mod.rs::resolve_specs_config`, default
`specs/` when `dir` is unset); the OpenSpec backend (`src/specs/openspec.rs`) scans `<dir>/<change>/`.
Read tools already confine paths by canonicalizing against a root (`src/mcp/query/source.rs`,
`docs.rs`).

Observed with `openspec` 1.1.1 in a throwaway repository: `openspec new change <name>` creates
`openspec/changes/<name>/.openspec.yaml` containing `schema: spec-driven` and `created: <YYYY-MM-DD>`
(a `README.md` only with `--description`), and rejects names that are not lowercase letters, digits
and hyphens. `openspec validate <name> --strict` treats a change without `proposal.md` as unknown,
requires at least one delta file with an `## ADDED|MODIFIED|REMOVED|RENAMED Requirements` header, and
rejects a requirement whose SHALL/MUST is not on its first body line or that has no
`#### Scenario:`.

## Goals / Non-Goals

**Goals:**
- One consent answer per repository — may the MCP server write here — off by default.
- The smallest tool set that lets an orchestrator author and check an OpenSpec change.
- Writes that cannot leave the change directory, even under prompt-injected input.

**Non-Goals:**
- Spec Kit, Markdown and Superpowers authoring (v0.19 `spec-backend-strategy`, D5).
- Running the project's own validator or any process from an MCP call (D4).
- Committing, staging or publishing authored files; broker events for writes (D7).
- Orchestrator skill changes — `tools/list` is the discovery surface.

## Decisions

### D1 — One switch, `[mcp] allow_repo_writes`, repository config only, read at startup

`McpConfig::allow_repo_writes: bool` (`serde(default)`, skipped when `false`). `merged_with` takes the
repository (overlay) value only, so a user-level value never applies; `RepoContext::for_root` resolves
it once, like `server_name`. Consent is per repository because the user-level file applies to every
repository; an environment variable or tool argument is something an agent can set. Reading at startup
keeps `tools/list` and the call path in agreement, and means a mid-session edit to the config (by a
worker, say) has no effect until the server restarts. *Rejected:* a spec-specific key
(`spec_authoring`) or per-tool keys — the user wants one repository-level answer; an env var.

### D2 — One gate for every repository-writing tool

A pure decision `repo_write_access(is_main_worktree, allow_repo_writes) -> Result<(), RepoWriteRefusal>`
(`src/mcp/repo_write.rs`) is applied both to the tool list and as the first step of each write tool, so
listing and calling cannot disagree. A refused tool is absent from `tools/list`; a by-name call gets the
refusal's message (the switch and file, or the worktree restriction), using the same mechanism
`mcp-orchestrator-write-tools` lands for root-only tools. Any future MCP tool that writes file content
into the repository goes through this gate; the closed-allowlist test lists which tools are gated.

### D3 — Lifecycle and verdict tools stay under the root-session rule, not this switch

They do not write caller-supplied content to a caller-chosen path: the lifecycle tools perform what the
CLI performs on git-paw-managed state (worktrees, session files, branches), and the verdict tools write
nothing to disk. They are the orchestrator's core loop, so gating them would force the switch on in
every repository and empty the consent of meaning, and `purge_session` already carries its own guard
(`confirm: true`, unmerged branches kept). If a lifecycle tool ever accepts caller content to write, it
joins this gate.

### D4 — `validate_spec` is a read tool, in-process, always available

It reads the change and returns findings; no process, no write — the same class as `get_spec`, so no
consent and every session (a worker amending a spec can check it). The rules are the subset observed in
the Context; the project's own validator stays authoritative and the orchestrator runs it as the
five-gate procedure says. *Rejected:* shelling out to `openspec` when on `PATH` (a `PATH` lookup and
subprocess driven by untrusted input, contrary to the server's deterministic-sources rule, and it would
bake git-paw's own `--strict` convention into an export); running `[supervisor] spec_validate_command`
(executes a configured command from an MCP call).

### D5 — OpenSpec only; git-paw writes the scaffold itself

`create_spec` writes `.openspec.yaml` exactly as the CLI does, so every write passes the confinement
check and the consumer needs no CLI installed. The scaffold and the structural check are pure
functions in `src/specs/openspec.rs`; the MCP tools are thin adapters. The other formats get one
"not supported for format <type>" error: their layouts differ (numbered feature directories, single
files), and v0.19 `spec-backend-strategy` is where per-format behaviour moves into the `SpecBackend`
Strategy — it should absorb these functions and add the other formats there. The format test compares
the resolved `[specs].type` string with `openspec`, as the opsx role guard does.

### D6 — Confinement

Lexical checks first: `change` is one segment (`create_spec`'s `name` is kebab-case, as OpenSpec
requires); `path` is relative with no `..` or `.git` component. Then: the canonical spec directory
must lie inside the canonical repository root and outside `<root>/.git`; each existing component from
the change directory to the target is checked with `symlink_metadata`, and one that resolves outside
the change directory is refused; missing directories are created one level at a time; a symlinked
target is refused; the content goes to a temporary file in the target directory and is renamed into
place (atomic, and a rename replaces the directory entry instead of writing through it).
`write_spec_artifact` creates or replaces — no overwrite flag (user direction); the response says
which.

### D7 — Visibility through git and the server log

Tools never stage or commit, so `git status` / `git diff` are the record of what was authored, and the
server log gets one line per write (path, bytes, replaced). No broker event: authoring usually happens
before a session exists, and every existing variant carries the wrong meaning (`agent.status` from
`supervisor` can replace the verdict status the drive loop reads for completion; `agent.artifact`
broadcasts to every worker; `agent.learning` lands in session learnings); a new variant is a
frozen-wire change.

### Threats (security-and-safety-review)

MCP input is untrusted: the orchestrator may be prompt-injected by content it read.

| Threat | Mitigation |
|---|---|
| Write outside the change directory — traversal, absolute path, symlink, `.git/hooks` or CI files for persistence | D6: lexical + canonical confinement to the change directory; `.git` refused |
| An agent grants consent — tool argument, env var, user-level config | D1: repository file only, read at startup, no parameter |
| A worker authors the root's specs | root-session gate, whatever the worktree's config says |
| A worker flips the root's switch (unsandboxed write, or a merged branch carrying the key) | the change shows in the diff a human reviews; the startup log line states writes are enabled; the sandbox deny tier. Not a defence against an unsandboxed rogue worker (as `broker-verdict-authority` D7) |
| Authored work overwritten | HEAD and index untouched; `replaced` + log line. Replacing an untracked file loses its old content — accepted |
| Command execution | none: no tool spawns a process |

### Export agnosticism

The surfaces are the three tool descriptions (agent-facing text) — no stack, commit format or
git-paw convention, covered by a guard test — the commented `# allow_repo_writes = false` line in the
init template's `[mcp]` stanza, and `.openspec.yaml`, written only when the consumer chose OpenSpec and
identical to what their own CLI writes.

## Risks / Trade-offs

- [`openspec` changes its scaffold or validation rules] → the scaffold and rules are small, pinned by
  tests, and named as a subset; the project validator stays authoritative.
- [A local process swaps a directory for a symlink between the check and the write] → accepted: that
  process can already write the tree; the window is one syscall.
- [Revoking consent needs a server restart] → documented beside the switch.

## Migration Plan

1. Config field + repository-only merge + init template comment.
2. `repo_write_access` + `RepoContext` consent; list filter and refusals.
3. Pure scaffold and structural check in `src/specs/openspec.rs`; confinement in `repo_write.rs`.
4. The three tools; closed allowlist; server instructions text.
5. Docs.

Rollback: revert. The key is additive and the config structs do not deny unknown fields, so older
binaries ignore it.

## Open Questions

- Does the `openspec` CLI take a project-default schema from the project's own configuration? To
  verify; if it does, `create_spec` should write that schema instead of `spec-driven`.
