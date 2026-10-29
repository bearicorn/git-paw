## 1. Store and lock (unused, unit-tested)

- [ ] 1.1 Add `session::store` (`SessionStore::{lock, create, update, delete}`), with the lock on `<sessions_dir>/<name>.lock` via `File::try_lock`, retried every 50 ms up to 10 s, and the timeout error naming the session and the lock path
- [ ] 1.2 `save_session_in`: unique temp name (`<name>.json.<pid>.<counter>.tmp`), `sync_all` before rename, temp removed on any error; keep v0.18's `0600` mode if `broker-verdict-authority` has landed
- [ ] 1.3 Lock files are never deleted by `delete` / `purge`

## 2. Route every writer through the store (one command per commit; suite green after each)

- [ ] 2.1 `cmd_stop`, `cmd_pause` → `SessionStore::update`
- [ ] 2.2 `restart_from_pause`, `recover_session` → `update`
- [ ] 2.3 `cmd_start`, `cmd_start_with_specs`, `cmd_supervisor` → `create`
- [ ] 2.4 `purge_with_prompt` (and through it `cmd_purge_stale` and `invalidate_if_stale`) → `delete`
- [ ] 2.5 `write_repo_discovery_file` runs inside the same `update` / `create` closure, after the session file

## 3. add/remove

- [ ] 3.1 `lock::SessionLock::acquire` uses `File::try_lock` on a persistent `.git-paw/.add-remove.lock`; same "in progress" error naming the file; drop the `rm` hint
- [ ] 3.2 `cmd_add` / `cmd_remove` take the add/remove lock before `find_session_for_repo`, and mutate via `SessionStore::update`
- [ ] 3.3 Assert by review and comment that no session lock is held across tmux calls, readiness gates or sleeps (the lock order is documented on `SessionStore`)

## 4. Tests (one or more per scenario)

- [ ] 4.1 `tests/session_lock_concurrency.rs` (isolated HOME/XDG): two child processes doing RMW on one session (one adds a worktree entry, the other flips status) many times in a loop; the final file reflects both
- [ ] 4.2 A SIGKILLed lock holder (helper child that locks then sleeps): the next `update` succeeds
- [ ] 4.3 `git paw status` completes while another process holds the session lock
- [ ] 4.4 A lock held for longer than 10 s makes `git paw pause` fail with the session name and lock path, leaving the file unchanged
- [ ] 4.5 Concurrent saves leave one complete file and no `*.tmp`; an injected rename failure leaves no temp file
- [ ] 4.6 After concurrent add/remove-style mutations the discovery file's agents match the session file's worktrees
- [ ] 4.7 `lock.rs`: a second concurrent acquire errors naming `.add-remove.lock`; after a SIGKILLed holder the acquire succeeds; release then reacquire works (replaces the "file removed on drop" assertion)
- [ ] 4.8 `remove` preserves a `paused` status written between invocation and lock acquisition (pause runs while the remove test holds a pre-lock barrier)
- [ ] 4.9 Existing suites unchanged: `tests/add_remove_e2e.rs`, `pause_e2e.rs`, `recover_integration.rs`, `session_stale_hygiene.rs`, `session::tests`

## 5. Docs

- [ ] 5.1 `docs/src/architecture.md`: the session store, the per-session lock, the lock order, and that lock files persist by design
- [ ] 5.2 Remove the stale-`.add-remove.lock` `rm` troubleshooting advice wherever it appears in `docs/` and README

## 6. Gates

- [ ] 6.1 `cargo fmt`, then `just lint`
- [ ] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 6.3 `just deny` (no new crate)
- [ ] 6.4 `mdbook build docs/`
- [ ] 6.5 `openspec validate session-state-locking --strict`
