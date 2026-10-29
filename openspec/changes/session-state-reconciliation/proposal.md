## Why

`session.json` is git-paw's record of a session, and the world changes underneath it: the
terminal session is killed outside git-paw, a worktree is `rm -rf`'d by hand, the broker host
dies, or a crash leaves the record claiming `active`. Today each consumer notices a different
fraction of this, and nothing makes the record true again:
- `status` and `doctor`'s Hygiene group flag a stale receipt. Hygiene's "worktree registrations"
  check flags missing worktree directories, but always offers `purge --stale` as the remedy,
  which does nothing for a live session.
- `git paw add` reports agents with no live pane, but only on the add path.
- `start` silently *purges* an `active` receipt whose terminal is gone, worktrees and branches
  included, even when the user wanted the session back.
- The per-repo discovery file is not rewritten by `--from-specs` start, recovery or
  restart-from-pause, so it drifts from the session file.
- `doctor` cannot repair anything, by requirement ("Diagnose-only (no `--fix` in v0.13.0)").

MILESTONE carries this item from the v0.13 intent so that the frozen v1.0.0 baseline can be
trusted.

## What Changes

- **A pure reconciliation decision.** `reconcile(recorded, observed) -> Vec<Drift>` compares
  the session record (and the discovery file) against observations gathered through ports:
  - the terminal backend: session liveness, and the live surfaces with the worktree each one
    is in;
  - version control: registered worktrees and whether each directory exists;
  - the broker client: whether `/status` answers, and for which repository;
  - the filesystem: the discovery file and leftover temp files.

  An indeterminate observation, such as the terminal backend being unavailable, yields no
  drift. The cheap staleness probe that `status`, `start` and `purge --stale` use is unchanged.
- **A doctor "Session" group** reports each drift kind with a remedy:
  - session gone while recorded active or paused;
  - terminal session up but every agent CLI exited;
  - recorded worktree missing;
  - stale git-paw worktree registration;
  - recorded agent without a live surface;
  - untracked directory under `.git-paw/worktrees/`;
  - broker unreachable, or serving another repository;
  - discovery file out of sync;
  - leftover temp files;
  - session file from a newer schema.

  Hygiene keeps only the `.gitignore` check. Its session checks move here, with correct
  remedies.
- **`git paw doctor --fix` repairs git-paw's own records only, under the session lock:**
  - a dead `active`/`paused` record is marked `stopped`, so `git paw start` recovers it instead
    of purging it;
  - entries for missing worktrees are dropped (only when the session is not alive);
  - git-paw-owned stale registrations are pruned (only when `git worktree prune`'s dry run
    touches nothing else);
  - the discovery file is rewritten for a live session;
  - git-paw's own leftover temp files are deleted.

  It never deletes a worktree directory or a branch, never kills or spawns a process, never
  touches a registration git-paw does not own, and never edits config. Everything else is
  report-only with a remedy. After repairing, doctor re-checks, and the exit code reflects the
  re-check. `--fix --json` adds a `repairs` array.
- `doctor` without `--fix` stays strictly read-only.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `session-state`: ADDED the reconciliation decision and its drift taxonomy, and the rule that
  a stopped-by-repair session is recovered, not purged, by `start`.
- `preflight-diagnostics`: ADDED the Session group, the `--fix` repair set and its safety
  bounds, and the `repairs` array in JSON. MODIFIED *`git paw doctor` runs read-only preflight
  checks* (read-only without `--fix`) and *Hygiene check* (session checks move to the Session
  group). REMOVED *Diagnose-only (no `--fix` in v0.13.0)*.

## Impact

- **Code:**
  - new `src/session/reconcile.rs` (pure: `Observations`, `Drift`, `reconcile`,
    `repair_plan`), generalizing `tmux::session::agents_without_live_pane`;
  - observation gathering in `src/doctor.rs` through the v0.18 `TerminalBackend`, the git
    worktree listing (`git worktree list --porcelain`, `git worktree prune --dry-run`) and the
    broker client;
  - repairs in `src/doctor.rs` through `SessionStore::update`;
  - `src/cli.rs` (`doctor --fix`);
  - `probe_hygiene` and `check_hygiene` slim down.
- **Tests:**
  - a reconcile decision table: fixture observations in, drifts out;
  - doctor e2e per drift kind (detected, repaired, re-checked);
  - safety e2e: a dirty untracked worktree, a user's non-git-paw stale registration, and a live
    session's missing worktree are never touched;
  - `--fix --json` repairs;
  - read-only without `--fix`.
- **Docs:** `docs/src/cli-reference.md` (`doctor --fix`), the doctor user-guide section
  (Session group, what `--fix` will and will not do), the session-lifecycle chapter (crash
  recovery: `doctor --fix` then `start`), and `--help`.
- **Enum-variant ripple:** none.
- **Security / safety:** `--fix` is the first doctor mode that writes. Writes are confined to
  the session file and discovery file under the lock, `git worktree prune` gated on a
  dry-run subset check, and git-paw-named temp files. No process control, no deletion of user
  data, no network.
- **Cross-version dependencies:**
  - Requires `session-state-locking` (this release) for repairs.
  - Uses `wire-format-dtos` (this release) for newer-schema detection, and `uniform-json-output`
    (this release) for the `repairs` key and check ids.
  - Requires v0.18 `terminal-backend-seam`: terminal observations go through the
    `TerminalBackend` port's session liveness and surface-owner listing, never raw tmux.
  - Requires v0.18 `broker-host-separation`: the broker host is the thing whose reachability is
    observed, and restart-from-pause is the remedy that recreates it.
  - Builds on v0.17 `broker-repo-identity`: the repository-aware probe distinguishes a foreign
    broker.
- **Overlap with the other v0.21.0 track:**
  - *Session resume UX* (better recovery messaging on `start`) should point at
    `git paw doctor --fix` for drifted records.
  - v0.17.0's `status-and-picker-truth` (DS-9) introduces the gone / agents-exited / alive liveness (`SessionLiveness::AgentsExited`) and already makes doctor report a dead-panes session as ⚠ with the remedy `git paw start`. This change reports it as the `AgentsExited` drift with its `start`/`stop` remedy; its *Hygiene check* MODIFIED block is built on the archived v0.17.0 text.
  - If `config check` wants a `--fix` for config normalization, it is a separate repair domain;
    this change scopes `--fix` to session state.
