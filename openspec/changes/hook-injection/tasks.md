## 1. WatchTarget and broker API

- [ ] 1.1 Define `pub struct WatchTarget { pub agent_id: String, pub worktree_path: PathBuf }` in `src/broker/mod.rs`
- [ ] 1.2 Update `start_broker` signature to accept `watch_targets: Vec<WatchTarget>`
- [ ] 1.3 Update all `start_broker` call sites to pass watch targets (empty vec when no worktrees known)

## 2. Git-status watcher

- [ ] 2.1 Create `src/broker/watcher.rs` module
- [ ] 2.2 Implement `async fn watch_worktree(state: Arc<BrokerState>, target: WatchTarget, cancel: CancellationToken)`
- [ ] 2.3 Run `git status --porcelain` at 2-second intervals using `tokio::process::Command`
- [ ] 2.4 Parse stdout into a sorted `Vec<String>` of paths (second field of each porcelain line)
- [ ] 2.5 Compare current snapshot to previous snapshot; publish only on change
- [ ] 2.6 Call `delivery::publish_message` with `agent.status { status: "working", modified_files: [...] }`
- [ ] 2.7 Add `pub mod watcher;` to `src/broker/mod.rs`
- [ ] 2.8 Spawn one watcher task per `WatchTarget` in `start_broker`
- [ ] 2.9 Ensure watchers stop when `BrokerHandle` is dropped (shutdown signal or tokio task abort)

## 3. Git hook installation

- [ ] 3.1 Create `pub fn install_git_hooks(worktree: &Path, broker_url: &str, agent_id: &str) -> Result<()>` in `src/agents.rs`
- [ ] 3.2 Generate `post-commit` hook script with pre-expanded broker URL and agent_id
- [ ] 3.3 Generate `pre-push` hook script that exits 1 with error message
- [ ] 3.4 Check for existing hooks before writing — if present, chain them (append git-paw hook after existing content)
- [ ] 3.5 Make hook files executable (`chmod +x` on Unix)
- [ ] 3.6 Handle worktree `.git` file (worktrees use `.git` file pointing to main repo's `.git/worktrees/<name>/`) — reuse `resolve_git_dir`
- [ ] 3.7 Call `install_git_hooks` from the session setup path when broker is enabled

## 4. Session launch wiring

- [ ] 4.1 In `cmd_start` and `launch_spec_session`: build `Vec<WatchTarget>` from worktree entries
- [ ] 4.2 In `cmd_dashboard` (pane 0): read session via `find_session_for_repo`, build `Vec<WatchTarget>`, pass to `start_broker`
- [ ] 4.3 Call `install_git_hooks` for each worktree when broker is enabled

## 5. Update coordination.md

- [ ] 5.1 Remove "MUST publish agent.status" requirement
- [ ] 5.2 Remove the status curl command from the required section
- [ ] 5.3 Add note about automatic status publishing
- [ ] 5.4 Keep agent.blocked and agent.artifact (with exports) curl commands as opt-in
- [ ] 5.5 Keep cherry-pick instructions and messages reference

## 6. Unit tests

- [ ] 6.1 Test: watcher publishes status when `git status --porcelain` reports new files
- [ ] 6.2 Test: watcher does NOT publish when snapshot is unchanged between ticks
- [ ] 6.3 Test: watcher respects `.gitignore` (file in ignored dir does not trigger publish)
- [ ] 6.4 Test: watcher maps worktree path to correct agent_id
- [ ] 6.5 Test: post-commit hook script content contains correct broker URL and agent_id
- [ ] 6.6 Test: pre-push hook script exits 1
- [ ] 6.7 Test: existing hook is preserved (chained) when installing
- [ ] 6.8 Test: coordination.md does NOT contain "MUST publish agent.status"
- [ ] 6.9 Test: coordination.md still contains agent.blocked and agent.artifact curl commands

## 7. Integration test

- [ ] 7.1 Test: start session with broker enabled, create file in worktree, verify broker receives agent.status
- [ ] 7.2 Test: commit in worktree, verify broker receives agent.artifact with committed files
- [ ] 7.3 Test: attempt `git push` in worktree, verify blocked by pre-push hook

## 8. Quality gates

- [ ] 8.1 `cargo fmt` clean
- [ ] 8.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 8.3 `cargo test` all pass
- [ ] 8.4 `just check` full pipeline green
- [ ] 8.5 `just deny` clean

## 9. Handoff readiness

- [ ] 9.1 Confirm `src/broker/watcher.rs` exists with git-status polling logic
- [ ] 9.2 Confirm git hooks installed in worktrees during session start
- [ ] 9.3 Confirm coordination.md updated to reflect automated status
- [ ] 9.4 Commit with message: `feat(broker): add git-status watcher and git hooks for auto-publishing`
