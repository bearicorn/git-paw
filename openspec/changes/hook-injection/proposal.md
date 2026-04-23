## Why

v0.3.0 and v0.4.0 dogfooding proved that AI agents never proactively publish broker status messages, even with explicit "MUST publish" instructions in their AGENTS.md coordination skill. The curl commands are treated as passive documentation, not active workflow. The dashboard shows "No agents connected" for the entire session.

This change replaces the "agents should curl" model with automated publishing driven by git state polling and git hooks, which works transparently across all CLIs.

## What Changes

### Git-status watcher (primary — all CLIs)

- Extend the broker process in pane 0 to poll each worktree's git state using `git status --porcelain`
- On each poll tick, diff the current status against the previous snapshot; if it changed, publish `agent.status` with `modified_files` listing the paths reported by git
- Poll interval: 2 seconds (acts as the debounce window — rapid edits within a tick collapse into one publish)
- `git status --porcelain` honours `.gitignore`, so `target/`, `node_modules/`, and other build artefacts are excluded automatically without extra configuration
- `.git/` internal state is never reported because git does not list it in status output
- Map worktree path → agent_id using the slugified branch name
- No new external dependency: git is already a hard runtime requirement for git-paw

### Git hooks (universal — all CLIs that use git)

- During worktree setup (`setup_worktree_agents_md`), install git hooks:
  - `post-commit`: publish `agent.artifact` with `modified_files` from `git diff HEAD~1 --name-only` and `status: "committed"`
  - `pre-push`: exit 1 with message "agents must not push — the supervisor handles merges"
- Hooks call the broker via the pre-expanded URL (no shell expansion needed)
- Hooks are executable shell scripts written to `.git/hooks/` in each worktree
- Cleanup: `purge` removes the hooks along with the worktrees

### Updated coordination.md

- Remove the "MUST publish agent.status" instructions (now automated by the watcher)
- Keep the `agent.blocked` and `agent.artifact` (done with exports) curl commands as opt-in actions the agent can take when it knows something the watcher can't detect
- Keep the cherry-pick instructions and messages-you-may-receive reference
- Document that status publishing is automatic: "git-paw publishes your status automatically when you edit files and commit. You only need to publish manually if you are blocked or done with specific exports."

## Capabilities

### New Capabilities

- `filesystem-watcher`: Poll worktree git state and auto-publish `agent.status` on detected changes
- `git-hook-injection`: Install post-commit and pre-push hooks in worktree `.git/hooks/`

### Modified Capabilities

- `agent-skills`: Update coordination.md to reflect automated status publishing
- `worktree-agents-md`: Install git hooks during worktree setup

## Impact

- **No new dependencies.** Uses the existing `git` runtime dependency and `std::process::Command`.
- **Modified files:** `src/broker/mod.rs` (add watcher loop to broker process), `src/agents.rs` (install git hooks), `assets/agent-skills/coordination.md` (update instructions)
- **New code:** git-status polling loop, git hook script generation, hook installation/cleanup
- **Depends on:** broker infrastructure from v0.3.0, worktree setup from v0.2.0
