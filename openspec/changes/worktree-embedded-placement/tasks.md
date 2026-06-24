## 1. Configuration field

- [ ] 1.1 Add an optional `worktree_placement` field to `PawConfig` in `src/config.rs` accepting `"child"` / `"sibling"`, defaulting (on absent) to `sibling`, with `#[serde(default, skip_serializing_if = ...)]` so default-valued configs round-trip without the field.
- [ ] 1.2 Wire `worktree_placement` into the repo-overrides-global merge as a scalar (repo wins).
- [ ] 1.3 Provide an accessor that returns the effective placement (`sibling` when absent).

## 2. Path resolution in git operations

- [ ] 2.1 In `src/git.rs`, resolve the `create_worktree` target path from the configured placement: `child` → `<repo_root>/.git-paw/worktrees/<branch-slug>` (creating `.git-paw/worktrees/` if absent); `sibling`/absent → `<repo_parent>/<project>-<branch-slug>`.
- [ ] 2.2 Derive `<branch-slug>` for the child layout from the branch name only (`/`→`-`, strip chars outside `[A-Za-z0-9._-]`), without the project prefix; reuse the existing branch sanitisation helper.
- [ ] 2.3 Keep the rest of `create_worktree` (rebase-onto-main, idempotent existence check, `git worktree add` fallback) unchanged — only the resolved target path varies.
- [ ] 2.4 Confirm purge/remove teardown operates on the concrete path recorded in the session JSON (placement-agnostic); adjust only if a code path re-derives the path from config.

## 3. Init default + gitignore

- [ ] 3.1 In `src/init.rs`, write an active `worktree_placement = "child"` into the generated default config for new repos.
- [ ] 3.2 Add `.git-paw/worktrees/` to the managed `.gitignore` entry set, appended only if absent and never duplicated on repeated init.

## 4. Session round-trip

- [ ] 4.1 Verify `src/session.rs` records the concrete worktree path produced by `create_worktree` for both layouts (no placement marker added to the session).
- [ ] 4.2 Confirm resume/status/purge read the recorded path and do not re-derive it from `worktree_placement`.

## 5. Tests

- [ ] 5.1 Config unit tests: `worktree_placement` parses for `child` and `sibling`; absent defaults to `sibling`; repo overrides global; round-trips through save/load; pre-existing config without the field loads without error.
- [ ] 5.2 git integration tests: child placement creates worktree at `<repo_root>/.git-paw/worktrees/<slug>`; sibling and absent both create at `<repo_parent>/<project>-<slug>`; child slug derivation for slash and unsafe-character branches.
- [ ] 5.3 Session round-trip tests: child-layout session save→reload→purge removes the worktree at the recorded child path; sibling-layout session does the same at the sibling path; config-flip case purges at the recorded path, not a re-derived one.
- [ ] 5.4 Init tests: generated config contains `worktree_placement = "child"`; `.gitignore` contains `.git-paw/worktrees/` after init; the entry is not duplicated on repeated init.

## 6. Docs

- [ ] 6.1 Configuration reference: document `worktree_placement` (`child` default for new repos, `sibling` default-on-absent), the child path `.git-paw/worktrees/<branch-slug>`, and the manual-gitignore note for repos that opt in without re-running init.
- [ ] 6.2 README/feature list: note the contained worktree layout as the v0.8.0 headline.
- [ ] 6.3 mdBook worktree/placement chapter: explain child vs sibling, the project-scoped permission benefit, and that existing sessions stay at their recorded paths.
- [ ] 6.4 `mdbook build docs/` succeeds.

## 7. Quality gates

- [ ] 7.1 `just check` (fmt + clippy + all tests) passes.
- [ ] 7.2 `just deny` passes.
- [ ] 7.3 No `unwrap()`/`expect()` in non-test code; all public items have doc comments.
- [ ] 7.4 Backward compatibility verified: a v0.7.0 config (no `worktree_placement`) and existing sibling sessions load and resume unchanged.
