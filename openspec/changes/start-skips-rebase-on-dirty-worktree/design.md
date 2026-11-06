## Context

`create_worktree_with` (`src/git.rs`) rebases an existing branch onto the default branch before its idempotency check, through `rebase_branch_onto_default_with`. That function runs the rebase in the worktree holding the branch, or switches the main checkout to the branch when none holds it. v0.19 `add-keeps-worktree-state` put a dirty-tree check (`rebase_skip_reason`) in `src/commands/add.rs` only. `start`, the supervised launch and MCP `start_session` call `create_worktree` directly and still fail on a dirty tree.

## Goals / Non-Goals

**Goals:**
- One rule for every creation-time rebase, decided in one place.
- `add`'s observable behaviour unchanged, including its notice text.

**Non-Goals:**
- Changing the supervisor's mid-session branch refresh.
- Changing recovery, which never rebases.

## Decisions

### 1. The check lives in `create_worktree_with`, not in `rebase_branch_onto_default_with`

The brief preferred either place. `rebase_branch_onto_default_with` also serves `rebase_branch_onto_default`, the supervisor's mid-session refresh, which has its own clean-tree precondition and its own skip reporting (`supervisor-branch-refresh`). A check inside the shared function would either duplicate that precondition or turn a skipped refresh into a silent success. `create_worktree_with` is the single caller for every creation-time rebase: `start` (two call sites), `launch`, and `add`, which reaches it through `launch`. So putting the check there covers every path in the brief and leaves the refresh contract alone. `add` also runs the same check up front (decision 3), and on a skip passes `no_rebase = true`, so the check inside `create_worktree` only runs for `add` when the tree looked clean.

The check runs only when `rebase_onto_main` is `true`, the branch exists and a worktree already holds it (`find_worktree_for_branch_with`). It inspects that worktree only.

### 1a. Rebase in the branch's worktree, never in the main checkout (human decision, review round 1)

A branch no worktree holds is no longer rebased in the main checkout. `create_worktree_with` creates (or reuses) its worktree first and then calls `rebase_branch_onto_default_with`, which now finds that worktree and rebases inside it. A worktree created moments ago is clean, so the dirty check never looks at the main checkout, and this removes the main-checkout branch switch (`git -C <repo_root> rebase <default> <branch>` and the HEAD restore) from every creation-time rebase. If the rebase in the new worktree fails, `create_worktree_with` removes that worktree (`git worktree remove --force`, safe because it holds only the fresh checkout) and returns the rebase error, so a failed launch attaches nothing, as before. `rebase_branch_onto_default_with` itself is unchanged; its main-checkout fallback is now reached only by callers that rebase a branch no worktree holds, which the creation path never does.

### 1b. Main checkout, rebase by path, stopped rebases (review round 2)

- **A branch checked out in the main checkout is refused, not skipped.** `find_worktree_for_branch_with` lists the main worktree too. Skipping the rebase would not help: git cannot check out that branch in a second worktree, so `git worktree add` would fail right after with a less useful error. `create_worktree_with` therefore returns an error up front naming the main checkout and the fix (switch it to another branch). `add`'s pre-check ignores that holder and leaves the error to `create_worktree`.
- **The rebase takes the worktree path.** `rebase_in_worktree_with(runner, repo_root, workdir)` rebases in the given directory only. The creation path passes the holding worktree, or the path it just created, so it never re-lists worktrees and can never fall back to the main-checkout branch switch in `rebase_branch_onto_default_with`, which stays unchanged for the supervisor refresh.
- **A stopped rebase is a skip reason.** A worktree whose git dir holds `rebase-merge` or `rebase-apply` is not rebased, so its `git rebase --abort` can never discard that rebase. Such a worktree has a detached HEAD and is missing from `git worktree list`'s branch lines, so `add_or_reuse_worktree` also reuses a listed worktree at the expected path whose stopped rebase's `head-name` is the branch; before this, a start over it failed with git's "already exists".

### 2. The skip is returned, not printed

`src/git.rs` has no output handle. `WorktreeCreation` gains `rebase_skipped: Option<String>` (the reason). Callers turn it into their notice through `Output::warn`, so CLI notices go to stderr and the MCP `CaptureOutput` collects them into `warnings`. `add` keeps its exact text: `notice: rebase skipped for '<branch>': <reason>. Pass --no-rebase to attach without a rebase.` `start` uses the same shape, with "to launch without a rebase". MCP `start_session` has no `--no-rebase`, so it strips that hint (`launch::REBASE_SKIP_CLI_HINT`) from its warnings and returns the notice with the reason only. A suffix strip keeps the wording decision in one place without adding a field to the three launch option structs.

A field adds struct-literal sites: the three constructions in `create_worktree_with` and two test literals. That is a small, bounded ripple. The alternative, a second return value or an out-parameter, would change every `create_worktree` call site, more than 40 across `src/` and `tests/`.

### 3. The detection moves from `add.rs` into `git.rs`

`rebase_skip_reason` moves into `src/git.rs` next to `create_worktree_with`, with its reason strings unchanged. It keeps `agents::is_managed_path` as the filter. `git` → `agents` is a new module edge; `agents` already imports from `git`, and the two sit in the same crate, so the direct call was used rather than a predicate parameter.

`add` keeps its own pre-check, now calling the shared `git::rebase_skip_reason`, and passes `no_rebase = true` on a skip. A reason returned from `create_worktree` is only available when creation succeeds, so a pre-check is what lets `add` print its notice even when the worktree creation that follows fails (pinned by `add_skips_the_rebase_when_the_worktree_status_cannot_be_read`). `start`, the supervised launch and MCP rely on the check inside `create_worktree`. If `add`'s pre-check found the worktree clean but `create_worktree` still skipped (it went dirty in between), `add` prints the returned reason with the same notice text.

## Risks / Trade-offs

- [Creating the worktree before the rebase means a conflicting branch briefly has a worktree] → it is removed before the error returns, and nothing is attached.
- [A launch that used to stop now proceeds on a stale branch] → intended. The notice says so, and the worker's branch can still be refreshed later.
