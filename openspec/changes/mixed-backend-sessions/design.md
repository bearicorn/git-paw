## Context

v1.3.0 (`headless-backend-selection`) records one backend per session (`Session.backend`, absent
meaning tmux), builds one adapter per session from it, and refuses a `--backend` that contradicts
an existing session. Its design lists mixed-backend sessions as unspecced. Several v1.3.0 rules are
phrased per session:

- the discovery file: "for a headless session … `headless_agents`; for a tmux session neither";
- status: "for a tmux session its human output is byte-identical";
- the broker host's wake: "SHALL NOT run for a session on an interactive backend".

v0.18.0's liveness probe requires at least one live agent pane.

After `terminal-port-surface-capabilities`, interactivity and layout position are answered per
surface. After `remote-surface-adapter`, a third adapter exists for remote surfaces.

## Goals / Non-Goals

**Goals:**
- One session holds tmux, headless and remote workers; each is driven, gated, recorded, recovered
  and reported by its own backend.
- Single-backend sessions are byte-identical to before (session file, discovery file, tmux argv,
  human status).
- Every gate runs before anything is created, whatever the mix.

**Non-Goals:**
- Switching a worker's backend, on recovery or otherwise (v1.3.0 open question; still later).
- A tmux worker inside a headless session (there is no tmux session to hold its pane).
- A configuration default per worker, or a backend declared in a spec's front matter. The flag and
  MCP parameter are enough to start; see Open Questions.
- The remote gate, landing and recovery details (`remote-work-landing`).

## Decisions

### D1 — The session backend hosts; workers choose

The session's backend (v1.3.0's `--backend` / `default_backend`) keeps its meaning: it owns the
session, the broker host service and, on tmux, the dashboard. Each worker has a worker backend,
by default the session's. The only forbidden pairing is a tmux worker in a headless session.

*Alternatives rejected:*
- **A third value `--backend remote` for the whole session.** A session still needs a host for
  its broker; an all-remote session is expressed as `--backend headless` with every worker remote.
- **A "remote CLI" in `[clis.*]`.** It would reuse per-branch CLI selection, but a CLI record
  describes a local executable (launch command, approval flags, sandbox), none of which a vendor
  session has, and the headless gate is keyed on it.

### D2 — `--worker-backend BRANCH=BACKEND`

A repeatable value flag on `start` and a plain value on `add`. It names the backend in the same
vocabulary as `--backend`. A branch the launch does not create is an error (exit 1, before
anything), not a silent no-op.

### D3 — Persisted state: add, never repurpose

- `WorktreeEntry.backend: Option<WorkerBackend>`, written only when it differs from the session's.
- Discovery: `agents` keeps only tmux workers, so `pane_index` keeps its type and meaning and
  `sweep.sh` (which targets panes) is unchanged; `headless_agents` (v1.3.0) now appears whenever a
  headless worker exists; `remote_agents` is new.
- Status JSON: per-entry `backend` and `remote` (`vendor`, `session`, `url`, `state`, `following`,
  `tasks`).

### D4 — One routing backend

The factory becomes a router built from the session record: the session backend's adapter plus one
adapter per other worker backend present. A surface operation goes to the adapter whose `SurfaceId`
it is (the router keeps the mapping; ids stay opaque); session operations go to the session
backend; listing is the union; closing goes to every adapter. Consumers (drive loop, inventory,
status, lifecycle commands) keep talking to one `TerminalBackend`. For a single-backend session the
router holds one adapter and forwards every call unchanged, so tmux fixtures are untouched.

*Alternative rejected:* a per-worker lookup at every call site. It spreads the backend `match` across
the commands, which v1.3.0's design kept in one factory.

### D5 — Per-worker drive loop

The observation carries the worker's backend alongside the per-surface interactivity:

| Worker | Prompt / screen decisions | Turn decisions (v1.3.0) | Drive-loop text |
|---|---|---|---|
| tmux | yes | no | yes (verified nudge) |
| headless | no | yes | as a message |
| remote | no | no | never |

A remote worker's gate failure is still published as `agent.feedback`; the remote follow sends it
once as the verify continuation (`remote-work-landing`). The correction loop does not count it, so
there is one bound (one continuation, then an escalation) and not two.

### D6 — Wake per headless worker

The wake loop's decision reads each worker's backend: headless workers are woken as v1.3.0 says,
in any session; tmux workers never; remote workers never (a vendor session cannot read the broker
inbox, and the follow relays answers instead).

### D7 — Liveness counts every kind of worker

A tmux session whose agent panes have all exited is still alive while a headless worker is alive
or a remote worker is followed. Otherwise a session that offloaded its only branch to a vendor
would read `stopped` while the work goes on. Staleness (the tmux session itself is gone) is
unchanged.

## Risks / Trade-offs

- **[A headless worker in a tmux session escapes the headless gate]** → The gate runs per headless
  worker whatever the session's backend (v1.3.0's gate, reused unchanged).
- **[Drive-loop keystrokes reach a remote or headless surface]** → No: screen decisions are
  restricted to tmux workers, and the port refuses interactive-only operations on other surfaces.
- **[Byte-identical regressions]** → Golden fixtures for the session file, the discovery file,
  tmux argv and human status for tmux-only sessions.
- **[`sweep.sh` sees fewer agents in a mixed session]** → Intended: it targets panes. `git paw
  status` covers every worker; the docs say so.

## Migration Plan

1. Worker backend kind, `WorktreeEntry.backend`, discovery `remote_agents`; single-backend fixtures
   unchanged.
2. The routing backend; lifecycle commands through it.
3. `--worker-backend` and the gate pass (headless reused; remote refused as "not yet available"
   until `remote-work-landing` lands, a task-level stopgap, not a spec rule).
4. Drive-loop observation and wake per worker; status; liveness.
5. MCP parameter; docs; manifest entries.

Rollback: revert. A mixed session created meanwhile needs this release to stop or purge; older
binaries ignore `WorktreeEntry.backend` and would treat every worker as the session's backend.

## Open Questions

- **A default per worker** (`[workers] backend`, or a spec front-matter key). *Recommendation:*
  wait for dogfood; Fleet's git-paw mode will pass the flag or the MCP parameter.
- **An interactive picker for worker backends.** *Recommendation:* not until a default exists.
