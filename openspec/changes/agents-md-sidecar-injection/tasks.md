# Tasks — agents-md-sidecar-injection

## 1. Injection target → gitignored sidecar
- [ ] 1.1 Define the sidecar instruction path constant (e.g. `.git-paw/AGENTS.local.md`) in `src/agents.rs`
- [ ] 1.2 Change `setup_worktree_agents_md()` (`src/agents.rs:246`) to build the combined view (root `AGENTS.md` + assignment section via `inject_into_content`) and write it to the sidecar, creating the `.git-paw/` directory if absent — NOT the tracked `AGENTS.md`
- [ ] 1.3 Ensure the combined content is byte-identical to today's worktree-`AGENTS.md` output (markers, skill, inter-agent rules unchanged)

## 2. Stop hiding the tracked AGENTS.md
- [ ] 2.1 Remove the `assume_unchanged(worktree_root, "AGENTS.md")` call at `src/agents.rs:281`
- [ ] 2.2 Remove the `exclude_from_git(worktree_root, "AGENTS.md")` call at `src/agents.rs:274`
- [ ] 2.3 Add a self-healing clear: run `git update-index --no-assume-unchanged AGENTS.md` on start so stale bits from older git-paw versions are removed and the tracked file becomes committable
- [ ] 2.4 Update the `setup_worktree_agents_md` doc comment (the "two layers of protection" block at `src/agents.rs:236-245`) to describe the sidecar approach

## 3. Point the CLI instruction file at the combined view
- [ ] 3.1 Ensure the CLI the worktree launches reads the combined sidecar content (pass the sidecar path where a flag exists, else place/point the auto-loaded instruction file at the sidecar)
- [ ] 3.2 Verify `build_task_prompt` (`src/main.rs:332`) guidance still resolves to a file the agent actually reads (update target path if needed)

## 4. Gitignore the sidecar
- [ ] 4.1 Add the sidecar path to the worktree ignore set (`exclude_from_git(worktree_root, "<sidecar path>")` or `.gitignore`), idempotently
- [ ] 4.2 Confirm `.git-paw/` (or the chosen sidecar location) is gitignored at the repo level so the ephemeral injection is never committed

## 5. Tests
- [ ] 5.1 Update `setup_worktree_root_exists` (`src/agents.rs:1088`): assert the tracked `AGENTS.md` is NOT `assume-unchanged` and that a hand edit to it appears in `git status --porcelain`
- [ ] 5.2 Add a test: a hand edit to the tracked `AGENTS.md` stages via `git add -A` and commits
- [ ] 5.3 Add a test: the managed `<!-- git-paw:start -->` block is present in the sidecar and reaches the agent's effective instruction view (combined = root + block)
- [ ] 5.4 Add a test: the tracked `AGENTS.md` does NOT contain a git-paw block written by git-paw, and `AGENTS.md` is NOT in `.git/info/exclude`
- [ ] 5.5 Add a test: the sidecar path IS in the worktree ignore set (not committable)
- [ ] 5.6 Add a test: a stale `assume-unchanged` bit on `AGENTS.md` set before setup is cleared after `setup_worktree_agents_md`
- [ ] 5.7 Adjust `setup_worktree_root_missing` / `setup_worktree_replaces_root_section` to read the sidecar instead of the worktree `AGENTS.md`

## 6. Docs
- [ ] 6.1 Update the worktree/AGENTS.md mdBook chapter(s) under `docs/src/` to describe the gitignored sidecar and that the tracked `AGENTS.md` is now committable mid-session
- [ ] 6.2 Note the v0.7.0 footgun resolution (finding F10) where appropriate
- [ ] 6.3 `mdbook build docs/` succeeds

## 7. Gates
- [ ] 7.1 `just check` (fmt + clippy + all tests) passes
- [ ] 7.2 `just deny` passes
- [ ] 7.3 No `unwrap()`/`expect()` in non-test code; all public items documented
- [ ] 7.4 Backward compat verified: an existing session recovers on next `git paw start` (sidecar re-injected, stale assume-unchanged cleared)
