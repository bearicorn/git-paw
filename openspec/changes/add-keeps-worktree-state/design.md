## Context

`add` builds an `AttachContext` with `no_rebase: false` hard-coded (`src/commands/add.rs`), and
`attach_agent` calls `git::create_worktree(repo_root, branch, !ctx.no_rebase, placement)`. For a
branch that already exists, `create_worktree` rebases it onto the default branch through
`rebase_branch_onto_default_with`, which runs `git rebase <default>` inside the worktree holding the
branch. Git refuses that rebase when the worktree has unstaged changes; `rebase_branch_onto_default_with`
aborts and returns `rebase onto main failed: …`, and `add` exits before spawning a pane. Nothing is
lost, but in a CLI swap `remove --keep-worktree` has already detached the agent, so the worker stays
out of the session (GP-87). When the tree is clean, the rebase succeeds and moves the worker's
commits onto the new default-branch tip in the middle of its task.

`start` already has `--no-rebase`. `remove` already has an uncommitted-work detector
(`git::uncommitted_files`, NUL-delimited porcelain) and a filter for git-paw's own injected files
(`agents::is_managed_path`). `supervisor-branch-refresh` already treats a dirty worktree as "leave
the branch untouched", not as an error.

## Goals / Non-Goals

**Goals:**
- `add` gets `--no-rebase` with `start`'s meaning.
- `add` never rebases a branch whose worktree holds uncommitted work, and never stashes, commits,
  resets or discards that work.
- A CLI swap (`remove --keep-worktree` then `add --cli <other>`) completes in one `add` call on a
  dirty tree.

**Non-Goals:**
- Changing the default for a clean worktree. `add` without the flag still rebases an existing
  branch, as `start` does; callers that want to keep the base (a CLI swap usually does) pass
  `--no-rebase`.
- Changing `create_worktree`, `rebase_branch_onto_default_with` or `start`. Their contracts in
  `git-operations` and `cli-parsing` stay as they are.
- A new verb that swaps a worker's CLI atomically (see Open Questions).
- The main checkout. When the branch is not checked out in any worktree, the existing contract
  (rebase in the main checkout, which git refuses on unstaged changes) is unchanged.

## Decisions

### D1: On a dirty worktree, skip the rebase and attach; do not refuse

`add` checks the worktree that holds the branch. If it has uncommitted changes, `add` behaves as if
`--no-rebase` had been passed, prints one stderr notice naming the branch and `--no-rebase`, and
attaches the agent on the branch's current HEAD.

Why skip rather than refuse:
- **Refusing can't undo the earlier `remove`.** By the time `add` runs in a swap, the agent is
  already out of the session. Refusing turns git's error into a clearer one, but the worker still
  stays out until someone re-runs `add` with the flag. That stranded worker is the harm GP-87
  reports.
- **The rebase on `add` is a freshness step, not a precondition.** Attaching an agent on its current
  base is always valid, and it is exactly what `--no-rebase` does. Skipping produces a state the user
  could have asked for; refusing produces nothing.
- **It matches existing doctrine.** `supervisor-branch-refresh` says a dirty worktree is never
  rebased and leaves the branch untouched without failing the merge. `add` follows the same rule.
- **It stays visible.** The notice names the skipped rebase and `--no-rebase`, so a user who wanted
  the rebase learns why it didn't happen. The notice goes to stderr so that v0.21.0 `add --json`
  stdout stays a single document.

Alternative considered: refuse before attaching, with a message naming `--no-rebase`. It is
explicit and never changes what the default does. It was rejected because it still fails the swap
it is meant to fix. The refusal behaviour is kept for one case, a rebase that is *attempted* and
then fails (conflict, or a state the detector did not report): `add` exits non-zero before
attaching, and the error now names `--no-rebase`.

### D2: "Uncommitted" means what `remove` means

The check reuses `git::uncommitted_files` filtered by `agents::is_managed_path`, the same
classification behind `remove`'s safety check. That gives the capability one definition of "user
work": a worktree whose only entries are git-paw's injected sidecar or `.git-paw/` state counts as
clean and is rebased as before. Untracked user files count as dirty. A rebase would usually leave
them alone, but they are the worker's in-progress work, and skipping a rebase is never destructive.

Edge case: a tracked `AGENTS.md` that carries only an old git-paw managed block counts as managed
(clean), yet git treats it as an unstaged change and refuses the rebase. That case goes to the D1
fallback: `add` fails before attaching, nothing is lost, and the error names `--no-rebase`. Current
git-paw no longer writes that block, so this only affects worktrees made by older versions.

### D3: The check lives in `add`, not in `create_worktree`

`add` finds the worktree holding the branch (the porcelain lookup `rebase_branch_onto_default_with`
already does), runs the check after taking the session lock and before `attach_agent`, and sets
`AttachContext.no_rebase` from `--no-rebase || dirty`. Putting the check in `create_worktree`
would change `start` too. `start`'s contract is specified in `git-operations` and `cli-parsing`,
and v0.21.0 `supervisor-vocabulary-split` modifies its flags requirement. Keeping the check in
`add` leaves that contract untouched and keeps this change small.

## Risks / Trade-offs

- [The default becomes conditional: a clean worktree is rebased, a dirty one is not] → The notice
  names the skip and the flag. Callers that need a fixed outcome pass `--no-rebase`. The
  alternative, failing the swap, is the reported bug.
- [The tree changes between the check and the rebase (time-of-check/time-of-use)] → The rebase still
  runs under `create_worktree`'s abort contract. Git refuses on unstaged changes and the abort
  restores the pre-rebase HEAD, so the worst case is the D1 fallback error and nothing is lost.
- [A clean-tree swap still rebases by default] → Accepted, to keep the change small and consistent
  with `start`. `--no-rebase` is the documented choice for a swap.

## Migration Plan

None. The flag is new and optional, and a clean worktree behaves as before. The only behaviour
change on an existing invocation is a dirty-worktree `add`, which used to fail and now succeeds
with a notice.

## Open Questions

- **An atomic swap-CLI verb** (candidate, not specced): one command, for example
  `git paw swap <branch> --cli <other>`, that relaunches a worker's surface with another CLI while
  keeping its worktree, uncommitted work, base and session entry. It would check everything first,
  so a swap that can't complete fails before the agent is detached. It would also remove the window
  between `remove` and `add` in which the worker has no surface. Decide once a CLI-swap workflow
  (for example a usage-limit or `cli-unavailable` hand-off) needs it as a first-class operation.
- **`start` over a surviving dirty worktree.** A fresh `start --branches feat/x` whose worktree
  survived with uncommitted changes hits the same git refusal. `start` already has `--no-rebase`,
  and recovery after `stop` does not rebase, so this is left to a later change if dogfood shows it.
