## 1. Worktree seeding

- [ ] 1.1 In `attach_agent` (`src/main.rs`), after helper provisioning: seed `<worktree>/.claude/settings.json` with helper prefixes (`setup_curl_allowlist`, broker/docs gating) and dev patterns (`setup_dev_allowlist`, `common_dev_allowlist` gating); create `.claude/` as needed; non-fatal warnings on failure
- [ ] 1.2 Recovery flow: loop restored worktrees with the same seeding, before prompt injection
- [ ] 1.3 Worktree-local ignore: append `.claude/` to the worktree's `info/exclude` (resolve via `git -C <worktree> rev-parse --git-path info/exclude`); idempotent; warn-and-continue on failure

## 2. Tests

- [ ] 2.1 Start seeds every worktree (patterns + helper prefixes present); add seeds the new worktree; recovery re-seeds
- [ ] 2.2 Merge preserves pre-existing custom entries; dedup holds
- [ ] 2.3 Gating: dev feature disabled → no dev patterns; broker disabled → no broker prefix
- [ ] 2.4 `git status` in a seeded worktree shows no `.claude/` entries (exclude effective); no tracked `.gitignore` modified
- [ ] 2.5 Works for embedded (`.git-paw/worktrees/`) and sibling worktree placements

## 3. Docs

- [ ] 3.1 Configuration reference: worktree seeding note under `[supervisor.common_dev_allowlist]` and the broker helper section
- [ ] 3.2 Coordination/user-guide note: agents no longer prompt on preset-safe commands
- [ ] 3.3 `mdbook build docs/` passes
