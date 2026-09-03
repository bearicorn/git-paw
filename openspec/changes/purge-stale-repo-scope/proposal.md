## Why

`git paw purge --stale` operates on a **global** receipt store: run inside one
repository, it purged a stale session belonging to a *different* repository
(peer-4-poker). It is stale-only today, but the cross-repo scope is undocumented and
unbounded — and if the staleness probe ever misfired, a global sweep could purge a
**live** session in another repository. That is a data-loss blast radius the operator
never opted into. The default blast radius should be the current repository, and the
"never purge a live session" guarantee should be fail-safe.

## What Changes

- `git paw purge --stale` is **scoped to the current repository by default** — it SHALL
  NOT purge or touch sessions belonging to any other repository.
- A machine-wide sweep of stale receipts across all repositories requires an explicit
  **`--all-repos`** opt-in, and even then purges only stale receipts.
- The live-never-purged guarantee is **fail-safe**: a session that is live per the probe
  — or whose liveness cannot be positively confirmed — SHALL NEVER be purged by
  `--stale`, in the current repository or any other.
- The cross-repo scope is documented.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `session-state`: `purge --stale` is repo-scoped by default with a fail-safe
  live-session guarantee; a global sweep is an explicit opt-in.
- `cli-parsing`: `purge` accepts an `--all-repos` flag (meaningful only with `--stale`).

## Impact

- **Code:** the `purge --stale` path — filter stale receipts to the current repository
  by default (reuse session-by-repository resolution); the `--all-repos` opt-in; make the
  liveness gate fail-safe (skip purge when liveness is indeterminate).
- **Backward compatibility:** a **deliberate safety narrowing** — the default `--stale`
  scope changes from machine-wide to current-repository; the machine-wide sweep moves
  behind `--all-repos`. Live sessions were, and remain, never purged.
- **Enum-variant ripple:** none.
- **Security/Safety:** removes an unbounded cross-repo delete blast radius; Gate-5 note.
- **Docs:** the purge chapter documents the repo scope, `--all-repos`, and the fail-safe
  live guarantee.
