## 1. Pin the conformance failures with tests

- [ ] 1.1 Add a test asserting that after a spec-driven launch without supervisor mode, each agent worktree contains an executable `.git-paw/scripts/broker.sh` — SHALL fail before the fix
- [ ] 1.2 Add the equivalent assertion for the supervisor launch and for `git paw add`
- [ ] 1.3 Add a test asserting a bundled helper resolves the active session when invoked from the repository root with no multiplexer environment — SHALL fail before the fix
- [ ] 1.4 Add a test asserting an empty repo-relative `.git-paw/sessions/` directory does not cause discovery to report no session

## 2. Authoritative session-state location

- [ ] 2.1 Confirm `session::session_state_dir()` already resolves the location the new requirement names, on both macOS and the XDG fallback; do NOT move it
- [ ] 2.2 Record the pre-freeze audit debt (the stale `.git-paw/sessions/…` mentions in `broker-watcher-and-state` and `approval-pattern-surfacing`) where the v0.21.0 spec audit will find it

## 3. Helper session discovery

- [ ] 3.1 Update `discover_session_name` / `discover_session_file` in `assets/scripts/sweep.sh` to resolve the platform data directory, handling both the macOS and XDG layouts
- [ ] 3.2 Keep the `$TMUX` inference as a secondary fallback, consulted after the authoritative lookup
- [ ] 3.3 Make "no session found" an explicit message rather than a silent degradation to empty output
- [ ] 3.4 Sync the tracked `.git-paw/scripts/sweep.sh` copy and run `bash -n` on both files

## 4. Worktree helper provisioning

- [ ] 4.1 Provision the bundled helper scripts into `<worktree>/.git-paw/scripts/` from `provision_worktree`, the seam every launch path already calls
- [ ] 4.2 Source the content from `init.rs`'s existing embedded constants rather than adding a second `include_str!` set, so the worktree copy cannot drift from the init copy
- [ ] 4.3 Set the executable bit; make the write idempotent so re-attach refreshes rather than fails
- [ ] 4.4 Gate `broker.sh` on the broker being enabled and `docs-fetch.sh` on `docs_base_url` being configured, per the requirement
- [ ] 4.5 Verify no launch path bypasses `provision_worktree`; if one does, fix the bypass rather than duplicating the provisioning call

## 5. Spec traceability

- [ ] 5.1 Verify every scenario in `specs/session-state/spec.md` maps to at least one test
- [ ] 5.2 Verify every scenario in `specs/broker-agent-helper/spec.md` maps to at least one test
- [ ] 5.3 Confirm the previously-unimplemented provisioning requirement now has covering tests, so the same conformance gap cannot reopen silently

## 6. Docs and gates

- [ ] 6.1 Document that worktree helper scripts are refreshed from the running binary on every attach, so local edits inside a worktree's `.git-paw/scripts/` are not preserved
- [ ] 6.2 Document that bundled helpers can be run from the repository root, not only from inside the session
- [ ] 6.3 `cargo fmt`, then `just lint`
- [ ] 6.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code rather than piped output
- [ ] 6.5 `openspec validate bundled-helper-discovery --strict`
