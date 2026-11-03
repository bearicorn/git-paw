## 1. Shared check

- [ ] 1.1 Move `rebase_skip_reason` from `src/commands/add.rs` into `src/git.rs` (reason strings unchanged); pick the tree from `find_worktree_for_branch_with`, else `repo_root` (design.md decision 3)
- [ ] 1.2 In `create_worktree_with`, run the check before `rebase_branch_onto_default_with` when `rebase_onto_main` is `true` and the branch exists; on a dirty or unreadable tree, skip the rebase and continue
- [ ] 1.3 Add `rebase_skipped: Option<String>` to `WorktreeCreation` with a doc comment; update the literals in `create_worktree_with` (~885, ~905, ~925) and the test literals (~1472, ~1559)
- [ ] 1.4 Leave `rebase_branch_onto_default` / `rebase_branch_onto_default_with` unchanged; confirm the `supervisor-branch-refresh` tests pass unchanged

## 2. Callers

- [ ] 2.1 `src/commands/start.rs` (both `create_worktree` sites) and `src/commands/launch.rs`: print the skip through `out.warn`, naming the branch and `--no-rebase`
- [ ] 2.2 `src/commands/add.rs`: drop its pre-check and print the returned reason with its existing notice text
- [ ] 2.3 Confirm MCP `start_session` carries the notice in `warnings` and writes nothing to stdout

## 3. Tests

- [ ] 3.1 `create_worktree_with` unit tests (fake runner): dirty worktree, dirty main checkout for a branch with no worktree, managed-only entries, status failure, clean tree still rebased, conflict still fails
- [ ] 3.2 Integration tests (real git + tmux) for each `git-operations` scenario: start over a dirty surviving worktree, start of a branch with no worktree over a dirty main checkout, supervised start, `--no-rebase` prints no notice, stderr/stdout split
- [ ] 3.3 MCP `start_session` test for the `warnings` scenario
- [ ] 3.4 Existing `tests/add_remove_e2e.rs` rebase tests pass unchanged (add's notice text is pinned there)

## 4. Docs

- [ ] 4.1 `docs/src/cli-reference.md`: `start --no-rebase` and the skip notice
- [ ] 4.2 `docs/src/user-guide/session-lifecycle.md`: generalise the add "never rebases over uncommitted work" note to every launch path, including the main-checkout case

## 5. Quality gates

- [ ] 5.1 `cargo fmt`, then `just lint`
- [ ] 5.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 5.3 `just deny`
- [ ] 5.4 `mdbook build docs/`
- [ ] 5.5 `openspec validate start-skips-rebase-on-dirty-worktree --strict`
