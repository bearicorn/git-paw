## Context

The resume decision is the same block in three places (`commands/start.rs::cmd_start`,
`cmd_start_with_specs`, `commands/supervisor.rs::cmd_supervisor`): `find_session_for_repo` →
`invalidate_if_stale` (has-session only) → `effective_status` → Paused (`restart_from_pause`) / Active
(`reattach_active_session_or_refuse`) / Stopped (`recover_session`). Recovery decides resume-vs-fresh
per worktree in `recover.rs::cli_command_for_recovery` but never says so. Existing git helpers:
`uncommitted_files` (porcelain -z), `unmerged_commit_count` (`rev-list --count default..branch`),
`default_branch`; no `--shortstat` helper exists.

## Goals / Non-Goals

**Goals:** a compact, truthful per-agent summary on every resume path; structured so non-CLI callers
get it too.

**Non-Goals:** changing recovery behaviour (what is relaunched, what is purged); the stale-receipt path
(`invalidate_if_stale`, owned by session-state reconciliation); interactive prompts; colour.

## Decisions

- **D1 — Facts, then a pure renderer.** `ResumeFacts { branch, cli, resume: Resume { Session(id) |
  Fresh | Running }, worktree_present, uncommitted, ahead, shortstat }` per agent; `render_resume_summary`
  is pure and unit-tested with fixtures. The resume mode reuses `cli_command_for_recovery`'s decision
  (extracted into a pure `resume_mode(entry, cli_profile)`), so the summary can never disagree with what
  is actually launched.
- **D2 — Diff summary against the merge base.** `git diff --shortstat $(merge-base default branch)..branch`
  shows the branch's own work, not upstream drift. A failing git probe renders `?` for that field and
  never fails the start.
- **D3 — Printed before attach, to stdout.** On a TTY the user sees it after detaching from tmux too
  (it stays in the scrollback); in non-TTY mode it precedes the "started in detached mode" hint.
- **D4 — Missing worktree is reported, not repaired.** The line reads `worktree missing — run
  git paw doctor`; what recovery does about it belongs to reconciliation.

## Risks / Trade-offs

- [Extra git calls per agent on start (3–4 each)] → bounded by agent count (≤ the layout cap); all
  read-only; measured in the e2e suite's timing budget.
- [Output noise for users who resume often] → one line per agent plus a header; no extra prompts.
