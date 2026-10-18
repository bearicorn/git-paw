## Why

The roadmap's MCP write set includes `create_spec` / `write_spec_artifact` / `validate_spec`, so an
orchestrator driving git-paw over MCP (possibly remotely, possibly without its own file tools) can
author the specs it then launches. `mcp-orchestrator-write-tools` deferred them because they break the
MCP server's standing "no tool writes a file" rule. The user's decision (2026-10-19): ship them, but
only when the repository has consented, through one switch in its own config file — "may the MCP server
write to this repository or not".

## What Changes

- **One consent switch.** `[mcp] allow_repo_writes` (default `false`), read only from the repository's
  own `.git-paw/config.toml` — never from the user-level config, an environment variable or a tool
  argument. Off: every MCP tool that writes file content into the repository is not advertised, and a
  by-name call returns an error naming the key. On: those tools work for the root session. Read tools
  are unaffected either way.
- **Root session only.** Under the root-session rule `mcp-orchestrator-write-tools` introduced (the
  server runs in the main working tree, decided by the shipped `RepoLayout::is_linked_worktree` check it
  reuses), a worktree session never gets the write tools, even with the switch on.
- **Three tools, OpenSpec format:**
  - `create_spec({name})` — creates `<[specs] dir>/<name>/` with the same `.openspec.yaml` metadata
    `openspec new change` writes; refuses an existing change or a non-kebab-case name. *(write)*
  - `write_spec_artifact({change, path, content})` — creates or replaces one file inside an existing
    change directory. *(write)*
  - `validate_spec({change})` — an in-process structural check (proposal present, delta headers,
    SHALL/MUST on each requirement's first line, at least one scenario). Read-only, so always available
    in every session.
  Under any other configured spec format the three tools return "not supported for format <type>".
- **Confinement.** Every write resolves inside the change directory, which resolves inside the
  repository working tree: `..`, absolute paths, a `.git` component and symlinks leading out are
  refused. Tools never stage or commit; each write is logged on the MCP server log, and `git status` /
  `git diff` show exactly what was authored.
- The lifecycle and verdict tools from `mcp-orchestrator-write-tools` stay governed by the root-session
  rule, not by this switch (design D3).

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `mcp-server`: the bounded write surface gains the consented, root-only spec-authoring write tools;
  the repository-write consent gate; write confinement; the read-only `validate_spec` tool.
- `core-configuration`: `[mcp]` gains `allow_repo_writes`, honoured from the repository config only.

## Impact

- **Order:** archive position 19 of 20 for v0.18, after `mcp-orchestrator-write-tools` (17:
  root-session detection, and it MODIFIES *The server exposes a bounded write surface alongside
  read-only tools* first — this change modifies that requirement again, starting from its text) and
  `orchestrator-skill-export` (18), whose `quick-start-orchestrator.md` the workflow docs extend.
  Supersedes `mcp-orchestrator-write-tools`' D5 (spec authoring left to its own change) and extends the
  pointer to these tools that its `mcp.md` task adds.
- **Other dependencies:** `broker-verdict-authority` (same root/worktree identity rule). v0.17
  `mcp-superpowers-support` modifies other `mcp-server` requirements and the valid-type set this change
  reads — no shared header. v0.19 `spec-backend-strategy` should absorb the OpenSpec scaffold and check
  into the `SpecBackend` Strategy (other formats gain authoring there) and drop its "spec-authoring
  deferred" non-goal.
- **Code:** `src/config/mod.rs` (`McpConfig::allow_repo_writes`, `merged_with` repo-only),
  `src/mcp/mod.rs` (`RepoContext` carries the consent), new `src/mcp/repo_write.rs` (consent + root
  decision, path confinement), new `src/mcp/tools/spec_authoring.rs`, `src/mcp/server.rs` (router,
  list filter, instructions), `src/specs/openspec.rs` (scaffold + structural check, pure), init template
  comment.
- **Tests:** `src/config/tests.rs`, `src/mcp/server.rs` registry tests, new `tests/mcp_spec_authoring.rs`.
- **Docs:** `docs/src/user-guide/mcp.md`, `docs/src/configuration/README.md`,
  `docs/src/quick-start-orchestrator.md`, `docs/src/user-guide/security-posture.md`.
- **Enum-variant ripple:** none. The format check compares the resolved `[specs].type` string to
  `openspec` (as the opsx role guard does) — a non-compile-forcing site for the §7 list.
- **Security / safety:** new file-write surface reachable from untrusted MCP input; reviewed against
  `security-and-safety-review` (design "Threats"). Off by default; no command execution.
- **Dependencies:** none.

## Rebase note (2026-10-25): audit against released v0.17.0

- Fixed a cross-reference: the D5 deferral is `mcp-orchestrator-write-tools`' D5 ("Spec-authoring tools
  live in their own change"), and its `mcp.md` task adds a pointer to these tools; `orchestrator-skill-export`
  has no such deferral (its D5 is "Guards in the same change") and no "spec-authoring tools not
  provided" doc line. Task 5.1 now extends that pointer.
- Root detection reuses the same shipped check as `mcp-orchestrator-write-tools` and `__authority`
  (`cli_setup::shell::RepoLayout::is_linked_worktree`), not "`.git` is a directory".
- Ordering prose follows the v0.18 archive order (position 19).
