## Context

The relocation deletes the supervisor pane (`supervisor-pane-removal`). The concept-decoupling
skill's seam-first rule says a half-typed role must become a type end to end *before* the thing
it describes is removed, so the deletion is a compiler-guided edit rather than archaeology in
`drive.rs` (8,660 lines, about 3,225 production).

Where the role lives in released v0.17.0 (`classifier-retirement` deleted `approval_send_gate`
and `approval_gate::SUPERVISOR_PANE_INDEX`, `approve::auto_approve_pane`, `poll.rs`,
`dashboard_should_auto_approve` and the dashboard's auto-approve `pane_map`, so those former
pane-0 sites are out of scope):

| Site | What it encodes |
|---|---|
| `drive.rs::resolve_pane_agent` | coding by `pane_current_path`; else index 0 → `Supervisor`, 1 → `Dashboard`, else `Unknown` — regardless of `SessionMode` |
| `drive.rs::PaneRole::agent_id` / `sweep_pane` | role flattened to `"supervisor"`, then `agent_id == SUPERVISOR_AGENT_ID` (`drive.rs:1865`) decides the orchestrator-CLI-unavailable exit and the pane-0 escalation identity |
| `supervisor::layout::SUPERVISOR_PANE_OFFSET` (= 2) | `Session::agent_pane_offset` (`session.rs:318`), `commands/supervisor.rs` logging / discovery / boot-loop offsets (`:816`, `:856`, `:902`), `commands/recover.rs:343`, `tmux/command.rs` + `tmux/layout.rs` row math |
| `Session::agent_pane_offset` readers | `commands/{pause,status,start,supervisor,add,remove}.rs`, `doctor.rs:1569`, and the liveness probe's `agent_pane_offset` parameter (`tmux/session.rs:150`) |
| bare inline offsets | `usize::from(broker_config.enabled)` (`start.rs:343/:382/:754/:760`), `if broker_config.enabled { idx + 1 } else { idx }` in `cmd_start`'s boot loop, `usize::from(broker_url.is_some())` (`recover.rs:240`) |
| recorded / targeted reserved indices | `dashboard_pane = Some(0)` (`start.rs:374/:787`), `Some(1)` (`supervisor.rs:846`); orchestrator `pane_index: 0` (`helpers.rs:66`, `supervisor.rs:859`, `recover.rs:346`); the supervisor pane's gate and submit at index 0 (`supervisor.rs:888-895`); `dashboard_pane.unwrap_or(0)` (`start.rs:856`, `pause.rs:54`) |
| `coordination::inventory::match_pane` | `"supervisor"` → `Some(0)` (`inventory.rs:188`) |
| `drive.rs`, `opsx/role_guard.rs`, `dashboard/view.rs` | three copies of the wire id `"supervisor"` |

`tmux/command.rs` and `tmux/layout.rs` import `crate::supervisor::layout` — an adapter-side module
depending on a feature module, the wrong direction under the target architecture.
`tmux/readiness.rs:336` also imports `supervisor::permission_prompt::capture_pane`; that is screen
capture, not layout, and `terminal-backend-seam` moves it behind the terminal port.

Not role decisions (wire identity, left alone): comparisons that identify a broker sender, a
roster row or a pane label as `supervisor` — `detect_completion`'s supervisor row and
`gate_failure`'s `payload.from` (`drive.rs:428/:463`), `inventory::candidate_ids`
(`inventory.rs:106`), `cmd_remove`'s refusal of `git paw remove supervisor` (`remove.rs:27`),
`opsx/role_guard.rs`, the dashboard's pinned row (`dashboard/view.rs`), and the `@paw_role`
liveness filter `NON_AGENT_PANE_ROLES` (`tmux/session.rs:188`).

## Goals / Non-Goals

**Goals:**
- One core `SessionLayout` value that owns every reserved-index fact, built from the recorded
  session (or from the launch plan before the session is saved).
- One resolver producing a closed `SurfaceRole`; every decision matches on the role.
- Grid geometry moves to core so `tmux/` no longer depends on `supervisor/` for layout (the
  readiness gate's capture import is `terminal-backend-seam`'s).
- Byte-identical behavior on every reachable path; existing suites unchanged.

**Non-Goals:**
- Deleting the supervisor role or pane (`supervisor-pane-removal`).
- Changing which surfaces may be typed into (GP-46's remainder — `sweep.sh approve`'s roster-based
  refusal — is closed by the deletion, with scenarios).
- The terminal port and `SurfaceId` (`terminal-backend-seam`); this change still speaks pane
  indices at the edges, it just stops deciding by literal ones.
- Renaming the `PaneEnumerator` / `PaneCapture` / `KeyDispatcher` seam traits.

## Decisions

### D1 — `SessionLayout` is derived, not persisted

`SessionLayout::of(&Session)` reads `mode`, `broker_port` and `dashboard_pane`; the launch paths
build the same value from the plan, and the values they record (`dashboard_pane`, the
discovery file's orchestrator `pane_index`) come from it rather than from literals. With
`dashboard_pane = None` the record keeps today's fallbacks: dashboard index 0 for pause and
restart, first agent index by mode. Nothing new is serialized. *Alternative rejected:* a persisted
layout enum — it would add a session-file field for a fact the existing fields already determine,
and the pane removal changes the layout anyway.

### D2 — Role resolution order is unchanged

Coding agents resolve first by owner path (canonicalised, as today), then the layout's reserved
indices, then `Unknown`. Keeping the order is what makes the refactor byte-identical: a coding
worktree whose pane happens to sit at a reserved index still resolves as `Coding`.

### D3 — Mode-aware resolution is behavior-preserving on reachable paths

The typed resolver consults the recorded mode, so a bare session's pane 0 resolves to
`Dashboard` where `resolve_pane_agent` would have said `Supervisor`. That difference is
unreachable today: `run_drive_loop` is only called from `cmd_supervisor`, and since v0.17.0 no
other Rust component types into a pane on its own. The audit for "unchanged" is the existing
suites plus a role table test.

### D4 — The surviving keystroke and escalation paths decide by role

After v0.17.0 the drive loop is the only Rust component that types into panes (nudges, stale-input
recovery, buffered-input submits) and it escalates pane-0 prompts as agent `supervisor`. Those
decisions match on the resolved role instead of the literal index or the `"supervisor"` string,
keeping today's targets. (`approval_gate::SUPERVISOR_PANE_INDEX` went with `approval_send_gate`
in v0.17.0.) `sweep.sh approve`'s pane-0 refusal is shell and is left alone
(`supervisor-pane-removal` makes it roster-based).

### D5 — One wire identity constant

`SurfaceRole::Supervisor` maps to the wire id through one function; `drive.rs`,
`opsx/role_guard.rs` and `dashboard/view.rs` import the single constant. The wire value stays
`"supervisor"` (frozen: the broker, the roster seed and role gating key on it). Comparisons
against the wire id that identify a message sender, roster row or pane label (listed under
Context) are not role decisions; the source audit allows them by name.

## Risks / Trade-offs

- [A reachable bare-mode path the analysis missed changes behavior] → the source audit lists every
  former literal site; each gets a test that pins the pre-change decision for both modes.
- [Churn on `drive.rs` tests that build `PaneRole`] → mechanical; the role table test replaces the
  per-index assertions.
- [Moving layout constants breaks `pub` paths] → re-export from `supervisor::layout` for one
  release (code-architecture's preserved re-exports rule), removed by the pane removal.

## Migration Plan

1. Add the core module with `SessionLayout`, `SurfaceRole`, geometry; re-export the old paths.
2. Route `drive.rs` (resolution, sweep, escalation identity, nudge / submit targeting,
   `orchestrator_pane_index`) through the role.
3. Route `inventory.rs`, `agent_pane_offset` and its readers, the bare inline offsets, the
   discovery-file offsets and the recorded reserved indices.
4. Point `tmux/` at the core geometry; collapse the wire constants.
5. Source audit + role table test; full suite green, unchanged.

Rollback: revert; nothing persisted changes.

## Open Questions

- None blocking. Whether `SessionLayout` should later become the persisted layout record is a
  `terminal-backend-seam` question (surface handles), not this one.
