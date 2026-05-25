## Context

The v0.5.0 dogfood session crashed mid-run when the tmux server died (cause undetermined — possibly an OS-level event, possibly the dashboard subprocess crashing in a way that took its parent down). All 11 agent worktrees retained valid uncommitted file edits from the live agents. The reasonable recovery action is to relaunch the tmux session pointing at those same worktrees, but `git paw start` fails on the first `git worktree add` because the worktree path already exists.

Repro is trivial:

```
$ git paw start --from-specs --supervisor   # launches a session with 11 worktrees
# … crash …
$ git paw start
error: Worktree error: git worktree add failed for branch 'feat/governance-config':
       fatal: '/Users/jieli/Development/personal/bearicorn/git-paw-feat-governance-config' already exists
```

The session state file at `~/Library/Application Support/git-paw/sessions/paw-<project>.json` still tracks the 11 worktrees with status `stopped` — git-paw "knows" what should be running; the code just doesn't connect the dots.

## Goals / Non-Goals

**Goals:**
- `git paw start` after a crash succeeds when the worktree directories from the prior session still exist on disk.
- The recovery path is automatic; no new flag or subcommand needed.
- Error messages for unrelated path collisions (a non-worktree directory at the expected path) stay informative.

**Non-Goals:**
- A new `git paw resume` subcommand.
- Detecting *stale* worktrees (e.g. branch has been force-pushed since the worktree was created; the branch HEAD doesn't match the worktree's working tree). Out of scope.
- Repairing corrupt session state files. The session JSON is read at start and rebuilt at end-of-launch; corrupted state is a separate failure mode.
- Cleaning up uncommitted work in surviving worktrees. The user owns commit/stash decisions; the recovery path is non-destructive by design.
- Per-CLI auto-relaunch of the inner CLI process inside each agent pane. The fresh tmux pane runs the configured CLI exactly the same way as on first launch; if the CLI keeps any session state of its own (Claude Code's `~/.claude/projects/...`), that's the CLI's concern.

## Decisions

### D1. Idempotency pre-check in `create_worktree`, not a new function

**Choice:** Add the idempotency check as a pre-flight inside `create_worktree`. Don't introduce a separate `resume_worktree` function or split the call sites.

**Why:**
- Every caller of `create_worktree` wants the same semantics: "make sure a worktree for this branch exists at the expected path". Whether it's a first-time creation or a resume should be invisible to the caller.
- The bug is in `create_worktree`; fixing it there reads naturally in code review.
- A separate `resume_worktree` would force every caller to decide which path to use — and they don't have enough information to make that decision (the resume vs. fresh-launch distinction lives in `cmd_start`'s session-state-file inspection, not at the worktree layer).

**Alternatives considered:**
- *New function `ensure_worktree`* — same semantics, different name. Rejected; the existing name already implies "create-if-needed". Renaming would churn every call site.
- *Two functions: `create_worktree` (errors on existing) + `ensure_worktree` (idempotent)* — gives callers explicit choice. Rejected; callers don't need the choice. The only legitimate use of the "error on existing" variant would be a defensive check, which the porcelain pre-check effectively gives them for free (existing-for-wrong-branch still errors).

### D2. Use `git worktree list --porcelain` to verify the existing worktree matches

**Choice:** When `worktree_path.exists()`, run `git worktree list --porcelain` and parse the output looking for a block whose `worktree` line matches the expected path AND whose `branch` line matches `refs/heads/<branch>`. Treat that combination as "successful idempotent creation".

**Why:**
- Authoritative source — `git worktree list` is the git-side registry. A file at the expected path that isn't a registered worktree (e.g. a leftover directory from `rm -rf`) needs different handling than a properly-registered worktree.
- Porcelain format is stable and easy to parse with `str::strip_prefix("worktree ")` and `str::strip_prefix("branch ")`. No JSON dependency, no regex.
- Cheap — one subprocess call per `create_worktree` invocation, and only when `worktree_path.exists()` (so first-time launches skip it).

**Alternatives considered:**
- *Just check `worktree_path.exists()` and return success* — Wrong; would mask real path collisions (e.g. user has an unrelated directory at the same name). Falling through to the `git worktree add` error is the right behavior for that case.
- *Parse `.git/worktrees/` directory entries* — gets the same answer via filesystem inspection. Rejected because porcelain is the documented API; `.git/worktrees/` is git internals.
- *Run `git -C <worktree_path> rev-parse --git-dir` and check if it returns a `.git/worktrees/<id>` path* — works but only confirms the directory is a worktree, not which branch it tracks. Need to combine with `git -C <path> branch --show-current`. Two subprocess calls vs. one for the porcelain approach.

### D3. Fall-through on mismatch preserves the v0.4 error contract

**Choice:** When `worktree_path.exists()` but the porcelain check doesn't find a matching `(path, branch)` block, fall through to the existing `git worktree add` call so the user sees the same `fatal: '<path>' already exists` they got in v0.4.

**Why:**
- Backward compatibility — users who hit a real path collision (a leftover non-worktree directory at the expected name) get the same actionable error as before. Existing scripts that grep for "already exists" still work.
- Conservative — the change should only short-circuit when we're confident the existing state matches what `create_worktree` would have produced. Anything else is "I'm not sure what's going on here, let git tell us".

**Alternatives considered:**
- *Custom error message ("path exists but isn't a worktree for this branch — investigate")* — would be more informative but invents a new error string. Rejected because v0.4's message is already actionable and consistent with `git`'s own phrasing.

## Risks / Trade-offs

- **[Porcelain output format changes in a future git release]** → Mitigation: the parsing is line-prefix-based and only looks for two specific prefixes (`worktree `, `branch `). Git's porcelain format guarantees stability for these. If a future git version changes the format, the test suite will catch it and the parse can be updated.

- **[The existing worktree is in a corrupt state (e.g. detached HEAD, locked, dirty index)]** → The pre-check only verifies path + branch identity. If the worktree is in an unusable state, `cmd_start` will proceed but the agent in that pane will run into trouble when trying to commit. Acceptable: the user gets a launched session and can investigate the bad worktree manually, which is strictly better than the current behavior (no launch at all).

- **[The expected branch doesn't exist in the local repo anymore]** → If the branch was deleted between the prior session crash and this resume, `git worktree list --porcelain` won't include it, and we'll fall through to `git worktree add` which will fail with `invalid reference`. The existing `-b` fallback path then creates a new branch from HEAD — which may NOT be what the user wants (the previous branch could have had unique commits). Documented as an open question below; not blocking for v0.5.0.

- **[Two `create_worktree` calls race on the same branch]** → Not a v0.5.0 concern. `cmd_start` is single-threaded and iterates worktrees sequentially.

## Migration Plan

This is a single-function code change. No data, no config, no schema.

1. **Code change** in `src/git.rs::create_worktree` — insert the existence + porcelain pre-check before the existing `git worktree add` call.
2. **Tests**:
   - Unit test asserting the resume case returns `Ok(WorktreeCreation { branch_created: false })` and doesn't run a second `git worktree add` (verified via the worktree's HEAD SHA being unchanged or via a process-call recorder).
   - Unit test asserting the path-exists-but-unrelated case still produces the v0.4 "already exists" error.
   - Integration test (against a real git repo): create a worktree, call `create_worktree` again, assert the second call succeeds without modifying the worktree state.
3. **Rollback** — revert the function. `git paw start` post-crash will fail again as it does today, but no other behavior regresses.

No flag, no opt-in. The fix is universally beneficial.

## Open Questions

- *Should the resume path detect that the branch's HEAD has changed since the worktree was created (e.g. someone force-pushed) and warn the user?* Out of scope for v0.5.0 — the worktree's working tree is the source of truth for the agent that owns it; force-push divergence is a merge-time problem, not a launch-time problem. Revisit in v0.6.0 if dogfood shows users hitting it often.

- *Should the resume path require the session state file to confirm the branch is one git-paw knows about?* Could prevent accidentally adopting an unrelated worktree someone hand-created. Out of scope for v0.5.0 — `create_worktree` is called with branches `cmd_start` already decided to launch, so the caller already does the equivalent check.

- *Should there be a `--force-fresh` flag to opt out of idempotent resume?* Not for v0.5.0. The user wanting a clean slate has `git paw purge` followed by `git paw start`. If dogfood shows users want a less destructive option, add the flag in v0.6.0.
