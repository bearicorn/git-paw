## Why

The supervisor pane cannot be deleted safely while its role is only half-typed. Today
`supervisor::drive::resolve_pane_agent` decides `PaneRole::Supervisor` from the literal pane
index 0, `PaneRole::agent_id` flattens it to the string `"supervisor"`, and `sweep_pane`
re-detects it with `agent_id == SUPERVISOR_AGENT_ID`. Separate "pane 0" and offset constants
encode the same layout fact in several places: `drive.rs`'s private
`SUPERVISOR_PANE_INDEX` / `DASHBOARD_PANE_INDEX`; `supervisor::layout::SUPERVISOR_PANE_OFFSET`
(read by `Session::agent_pane_offset`, `commands/supervisor.rs`'s logging, discovery and boot
offsets, `commands/recover.rs`, `tmux/command.rs` and `tmux/layout.rs`); the bare paths' inline
offsets (`usize::from(broker_config.enabled)` in `commands/start.rs`, `usize::from(broker_url.is_some())`
in `commands/recover.rs`); the reserved indices the launches record or target (`dashboard_pane =
Some(0)` / `Some(1)`, the discovery file's orchestrator `pane_index: 0`, the supervisor pane's
gate and submit at index 0, `dashboard_pane.unwrap_or(0)` in pause and restart); and
`coordination::inventory::match_pane` (`"supervisor"` → `Some(0)`). Each site re-derives the
fact on its own, and the drive loop's resolver ignores the session's recorded mode. (The approval sites that used to carry the same fact —
`approval_gate::approval_send_gate` with its `SUPERVISOR_PANE_INDEX`,
`approve::auto_approve_pane`, `poll.rs`, `dashboard_should_auto_approve` and the dashboard's
auto-approve pane map — were deleted by v0.17.0 `classifier-retirement`.) This is step 1 of the
cycle's implementation order: make the role a real type end to end so the next change can
delete it by following the compiler.

## What Changes

- Introduce one core layout record (`SessionLayout`) derived from the recorded session —
  mode, whether a dashboard exists, and its index — that owns every reserved-index fact: which
  index is the supervisor surface, which is the dashboard, and where coding agents start.
- Introduce a closed `SurfaceRole` enum (`Supervisor`, `Dashboard`, `Coding(agent id)`,
  `Unknown`) produced by ONE resolver over that layout plus the surface's owning path. Coding
  roles still resolve by owner path first, never by index.
- Route every consumer through the typed role: the drive loop's sweep, escalation, nudge and
  buffered-input targeting (no `agent_id == "supervisor"` re-detection, no
  `SUPERVISOR_PANE_INDEX` literal), the inventory join, `agent_pane_offset` and every reader of
  it, the bare paths' inline offsets, the discovery-file offset each launch passes, and the
  reserved indices the launches record (`dashboard_pane`, the orchestrator entry's `pane_index`).
  The broker wire identity `"supervisor"` is produced from the role in one place; the three
  duplicate wire constants collapse into one. Comparisons that identify a broker sender, a
  roster row or a pane label as `supervisor` (completion and gate-feedback checks, inventory
  candidates, the `remove supervisor` refusal, role gating, the `@paw_role` liveness filter) are
  wire identity, not surface-role decisions, and stay.
- Move the grid geometry (`SUPERVISOR_PANE_OFFSET`, `SUPERVISOR_AGENTS_PER_ROW`,
  `SUPERVISOR_MAX_AGENTS`, `SupervisorLayout`, `supervisor_layout`, `layout_for`) out of
  `supervisor/` into the core layout module, so `tmux/` stops importing `supervisor::layout` (the
  dependency currently points outward). `tmux/readiness.rs`'s import of
  `supervisor::permission_prompt::capture_pane` is screen capture, not layout, and moves with
  `terminal-backend-seam`.
- **Behavior-preserving.** For every layout the current binary can launch, each surface
  resolves to the same role and every sweep / escalate / keystroke-target / skip / exit / offset
  decision is identical. No CLI, config, wire or session-file change.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `code-architecture`: adds the requirement that session surface roles are an explicit enum
  resolved once from the recorded layout, with no pane-index literal or role-string comparison
  in decision code (the `WorkerPhase` precedent applied to surface roles).

## Impact

- **Predecessors (implementation order):** archive position 1 of 20 — first change of the
  cycle; no v0.18 predecessor. Built on released v0.17.0, whose `drive.rs` bodies
  (`live-prompt-tail-window`, `idle-pane-nudge-submission`, `cli-unavailable-detection-gaps`)
  leave the pane-0 sites in place, and whose `classifier-retirement` deleted the approval sites
  (`approval_send_gate`, `approval_gate::SUPERVISOR_PANE_INDEX`, `auto_approve_pane`,
  `approve.rs`, `poll.rs`, `dashboard_should_auto_approve`, the auto-approve pane map), moved
  `KeyDispatcher` to `src/supervisor/keys.rs`, and kept the drive loop sweeping pane 0 for
  escalation, CLI unavailability and buffered input only. This change types what is left; no
  requirement header is shared.
- **Successors:** `supervisor-pane-removal` (position 6) deletes `SurfaceRole::Supervisor`;
  `terminal-backend-seam` (position 12) keys surfaces by `SurfaceId`, reuses `SurfaceRole`, and
  moves `tmux/readiness.rs`'s screen capture out of `supervisor/`.
- **Code:** new core module (e.g. `src/surface.rs`) holding `SessionLayout` + `SurfaceRole`;
  `src/supervisor/{drive,layout}.rs`, `src/session.rs` (`agent_pane_offset`),
  `src/commands/{helpers,supervisor,start,add,remove,recover,pause,status}.rs` (offset readers,
  inline offsets, recorded `dashboard_pane` and orchestrator `pane_index`, the supervisor pane's
  index-0 gate/submit), `src/doctor.rs` (offset reader), `src/tmux/session.rs` (liveness offset
  parameter), `src/coordination/inventory.rs` (`match_pane`), `src/tmux/{command,layout}.rs`
  (imports), `src/opsx/role_guard.rs` + `src/dashboard/view.rs` (duplicate `"supervisor"`
  constants). `src/main.rs::write_repo_discovery_file` already takes the offset as a parameter;
  only its callers change.
- **Persisted state / wire:** none. `pane_index`, `dashboard_pane`, `mode` unchanged.
- **Enum-variant ripple:** `BrokerMessage` and `SpecBackendKind` untouched. `PaneRole` →
  `SurfaceRole` ripples through every `match` in `drive.rs`; that is the intended compiler
  enumeration.
- **Tests:** drive-loop unit tests that construct `PaneRole`; a table test of
  `SessionLayout` × index → role; a source audit for pane-index literals and role-string
  comparisons; existing suites unchanged (`unattended_drive_e2e.rs`, `add_remove_e2e.rs`,
  `sweep_sh_approve*.rs`, `coordination_inventory_integration.rs`,
  `discovery_file_pane_indices.rs` for the recorded offsets and orchestrator entry, `pause_e2e.rs`
  for the dashboard index).
- **Docs:** `docs/src/architecture.md` module list gains the layout module (module-list guard).
- **Security / safety:** none changed — the drive loop types into exactly the surfaces it did
  before (nudges and buffered-input submits only; it answers no prompt since v0.17.0), and
  `sweep.sh approve`'s pane refusal is untouched.
- **Dependencies:** none.

## Rebase note (2026-10-20)

v0.17.0 `classifier-retirement` deleted the Rust approval-send gate and the attended approver, and
dropped `broad-grant-scope-safety`. The approval sites were removed from this change's scope; the
behavior-preservation check now covers the drive loop's sweep / escalate / keystroke-target / exit
decisions instead of the gate's refused-target set.

## Rebase note (2026-10-25): audit against released v0.17.0

- Dropped the `approval_gate.rs` task: v0.17.0 already removed `SUPERVISOR_PANE_INDEX` there (the
  file holds only `live_prompt_in_tail` and `approval_dedup_key`).
- Widened the offset scope to every site that encodes it: the bare inline offsets
  (`start.rs:343/:382/:754/:760`, `start.rs`'s `idx + 1` boot loop, `recover.rs:240`) and every
  `agent_pane_offset()` reader (pause, status, doctor, start, supervisor, add, remove,
  `tmux/session.rs:150`).
- Added the hard-coded reserved indices: orchestrator `pane_index: 0` (`helpers.rs:66`,
  `supervisor.rs:859`, `recover.rs:346`), `dashboard_pane = Some(0)` (`start.rs:374/:787`) and
  `Some(1)` (`supervisor.rs:846`), the supervisor pane's index-0 gate/submit
  (`supervisor.rs:888-895`), and `dashboard_pane.unwrap_or(0)` (`start.rs:856`, `pause.rs:54`).
- Added `supervisor_layout()` to the geometry that moves to core (`layout_for` is its alias).
- Narrowed the import audit to layout geometry: `tmux/readiness.rs:336` still imports
  `supervisor::permission_prompt::capture_pane`, which only `terminal-backend-seam` moves.
- Carved the broker wire identity out of the role-string audit: `supervisor` comparisons that
  identify a message sender, roster row or pane label (`detect_completion`, `gate_failure`,
  `inventory::candidate_ids`, `remove.rs`'s refusal, `opsx/role_guard.rs`, `dashboard/view.rs`,
  `NON_AGENT_PANE_ROLES`) are not surface-role decisions.
- Corrected counts: `drive.rs` is 8,660 lines (about 3,225 production); `main.rs`'s discovery
  writer takes its offset from callers rather than reading the constant.
