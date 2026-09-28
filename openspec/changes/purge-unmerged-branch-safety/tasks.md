## 1. Teardown guard (`src/main.rs::detach_worktree`, `src/git.rs`)

- [x] 1.1 The three `git.rs` ref helpers have **already landed** via `drive-loop-exit-summary-truth` (`6c98edc`): `branch_is_merged_into` / `branch_is_merged_into_with`, `unmerged_commit_count` / `unmerged_commit_count_with`, and `branch_tip_short` / `branch_tip_short_with`, each with a `CommandRunner`-seam variant. Reuse them as-is — do NOT add parallel helpers. They are `pub(crate)`, which is sufficient: `purge` and `detach_worktree` live in the same crate. Confirm the exit-code contract still matches (`0` → merged, `1` → not, other → `Err`) before relying on it
- [x] 1.2 In `detach_worktree`: resolve the default branch via `git::default_branch`; delete only when `branch_created` AND merged; on `Err` from the resolver or the comparison, retain and print that the merge state could not be determined
- [x] 1.3 Print `Deleted branch <name> (was <sha>)` on deletion; on retention print the branch, tip SHA, unmerged count, and the `git worktree add <path> <branch>` / `git branch -D <branch>` hint, on stderr with the existing progress markers
- [x] 1.4 Unit tests (sandbox repo): created + unmerged → retained at same tip, hint printed; created + merged → deleted, `was <sha>` printed; pre-existing (`branch_created = false`) → untouched; comparison failure → retained with reason

## 2. Purge UX (`src/main.rs::purge_with_prompt`)

- [x] 2.1 Replace `collect_unmerged_branches`' `git log --not` counting with the ref helpers; treat a failed comparison as unmerged (never `continue`); return count and tip per branch
- [x] 2.2 Replace `resolve_default_branch` with `git::default_branch` and delete the local function; when resolution fails, warn and retain every created branch
- [x] 2.3 Rewrite the warning block (per-branch count + tip, "will be kept") and the confirmation sentence (worktrees incl. uncommitted changes + session state are removed); keep the stderr flush before the prompt
- [x] 2.4 Update existing purge tests: `purge_force_skips_confirm_but_still_warns` and `purge_with_unmerged_commits_emits_warning_to_stderr` assert the branch survives at the same tip and the new wording; add the confirmed-"Y"-retains test, the undeterminable-state test, and a `master`-default / no-`origin/HEAD` sandbox test

## 3. Help text and docs

- [x] 3.1 `src/cli.rs`: `Purge` `about` / `long_about` ("removes all worktrees and session state; deletes only branches git-paw created that are fully merged — branches with unmerged commits are kept"), `--force` help ("Skip the confirmation prompt; never deletes a branch with unmerged commits"), and the `Stop` `long_about` line describing purge
- [x] 3.2 `docs/src/cli-reference.md` `purge` section and its `--help` block; README CLI table row for `purge`; `docs/src/user-guide/session-lifecycle.md` teardown wording for `remove`
- [x] 3.3 `--help` assertion test for the new wording (`tests/cli_tests.rs` or the existing help-text test file)

## 4. E2E and gates

- [x] 4.1 `tests/add_remove_e2e.rs`: `remove` of a created branch with an unmerged commit keeps the branch and prints the hint; `remove` of a merged created branch deletes it
- [x] 4.2 E2E purge (`tests/e2e_tests.rs` or `tests/git_integration.rs`): `purge --force` with an unmerged created branch removes worktree + session, keeps the branch, prints tip SHA
- [x] 4.3 Every scenario maps to a named test; `cargo fmt`; `just check` + `just deny` green; `mdbook build docs/` succeeds

## 6. Amendment — stale docs in this change's area (folded in 2026-10-18, from a docs sweep)

- [x] 6.1 `docs/src/cli-reference.md` purge entry does not say purge deletes the branches git-paw created; fix it, and the one-column misalignment in its help block
- [x] 6.2 README purge section does not mention branch deletion or `--stale`; add both
- [x] 6.3 `docs/src/architecture.md` describes teardown as "`git worktree remove` followed by prune"; the code runs the `on_remove` hook, then `worktree remove --force`, then deletes created branches, with no prune — describe what it actually does after this change

## 7. Post-review follow-ups (gate review 2026-10-19)

- [x] 7.1 SAFETY: qualify every ref in `branch_is_merged_into_with`, `unmerged_commit_count_with` and `branch_tip_short_with` as `refs/heads/<branch>` via a shared `heads_ref` helper — `rev-parse` tries `refs/tags/<name>` first, so a same-named tag on merged history would pass the gate and `branch -D` would delete the real unmerged branch; add a same-named-tag test
- [x] 7.2 Record the start point on `WorktreeEntry.base_branch` (`#[serde(default, skip_serializing_if)]`, old sessions load as `None`), populated from the main checkout's current branch when git-paw creates the branch
- [x] 7.3 Treat "ancestor of the default branch OR of the recorded base" as safe to delete, and count unmerged commits against the recorded base when there is one — `git worktree add -b` has no start point, so a wave launched from an integration branch otherwise keeps every merged branch and reports the whole integration backlog as its count
- [x] 7.4 Suggest `git branch -d` where it would succeed (the branch is contained in the checked-out branch), `-D` otherwise
- [x] 7.5 Quote the worktree path in the resume hint
- [x] 7.6 Fix the stale `invalidate_if_stale` doc comment that says stale invalidation purges branches
- [x] 7.7 Tests: same-named tag; created-from-integration merged into integration is deleted; unmerged one kept with a count against the base; `base_branch` round-trip, omission, and legacy load; hint quoting and `-d`/`-D` choice. Scenarios added to both delta specs; `openspec validate --strict`; `cargo fmt`; `just lint`; full suite with `--no-fail-fast`
