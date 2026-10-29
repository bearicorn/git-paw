## Context

What exists today:
- **Liveness.** `tmux::session_liveness_for(name, created_at, offset)` returns `Alive`, `Stale`
  or `Indeterminate`. It requires a live agent CLI once the launch grace period has passed
  (*Liveness reflects dead agent panes*). `DisplayStatus::from_receipt` maps it to
  active, paused, stopped or stale.
- **Pure precedent.** `tmux::session::agents_without_live_pane(agents, live_pane_paths)` is a
  pure JSON↔terminal comparison, used only on the `add` path.
- **Doctor Hygiene** (`doctor::probe_hygiene` / `check_hygiene`) has three checks: `.gitignore`;
  `session state` (stale receipt); and `worktree registrations` (recorded paths missing on
  disk). Both session checks offer the remedy `git paw purge --stale`, which only acts on stale
  sessions.
- **Start.** `main.rs::invalidate_if_stale` purges an `active` receipt whose terminal is gone,
  including worktrees and created branches, before launching. A `stopped` receipt is instead
  recovered by `recover_session`, which rebuilds surfaces from the record and resumes workers
  through their `cli_session_id`.
- **Discovery file.** `write_repo_discovery_file` is called by `cmd_start` (bare),
  `cmd_supervisor`, `cmd_add` and `cmd_remove`, and not by `cmd_start_with_specs`,
  `recover_session` or `restart_from_pause`.
- **Constraints.** `session-state` *Liveness probe is cheap* requires that the receipt-staleness
  check used by `status`, `start` and `purge --stale` is a single liveness call. `doctor` is
  read-only by requirement.

After v0.18, terminal facts come from the `TerminalBackend` port (session liveness and the
surface listing with each surface's owning worktree; the pre-draft names these `session_alive`,
`surfaces` and `owner_of`, and MILESTONE names them `liveness` and `list_surfaces`). Broker
facts come from the broker client against the separately hosted broker.

## Goals / Non-Goals

**Goals:**
- Make the record truthful again after a crash or an external change, so `git paw start`
  recovers instead of discarding.
- Report every drift in one place, with a correct remedy.
- Keep repairs to git-paw's own records.

**Non-Goals:**
- Automatic self-healing on every command. `start`'s existing stale invalidation is unchanged.
- Restarting the broker host or relaunching agents from `doctor`.
- Deleting user data.
- Reconciling another repository's sessions.
- Config repair.

## Decisions

### D1 — Observe, decide, apply
`session::reconcile` is pure:
`reconcile(&SessionRecord, Option<&DiscoveryRecord>, &Observations) -> Vec<Drift>` and
`repair_plan(&[Drift], &Observations) -> Vec<Repair>`.

`Observations` are plain data:
- `terminal: Liveness` (alive, agents-exited, gone, or indeterminate, matching the classification v0.17.0's `status-and-picker-truth` introduces), with
  `surfaces: Vec<(SurfaceId, Option<PathBuf>)>`;
- `worktrees_registered: Vec<(PathBuf, exists: bool)>`;
- `prune_dry_run: Option<Vec<PathBuf>>`;
- `broker: BrokerObs` (`Running { repo_root }`, `Unreachable`, `NotRecorded`, or
  `Indeterminate`);
- `untracked_dirs` under `.git-paw/worktrees/`;
- `temp_files` (name and age).

Doctor gathers them through the ports and applies the plan through `SessionStore::update`.
Decisions are table-tested with fixtures and no fakes. This is the "functional core" rule from
`architecture-direction.md`.

An indeterminate observation never produces drift. That matches today's fail-safe that a probe
which could not run is never reported stale.

### D2 — Drift kinds and their handling

| Drift | Detected when | Severity | `--fix` repair |
|---|---|---|---|
| `SessionGone` | record `active`/`paused`, terminal session gone | ⚠ | set status `stopped` |
| `WorktreeMissing` | recorded `worktree_path` does not exist | ⚠ | if the terminal is gone: drop the entry (and its discovery-file row). If alive: report only, remedy `git paw stop` then `git paw doctor --fix` |
| `RegistrationStale` | git lists a git-paw-owned worktree (recorded, or under `.git-paw/worktrees/`) whose directory is gone | ⚠ | `git worktree prune`, only if every path in the `--dry-run` output is git-paw-owned; otherwise report only, remedy `git worktree prune` |
| `SurfaceMissing` | session alive, worktree exists, no surface owned by it | ⚠ | report only, remedy `git paw remove <b>` then `git paw add <b>` |
| `AgentsExited` | terminal session up, but every agent CLI has exited past the launch grace window (the three-way liveness from v0.17.0's `status-and-picker-truth`) | ⚠ | report only, remedy `git paw start` (relaunches the CLIs) or `git paw stop` |
| `UntrackedWorktree` | directory under `.git-paw/worktrees/` not in the record | ⚠ | report only, remedy inspect then `git worktree remove <path>` |
| `BrokerUnreachable` | session alive, not paused, broker recorded, `/status` does not answer | ⚠ | report only, remedy `git paw pause` then `git paw start` (restart-from-pause recreates the broker host) |
| `BrokerForeign` | `/status` answers for another repository | ✗ | report only, remedy set a distinct `[broker] port` |
| `DiscoveryStale` | session alive and the discovery file is missing, or its agents differ from the record | ⚠ | rewrite it from the record plus the backend's surface listing |
| `TempLeftover` | `<session>.json.*.tmp` older than 60 s in the sessions dir | ⚠ | delete those files |
| `NewerSchema` | this repository's session file declares a newer schema | ✗ | report only, remedy upgrade git-paw |

A drift kind with nothing to report produces a ✓ check, so the group always lists what was
checked. Check ids follow `uniform-json-output`: `session.gone`, `session.worktree-missing`, and
so on.

### D3 — `--fix` scope and safety bounds
Repairs write only:
- the session file and the discovery file, inside `SessionStore::update`;
- git's worktree metadata, through `git worktree prune`, only after the dry-run subset check;
- git-paw-named temp files.

`--fix` never removes a worktree directory, never deletes or moves a branch, never kills or
spawns a process (including the broker host or an agent CLI), never changes config, and never
acts on another repository's session.

`WorktreeMissing` drops an entry only when the terminal session is gone. A live surface in a
deleted directory needs a human decision, and dropping its record would orphan the surface.

`SessionGone` → `stopped` is the crash-recovery step: `git paw start` then takes the existing
recovery path (rebuilding surfaces and resuming workers by `cli_session_id`) instead of
`invalidate_if_stale`'s purge.

*Alternatives:*
- A separate `git paw repair` command. That adds a verb to freeze, while MILESTONE says
  "extends doctor".
- Auto-reconciling on `status` or `start`. That would violate *Liveness probe is cheap* and
  turn read commands into writers.
- Auto-restarting the broker host from doctor. That makes doctor an actuator (open question 2).

### D4 — Output and exit code
Without `--fix`, behavior is unchanged in kind: read-only, with a ✗ meaning a non-zero exit.

With `--fix`, doctor:
1. runs all checks;
2. computes and applies the repair plan;
3. prints each repair (`fixed: …`);
4. re-runs the Session group and reports the re-check;
5. exits non-zero if any ✗ remains.

`--fix --json` adds
`repairs: [{id, action, target, outcome: "applied" | "skipped" | "failed", detail}]` to the
document, which is additive under `cli-machine-output`. A repair that fails is reported, and the
remaining repairs still run.

### D5 — Hygiene keeps only `.gitignore`
The `session state` and `worktree registrations` checks move to the Session group as
`SessionGone` and `WorktreeMissing` / `RegistrationStale`, with correct remedies. The Hygiene
requirement is MODIFIED rather than duplicated.

### D6 — The prune dry-run format is to be verified
`git worktree prune --dry-run --verbose` prints one line per registration it would remove. The
exact text ("Removing worktrees/<id>: …") must be verified against git ≥ 2.30 during
implementation, and the parser accepts the stable `worktrees/<id>` token, mapping `<id>` to a
path through `git worktree list --porcelain` (*to verify*).

## Risks / Trade-offs

- [`--fix` marks a session `stopped` whose terminal was only briefly unreachable] → an
  indeterminate liveness never drifts, and only a determinate "gone" does. A stopped record of a
  live session would show as recoverable, and `start` reattaches when the session is alive.
- [`git worktree prune` touches a user's own stale registration] → the dry-run subset gate. If
  anything non-git-paw would be pruned, `--fix` skips the prune and reports.
- [Repair races a concurrent command] → every write is under the session lock, and doctor
  re-reads under the lock before applying.
- [The Session group duplicates `status`'s stale display] → they share one liveness fact.
  `status` stays the cheap view and doctor the thorough one.

## Migration Plan

1. Add the pure `reconcile` / `repair_plan` with table tests.
2. Add the doctor Session group (read-only) and slim Hygiene.
3. Add `--fix` with the repairs, the safety e2e tests, and JSON `repairs`.
4. Docs.

Rollback is a revert. Repaired records are ordinary session files.

## Open Questions

1. Should `--fix` drop a missing worktree's entry from a *live* session after stopping its
   orphaned surface? Recommended: no (report only) for v0.21.0.
2. Should `--fix` restart a dead broker host for a live session through the restart-from-pause
   routine? Recommended: no for v0.21.0. Keep doctor a records-only repairer and revisit with
   v0.18 broker-host dogfood evidence.
