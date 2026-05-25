## Why

`git paw start --help` advertises "recovers if stopped/crashed" but the recovery path is currently broken when the session's worktrees still exist on disk. `create_worktree` (`src/git.rs`) calls `git worktree add <path> <branch>` unconditionally; if the path already exists from a prior session, git returns `fatal: '<path>' already exists` and `cmd_start` exits with a fatal error before reaching `tmux session creation`. The user is left with valid worktrees on disk, valid agent file edits in those worktrees, and no way to relaunch the session short of `git paw purge` (which deletes all worktrees and any uncommitted work in them) or manual tmux session construction.

Surfaced during v0.5.0 dogfood: the supervisor session crashed (tmux server died, broker with it). All 11 worktrees retained uncommitted agent file edits. `git paw start` reported `git worktree add failed for branch 'feat/governance-config': fatal: '<path>' already exists` on the first worktree it tried to re-add, exiting before the rest of the recovery flow could run.

This change makes `create_worktree` idempotent in the resume case: when the target path already exists AND is a registered git worktree for the same branch, treat it as a successful creation and return without re-running `git worktree add`. The path-but-wrong-state cases (path exists but isn't a worktree, path exists for a different branch) keep their existing error behaviour.

## What Changes

**Code fix in `git::create_worktree`** (`src/git.rs`):

Before the `git worktree add` call, the function SHALL check:

1. Does `worktree_path` exist on disk?
2. If yes, parse `git worktree list --porcelain` and look for a block whose `worktree` line matches `worktree_path` AND whose `branch` line matches `refs/heads/<branch>`.
3. If both match: return `Ok(WorktreeCreation { path: worktree_path, branch_created: false })` immediately. The worktree is already in the expected state from a prior session.
4. If the path exists but the worktree is registered for a different branch (or isn't registered at all), fall through to the existing `git worktree add` call so the user sees the actual `fatal: '<path>' already exists` error, which is the right signal — the user's environment has a path collision unrelated to git-paw's session state.

**Affected sites NOT changed:**

- `git worktree add -b <branch>` (the new-branch creation path) — this is the second `git worktree add` call inside `create_worktree`, used when the first call's stderr contains `invalid reference`. The pre-check above runs before either call, so the new-branch path benefits from the same idempotency.
- `remove_worktree` — does its own `git worktree remove` + prune; not affected.
- `cmd_purge` — still removes worktrees; users wanting a clean slate keep that escape hatch.

**Not in scope:**

- A separate `git paw resume` subcommand. The user's mental model is that `git paw start` does the right thing whether the session is fresh or recovering; making the existing implementation honor that is simpler than adding a new command.
- Per-branch staleness checks (e.g. "this worktree was created against main@<old SHA>; the branch is now far behind"). The user owns rebase/sync decisions; `create_worktree` only needs to be idempotent on the existence check.
- Repairing a corrupt session state file (`~/Library/Application Support/git-paw/sessions/paw-<project>.json`). That's a separate corruption-recovery story; this change only addresses the worktree-already-exists case.

## Capabilities

### New Capabilities
*(none — extends an existing capability)*

### Modified Capabilities

- `git-operations`: the existing "Create worktrees as siblings of the repository" requirement gains an idempotent-resume scenario covering the case where `create_worktree()` is called for a branch whose worktree already exists at the expected path.

## Impact

**Code**:
- `src/git.rs::create_worktree` — adds a pre-check that parses `git worktree list --porcelain` and returns success when the existing worktree matches the expected `(path, branch)` pair. ~30 lines added at the top of the function. Existing fall-through to `git worktree add` remains for path-collision-with-wrong-state cases.

**Tests**:
- Unit test: `create_worktree_resume_returns_success_when_worktree_already_exists` — creates a worktree, deletes the tmux session metadata (or just simulates the resume by not removing the worktree), calls `create_worktree` again, asserts the return is `Ok(WorktreeCreation { path: ..., branch_created: false })` and that no second `git worktree add` was executed (verified indirectly by the worktree's HEAD SHA being unchanged from the original add).
- Unit test: `create_worktree_resume_falls_through_when_path_exists_but_unrelated` — creates a non-worktree directory at the expected path, calls `create_worktree`, asserts the error message contains "already exists" (preserving the existing v0.4 contract for unrelated path collisions).
- Integration test: end-to-end resume verification — launch a session, kill the tmux server (simulating a crash), call `cmd_start` again, assert the same tmux session is recreated against the existing worktrees without errors.

**Backward compatibility**: fully additive. The pre-check only short-circuits when the existing-worktree-matches-expected-branch condition holds; every other path keeps its existing behaviour. Sessions that didn't have a prior worktree (first-time launches) skip the pre-check on the existence guard and run `git worktree add` exactly as before. Sessions with conflicting path state (unrelated dir at the same name) get the same error they got in v0.4.

**Mismatches resolved**:
- Dogfood D-resume (tmux server crash → `git paw start` fails on "already exists" before reaching tmux recovery) — resolved by the pre-check.
- The "Smart start" `--help` description ("recovers if stopped/crashed") now actually delivers on the second half.
