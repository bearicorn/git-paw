## Context

The terminal port as v0.18.0 archived it (`feat/v0.18.0-specs`, `src/terminal/mod.rs`) has these
members: `open_session`, `session_alive`, `close_session`, `set_session_env`,
`liveness(session, created_at, first_agent)`, `surface_at(session, position) -> SurfaceId`,
`list_surfaces -> Vec<SurfaceInfo { id, position: usize, owner: Option<PathBuf> }>`,
`surface_owned_by`, `spawn_surface(session, spec, placement)`, `kill`, `label_surface`,
`label_of`, `send_text`, `prefill_text`, `relaunch_surface`, `answer_dialog`, `send_key`,
`capture_tail -> Option<String>`, `foreground_process -> Option<String>`, `stream_to`, `layout`,
`reflow_rows`, `attach` and `detach_clients`. `broker-host-separation` adds `spawn_service`,
`stop_service` and `service_alive`. v0.18.0 is locked (user decision, 2026-10-28).

Where the port assumes a local process with a screen:

| Member | Assumption |
|---|---|
| `SurfaceInfo.position: usize`, `surface_at -> SurfaceId` | every surface sits at a layout position, and every position names a surface |
| `foreground_process` | every surface has a foreground process (the bare-interpreter check) |
| `prefill_text`, `relaunch_surface`, `answer_dialog` | every surface has a composer, a re-runnable shell command and a first-run dialog |
| `capture_tail` ("recent screen") | the tail is a screen, so screen predicates (idle markers, prompts, buffered input, banners) apply to it |
| `SurfaceSpec.launch_command`, `SurfacePlacement::AppendToGrid` | a surface runs a local shell command in a tiled grid |
| `liveness(.., first_agent)` / `AgentsExited` | "agents exited" means an interpreter is in the foreground of every agent pane |
| `attach` | the user attaches as a client |

v1.3.0 (`headless-backend-adapter`, pending on `feat/v1.3.0-specs`) adds
`surfaces_are_interactive()` for a whole backend, `activity(surface)`, an optional turn plan on
`SurfaceSpec`, and `PawError::Unsupported`. A headless-only session can live with the
assumptions above because every one of its surfaces is the same kind. A session that holds tmux,
headless and remote workers cannot, and a remote surface (a vendor's cloud session) has neither a
process nor a screen nor a position.

## Goals / Non-Goals

**Goals:**
- One port that describes every surface kind by what it can do, so the remote adapter and mixed
  sessions need no second port.
- tmux byte-identical; headless-only sessions unchanged.
- Callers cannot type into, relaunch or screen-read a surface that has no screen.

**Non-Goals:**
- The remote adapter itself (`remote-surface-adapter`) and the routing of a mixed session
  (`mixed-backend-sessions`).
- Changing the persisted handles (`pane_index`, `dashboard_pane`).
- Session-level liveness rules for mixed sessions (`mixed-backend-sessions` modifies them).

## Decisions

### D1 — Capability flags and `Unsupported`, not an interactive-terminal extension trait

The port answers, per surface, `interactive` and `position`. The interactive-only members
(`foreground_process`, `prefill_text`, `relaunch_surface`, `answer_dialog`) are defined for
interactive surfaces; elsewhere they return `PawError::Unsupported`, and `foreground_process`
returns `None`.

*Alternatives rejected:*
- **A core port plus an `InteractiveTerminal` extension trait.** It makes "readiness gate on a
  remote surface" a compile error, which is attractive. But a mixed session is served by one
  routing backend (`mixed-backend-sessions`) that must implement both traits, so a routed call to
  a remote surface is still a runtime question and still needs `Unsupported`. The split would also
  churn every caller signature in locked v0.18.0 code for no extra safety in the mixed case.
- **A second port for remote surfaces.** The drive loop, inventory, status and the broker host
  would then have two ways to reach a worker. That is the outcome this change exists to avoid.

### D2 — Positions are optional

`SurfaceInfo.position` becomes `Option<usize>` and `surface_at` returns `Option<SurfaceId>`. tmux
always answers `Some`, so its fixtures are unchanged. The role resolver already matches owners
before reserved indices; a surface with no position simply skips the index rules and can never be
a dashboard or supervisor surface. Persisted `pane_index` values are untouched: they remain the
tmux adapter's encoding.

### D3 — The foreground read keeps its signature

`foreground_process -> Option<String>` already means "`None` = do not type". A non-interactive
surface answers `None`. Changing it to a `Result` would add an error path every caller already
handles as `None`. Callers ask only after finding the surface interactive (D5), so `None` is the
belt to that brace.

### D4 — Interactivity per surface; the backend-wide answer becomes a convenience

v1.3.0's `surfaces_are_interactive()` is kept for single-kind backends (tmux: `true`, headless:
`false`) and must agree with every per-surface answer. Every decision about one surface (drive-loop
observation, boot-prompt addendum, wake) reads the per-surface answer, so a routing backend can
answer correctly for each surface.

### D5 — Callers guard; screen predicates run on screens only

The callers listed in the ADDED requirement check `interactive` first and record a skipped
operation as not applicable, so a remote or headless surface is never relaunched, never
dialog-answered, never prefilled and never screen-read. The output tail of a non-interactive
surface is data for display and capture only.

### D6 — Launch command and placement are ignored where they mean nothing

A spec still carries `launch_command` and a placement, because every launch flow builds one. A
backend whose surfaces run no local command in a grid ignores both and takes what it needs from
its own plan (v1.3.0's turn plan; this release's remote task plan).

## Risks / Trade-offs

- **[A caller forgets the guard]** → The adapter still refuses with `Unsupported` (or `None`), so
  the failure is loud, never a stray keystroke. Fake-backend tests cover each guarded caller.
- **[tmux drift]** → Golden argv fixtures from `terminal-backend-seam` pin tmux behaviour; this
  change adds no tmux invocation.
- **[Churn in locked code paths]** → The `Option` changes are mechanical and confined to the port,
  the fake, the resolver and the guarded callers.

## Migration Plan

1. Add the per-surface query and the `Option` position to the port; tmux answers interactive and
   positioned; fake gains per-surface scripting. Suite green and tmux fixtures unchanged.
2. Headless adapter answers non-interactive and unpositioned; interactive-only members return
   `Unsupported` / `None`.
3. Guard the callers; resolver handles position-less surfaces.
4. Tests, audits, architecture docs.

Rollback: revert. No persisted state changes.

## Open Questions

- **Should this fix move earlier than this release?** v1.3.0's headless surfaces already have no
  screen and no natural layout position, but v1.3.0 sessions hold one kind of surface, so its
  backend-wide answers suffice. *Recommendation:* keep it here; v1.3.0 implementers should give
  headless surfaces no reserved index and rely on owner resolution, which this change then
  formalises.
