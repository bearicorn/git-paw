## Why

Every mutation of the global session file is an unguarded read-modify-write:
`find_session_for_repo` → clone and mutate → `save_session`. This happens in
`stop` (status → Stopped), `pause` (→ Paused), `restart_from_pause` (→ Active, taking
`pending_boot_prompt`), `recover_session` (→ Active, `mode`), `add` (pushes a worktree) and
`remove` (drops a worktree). Two git-paw processes interleaving lose one update. Examples: a
`git paw add` racing a `git paw pause`; a `remove` racing the `--unattended` drive loop's
operator in another terminal; the v0.18 MCP lifecycle tools racing the CLI. Three things make
it worse:
- `add` and `remove` load the session *before* taking their `.add-remove.lock`, so even they
  can act on a stale copy;
- `save_session_in` writes every session through one fixed temp name (`<name>.tmp`), so two
  concurrent writers truncate each other's temp file before the rename;
- `.add-remove.lock` is a create-if-absent marker with no owner, so a crashed `add` leaves it
  behind until a human runs the `rm` its error message prints.

This is the prevention half of the robustness item. `session-state-reconciliation` is the
repair half.

## What Changes

- **A per-session exclusive lock around every session-file write.** A `session::store` API runs
  each create, read-modify-write and delete under an advisory exclusive lock on
  `<sessions_dir>/<session>.lock`, using std's `File::lock` family (`flock` on Unix). The
  operating system releases the lock when its holder exits, so a crash never leaves a stale
  lock. A read-modify-write always re-reads the file *under* the lock. Acquisition waits up to
  a bounded time (10 s) and then fails with an error naming the session and the lock file.
  Readers stay lock-free, because the write is an atomic rename.
- **Writers never share a temp file.** Each write uses a temp name unique to the process and
  the call, flushes the file data before the rename, and leaves no temp file behind on failure.
- **The per-repo discovery file is written under the same lock** as the session file it is
  derived from, so the two cannot interleave.
- **The `add`/`remove` lock is released by the OS, and `add`/`remove` read under it.** The lock
  becomes a non-blocking `File::try_lock` on a persistent `.git-paw/.add-remove.lock`. The
  contention error is unchanged ("operation in progress", naming the file). `add` and `remove`
  take it before reading session state. A previously crashed holder no longer blocks the next
  `add`.
- **Lock order is fixed:** the repo's add/remove lock first, then the session lock. No session
  lock is held across terminal operations, sleeps, or the drive loop's sweep.

No new dependency (std `File::lock` is stable since Rust 1.89; the MSRV is 1.96). No change to
any persisted byte or to the CLI surface.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `session-state`: ADDED the per-session write lock with re-read under lock, bounded wait,
  unique temp files with flush, and the discovery file written under the lock.
- `git-add-remove-branch`: ADDED the OS-released add/remove lock, and session state read only
  under it.

## Impact

- **Code:**
  - `src/session.rs`, or a new `src/session/store.rs` (`SessionStore::{create, update, delete}`
    and the lock guard); `save_session_in` (unique temp file plus `sync_all`);
  - `src/lock.rs` (`SessionLock` moves onto `File::try_lock`, and the file persists);
  - `src/commands/{add,remove,stop,pause,start,recover,supervisor}.rs` and
    `src/main.rs` (`purge_with_prompt`, `cmd_purge_stale`, `invalidate_if_stale`,
    `write_repo_discovery_file`) route through the store.
  - After v0.18, the `cmd_*` library functions and the MCP lifecycle tools inherit it by
    calling the same store.
- **Tests:**
  - concurrency e2e: two processes doing read-modify-write on one session lose no update;
  - a killed lock holder does not block the next writer;
  - timeout error text;
  - temp-file uniqueness;
  - `lock.rs` tests rewritten for OS release, since the file now persists after drop by design;
  - every existing session and add/remove test unchanged.
- **Docs:** `docs/src/architecture.md` (session store and lock order); the user guide's
  troubleshooting note on the old stale `.add-remove.lock` `rm` hint (now unnecessary).
- **Enum-variant ripple:** none.
- **Security / safety:** lock files live in the user-private data dir and `.git-paw/`, which is
  gitignored. Lock files are never deleted, which avoids the unlink-while-waiting race. No new
  input surface.
- **Cross-version dependencies:**
  - Builds on the archived v0.17.0 `test-session-state-isolation` (concurrency tests must use an
    isolated HOME/XDG data dir).
  - Must preserve v0.18 `broker-verdict-authority`'s `0600` session-file mode in the new write
    path.
  - v0.18 `cmd-library-surface` and the MCP lifecycle tools become store callers.
  - v0.18 `broker-host-separation` writes no session state.
  - `wire-format-dtos` (this release) changes what is serialized inside the lock, not the
    locking. Either order works.
  - `session-state-reconciliation` depends on this change for its repairs.
- **Overlap with the other v0.21.0 track:** `git paw finish` removes a worktree and rewrites
  session state, so it must use this store and take the add/remove lock first.
