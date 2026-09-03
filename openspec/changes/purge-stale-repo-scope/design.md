## Context

`git paw purge --stale` sweeps a global receipt store and, run in one repo, purged
another repo's stale session (peer-4-poker). Stale-only today, but the cross-repo scope
is undocumented and the delete blast radius is unbounded — a probe misfire could take a
live session in an unrelated repo. This narrows the default scope and hardens the
liveness gate.

## Goals / Non-Goals

**Goals:** the default `--stale` blast radius is the current repository; a live session
is never purged in any repo, even under liveness uncertainty; a machine-wide sweep is
explicit.

**Non-Goals:** changing the receipt-store layout (it stays global on disk); session-state
reconciliation (v0.20).

## Decisions

- **Default to current-repository scope.** Filter stale receipts to the current
  repository (reuse session-by-repository resolution) before purging. This is a
  deliberate, safety-motivated narrowing of the previous machine-wide default — the
  surprising cross-repo delete is exactly the bug.
- **Machine-wide behind `--all-repos`.** The capability is preserved but explicit, so a
  global sweep is a conscious choice, never a side effect of running purge in a repo.
- **Fail-safe liveness gate.** A receipt is purged only when the probe positively
  confirms it is stale; if liveness cannot be determined, the session is left intact.
  Alternative rejected: purge-on-uncertainty (the data-loss path).

## Risks / Trade-offs

- **A user relied on the machine-wide default** → they add `--all-repos`; the safety win
  (no accidental cross-repo delete) outweighs the one-flag friction, and it is documented.

## Migration Plan

Behavioural narrowing; no receipt-format change. Existing receipts load. The only visible
change: `--stale` alone no longer reaches other repos; `--all-repos` restores it.

## Open Questions

None.
