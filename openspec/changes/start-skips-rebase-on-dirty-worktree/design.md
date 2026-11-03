## Context

`create_worktree_with` (`src/git.rs`) rebases an existing branch onto the default branch before its idempotency check, through `rebase_branch_onto_default_with`. That function runs the rebase in the worktree holding the branch, or switches the main checkout to the branch when none holds it. v0.19 `add-keeps-worktree-state` put a dirty-tree check (`rebase_skip_reason`) in `src/commands/add.rs` only. `start`, the supervised launch and MCP `start_session` call `create_worktree` directly and still fail on a dirty tree.

## Goals / Non-Goals

**Goals:**
- One rule for every creation-time rebase, decided in one place.
- `add`'s observable behaviour unchanged, including its notice text.

**Non-Goals:**
- Changing the supervisor's mid-session branch refresh.
- Rebasing a branch without touching the main checkout (for example in a scratch worktree). That would make the no-worktree case never skip, but it changes the rebase mechanics and belongs in its own change.
- Changing recovery, which never rebases.

## Decisions

### 1. The check lives in `create_worktree_with`, not in `rebase_branch_onto_default_with`

The brief preferred either place. `rebase_branch_onto_default_with` also serves `rebase_branch_onto_default`, the supervisor's mid-session refresh, which has its own clean-tree precondition and its own skip reporting (`supervisor-branch-refresh`). A check inside the shared function would either duplicate that precondition or turn a skipped refresh into a silent success. `create_worktree_with` is the single caller for every creation-time rebase: `start` (two call sites), `launch`, and `add`, which reaches it through `launch`. So putting the check there covers every path in the brief and leaves the refresh contract alone.

The check runs only when `rebase_onto_main` is `true` and the branch exists, exactly where the rebase would run. The tree it inspects is the one the rebase would use: `find_worktree_for_branch_with`, else `repo_root`.

### 2. The skip is returned, not printed

`src/git.rs` has no output handle. `WorktreeCreation` gains `rebase_skipped: Option<String>` (the reason). Callers turn it into their notice through `Output::warn`, so CLI notices go to stderr and the MCP `CaptureOutput` collects them into `warnings`. `add` keeps its exact text: `notice: rebase skipped for '<branch>': <reason>. Pass --no-rebase to attach without a rebase.` `start` uses the same shape, with "to launch without a rebase".

A field adds struct-literal sites: the three constructions in `create_worktree_with` and two test literals. That is a small, bounded ripple. The alternative, a second return value or an out-parameter, would change every `create_worktree` call site, more than 40 across `src/` and `tests/`.

### 3. The detection moves from `add.rs` into `git.rs`

`rebase_skip_reason` moves into `src/git.rs` next to `create_worktree_with`, with its reason strings unchanged. It keeps `agents::is_managed_path` as the filter. `git` → `agents` is a new module edge; if the dependency direction forbids it, `create_worktree_with` takes the managed-path predicate as a parameter instead, and the implementer records which option was used.

## Risks / Trade-offs

- [A main checkout with any untracked file now skips the rebase for every branch that has no worktree] → this follows the user's "same rule as add" decision. The notice names the branch and `--no-rebase`, so it is visible. Rebasing in a scratch worktree (a non-goal above) is the follow-up that would remove it.
- [A launch that used to stop now proceeds on a stale branch] → intended. The notice says so, and the worker's branch can still be refreshed later.
