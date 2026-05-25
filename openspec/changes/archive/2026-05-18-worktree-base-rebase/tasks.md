## 1. Signature change to `create_worktree`

- [x] 1.1 In `src/git.rs`, change the signature of `create_worktree` from `fn create_worktree(repo_root: &Path, branch: &str) -> Result<WorktreeCreation, PawError>` to `fn create_worktree(repo_root: &Path, branch: &str, rebase_onto_main: bool) -> Result<WorktreeCreation, PawError>`.
- [x] 1.2 Update the function's doc comment to describe the new parameter (when `true`, rebase the branch onto `default_branch()` before the existence check; abort on conflict).

## 2. Rebase block implementation

- [x] 2.1 At the top of `create_worktree`, after computing `worktree_path` and BEFORE the existing existence-check (idempotency) block from `worktree-resume-fix`, insert the rebase block.
- [x] 2.2 The block runs only when `rebase_onto_main == true`. Short-circuit to the existing logic otherwise.
- [x] 2.3 The block runs `git rev-parse --verify refs/heads/<branch>` first. If that fails (branch doesn't exist locally), skip the rebase (the `-b` fallback later creates the branch from current HEAD).
- [x] 2.4 The block calls `default_branch(repo_root)` to resolve the rebase target. Propagate any error as-is (the existing `BranchError` from that helper).
- [x] 2.5 The block invokes `git -C <repo_root> rebase <default-branch> <branch>` (or equivalent two-step: `git checkout <branch>` then `git rebase <default-branch>`, depending on cleanest implementation). If the rebase exits zero (including the no-op up-to-date case), proceed to the existence check.
- [x] 2.6 If the rebase exits non-zero, run `git -C <repo_root> rebase --abort` (best-effort; ignore the abort's exit code) and return `Err(PawError::WorktreeError(format!("rebase onto main failed: {stderr}")))` with the original rebase stderr included.
- [x] 2.7 Verify no `.git/rebase-merge` or `.git/rebase-apply` directory remains after the abort (the abort handles this; test asserts it).

## 3. CLI flag `--no-rebase`

- [x] 3.1 In `src/cli.rs`, add `no_rebase: bool` to the `StartArgs` struct with `#[arg(long, default_value_t = false, help = "Skip rebasing existing agent branches onto the default branch before opening worktrees")]`.
- [x] 3.2 Update the `start --help` `long_about` to mention the new flag and the default-on rebase behaviour, with a short paragraph explaining when to use `--no-rebase`.

## 4. Call-site updates

- [x] 4.1 In `src/main.rs` (or wherever `cmd_start` lives), update every `create_worktree(repo_root, branch)` call to `create_worktree(repo_root, branch, !args.no_rebase)`.
- [x] 4.2 Repeat for `cmd_start_from_specs` and `cmd_supervisor` (if it constructs worktrees directly).
- [x] 4.3 Repeat for any other internal caller surfaced by `rg "create_worktree\("` — every caller passes a `rebase_onto_main` value sourced from the CLI args (or `true` for new callers that don't expose the flag).
- [x] 4.4 Update existing tests in `src/git.rs::tests` that call `create_worktree(...)` to pass `false` for `rebase_onto_main` so their existing assertions remain stable (those tests verify v0.5 contracts that don't involve rebase).

## 5. Unit tests in `src/git.rs::tests`

- [x] 5.1 `create_worktree_rebases_branch_when_behind_main`: set up a repo with `main` advanced by 2 commits past `feat/example`. Call `create_worktree(repo, "feat/example", true)`. Assert: result is `Ok(WorktreeCreation { branch_created: false, .. })`; `git rev-list --count <feat>..main` is `0` (feat now contains main's commits); the worktree directory is created.
- [x] 5.2 `create_worktree_rebase_noop_when_branch_up_to_date`: branch is at the same SHA as `main`. Call with `true`. Assert: branch HEAD unchanged; result is `Ok`; no error.
- [x] 5.3 `create_worktree_rebase_conflict_aborts_and_errors`: induce a conflict by modifying the same line on both branches. Call with `true`. Assert: result is `Err(PawError::WorktreeError(msg))` with `msg.contains("rebase onto main failed")`; branch HEAD equals pre-call HEAD; no `.git/rebase-merge` or `.git/rebase-apply` directory in `repo.path().join(".git")`.
- [x] 5.4 `create_worktree_no_rebase_preserves_v0_5_behaviour`: branch is 2 commits behind `main`. Call with `false`. Assert: branch HEAD unchanged; result is `Ok`; worktree created at the old SHA.
- [x] 5.5 `create_worktree_new_branch_skips_rebase_regardless_of_flag`: branch does not exist. Call with `true`. Assert: no rebase invocation (verified indirectly — branch is created from current HEAD via the `-b` fallback); result is `Ok(WorktreeCreation { branch_created: true, .. })`.

## 6. Integration test

- [x] 6.1 In `tests/start_integration.rs` (or a new `tests/rebase_integration.rs`), add an end-to-end test: initialise a repo, create a branch `feat/example` at the initial commit, advance `main` by 3 commits, run `git-paw start --branches feat/example` from the binary (via `assert_cmd`), assert the resulting worktree's `git log --oneline` shows the 3 main commits as ancestors of `feat/example`.
- [x] 6.2 Companion integration test with `--no-rebase`: same setup, run with `--no-rebase`, assert the worktree's `feat/example` HEAD does NOT contain the 3 new commits.

## 7. CLI parsing tests

- [x] 7.1 In `src/cli.rs::tests` (or wherever `StartArgs` parsing is tested), add `start_with_no_rebase_flag_sets_no_rebase_true`: parse `["git-paw", "start", "--no-rebase"]`, assert `no_rebase == true`.
- [x] 7.2 `start_without_no_rebase_defaults_to_false`: parse `["git-paw", "start"]`, assert `no_rebase == false`.
- [x] 7.3 `start_no_rebase_combines_with_supervisor`: parse `["git-paw", "start", "--no-rebase", "--supervisor"]`, assert both `no_rebase` and `supervisor` are `true`.

## 8. Quality gates

- [x] 8.1 `just check` (fmt + clippy + all tests) passes on the change branch. _Pre-existing `clippy::pedantic` warnings in `src/tmux.rs` (cast_sign_loss, doc_markdown, too_many_lines) and 4 environment-specific config-integration test failures already fail on the base commit `52d450c`; nothing in this change introduces new lint or test regressions. New unit + integration tests for rebase all pass, and `cargo clippy --all-targets -- -D warnings` reports zero warnings against any file touched here._
- [x] 8.2 `just deny` passes (no new dependencies).
- [x] 8.3 No `unwrap()` / `expect()` introduced in non-test code (the new rebase block uses `?` and `.ok()` per project conventions).
- [x] 8.4 The new rebase block has an inline doc comment explaining the drift it resolves (link to drift item 48 in `MILESTONE.md` or summarise inline).

## 9. Documentation

- [x] 9.1 Update `src/cli.rs` `start` `long_about` to mention the rebase default and the `--no-rebase` opt-out. Include an example.
- [x] 9.2 Update `README.md` "Smart start" paragraph to note that existing branches are rebased onto the default branch by default at launch.
- [x] 9.3 Update or add an mdBook chapter under `docs/src/user-guide/` (e.g. `session-lifecycle.md` or `troubleshooting.md`) describing the rebase-on-start behaviour, when to use `--no-rebase`, and what to do when a rebase conflict aborts.
- [x] 9.4 Run `mdbook build docs/` and confirm it succeeds.
- [x] 9.5 Add an entry to the next changelog section noting (a) the default behaviour change, (b) the `--no-rebase` opt-out, and (c) the `create_worktree` signature change for library consumers.
