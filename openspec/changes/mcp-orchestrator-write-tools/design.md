## Context

The MCP server lives in the library, holds no state, resolves the repository and broker URL from the
session file, and exposes read tools plus four identity-derived worker publish tools
(`mcp-agent-publish`); `mcp-server` forbids any file / git / config write tool and
`mcp-agent-publish` excludes the verdict verbs. After `cmd-library-surface` the commands are callable
in-process with an output sink; after `broker-verdict-authority` the broker accepts verdicts only with
the root credential, which lives in the user-data `Session`. At the tip a root MCP session's publish
tools derive `agent_id` from the root's checked-out branch (`resolve_agent_id` →
`current_branch(root)`), which is meaningless for an orchestrator.

## Goals / Non-Goals

**Goals:**
- Root-only lifecycle and verdict tools, structurally gated; worker tools unchanged.
- Keep the `mcp-server` bound: no arbitrary file / git / config / command write.

**Non-Goals:**
- Spec-authoring tools: they ship in `mcp-spec-authoring-tools` (D5).
- `start_session` running the unattended drive loop (D3).
- A remote MCP transport; the stdio server is unchanged.

## Decisions

### D1 — Root detection: the shipped linked-worktree check

v0.17 already decides "main checkout or linked worktree" once, in
`cli_setup::shell::resolve_layout` + `RepoLayout::is_linked_worktree` (the checkout's git dir differs
from the common git dir — correct under `--separate-git-dir`, where a "`.git` is a directory" test is
not). `git paw __authority` (`broker-verdict-authority`) uses it; the MCP server uses it too, evaluated
once at startup, rather than a third variant. Root-only tools are registered only for the main working
tree; a by-name call from a worktree returns the existing "tool not found" protocol error with a clearer
message naming the worktree restriction.

`RepoContext::for_root` resolves the session with `find_session_for_repo_or_owner`
(`src/mcp/mod.rs:88`), so a worktree server also finds the owner's `Session` — which now carries the
credential. `RepoContext::authority` is therefore populated only when root detection says main working
tree; a worktree server never holds the credential in memory.

### D2 — Lifecycle tools call the library

Each tool builds a `CommandContext` from `RepoContext`, calls `git_paw::commands::<verb>` with a
capturing sink, and returns the outcome plus captured lines as JSON. No subprocess, no `PATH` lookup.

### D3 — `start_session` is detached-only

`--unattended` blocks in the drive loop for the life of the wave, and a detached unattended loop has a
known early-exit failure under no-TTY. A tool that blocks the server for hours is wrong, and one that
spawns a detached loop would ship a known bug. An orchestrator that wants unattended runs
`git paw start --unattended` itself; `start_session` covers the attach-later launch.

### D4 — Stop and purge follow the CLI as it is

`stop_session` has no parameters (`stop-without-confirmation`: stopping is recoverable, `--force` is
gone). `purge_session` keeps the CLI's confirmation as a required `confirm: true`, carries
`destructiveHint`, and — per `purge-unmerged-branch-safety` — keeps branches with unmerged commits,
returning them with their tips so the orchestrator can report them.

### D5 — Spec-authoring tools live in their own change

`create_spec` / `write_spec_artifact` / `validate_spec` are not part of this change. They ship in the
follow-on `mcp-spec-authoring-tools`, which gates every repo-writing MCP tool behind one consent key in
the repository's own config (`[mcp] allow_repo_writes`, off by default, root session only). This
change's lifecycle and verdict tools are governed by the root-session rule alone.

### D6 — Verdict tools mirror the helpers

`publish_verified({agent, message})` and `publish_feedback({agent, gate?, errors})` take the *target*
(as `sweep.sh verified <agent>` does), set `verified_by` / `from` to `supervisor`, and attach the
credential read through `RepoContext`. Wire-identical to the helpers. In a root session the four worker
tools publish as `supervisor` (the root identity) instead of the root branch slug.

## Risks / Trade-offs

- [`start_session` returns before agents finish booting] → it returns after the launch flow's readiness
  gates, exactly as the CLI returns to the shell; the response says so.
- [A root MCP session on a shared machine] → root-only tools need the server started at the main
  working tree, verdicts still need the credential, purge needs `confirm`.
- [Two orchestrators act at once (interactive + one-shot escalation)] → lifecycle calls take the session
  lock the CLI takes; approvals stay behind the per-pane claim.

## Migration Plan

1. `RepoContext::{is_main_worktree, authority}` (authority only for the main working tree); conditional
   router composition; root identity for the worker tools; `get_info` instructions;
   `ALL_ALLOWED_TOOL_NAMES`.
2. Lifecycle tools + schemas + tests.
3. Verdict tools + schemas + tests (root 202; worktree not advertised; by-name error, nothing published).
4. Docs.

Rollback: revert; no persisted state or wire changes.

## Open Questions

- Should `start_session` accept the full spec selection (`from_all_specs`, named specs) or only
  `branches`? Recommendation: the same selection inputs as the CLI (one options struct), with `dry_run`
  as the documented preview.
