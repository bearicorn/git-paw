## Why

v0.19 `add-keeps-worktree-state` made `git paw add` skip its rebase, with a one-line stderr notice and the tree left untouched, when the branch's worktree has uncommitted changes or its status check fails. Every other path that rebases an existing branch still runs the rebase blindly and dies with git's `cannot rebase: You have unstaged changes`:

- `git paw start`: a fresh start (no saved session) over worktrees left on disk, plain or spec-driven (`src/commands/start.rs`).
- `git paw start --supervisor` (the shared launch path in `src/commands/launch.rs`).
- The MCP `start_session` tool, which always passes `no_rebase: false`.
- A branch with **no** worktree, which is rebased in the main checkout (`git -C <repo_root> rebase <default> <branch>`) and fails whenever the main checkout is dirty.

The user's choice is one rule everywhere: never rebase over uncommitted work. A clean worktree on a moved default branch is still rebased.

## What Changes

- `create_worktree`'s creation-time rebase runs only inside the branch's own worktree, never in the main checkout. A branch with no worktree gets its worktree first and is rebased there; if that rebase fails, the new worktree is removed and the error returned as before.
- A worktree that already holds the branch is checked before rebasing, with the detection `add` uses (`git::uncommitted_files` + `agents::is_managed_path`; a failed status check counts as dirty).
- On a dirty or unreadable worktree the rebase is skipped. The worktree is created or reused on the branch's current HEAD, and the skip reason is returned to the caller. Nothing is stashed, committed, reset or discarded.
- Recovering a stopped or crashed session (`src/commands/recover.rs`) does not call `create_worktree` and never rebases, so it is unaffected. The brief listed it; reading the code shows it is not a failing path.
- Every caller reports the skip: `start` (plain, supervised, spec-driven) prints one stderr notice naming the branch and `--no-rebase`, and the MCP `start_session` returns it in its `warnings` without the `--no-rebase` hint (the tool has no such parameter). `add` keeps its existing notice text and its pre-check, which now calls the shared check, so its notice prints even when worktree creation later fails.
- A clean tree is still rebased, and a rebase that fails for another reason (a conflict) still fails as today.
- `--no-rebase` skips the check and prints no notice, as in `add`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `git-operations`: new requirements. The creation-time rebase never runs over uncommitted work, judges it as `remove` does, and reports a skip to its caller instead of failing.

`git-add-remove-branch` is unchanged. Its `add` rebase requirements stay true, and their scenarios are the regression guard for moving `add`'s check into the shared step.

## Impact

- **Code**: `src/git.rs` (`create_worktree_with` gains the check before `rebase_branch_onto_default_with`; `WorktreeCreation` gains the skip reason), `src/commands/start.rs` (the two `create_worktree` call sites, lines ~281 and ~780), `src/commands/launch.rs` (~230), `src/commands/add.rs` (`rebase_skip_reason` moves to `src/git.rs`; `add` keeps its pre-check through it, with its existing notice text), `src/commands/supervisor.rs` (prints the skip `attach_agent` returns), `src/mcp/tools/session.rs` (strips the `--no-rebase` hint from the notice it collects into `warnings`).
- **Struct-literal ripple (`WorktreeCreation`)**: the three constructions in `create_worktree_with` (`src/git.rs` ~885, ~905, ~925) and two test literals compared with `assert_eq!` (`src/git.rs` ~1472, ~1559). `src/` and `tests/` have no other `WorktreeCreation {` literal.
- **Not touched**: `rebase_branch_onto_default` / `rebase_branch_onto_default_with` keep their contract. The supervisor's mid-session branch refresh (`supervisor-branch-refresh`) has its own clean-tree precondition and must keep reporting its own skip. See design.md.
- **No enum ripple**.
- **Docs**: `docs/src/cli-reference.md` (`start --no-rebase`, the skip notice), `docs/src/user-guide/session-lifecycle.md` (the add section generalises to every launch path).
- **Behaviour change**: a launch that used to abort on a dirty tree now succeeds on the un-rebased branch, with a notice.
