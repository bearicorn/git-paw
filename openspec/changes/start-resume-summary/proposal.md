## Why

Resuming is `git paw start` (the phantom `resume` verb was struck in v0.16.0), and since v0.17.0 a
recovered worker relaunches with its recorded CLI session id. But what `start` *says* when it resumes is
one line — `Restarting paused session '…' (broker + reattach)...`, `Recovering session '…'...` or
`Reattaching to session '…'...` — and then the operator is dropped into tmux. Nothing says which agents
come back with their conversation and which start cold, whether each worktree still exists, or what work
is sitting in each one (uncommitted files, commits not yet integrated). After a crash or a long pause
that is exactly what the operator needs before deciding what to do next.

## What Changes

- When `git paw start` resumes an existing session — restarting a paused session, recovering a stopped
  one, or reattaching to an active one — it prints a **resume summary** before attaching (or before the
  detached-mode hint): one line per agent with its branch, CLI, how it resumes (`resumes session <id>`
  vs `starts fresh`, or `running` on reattach), and a worktree diff summary — uncommitted file count,
  commits ahead of the default branch, and files changed / insertions / deletions against the merge
  base. A missing worktree directory is flagged on its line.
- The summary is a structured part of the start outcome, so the library surface (v0.18) and its MCP
  `start_session` tool can return it without parsing text.
- Recovery messaging names the next step when something is off (missing worktree → `git paw doctor`).

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `session-state`: ADDED — `start` prints a per-agent resume summary when it resumes a session.

## Impact

- **Code:** pure `ResumeSummary` builder + renderer (new, in the session/commands area) over facts
  gathered by a small probe: `git::uncommitted_files`, `git::unmerged_commit_count`, a new
  `git::diff_shortstat_with` (`git diff --shortstat <merge-base>..<branch>`), `WorktreeEntry::cli_session_id`
  plus the CLI's resume-template resolution (`cli-resolution`, already used by
  `recover.rs::cli_command_for_recovery`); callers in `commands/start.rs` (the paused / stopped / active
  branches of `cmd_start` and `cmd_start_with_specs`) and `commands/supervisor.rs` (same block), printed
  before `attach_or_print_hint`.
- **Enum-variant ripple:** none.
- **Docs:** `docs/src/user-guide/session-lifecycle.md` (resuming), `docs/src/cli-reference.md` (`start`
  output on resume).
- **Cross-version dependencies:** v0.17 `worker-session-id-capture` (already archived into
  `session-state` / `cli-resolution`) supplies the session id and resume templates — no overlap, this
  only reports them. v0.18 `cmd-library-surface` — the summary becomes a field of the start outcome.
  This release's **session-state reconciliation** (other half) owns *what* recovery does with a
  missing worktree, a dead broker or a stale receipt (including today's stale-receipt auto-purge in
  `invalidate_if_stale`); this change only *reports*, and must render whatever reconciliation decides
  (rebase over it if it lands first). The stale-receipt path is deliberately not touched here.
