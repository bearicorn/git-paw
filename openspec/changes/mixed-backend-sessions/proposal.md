## Why

The point of remote workers is to offload some branches while others keep running locally. v1.3.0
fixed one backend per session and listed mixed-backend sessions as unspecced. This change specs
them: one session holding tmux, headless and remote workers, each treated by its own backend. It
comes before the vendor adapter and the landing work so that those changes plug into a session
model that already routes per worker.

## What Changes

- **A backend per worker.** Each worker is `tmux`, `headless` or `remote`, defaulting to the
  session's backend (`--backend` / `default_backend`, unchanged). A tmux worker needs a tmux
  session; headless and remote workers fit either. A worker's backend never changes.
- **Selection.** `git paw start --worker-backend <BRANCH>=<BACKEND>` (repeatable),
  `git paw add --worker-backend <BACKEND>`, and MCP `start_session.worker_backends`.
- **One gate pass before anything is created.** Every headless worker passes v1.3.0's headless
  eligibility gate whatever the session's backend; every remote worker passes the remote gate
  (`remote-work-landing`); refusals from both are reported together, exit 1, nothing created.
- **Recorded per worker.** `WorktreeEntry.backend`, written only when it differs from the session's.
  The discovery file lists tmux workers in `agents` (so `pane_index` and `sweep.sh` are unchanged),
  headless workers in `headless_agents` and remote workers in a new `remote_agents`.
- **One routing backend.** Built from the session record: surface operations go to the adapter that
  created the surface; session operations go to the session's backend; listing and closing span
  every adapter; `attach` prints remote URLs, then attaches as before.
- **Per-worker drive loop.** Screen predicates and prompt escalation for tmux workers only; v1.3.0's
  turn decisions for headless workers only; no drive-loop text to remote workers (their gate
  failures go to the remote verify continuation).
- **Broker host.** The inbox wake runs for headless workers in any session, never for tmux or
  remote workers (MODIFIED from v1.3.0's per-session rule).
- **Status.** Per-worker backend and state in human output (tmux-only sessions byte-identical);
  JSON gains per-entry `backend` and `remote`.
- **Liveness.** A live headless or remote worker keeps a tmux session `Active` when its panes have
  exited (MODIFIED from v0.18.0's pane-only rule).

## Capabilities

### New Capabilities

- `session-mixed-backend`: per-worker backends — choice and combination rules, the per-backend
  gates before creation, the session-file field, the routing backend, the drive loop and the
  lifecycle commands per worker.

### Modified Capabilities

- `session-state`: the discovery file lists each worker under its backend; status reports each
  worker's backend and state; liveness counts live headless and remote workers (all MODIFIED).
- `broker-runtime`: the inbox wake runs per headless worker, in any session (MODIFIED).
- `cli-parsing`: `--worker-backend` on `start` and `add` (added).
- `mcp-server`: `start_session` accepts `worker_backends` (added).

## Impact

- **MODIFIED requirement headers and the text each is based on:**
  - *The per-repo JSON describes headless workers additively* — `feat/v1.3.0-specs`,
    `openspec/changes/headless-backend-selection/specs/session-state/spec.md` (ADDED there).
  - *Status reports a headless session's backend and worker activity* — same file on
    `feat/v1.3.0-specs`.
  - *Liveness reflects dead agent panes, not just session existence* — archived text on
    `feat/v0.18.0-specs`, `openspec/specs/session-state/spec.md`. No pending change modifies it.
  - *The broker host wakes an idle headless worker for a directed message* — `feat/v1.3.0-specs`,
    `openspec/changes/headless-drive-adaptation/specs/broker-runtime/spec.md` (ADDED there).
- **Builds on:**
  - this release: `terminal-port-surface-capabilities` (per-surface interactivity, optional
    positions); `remote-surface-adapter` (the remote adapter, the follow, `WorktreeEntry.remote`).
  - v1.3.0: all three changes (the headless adapter, drive adaptation, selection and its gate,
    `Session.backend`, `headless_agents`, status `activity`).
  - v0.18.0 (locked): `terminal-backend-seam`, `broker-host-separation` (`POST /watch`
    registration), `drive-loop-functional-core`, `mcp-orchestrator-write-tools` (`start_session`),
    `supervisor-pane-removal` (coding-agents-only `agents`).
  - v0.17.0 `supervisor-correction-loop` (re-engagement and cycle counting, which remote workers
    bypass); v0.21.0 `uniform-json-output` and `wire-format-dtos` (status JSON and wire DTOs);
    v0.21.0 `start-resume-summary` (one line per worker, which gains the backend).
- **Code:** the backend factory becomes a routing backend in `src/terminal/`; `src/cli.rs`
  (`--worker-backend`); `src/session.rs` (`WorktreeEntry.backend`; discovery `remote_agents`);
  `src/commands/{start,add,remove,recover,pause,stop,purge,status,attach}.rs` and their library
  forms; the drive loop's observation (worker backend); the `__broker` wake decision; the MCP
  `start_session` tool; the tmux liveness probe.
- **Enum-variant ripple:** none (`BrokerMessage` and `SpecBackendKind` untouched). The worker
  backend kind (`tmux`, `headless`, `remote`) is closed and matched only in the routing-backend
  construction and the wire mappers.
- **Contract:** additive only — `--worker-backend` and its values; MCP
  `start_session.worker_backends`; `WorktreeEntry.backend`; discovery `remote_agents`; status JSON
  entry `backend` and `remote`. Each is an additive manifest entry under v1.0.0's freeze. No exit
  code changes (refusal 1, bad value 2).
- **Security / safety:** a headless worker in a tmux session still passes the headless gate; no
  drive-loop keystroke ever reaches a remote surface. Threats in design.md.
- **Dependencies:** none.
- **Version notes:** "v1.6.0" is a working number for this release and appears only in prose.
