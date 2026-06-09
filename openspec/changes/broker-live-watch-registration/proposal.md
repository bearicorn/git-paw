# Live broker watch-target registration for hot-added agents

## Why

`git paw add` (the `git-paw-add` capability, v0.6.0) hot-attaches a
new worktree + pane to a running session. The new agent self-registers
with the broker via its boot-block `curl /publish` — so a **real** CLI
appears in `/status` fine. But the broker's filesystem-watcher targets
are fixed at `git paw start` (seeded once from the session JSON), and
the broker exposes no endpoint to add a target at runtime. So the
watcher **fallback** — which surfaces an agent from worktree activity
even before/without a self-published status — does **not** cover a
hot-added worktree.

The v0.6.0 dogfood (2026-06-09) confirmed this: a hot-added agent
driven by a non-self-registering CLI never appeared in `/status`,
because neither the boot block (that CLI didn't curl) nor the watcher
(worktree not a target) surfaced it. For real CLIs this is invisible
(the boot block registers them), but the watcher fallback should cover
hot-added worktrees too, for parity with start-time agents.

## What Changes

- **ADDED** a broker endpoint `POST /watch` that registers a new
  filesystem watch target (agent id + worktree path + cli) on the
  running broker, so the watcher begins surfacing that worktree's
  activity immediately.
- `git paw add` POSTs the new worktree to `/watch` after creating it,
  so a hot-added agent has watcher coverage identical to a start-time
  agent — it appears in `/status` from worktree activity even if its
  CLI has not yet self-published.
- `git paw remove` SHOULD correspondingly deregister the target (or
  the broker prunes a target whose worktree disappears).

## Impact

- Target release: **v0.7.0** (follow-up to v0.6.0's `git-paw-add`).
- Affected specs: `broker-endpoints` (new `/watch` route), and a
  cross-reference from `git-paw-add` / `filesystem-watcher`.
- Affected code (when implemented): `src/broker/server.rs` (route),
  `src/broker/mod.rs` (live target list), `src/main.rs` (the `add`
  call site).
- Backward compatible — a broker without the endpoint, or an `add`
  that does not POST, behaves exactly as v0.6.0 (CLI self-registration
  only). This change only adds the watcher-fallback coverage.
