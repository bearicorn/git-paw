## Why

Nothing in git-paw records a worker CLI's conversation/session identity. When a worker pane dies (a
crash, an accidental `tmux kill-pane`, a machine sleep that tears the server down) or the operator
deliberately relaunches it, recovery spawns a *fresh* CLI process that has lost every token of context
the worker had built; the worktree survives on disk, but the conversation is gone. Every supported CLI
that offers a `--resume <id>` (or equivalent) could restore that context — if git-paw knew the id.

This is the v0.17.0 cycle's key feature, pulled forward from the (now v0.18.0) relocation backlog. It is small, per-CLI, and pays off under every terminal
backend, including staying on tmux forever.

## What Changes

- **Assign, don't scrape.** At launch git-paw generates a session identifier and, for any CLI that
  accepts one at start-up, passes it on the launch command (built-in row: Claude Code's
  `--session-id <uuid>`; any other CLI via a `[clis.<name>]` template). The id is therefore known
  deterministically before the CLI starts — no pane scraping, no per-CLI log-directory parsing.
- **Persist it.** `WorktreeEntry` gains `cli_session_id: Option<String>` (additive, `#[serde(default)]`,
  omitted when `None`) so older session files load unchanged and sessions without an assignable CLI
  serialize byte-identically to today.
- **Use it on recovery.** When `git paw start` recovers a stopped session (or relaunches a dead worker
  pane), a worktree whose entry carries a session id and whose CLI has a resume template is relaunched
  with the resume arguments appended (built-in row: Claude Code's `--resume <uuid>`), restoring the
  worker's context. Entries without an id, or CLIs without a resume template, launch exactly as today.
- **Expose it.** `git paw status --json` includes each agent's `cli_session_id` so an operator (or the
  external orchestrator) can resume a worker by hand.
- Per-CLI knowledge lives in the existing `[clis.<name>]` table as two new optional templates,
  `session_id_args` and `resume_args`, with a `{{SESSION_ID}}` placeholder — the same shape as
  `approval_args`. Built-in rows exist only where the CLI's documented flags are known; unknown CLIs
  get no assignment and no resume, and launch unchanged.
- No new dependency: the identifier is a version-4-formatted UUID built from OS randomness.

## Capabilities

### New Capabilities

<!-- None — the feature distributes into session persistence and CLI launch resolution. -->

### Modified Capabilities

- `session-state`: `WorktreeEntry` records the worker's `cli_session_id`; recovery relaunches a worker
  with its recorded id when the CLI can resume; `status --json` exposes it.
- `cli-resolution`: per-CLI session-id assignment and resume argument templates
  (`[clis.<name>].session_id_args` / `resume_args`, built-in Claude rows) and their application on the
  launch command.

## Impact

- **Code:** `src/session.rs` (`WorktreeEntry.cli_session_id`), `src/config/cli.rs` (`CustomCli`
  templates), `src/config/supervisor.rs` neighbourhood (a `resolve_session_id_args` /
  `resolve_resume_args` pair beside `resolve_approval_flags`), `src/main.rs::attach_agent` (assign at
  launch), `src/commands/recover.rs` (append resume args), `src/commands/status.rs` (`--json` field), a
  small OS-randomness UUID helper (also reusable by the later broker-verdict-authority change).
- **Persisted state:** additive optional field only; pre-existing session files load unchanged; a
  session whose CLIs have no assignment template serializes byte-identically to today.
- **Config:** two optional `[clis.<name>]` keys; `git paw init`'s generated config gains commented
  examples only.
- **Docs:** configuration reference (`[clis.<name>]` table), `docs/src/user-guide/session-lifecycle.md`
  (recovery resumes context), `supported-clis.md` (which CLIs resume), `--help` for `status --json`.
- **Enum-variant ripple:** none.
- **Security / safety:** the id is not a secret (it is visible on the CLI's own command line) and is
  passed through the existing `TmuxCommand` quoting; assigning a fresh UUID cannot collide with an
  existing CLI session. No new privilege.
- **Dependencies:** none (`uuid` is not in the approved set; the helper reads OS randomness and formats
  a v4 UUID itself).
