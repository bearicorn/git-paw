## 1. Failing tests first

- [ ] 1.1 After a launch, each agent worktree contains an executable `.git-paw/scripts/broker.sh` — supervisor, bare, spec-driven and `git paw add` paths (fails before the fix)
- [ ] 1.2 `sweep.sh`'s pane-to-agent resolution finds `feat-x` from a discovery file in the `agents` roster shape (fails before the fix)
- [ ] 1.3 `find_session_for_repo()` called with an agent worktree's root returns the repository's session (fails before the fix)

## 2. Worktree helper provisioning (GP-28)

- [ ] 2.1 Provision the bundled helpers into `<worktree>/.git-paw/scripts/` from `provision_worktree`, sourcing content from `init.rs`'s existing embedded constants
- [ ] 2.2 Set the executable bit; make it idempotent so a re-attach refreshes rather than fails
- [ ] 2.3 Gate `broker.sh` on the broker being enabled and `docs-fetch.sh` on `docs_base_url`
- [ ] 2.4 Confirm no launch path bypasses `provision_worktree`

## 3. Discovery-file shape (GP-42)

- [ ] 3.1 Make `resolve_agent_for_path` in `assets/scripts/sweep.sh` read the `agents` roster (`branch_id`, `worktree_path`)
- [ ] 3.2 Align `tests/sweep_sh_session_discovery.rs` and `tests/sweep_sh_tmux_fallback.rs` fixtures to the shape `write_repo_discovery_file` produces
- [ ] 3.3 Make "no session found" an explicit message; keep the `$TMUX` inference as a secondary path
- [ ] 3.4 Sync the tracked `.git-paw/scripts/sweep.sh`; `bash -n` both

## 4. Worktree-aware session lookup (GP-43)

- [ ] 4.1 In `find_session_for_repo_in`, resolve a linked worktree to its owning repository before comparing `repo_path`
- [ ] 4.2 Add a test that `git paw mcp`'s repository context, resolved from inside an agent worktree with no `--repo`, carries the session's broker URL
- [ ] 4.3 Leave `mcp::resolve_repo`'s behaviour for file operations unchanged

## 5. Docs and gates

- [ ] 5.1 Document that worktree helpers are refreshed from the running binary on each attach
- [ ] 5.2 Verify every scenario in `specs/` maps to a test
- [ ] 5.3 `cargo fmt`, then `just lint`
- [ ] 5.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 5.5 `openspec validate bundled-helper-discovery --strict`
