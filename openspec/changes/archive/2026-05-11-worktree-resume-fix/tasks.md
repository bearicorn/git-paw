## 1. Code fix in `git::create_worktree`

- [x] 1.1 At the top of `create_worktree()` in `src/git.rs`, after computing `worktree_path` and before the existing `git worktree add <path> <branch>` call, insert an idempotency pre-check.
- [x] 1.2 The pre-check runs only when `worktree_path.exists()` (skipped on first-time launches so there's no per-call subprocess cost in the common path).
- [x] 1.3 When the path exists, run `git worktree list --porcelain` and parse line-by-line. Track each block's `worktree <path>` line and check the following `branch <ref>` line. If a block matches both the expected path AND `refs/heads/<branch>`, return `Ok(WorktreeCreation { path: worktree_path, branch_created: false })`.
- [x] 1.4 If the porcelain output doesn't contain a matching `(path, branch)` block, fall through to the existing `git worktree add` call so the user sees git's `fatal: '<path>' already exists` error directly (preserving v0.4 contract for unrelated path collisions).

## 2. Tests

- [x] 2.1 Unit test `create_worktree_resume_returns_success_when_worktree_already_exists` in `src/git.rs::tests`: create a worktree via `create_worktree`, call it again with the same branch, assert the second call returns `Ok(WorktreeCreation { branch_created: false })` and that the worktree's HEAD SHA hasn't changed.
- [x] 2.2 Unit test `create_worktree_resume_falls_through_when_path_exists_but_unrelated` in `src/git.rs::tests`: create a regular directory at the expected path (no `.git` link), call `create_worktree`, assert the returned error message contains the substring `already exists`.
- [x] 2.3 Unit test `create_worktree_resume_falls_through_when_path_exists_for_different_branch` in `src/git.rs::tests`: create a worktree for branch `feat/a`, attempt `create_worktree` for `feat/b` at the same path, assert error.
- [ ] 2.4 Integration test in `tests/recover_integration.rs` (or `tests/session_integration.rs`): launch a session, kill the tmux session (simulating a crash), call `cmd_start` again, assert the same tmux session is recreated with all original worktrees intact.

## 3. Quality gates

- [x] 3.1 `just check` (fmt + clippy + tests) passes on the change branch.
- [x] 3.2 `just deny` passes (no new dependencies).
- [x] 3.3 No `unwrap()`/`expect()` introduced in the new code.
- [x] 3.4 The new pre-check has an inline doc comment explaining the resume rationale.

## 4. Docs

- [x] 4.1 If `docs/src/user-guide/` has a session-lifecycle / recovery section, update it to mention that `git paw start` after a crash now succeeds when worktrees survive. If no such section exists, add a small one under troubleshooting.
- [x] 4.2 No `--help` text changes — the recovery is implicit per the existing `start` help description ("recovers if stopped/crashed").
- [x] 4.3 No README changes — behaviour is now what the existing docs already claim.
