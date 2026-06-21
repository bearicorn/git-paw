## Why

`git paw remove <branch>` on a just-started, otherwise-clean agent worktree
intermittently fails with `worktree for '<b>' has uncommitted changes:` whose
listed file is git-paw's OWN injected coordination state — the gitignored
sidecar `.git-paw/AGENTS.local.md` written by `setup_worktree_agents_md`. This
is a v0.8.0 regression: the remove command's D7 uncommitted-work safety check
calls `git status --porcelain` and treats the injected sidecar as user dirt
because `exclude_from_git` (which writes the worktree `info/exclude` entry) runs
*after* the sidecar file is written, leaving a window where `git status` still
reports it. The user lost nothing; git-paw is refusing to remove a worktree
because of its own bookkeeping. Two e2e tests already flake on this and it
blocks the v0.9.0 dogfood loop.

## What Changes

- The `remove` uncommitted-work safety check SHALL exclude git-paw's own
  managed/injected files from the set of "dirty" files it counts and reports.
  Specifically the injected sidecar `.git-paw/AGENTS.local.md` and any residual
  managed `<!-- git-paw:start -->` block in `AGENTS.md` are git-paw injection,
  not user work, and SHALL NOT, on their own, cause `remove` to refuse.
- A worktree whose ONLY uncommitted entries are git-paw-managed files SHALL be
  treated as clean by `remove`: the pane closes, the worktree is removed, and
  the session entry is dropped without `--force`.
- A worktree containing ANY genuine user-authored uncommitted change SHALL
  still be refused without `--force`, and the refusal message SHALL list only
  the user files (managed files filtered out so the user sees what actually
  matters).
- Defense in depth: `exclude_from_git` SHALL write the `info/exclude` entry
  *before* the sidecar file is written so the file is excluded the moment it
  lands on disk, closing the race rather than relying solely on the filter.
- The two flaky e2e tests are made deterministic, and a regression test asserts
  the clean-but-injected case succeeds while the genuine-edit case is still
  refused.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `remove-branch`: the "Uncommitted-work safety" requirement is modified so the
  dirty-check ignores git-paw's own managed/injected files (the injected
  sidecar and the managed AGENTS.md block) while still refusing on genuine user
  edits.
- `worktree-agents-md`: the "Exclude worktree AGENTS.md from git" requirement
  is modified so the sidecar exclude entry is written BEFORE the sidecar file is
  created, closing the write-then-exclude race (the `git-operations`
  `exclude_from_git` helper itself is unchanged; the ordering lives in
  `setup_worktree_agents_md`, which `worktree-agents-md` governs).

## Impact

- `src/main.rs` (`remove` command, ~line 2575): filter the `uncommitted_files`
  result against the git-paw-managed set before the refusal check.
- `src/git.rs`: `uncommitted_files` is unchanged (still reports raw status); the
  filtering lives at the `remove` call site or a small helper. `exclude_from_git`
  / `setup_worktree_agents_md` ordering tightened so the exclude entry precedes
  the sidecar write.
- `src/agents.rs` (`setup_worktree_agents_md`, ~line 297/311): reorder so
  `exclude_from_git(SIDECAR_REL_PATH)` runs before `fs::write(sidecar)`.
- Tests: `tests/add_remove_e2e.rs::remove_clean_agent_detaches_and_updates_session`
  and `tests/session_orchestration_robustness_e2e.rs::remove_middle_agent_kills_only_that_pane`
  de-flaked; new regression coverage for managed-only-vs-genuine-edit.
- No config, CLI surface, or wire-format changes. `--force` and `--keep-worktree`
  semantics are unchanged.
