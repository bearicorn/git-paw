## Why

`git paw purge --force` can destroy verified but unmerged work. In the 0.16.0 external dogfood
(peer-4-poker, 2026-09-14) the operator followed the documented flow — drive loop reports
`completed, 0 escalations` → `git paw purge` — and the worker branches, each carrying commits that
had passed every gate but were never merged, were deleted along with their worktrees. `git branch -D`
removes the branch's reflog with the ref, and the worktree (whose `HEAD` reflog was the other
recovery path) is gone too, so there is no ordinary way back.

The offending site is `detach_worktree` in `src/main.rs`, the per-worktree teardown shared by
`purge` and `remove`: after `git::remove_worktree` it runs `git::delete_branch` (`git branch -D`)
whenever `WorktreeEntry.branch_created` is set — i.e. for every branch git-paw itself created, which
under `--from-specs` / `--from-all-specs` is all of them. The only guard is upstream in
`purge_with_prompt`: `collect_unmerged_branches` prints a warning and, without `--force`, asks for
confirmation; with `--force` the warning scrolls past and deletion proceeds. `remove` has no
unmerged check at all (only the dirty-tree "uncommitted-work safety").

Two smaller defects in the same path make the guard weaker than it looks. `collect_unmerged_branches`
treats a failed `git log <branch> --not <default>` as "no unmerged commits" (`continue`), so a branch
whose comparison cannot be made is deleted silently. And it resolves the default branch with a
purge-local `resolve_default_branch` that falls back to a literal `main` — not the single resolver
`git-operations` requires (GP-12) — so in a `master`-default repository with no `origin/HEAD` every
comparison fails and every created branch is deleted without so much as a warning.

Code archaeology narrows the milestone's framing: the `branch_created` gate and the deletion have been
in place since the broker era (`bd517d3f`) and are byte-identical between `v0.15.0` and `v0.16.0`
(`git diff v0.15.0 v0.16.0 -- src/main.rs src/git.rs` shows no change to `delete_branch`,
`branch_created`, or `detach_worktree`). What changed for the dogfooder is that 0.16.0's spec-driven
launch *created* the branches (`branch_created = true`) where the 0.15.0 run had used pre-existing
ones. The bug is real and has been shipping for several releases; it is not a 0.16.0 regression.

This change chains with `drive-loop-exit-summary-truth` (GP-21): a false `completed` is what walks
an operator into `purge`, and this change is the second, independent guard — even a truthful summary
must not be the only thing standing between a `--force` and a deleted branch.

## What Changes

- **Teardown never deletes unmerged commits.** `detach_worktree` deletes a branch only when
  git-paw created it **and** its tip is an ancestor of the default branch
  (`git merge-base --is-ancestor <branch> <default>`, default via the single resolver). A branch
  with commits not in the default branch — or whose merge state cannot be determined — is
  **retained** after its worktree is removed. This applies to `purge` (with or without `--force`)
  and to `remove` alike, because they share the helper.
- **Every outcome is printed with SHAs.** A deletion prints `Deleted branch <name> (was <sha>)`; a
  retention prints the branch, its tip SHA, its unmerged-commit count (or that the comparison
  failed), and how to resume (`git worktree add <path> <branch>`) or discard (`git branch -D`) it
  by hand.
- **The purge warning and prompt tell the truth.** The warning lists each unmerged branch with
  count and tip and says the branches will be *kept*; the confirmation now guards what is actually
  destroyed — worktrees (including uncommitted changes) and session state — instead of claiming
  "those commits will be lost".
- **Fail-safe comparison.** The unmerged check uses the single default-branch resolver and treats
  an undeterminable comparison as unmerged.
- **Help text corrected.** `purge`'s `about` / `long_about` / `--force`, the `stop` `long_about`
  line that describes purge as "removes worktrees, branches, and state", the README CLI table row,
  and the CLI reference state that branches with unmerged commits are kept.

## Capabilities

### New Capabilities

<!-- None — this hardens the existing purge/remove teardown. -->

### Modified Capabilities

- `supervisor-launch`: **Purge warns about unmerged commits** — the check is ref-based and
  fail-safe, uses the single default-branch resolver, `--force` no longer deletes an unmerged
  branch, the warning/prompt wording and the help text change.
- `git-add-remove-branch`: new requirement — the shared per-worktree teardown retains a branch
  carrying unmerged commits and prints SHAs for every deletion or retention (covers `remove` too).

## Impact

- **Code — offending sites:** `src/main.rs::detach_worktree` (the unconditional
  `branch_created → git::delete_branch`), `src/main.rs::purge_with_prompt` (warning / prompt text),
  `src/main.rs::collect_unmerged_branches` (silent `continue` on a failed comparison) and
  `src/main.rs::resolve_default_branch` (purge-local resolver with a literal `main` fallback — to be
  replaced by `git::default_branch`). `src/git.rs`: the `branch_is_merged_into_with` /
  unmerged-count / tip helpers (shared with `drive-loop-exit-summary-truth`; whichever change lands
  first adds them). `src/cli.rs`: `Purge` and `Stop` help strings. `src/commands/remove.rs` is
  unchanged in code but changes in behaviour through the shared helper.
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched).
- **Docs:** `docs/src/cli-reference.md` (`purge` section and the `--help` block), README CLI table
  row for `purge`, `docs/src/user-guide/session-lifecycle.md` (the `remove` teardown step and any
  "purge removes everything" wording), the `stop` long_about text.
- **Tests:** `src/main.rs` purge tests (`setup_purge_sandbox`, `purge_force_skips_confirm_but_still_warns`
  must now assert the branch survives), `tests/add_remove_e2e.rs` (`remove` of a created, unmerged
  branch retains it), `tests/e2e_tests.rs` / `tests/git_integration.rs` purge coverage.
- **Behaviour an existing user notices:** after `purge --force` (or a confirmed interactive purge),
  branches with unmerged commits remain in the repository where they used to be deleted; the
  command prints them with SHAs and a one-line way to discard. Fully merged created branches are
  still deleted; pre-existing branches were never deleted and still are not. `remove` of an
  unmerged created branch now keeps the branch. Exit status is unchanged in every case.
- **Security / safety:** this is a pure blast-radius reduction — the only irreversible git operation
  in the teardown (`git branch -D`) becomes conditional on a read-only ref check, and an
  undeterminable state fails closed. Branch names come from the session file and are passed as
  discrete arguments (existing `CommandRunner` seam), never through a shell. No allowlist, spawn
  surface, or privilege changes. Reviewed against `security-and-safety-review` — "irreversible git
  beyond scope" is exactly the catastrophe this closes.
- **Dependencies:** none.
