## 1. Managed-path classification (git-operations/agents)

- [ ] 1.1 Add a `is_managed_path(worktree_root: &Path, rel: &str) -> bool` helper in `src/agents.rs` (co-located with `SIDECAR_REL_PATH`) that returns `true` for `SIDECAR_REL_PATH` (`.git-paw/AGENTS.local.md`), and `true` for `AGENTS.md` only when the on-disk `AGENTS.md` still carries a `<!-- git-paw:start -->` block AND is otherwise unmodified vs HEAD (any user hunk outside the block → `false`). Doc-comment the function.
- [ ] 1.2 Add a unit test in `src/agents.rs` asserting `is_managed_path` returns `true` for the sidecar path, `true` for a managed-block-only `AGENTS.md`, and `false` for `src/foo.rs` and for an `AGENTS.md` with a user edit outside the managed block.

## 2. Filter the remove dirty-check (remove-branch)

- [ ] 2.1 In `src/main.rs` remove command (~2575), after `git::uncommitted_files(&target.worktree_path)`, partition the returned paths into git-paw-managed vs residual user files using `is_managed_path`.
- [ ] 2.2 Refuse only when the residual (user) list is non-empty; build the refusal message from the residual list only so managed files (`.git-paw/AGENTS.local.md`, the managed block) are never listed. When residual is empty, proceed exactly as the clean path (no `--force` required).
- [ ] 2.3 Verify `--force` still removes regardless and `--keep-worktree` still bypasses the check entirely (no behavioral change to those flags).

## 3. Close the write-then-exclude race (worktree-agents-md)

- [ ] 3.1 In `src/agents.rs::setup_worktree_agents_md`, move `exclude_from_git(worktree_root, SIDECAR_REL_PATH)?` to run BEFORE `fs::write(&sidecar, …)` so the sidecar path is excluded before the file exists. Keep the `.git-paw/` parent-dir creation before the exclude/write as needed.
- [ ] 3.2 Update the doc comment on `setup_worktree_agents_md` to describe the exclude-before-write ordering and why (race fix, defense in depth).

## 4. De-flake the e2e tests + regression coverage

- [ ] 4.1 Confirm `tests/add_remove_e2e.rs::remove_clean_agent_detaches_and_updates_session` now passes deterministically (start → immediate remove of a clean agent succeeds) with the filter+reorder in place; remove any now-unneeded retry/sleep workaround.
- [ ] 4.2 Confirm `tests/session_orchestration_robustness_e2e.rs::remove_middle_agent_kills_only_that_pane` passes deterministically, including under `cargo llvm-cov` (the race window the instrumentation widened is gone).
- [ ] 4.3 Add a regression e2e test: `start` a single clean agent, immediately `git paw remove <branch>` WITHOUT `--force`, assert success, pane closed, worktree gone, session entry dropped — and assert the command output does NOT mention `.git-paw/AGENTS.local.md`.
- [ ] 4.4 Add a regression test: an agent worktree with a genuine user edit (e.g. `src/foo.rs`) PLUS the injected sidecar is still refused without `--force`; assert exit non-zero, that `src/foo.rs` is listed, and that `.git-paw/AGENTS.local.md` is NOT listed.
- [ ] 4.5 Add a unit/integration test for the reorder: after `setup_worktree_agents_md`, a `git status --porcelain` in the worktree does NOT report the sidecar (Scenario "Sidecar is excluded the moment it is written").

## 5. Quality gates

- [ ] 5.1 Run `just check` (fmt + clippy + all tests); no `unwrap()`/`expect()` in non-test code; all public items documented.
- [ ] 5.2 Serialize the E2E remove tests if needed (`#[serial]`) and confirm they pass in isolation and under coverage.
- [ ] 5.3 Run `just deny`; confirm no new dependencies were added.
- [ ] 5.4 Run `openspec validate "remove-dirty-check-ignores-managed-files" --strict` and confirm it reports valid.
