## Why

A worker branch is rebased onto the default branch exactly once — at worktree creation,
and only if the branch already existed (`create_worktree`'s `rebase_onto_main`,
`src/git.rs:412-414`, `461-467`). Nothing refreshes it again. Over a long wave the
supervisor merges peer after peer into the base while every live worker keeps committing
on the baseline it started from, so each branch drifts further from what it will
eventually be merged into.

The only existing recourse is worker-side and advisory: the `agent.advanced-main` event
plus the *When main advances* discipline, which explicitly and correctly tells the worker
**not** to auto-rebase but to fetch, inspect, and decide. That is the right rule for an
agent holding uncommitted work — but it means refresh happens only when a worker chooses
to act, and a worker deep in a task usually does not. The peer-4-poker dogfood (GP-18)
showed the end state: a worker that branched at `939dc1c` and never refreshed, whose
branch had to be reconciled by hand.

Removing the peer cherry-pick shortcut (`peer-cherry-pick-removal`) closes the unsafe
workaround workers were using to pull peer work forward. This change supplies the safe
replacement on the supervisor side, where the merge order is actually known.

## What Changes

- The supervisor MAY, when explicitly enabled, rebase an **idle, clean** worker branch
  onto the default branch after a successful merge — turning branch refresh from a
  worker judgment call into a supervisor operation performed at a moment when it is
  provably safe.
- The operation is gated by a conjunction of preconditions, ALL of which must hold; if
  any fails the branch is left untouched and the attempt is recorded rather than retried
  destructively:
  1. the worker's working tree is clean (no uncommitted changes),
  2. the worker's pane is not mid-response,
  3. the supervisor holds the pane claim (the existing TOCTOU gate),
  4. the rebase is predicted conflict-free,
  5. the branch is actually behind the default branch.
- On a rebase that fails or conflicts, the existing `git rebase --abort` recovery applies
  and the branch is left at its pre-rebase HEAD — the change adds no new failure mode
  beyond what `create_worktree`'s rebase already handles.
- After a successful refresh the worker is told its HEAD moved, so it does not keep
  reasoning about files at stale SHAs.
- **Default off.** History rewriting under a live agent is opt-in.

**Not breaking:** the feature is disabled by default; with it disabled, behaviour is
identical to today. `create_worktree`'s creation-time rebase is unchanged.

## Capabilities

### New Capabilities

- `supervisor-branch-refresh`: the supervisor-side, precondition-gated rebase of idle
  worker branches onto the default branch after a merge, its safety gates, its failure
  handling, and how the refreshed worker is notified.

### Modified Capabilities

- `git-operations`: a rebase entry point usable against a live worker branch
  mid-session, separate from `create_worktree`'s creation-time
  `rebase_onto_main` path. The existing creation-time contract is unchanged.

## Impact

- **Code:** `src/git.rs` — expose a mid-session rebase built on the existing
  `rebase_branch_onto_default_with` (`:276`), which already runs the rebase inside the
  worktree where the branch is checked out and already aborts + restores on failure.
  `src/supervisor/` — the post-merge hook that evaluates the gates and invokes it;
  reuse `PaneClaim` (`src/supervisor/claim.rs:62`) for gate 3 and the existing
  mid-response detection (`MID_RESPONSE_MARKERS`, `src/supervisor/drive.rs:1786`) for
  gate 2. `src/config/supervisor.rs` — the opt-in field.
- **Cleanliness signal:** the broker watcher already tracks each agent's
  `modified_files`; gate 1 should read that rather than introduce a second dirty-state
  probe, so the supervisor and the detector cannot disagree about whether a worktree is
  clean.
- **Enum-variant ripple: NONE, deliberately.** The worker notification reuses
  `agent.feedback` with a distinguishing bracket tag (the pattern the conflict detector
  already uses via `[conflict-detector]`, `src/broker/conflict.rs`) rather than adding a
  `BrokerMessage` variant. Adding a variant would ripple through `messages.rs`,
  `dashboard/broker_log.rs`, `learnings.rs`, and `delivery.rs` per the AGENTS.md
  enum-ripple checklist — disproportionate for a notification. `agent.advanced-main`
  is NOT reused: it means "the base moved", whereas this means "your own HEAD was
  rewritten", and conflating them would corrupt the worker-side discipline that keys
  off it.
- **Interaction with `skill-agent`:** the worker-side *When main advances* rule ("do NOT
  auto-rebase") remains correct and unchanged — it governs the **worker**. This change
  gives the **supervisor** a rebase it performs only when the worker demonstrably holds
  no uncommitted work, which is precisely the condition that makes the worker-side
  prohibition necessary.
- **Backward compatibility:** new config field with `#[serde(default)]` and
  `skip_serializing_if`; existing configs load unchanged and the feature stays off.
- **Docs:** a supervisor user-guide section on branch refresh and its gates; the
  configuration reference entry for the new field; `--help` if a flag is added.
