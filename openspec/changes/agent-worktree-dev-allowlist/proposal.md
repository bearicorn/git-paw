## Why

Dogfood learning 2026-07-07 (6× friction in one wave): coding agents prompt on every preset-safe command (`git status`, `grep`, `sed -n`, `find`) because no allowlist ever reaches their worktree. All seeding today targets `<repo>/.claude/settings.json` plus configured home-level `settings_path` files (`seed_supervisor_session`, `src/supervisor/dev_allowlist.rs:300-326`; `setup_curl_allowlist` call sites in `src/main.rs`) — but a claude-format CLI resolves PROJECT settings from its working directory, and each agent's cwd is its own worktree, whose git root is the worktree itself. The repo-root file never applies there; home-level files depend on optional config and pre-existing directories. Meanwhile the CLI's own "don't ask again" pins the full varying command string, so it never matches the next invocation. v0.10.0's `worktree-helper-provisioning` fixed the same class of gap for helper SCRIPTS; this fixes it for the allowlists.

## What Changes

- **Per-worktree allowlist seeding**: at agent attach (shared by `start` and `add`, alongside helper provisioning in `attach_agent`) and at session recovery, merge the allowlists into `<worktree>/.claude/settings.json` for every agent worktree:
  - the helper-path prefixes (broker.sh / sweep.sh / docs-fetch.sh, per `curl-allowlist`), under the same broker/docs gating as the repo-root seeding;
  - the resolved dev-command patterns (universal + named stacks + `extra`, per `dev-command-allowlist`), when `[supervisor.common_dev_allowlist]` is enabled.
- **Version-control hygiene**: the seeder ensures `.claude/` is excluded via the WORKTREE-LOCAL ignore (`info/exclude`) so an agent's `git add .` can never commit the seeded file; tracked `.gitignore` is never edited.
- Merge semantics identical to the existing targets (preserve entries, dedup, fail non-fatal with a warning).
- Repo-root and configured `settings_path` seeding continue unchanged.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `dev-command-allowlist`: ADDED requirement — per-worktree placement for agent panes (start/add/recovery, worktree-local ignore, enable-gating).
- `curl-allowlist`: ADDED requirement — helper-path allowlist seeded into each agent worktree under the existing gating.

## Impact

- `src/main.rs` `attach_agent` (single per-worktree setup point, `~:991`) + the recovery flow; `src/supervisor/dev_allowlist.rs` / `curl_allowlist.rs` reused as-is (they already take a settings path).
- Worktree-local `info/exclude` handling (worktrees resolve to `<repo>/.git/worktrees/<name>/info/exclude`).
- Tests: worktree file contents after start/add/recovery, merge-preserve, disabled-feature gating, exclude entry, embedded (`.git-paw/worktrees/`) and sibling placements.
- Docs: configuration reference + coordination chapter note (agents stop prompting on preset-safe commands).
- Backward compatible: purely additive writes into agent worktrees git-paw creates; consumer repos see no tracked-file changes.
