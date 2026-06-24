## Why

Agent worktrees are created as **siblings** of the repo (`../<project>-<branch>/`), scattering them outside the project root, polluting the parent directory, and making per-project permission grants awkward (an agent's worktree lives outside the repo a client/CLI is scoped to). Claude Code and similar tools keep worktrees *inside* the project. v0.8.0's headline feature moves agent worktrees into the repo at `.git-paw/worktrees/<branch-slug>/`, configurable, defaulting to the contained layout for new repos while preserving the sibling layout for existing ones.

## What Changes

- **New `worktree_placement` config** (`"child" | "sibling"`). `child` creates worktrees at `<repo>/.git-paw/worktrees/<branch-slug>/`; `sibling` keeps the v0.7.0 `../<project>-<branch>/` behaviour.
- **`git paw init` writes `worktree_placement = "child"`** into the generated config for new repos and gitignores `.git-paw/worktrees/` (as it already does for `.git-paw/tmp/`). So new repos get the contained layout out of the box.
- **Absent-config fallback stays `sibling`** — existing repos/sessions created before this field (whose worktrees already live as siblings and whose session JSON records the real paths) are unaffected until they re-init or opt in.
- `create_worktree` (and purge/remove teardown) resolve the worktree path from the configured placement; the session JSON continues to record the concrete path so resume/status/purge work regardless of placement.
- Enables a contained, project-scoped permission model: one grant for `.git-paw/worktrees/` instead of scattered sibling dirs.

## Capabilities

### New Capabilities
- `worktree-embedded-placement`: the `worktree_placement` setting, the child-vs-sibling path resolution, init's child-default + gitignore seeding, and the absent-config sibling fallback.

### Modified Capabilities
- `git-operations`: `create_worktree` resolves the target path from the configured placement (was always sibling).
- `configuration`: new `worktree_placement` field (optional; default-on-absent = sibling).
- `project-initialization`: `git paw init` writes `worktree_placement = "child"` and gitignores `.git-paw/worktrees/`.

## Impact

- Affected code: `src/git.rs` (`create_worktree` path resolution + teardown), `src/config.rs` (`worktree_placement` field), `src/init.rs` (default + gitignore), `src/session.rs` (path recording unchanged but verify it round-trips both layouts).
- Docs: configuration reference (`worktree_placement`), README/feature list, mdBook worktree/placement notes.
- Backward compatible: pre-existing configs (no field) → sibling, identical to v0.7.0; existing sessions resume from their recorded sibling paths. New repos → child.
