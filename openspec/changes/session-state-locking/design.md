## Context

Session files live at `dirs::data_dir()/git-paw/sessions/<session_name>.json` (`session.rs`,
private `sessions_dir()`). `save_session_in` writes `<name>.tmp` and renames it over
`<name>.json`, with no `fsync`. The read-modify-write sites:

| Site | Mutation | Lock today |
|---|---|---|
| `commands/stop.rs::cmd_stop` | `status = Stopped` | none |
| `commands/pause.rs::cmd_pause` | `status = Paused` | none |
| `commands/start.rs::restart_from_pause` | `status = Active`, `pending_boot_prompt.take()` | none |
| `commands/recover.rs::recover_session` | `status = Active`, `mode` | none |
| `commands/add.rs::cmd_add` | `worktrees.push` | `.add-remove.lock`, taken *after* the load |
| `commands/remove.rs::cmd_remove` | `worktrees.remove` | `.add-remove.lock`, taken *after* the load |

Create-only writes happen in `cmd_start`, `cmd_start_with_specs` and `cmd_supervisor`. Deletes
happen in `main.rs::purge_with_prompt`, which `cmd_purge_stale` and `invalidate_if_stale` also
reach. The per-repo discovery file is rewritten by `write_repo_discovery_file` from `start`,
`supervisor`, `add` and `remove`. The drive loop
(`supervisor/drive.rs::FileSessionInstanceGuard`), MCP, `status`, `attach`, `approvals`,
`doctor` and `init` only read.

`src/lock.rs::SessionLock` uses `OpenOptions::create_new`, removes the file on drop, and has no
owner or TTL.

## Goals / Non-Goals

**Goals:** no lost update between git-paw processes; no corruption through shared temp files; no
stale lock after a crash; no deadlock.

**Non-Goals:**
- Locking against non-git-paw editors of the file.
- Locking readers.
- Cross-machine (NFS) guarantees beyond what `flock` gives.
- Repairing drift. That is `session-state-reconciliation`.

## Decisions

### D1 — OS advisory lock via std, keyed per session
`SessionStore::lock(name)` opens (creating if absent) `<sessions_dir>/<name>.lock` and calls
`File::try_lock()`. On `WouldBlock` it retries every 50 ms for up to 10 s. On timeout it returns
`PawError::SessionError`:

```
session state for '<name>' is locked by another git-paw process (lock: <path>); retry when it finishes
```

The guard holds the `File`. Dropping it unlocks, and process death releases the lock in the
kernel, so a crash cannot leave a stale lock and no PID or TTL bookkeeping is needed. std
`File::lock` / `try_lock` has been stable since Rust 1.89, below the 1.96 MSRV, so there is no
new crate (`fs2` / `fd-lock` are not on the approved list).

*Alternatives:*
- Reuse the `create_new` marker pattern. It leaves a stale lock on crash; that is the current
  `.add-remove.lock` problem.
- A lock per repository rather than per session. The global file is per session, and two
  repositories never share one.

### D2 — Read-modify-write always re-reads under the lock
`SessionStore::update(name, |&mut Session| -> Result<T>) -> Result<T>` takes the lock, loads the
file, applies the closure, saves, and releases the lock. Callers no longer save a `Session`
they loaded earlier. `create` (refusing to clobber an existing different instance is the job of
the existing start logic) and `delete` take the same lock. Where the per-repo discovery file
must change, it is written inside the same `update`, after the session file and before the
lock is released.

### D3 — Unique temp file and flush
The temp name is `<name>.json.<pid>.<monotonic counter>.tmp`. The writer calls `sync_all()` on
the temp file before the rename, and on any error removes the temp file. `find_session_for_repo`
already ignores non-`.json` names, so a temp file left by a crash is never read as a session.
Leftover `*.tmp` files are reported (and removable) by `session-state-reconciliation`.

### D4 — The add/remove lock becomes OS-released and non-blocking
`lock::SessionLock::acquire(repo_root)` opens or creates `.git-paw/.add-remove.lock` and calls
`File::try_lock()`. On `WouldBlock` it returns the same "another `git paw add` / `git paw remove`
operation is in progress" error, which still names the file, without the `rm` hint. The file is
never removed.

`cmd_add` and `cmd_remove` take this lock **before** reading session state, and do their session
mutation through `SessionStore::update` inside it.

The lock order is the add/remove lock, then the session lock, and never the reverse. A session
lock is held only for the read-modify-write itself, never across tmux calls, readiness gates,
sleeps, or the drive loop. That rules out deadlock and keeps the 10 s wait theoretical in
practice.

### D5 — Lock files are never deleted
Unlinking a lock file while another process waits on it lets a third process lock a fresh inode,
so two holders would exist at once. Lock files are a few bytes each and persist. `purge` leaves
them alone.

## Risks / Trade-offs

- [`flock` behavior on network filesystems] → the session dir is the local user data dir. The
  repo-side add/remove lock on an NFS-mounted repo degrades to advisory-only, which is no worse
  than today.
- [A long-running holder makes others wait 10 s and fail] → the critical sections are
  milliseconds by construction (D4). The error names the lock file.
- [`sync_all` adds latency] → about one file per command. Negligible.
- [The unit tests that assert the lock file disappears on drop now fail] → they assert an
  implementation detail and are rewritten as behavioral tests: a released lock can be
  reacquired, and a dead holder releases.

## Migration Plan

1. Add the store and lock (unused), with unit tests.
2. Route the create/update/delete sites through the store, one command per commit, with the
   suite green after each.
3. Move `SessionLock` to `try_lock` and reorder `add`/`remove`.
4. Add the concurrency e2e tests.

Rollback is a revert. No persisted state changes; lock files are inert to an older binary.

## Open Questions

None. The wait bound (10 s) is a constant, not configuration. Make it configurable only if
evidence demands it.
