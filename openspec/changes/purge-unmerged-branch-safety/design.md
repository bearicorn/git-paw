## Context

`detach_worktree` (`src/main.rs`) is the single teardown both `purge_with_prompt` and `cmd_remove`
call: run the consumer's `on_remove` hook, `git worktree remove`, then — if
`WorktreeEntry.branch_created` — `git branch -D`. The `branch_created` flag has gated deletion since
the broker era; under `--from-specs` git-paw creates every branch, so in practice every agent branch
is deleted at purge. The upstream guard, `purge_with_prompt`, warns about unmerged commits (counting
`git log <branch> --not <default>`) and asks for confirmation unless `--force`; it never *withholds*
deletion, it swallows a failed `git log` as "merged", and it resolves the default branch locally
(`origin/HEAD` else the literal `main`) instead of through `git::default_branch`. `remove` has no
unmerged guard at all. `git branch -D` discards the branch reflog with the ref, and the worktree's
`HEAD` reflog is deleted with the worktree, so a deleted unmerged branch is recoverable only via
`git fsck --lost-found` — not an "ordinary" path.

## Goals / Non-Goals

**Goals:**
- No command path in git-paw deletes a branch whose commits are not in the default branch.
- Every deletion and every retention is printed with the SHA that matters.
- The warning and the prompt describe what is actually destroyed.
- The purge path obeys the single default-branch resolver and fails closed.
- Help text and docs match.

**Non-Goals:**
- A dirty-tree (uncommitted changes) guard for `purge` — `remove` has one; purge's prompt now says
  uncommitted changes are lost, and adding the guard is a separate decision.
- A `--delete-unmerged` opt-in flag. The printed `git branch -D <branch>` one-liner is the opt-in;
  an extra flag on a "nuclear" command is a footgun this release should not add.
- Changing `stop` (it does not delete branches) or `purge --stale` semantics beyond inheriting the
  retention rule through the shared helper.

## Decisions

### D1 — Gate the deletion in the shared helper, not in each caller

The check belongs in `detach_worktree`: it is the only place `git branch -D` is issued, and putting
the rule there covers `purge`, `purge --stale`, and `remove` with one implementation and one set of
tests. Callers keep their own UX (purge's warning + prompt; remove's dirty-tree guard); the helper
guarantees the invariant regardless of how it was reached or which flags were passed.

### D2 — Retain rather than refuse

Two candidate `--force` behaviours: (a) refuse the whole purge when any branch is unmerged; (b)
proceed with the purge but retain the unmerged branches. (b) is chosen. Purge's job — kill the
session, remove worktrees and state — is fully done; only the irreversible step is withheld, and the
branch stays exactly where `git worktree add` can pick it up again. (a) would break every cleanup
script that relies on `--force` being non-interactive and would still leave the operator to delete
the branches by hand. The milestone's "refuse (or prompt)" is satisfied at the level that matters:
the destructive action is refused; the non-destructive teardown is not.

### D3 — Fail closed

`branch_is_merged_into_with` (from `git.rs`, shared with `drive-loop-exit-summary-truth`) returns
`Err` for anything other than exit `0`/`1`; the helper treats `Err` as "retain, say why". The
current `continue`-on-failure in `collect_unmerged_branches` is inverted. Combined with D4 this closes
the silent-deletion path in `master`-default repositories without `origin/HEAD`.

### D4 — One default-branch resolver

`purge_with_prompt` and `detach_worktree` call `git::default_branch(repo_root)`. The purge-local
`resolve_default_branch` is deleted (it is the orphan this change creates). If the resolver itself
fails, every branch is retained (D3) and the message says the default branch could not be resolved.

### D5 — Print SHAs on both paths

Deletion: capture the tip before `git branch -D` and print `Deleted branch <name> (was <sha>)` — the
same line git prints, which the helper currently swallows. Retention: `Kept branch <name> at <sha>
(<n> commit(s) not in <default>)` followed by the two-line hint (`git worktree add <path> <branch>`
/ `git branch -D <branch>`). Both go to stderr with the existing per-worktree progress markers so the
`--force` "freeze" fix (Bug D) is preserved.

### D6 — Prompt wording follows the new truth

With unmerged branches: `Warning: N branch(es) have unmerged commits and will be kept:` + per-branch
lines, then `Their worktrees (including any uncommitted changes) and the session state will be
removed. Continue?`. Without: the existing sentence. The stderr flush before `Confirm::interact`
(Bug C) is kept.

### D7 — Unhappy paths after this change

- **Truthful `escalated-for-review` summary, operator purges anyway:** worktrees and session go;
  branches remain with printed tips; `git worktree add` resumes any of them.
- **False `completed` from an older binary, or the operator ignores the summary:** same as above —
  this guard does not depend on the summary being right.
- **Detached / scripted `purge --force`:** non-interactive as before; the retained-branch lines are
  in the log; exit status unchanged.
- **Default branch unresolvable (fresh clone, odd remote layout):** nothing is deleted; the message
  names the resolver failure; the operator can delete by hand once they have looked.

## Risks / Trade-offs

- **Leftover branches after `--force`.** Intentional; each is printed with a discard one-liner. A
  repository that runs many waves accumulates only branches that were never merged — which is the
  point.
- **`remove` behaviour change** for consumers who used it to throw away an abandoned branch: the
  branch now survives; the hint tells them how to finish the job. Their worktree removal, pane
  closure, and session update are unchanged.
- **Two git spawns per branch** (`merge-base`, `rev-list --count`, plus `rev-parse` for the tip) —
  trivial next to `git worktree remove`.
- **Shared helper with GP-21:** if the two changes are implemented in parallel the git helper is
  written twice; the recommended order (GP-21 first) avoids this, and the second implementer
  rebases onto the first.
