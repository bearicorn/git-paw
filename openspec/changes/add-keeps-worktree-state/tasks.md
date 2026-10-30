## 1. Reproduce

- [ ] 1.1 Integration test (throwaway repo, stub CLI, live tmux session): `remove feat/x --keep-worktree` on a worktree with a modified tracked file and an untracked file, advance the default branch, then `add feat/x --cli <stub-b>`; assert it currently fails with `rebase onto main failed` (the GP-87 repro, which becomes the passing swap test in 3.1)

## 2. `--no-rebase` on `add`

- [ ] 2.1 `src/cli.rs`: add `no_rebase: bool` (`long`, `default_value_t = false`, a `help` string matching `start`'s wording) to the `add` arguments; add a `--no-rebase` example to `add`'s `long_about`
- [ ] 2.2 `src/commands/add.rs`: thread the flag into the add options and set `AttachContext.no_rebase` from it in place of the hard-coded `false`
- [ ] 2.3 Parse tests: `add feat/x --no-rebase --cli my-agent` gives `no_rebase == true`; `add feat/x` gives `false`; `git paw add --help` lists `--no-rebase`

## 3. Never rebase over uncommitted work

- [ ] 3.1 Make the porcelain lookup of the worktree that holds a branch (`find_worktree_for_branch_with` in `src/git.rs`) `pub(crate)` with no change in behaviour
- [ ] 3.2 In `add`, after taking the session lock and before `attach_agent`, when `--no-rebase` is not set: find the worktree holding the branch; if there is one, run `git::uncommitted_files` on it and drop entries for which `agents::is_managed_path` is true; if any remain, set `no_rebase = true` and write the skip notice through `out.warn` (stderr) naming the branch and `--no-rebase`
- [ ] 3.3 When `attach_agent` fails with a `rebase onto main failed` worktree error, add a `--no-rebase` hint to the error `add` returns, keeping the original text
- [ ] 3.4 Integration tests: the dirty swap completes (exit 0, HEAD unchanged, files byte-identical, `git stash list` unchanged, notice on stderr only); managed-files-only worktree is rebased with no notice; dirty worktree with `--no-rebase` prints no notice; a clean kept worktree is rebased by default and unchanged with `--no-rebase`; a conflicting clean worktree fails naming `--no-rebase` with no pane spawned and the session JSON unchanged
- [ ] 3.5 Confirm `create_worktree` and `start` tests pass unchanged (the `git-operations` and `cli-parsing` contracts are untouched)

## 4. Spec traceability

- [ ] 4.1 Map every scenario in `specs/git-add-remove-branch` to a named test

## 5. Docs

- [ ] 5.1 `docs/src/cli-reference.md` `git paw add`: refresh the usage block with `--no-rebase`, add the dirty-worktree skip, and give the CLI-swap recipe `git paw remove <b> --keep-worktree` then `git paw add <b> --cli <other> --no-rebase`
- [ ] 5.2 `docs/src/user-guide/session-lifecycle.md`: under "Rebases that don't happen", add `add --no-rebase` and an `add` onto a worktree with uncommitted changes; extend the `git paw add` section with the swap recipe
- [ ] 5.3 README CLI table, if it lists `add`'s flags

## 6. Quality gates

- [ ] 6.1 `cargo fmt`, then `just lint`
- [ ] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 6.3 `just deny`
- [ ] 6.4 `mdbook build docs/`
- [ ] 6.5 `openspec validate add-keeps-worktree-state --strict`
