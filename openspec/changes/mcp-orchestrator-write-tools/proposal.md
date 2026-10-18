## Why

The orchestrator now drives the session from outside it — an agent CLI with git-paw's MCP server
attached, possibly remote-controlled from a phone. It can read everything through MCP today, but every
action (start, pause, stop, purge, publish a verdict) still needs a shell. The roadmap's MCP write tools
close that gap; they were blocked on two things this cycle delivers first: the commands as a library
(`cmd-library-surface`) and verdict authority enforced by the broker (`broker-verdict-authority`),
which is what makes a verdict-publishing tool safe to ship. The current `mcp-agent-publish` capability
deliberately excludes the verdict verbs from MCP entirely; with the root/worktree rule in the broker,
they can be offered to the root session alone.

Archive position 17 of 20 for v0.18 (after `cmd-library-surface` and `broker-verdict-authority`).

## What Changes

- **Root-session detection.** `RepoContext` learns whether the server runs in the repository's main
  working tree or in a linked worktree, with the check v0.17 already ships
  (`cli_setup::shell::RepoLayout::is_linked_worktree`: git dir vs common git dir, which also handles
  `--separate-git-dir`; `git paw __authority` uses the same one). Root-only tools are advertised only for
  the main working tree and a by-name call from a worktree returns an MCP-level error. Because
  `RepoContext::for_root` resolves the owner's `Session` even from a worktree
  (`find_session_for_repo_or_owner`, `src/mcp/mod.rs:88`), the authority credential is loaded into
  `RepoContext` only when the server resolved the main working tree.
- **Session lifecycle tools (root only):** `start_session` (detached; never runs the drive loop),
  `pause_session`, `stop_session` (no confirmation or force parameter — stop is recoverable and the CLI
  no longer has `--force`), `purge_session` (requires `confirm: true`, annotated `destructiveHint`,
  keeps branches with unmerged commits and reports them), each an in-process call into
  `git_paw::commands` returning structured JSON.
- **Verdict publish tools (root only):** `publish_verified` and `publish_feedback`, publishing as
  `supervisor` with the session credential read from the user-data `Session` — identity-derived and
  parameter-free like the worker tools; no credential or publisher parameter.
- **The server's self-description follows.** The `get_info` instructions text ("Supervisor authority
  verbs … are not exposed", `src/mcp/server.rs:64-73`) and the closed `ALL_ALLOWED_TOOL_NAMES` registry
  list are updated for the root-only categories.
- **Not in this change: the spec-authoring tools** (`create_spec`, `write_spec_artifact`,
  `validate_spec`). They ship in the follow-on `mcp-spec-authoring-tools`, behind one consent key in
  the repository's config (`[mcp] allow_repo_writes`, off by default).

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `mcp-server`: the bounded write surface gains the root-session lifecycle category and the root-only
  verdict tools; root-session detection; still no arbitrary file / git / config / command write.
- `mcp-agent-publish`: the fixed tool set is four worker tools plus two root-only verdict tools;
  identity derivation adds the root case (`supervisor`); the exclusion of verdict verbs applies to
  worktree sessions.

## Impact

- **Predecessors (archive order):** position 17, after `broker-host-separation` (13, a broker that
  outlives any view), `cmd-library-surface` (15, the in-process command surface) and
  `broker-verdict-authority` (16, credential + broker rule). `orchestrator-read-surface` (3) adds
  `wait_for_events` to `ALL_ALLOWED_TOOL_NAMES` first. `agent-id-slug-safety` (5) MODIFIES
  *Publish tools act as the calling agent only* first, so the worker publish tools in the main
  working tree refuse instead of deriving an id from its branch (`main` → `agent-main`); this
  change's delta is written on that text and replaces the refusal with the root identity `supervisor`. v0.17: `mcp-superpowers-support` MODIFIES *Hard errors only for malformed
  configuration* and *Project knowledge tools* in `mcp-server` — different requirements, no overlap;
  `shipped-text-corrections` ADDS *Publish tool descriptions do not invite peer cherry-picking* to
  `mcp-agent-publish` — the two new tools' descriptions must satisfy it; `bundled-helper-discovery`
  resolves a worktree's session for MCP (the worktree side of root detection);
  `stop-without-confirmation` and `purge-unmerged-branch-safety` define the stop/purge semantics the
  tools expose.
- **Code:** `src/mcp/mod.rs` (`RepoContext::{is_main_worktree, authority}`; root detection through
  `cli_setup::shell::{resolve_layout, RepoLayout::is_linked_worktree}`, not a third copy),
  `src/mcp/tools/{session,publish}.rs` (new tools, conditional registration), `src/mcp/server.rs`
  (router composition, `get_info` instructions, `ALL_ALLOWED_TOOL_NAMES` and the registry tests),
  `src/mcp/query/session.rs`.
- **Tests:** `tests/mcp_e2e.rs` + new `tests/mcp_lifecycle_e2e.rs` (start/pause/stop/purge from MCP;
  worktree session sees no root tools; `publish_verified` from root → 202; worktree by-name call →
  error and nothing published); schema tests (no credential / publisher / force parameters).
- **Docs:** `docs/src/user-guide/mcp.md` (tool reference: lifecycle + verdict categories, root-only,
  `start_session` detached-only, purge semantics), `architecture.md` (MCP section).
- **Enum-variant ripple:** none.
- **Security / safety:** every new write tool is root-only by structure (advertised only when the server
  resolved the main working tree) and, for verdicts, by the broker's credential check; `purge_session`
  needs `confirm` and keeps unmerged work; `start_session` cannot start an unattended loop; no tool takes
  a path outside the repository or an arbitrary command. Reviewed against the
  `security-and-safety-review` skill (process spawning, git operations, untrusted input).
- **Dependencies:** none.

## Rebase note (2026-10-25): audit against released v0.17.0

- `RepoContext::for_root` resolves the owner `Session` even from a worktree
  (`find_session_for_repo_or_owner`, `src/mcp/mod.rs:88`), so the spec now says the authority
  credential is loaded only when the server resolved the main working tree.
- Root detection reuses the shipped `cli_setup::shell::RepoLayout::is_linked_worktree` (git dir vs
  common dir, handles `--separate-git-dir`) instead of "`.git` is a directory", so the check exists once
  (`setup-cli`, `__authority`, MCP).
- Added tasks for the `get_info` instructions text (`src/mcp/server.rs:64-73`, which says authority verbs
  are not exposed) and for `ALL_ALLOWED_TOOL_NAMES` plus the registry tests, which become root/worktree
  aware.
- Ordering prose follows the v0.18 archive order (position 17).
- Impact notes that `agent-id-slug-safety` (5) makes the worker publish tools refuse in the main
  working tree (no `agent-main` phantom) by modifying *Publish tools act as the calling agent only*
  first; this change's delta replaces that refusal with the root identity `supervisor`.
