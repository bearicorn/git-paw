## Why

The LLM supervisor pane is redundant, is the most vendor-coupled surface in the codebase, and has
been the most persistent source of dogfood friction across cycles (self-stall deadlock, slow
approvals, addressing the user in-pane, idle nudges, layout complaints). The operator driving a
git-paw session already *is* a capable agent CLI; a second one inside tmux duplicates it.
git-paw's "supervisor" is two separable things — the LLM pane (judgment) and the mechanical drive
loop in `src/supervisor/` (polling, detecting and escalating permission prompts, stuck detection,
correction cycles, branch refresh) — and only the first goes. The no-pane path already exists and is tested:
`supervisor-autonomous-orchestrator` specifies that with no supervisor pane nothing is injected and
a pure `--unattended` run keeps its prior behavior. This change makes that path the only path.

Frame it as **relocating the orchestrator from inside the session to outside it**, not as removing
a capability. It is step 2 of the cycle's implementation order.

## What Changes

- **Delete the supervisor pane.** `cmd_supervisor` no longer creates a pane running an agent CLI at
  the repo root, no longer injects a boot block + framing prompt into it, and no longer requires
  `[supervisor].cli` / `default_cli` to launch. A supervised session becomes: pane 0 = dashboard
  (full-width top row), panes 1..N = coding agents in the existing ≤5-per-row grid. The pane-1/2
  `swap-pane` and its compensated `-c` assignment go away.
- **Compiler-guided deletion.** `supervisor-role-typing` made the role a closed `SurfaceRole`;
  this change deletes `SurfaceRole::Supervisor` and follows every resulting compile error.
- **Close GP-46 — one surface-eligibility rule for every keystroke sender.** After v0.17.0
  `classifier-retirement` the only approval sender is the explicit `sweep.sh approve <pane>`
  (always `1` + `Enter`), and the drive loop's only keystrokes are nudges, stale-input recovery
  and buffered-input submits. Both reach only surfaces resolved to a coding agent, in every
  session mode: `sweep.sh approve` refuses any pane the discovery roster does not list as a coding
  agent (not merely index 0), and the drive loop types only into `Coding` surfaces. The
  index-based *Blind send-keys excludes the supervisor pane 0* is replaced by the role-based
  *Keystrokes reach only coding-agent surfaces*. Text shown in the dashboard can never be read as
  an agent's prompt or CLI-unavailability banner.
- **Keep the drive loop.** Detect-and-escalate, correction loop, learnings, branch refresh,
  heartbeat and exit summary are unchanged; prompt and CLI-unavailability detection cover coding
  agents only.
- **Escalations stay broker-only in this change.** The pane-injection hand-off and the
  orchestration nudge are removed. Every live permission prompt is escalated to the broker, the
  exit summary and the manual-decision log, and is never typed into a surface;
  `orchestrator-escalation-handoff` adds headless one-shot delivery for questions and merge
  decisions only (permission prompts stay on the broker). Between the two, behavior is exactly
  today's tested no-pane fallback.
- **Orchestrator context survives, pane-less, with governance kept (DS-1).** The rendered
  orchestrator skill (gate commands, drive-loop directive when unattended, and the "Governance
  documents" section) is written to the repo-root sidecar `.git-paw/AGENTS.local.md` exactly as
  v0.17.0 already does: `cmd_supervisor` renders it through `skills::supervisor_boot_context`
  (`skills.rs:928`, called at `commands/supervisor.rs:677`), pinned by
  `tests/governance_supervisor_boot.rs`. What changes is the reader: no pane reads the file any
  more, it is the context for the operator's own orchestrator session, and `start` prints its
  path. GP-63 closes here: the position rule becomes "after the orchestrator skill content", and
  the orchestrator context is specified to carry no per-agent task content (no `### Spec`, no
  `### File Ownership`, which `agents.rs` renders ahead of the skill), instead of the released
  "before any per-agent task content" wording that held only by that unstated precondition.
- **Config back-compat, no breaking change.** `[supervisor].cli` and `[supervisor].approval` stay
  parseable; `--supervisor` / `--no-supervisor` / `[supervisor] enabled` keep meaning "a supervised
  session" (worktree provisioning, boot prompts, broker, dashboard, drive loop on `--unattended`).
- **State back-compat.** `dashboard_pane = Some(0)` for every new supervised session (the
  supervised layout always places the dashboard at pane 0); the first agent index is derived from
  the recorded `dashboard_pane` (`Some(n)` → `n + 1`), and a session recorded with `None` follows
  the earlier (pre-relocation) rules, so a pre-relocation session still alive after upgrade
  (`Some(1)`, agents from 2; or a broker-less supervised session with `None`, agents from 2)
  keeps resolving. `RepoSessionFile.orchestrator` is never written again but still deserializes.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `supervisor-launch`: auto-start flow without the pane; approval-flags requirement removed;
  orchestrator context (with governance) rendered to the sidecar and announced; default-config
  fallback without a required orchestrator CLI; self-registration, boot-prompt, recovery-cwd,
  first-agent split, titles, border label, architecture and quick-start requirements re-indexed.
- `supervisor-config`: `approval` is retained but affects no pane command.
- `tmux-orchestration`: supervised layout = full-width dashboard row + agent grid.
- `session-state`: orchestrator-pane record removed; `dashboard_pane` drives the agent offset
  (`None` keeps the earlier rules); recovery rebuilds `N + 1` panes; the discovery file is read
  from the operator's session.
- `supervisor-unattended-operation`: the pane-0 coverage requirement is removed; the loop sweeps
  coding surfaces only; every live permission prompt is escalated to the broker, the summary and
  the manual-decision log and is never handed to a pane; CLI-unavailability detection is
  coding-only (the `cli-unavailable` exit outcome goes with the orchestrator pane); the exit
  summary drops its orchestrator-pane wording.
- `approval-auto`: the index-based pane-0 exclusion is removed and replaced by one role-based
  surface-eligibility rule for every keystroke sender (GP-46).
- `broker-agent-helper`: `sweep.sh approve` refuses any non-agent pane per the discovery roster.
- `boot-block`: coding panes only; governance section and drive-loop directive target the
  orchestrator context (the sidecar v0.17.0 already writes), which carries no per-agent task
  content. The directive tells the orchestrator to relay escalated prompts to the human.
- `cli-resolution`: the split-send profile applies to coding panes (no supervisor pane).
- `broker-runtime`: start flow places the dashboard at pane 0 in every mode.
- `broker-dashboard`: the dashboard chapter and the no-inbox requirement drop the supervisor pane.
- `supervisor-autonomous-orchestrator`: pane hand-off and orchestration nudge removed; broker
  review items are the escalation path until `orchestrator-escalation-handoff`.

## Impact

- **Predecessors (implementation order):** archive position 6 of 20, after
  `supervisor-role-typing` (1, the hard dependency: `SurfaceRole` / `SessionLayout`),
  `setup-cli-adopted-entries` (2), `orchestrator-read-surface` (3), `doctor-spec-workflow-check`
  (4) and `agent-id-slug-safety` (5), none of which shares a requirement header with this change.
  v0.17 changes it rebases over: `classifier-retirement` (see the rebase note below for every
  shared header), `cli-unavailable-detection-gaps` (MODIFIES *CLI unavailability is an explicit
  non-success exit*; `classifier-retirement` modifies it again and this change builds on that
  text), `bundled-helper-discovery` (sweep.sh reads the discovery `agents[]` roster — the approve
  refusal uses it), `start-spec-launch-parity` and `live-prompt-tail-window` (MODIFIES another
  `approval-auto` requirement; no shared header).
- **Successors that modify the same requirements again:** `orchestrator-skill-export` (18:
  *Supervisor AGENTS.md from supervisor skill template*, *Quick Start Supervisor chapter is
  internally consistent*) and `orchestrator-escalation-handoff` (20: *No-orchestrator fallback
  preserves prior behavior*, *Supervisor-specific approval level resolution*). Archive this change
  first. `pane-less-session-parity` (9) follows on the same files (add/remove on every session,
  the restart-from-pause gate). The released *Pane-to-agent resolution via pane_current_path*
  still says "Pane 0 (supervisor) and pane 1 (dashboard) resolve to the repo root"; it is
  rewritten in one place, by `terminal-backend-seam` (12), whose text drops the supervisor pane
  and identifies the dashboard by the layout record. Until then this change's own requirements
  (the loop sweeps and types into coding surfaces only) decide which surfaces are acted on.
- **Code deleted:** `tmux::build_supervisor_session`'s supervisor pane + swap; the supervisor
  `PaneSpec`, `resolve_supervisor_flags` (called at `commands/supervisor.rs:574` and
  `commands/recover.rs:290`), `build_supervisor_framing` in `commands/supervisor.rs`;
  `recover_supervisor_session`'s supervisor pane; `drive.rs` orchestrator actuation
  (`hand_to_orchestrator`, `orchestrator_ready`, `orchestrator_pane_index`, `orchestrator_present`,
  `ORCHESTRATION_NUDGE_INTERVAL` and its `GIT_PAW_DRIVE_ORCHESTRATION_NUDGE_MS` test override,
  `orchestration_nudge_text`, `SweepContext.orchestrator*`, `DriveConfig.supervisor_cli`,
  `PaneSweepOutcome::OrchestratorCliUnavailable`); `commands/helpers.rs::orchestrator_entry_for_mode`;
  `assets/supervisor-framing.md`; `SurfaceRole::Supervisor`. Kept as pure renderers for the
  hand-off change: `Escalation::handoff_text`, `question_handoff_text`, `merge_handoff_text`.
- **Code changed:** `drive.rs` (prompt detection, escalation, nudges, stale-input recovery and
  buffered-input submits target `Coding` surfaces only), the core layout record (offset from
  `dashboard_pane`, `None` keeps the earlier rules), the `RepoOrchestratorEntry` writers, which
  live in the commands, not `session.rs` (`commands/supervisor.rs:857`, `commands/recover.rs:344`,
  `commands/start.rs:1008` via `helpers.rs::orchestrator_entry_for_mode` in `restart_from_pause`,
  and the carry-forwards in `commands/add.rs:249` / `commands/remove.rs:162`),
  `coordination/inventory.rs` (the `supervisor` row gets no pane), `assets/scripts/sweep.sh`
  (`cmd_approve` refusal; the `.git-paw/scripts/` copy is gitignored, written by `init`). Unchanged:
  `skills::supervisor_boot_context` already renders the governance section into the sidecar, and
  the broker's `supervisor` CLI seed (`main.rs:1024`) stays.
- **Persisted state:** `dashboard_pane = Some(0)` for every new supervised session;
  `RepoSessionFile.orchestrator` no longer written; `RepoAgentEntry.pane_index` starts at 1.
- **Config:** no field removed; `[supervisor].cli` optional at launch.
- **Tests affected:** `tests/e2e_supervisor_launch.rs`, `e2e_supervisor_approval_flags.rs`,
  `e2e_supervisor_returns.rs`, `e2e_supervisor_stop.rs`, `supervisor_integration.rs`,
  `recover_integration.rs`, `unattended_drive_e2e.rs`, `sweep_sh_approve*.rs`,
  `dashboard_render.rs`, `source_audit.rs`, `session_stale_hygiene.rs`,
  `discovery_file_pane_indices.rs` (the orchestrator-entry tests, e.g.
  `restart_from_pause_derives_the_orchestrator_entry_when_the_file_is_gone`, become "no entry is
  written"), `cli_supervisor_no_config.rs` (the missing-CLI error now names `--cli` and
  `default_cli`), `governance_supervisor_boot.rs` (kept: still pins the sidecar's governance
  section), the inventory unit tests that pin `supervisor` → pane 0, drive-loop unit tests for
  pane-0 coverage, orchestrator hand-off and nudge (deleted with their requirements).
- **Docs:** `docs/src/architecture.md`, `quick-start-supervisor.md`, `user-guide/supervisor.md`,
  `user-guide/dashboard.md`, `user-guide/governance.md`, `configuration/README.md`
  (`[supervisor].cli` / `approval`), README feature bullets + Supervisor Mode quick start,
  `start --supervisor --help`.
- **Enum-variant ripple:** `BrokerMessage`, `SpecBackendKind` untouched. `SurfaceRole` loses
  `Supervisor` (intended). No Rust approval-send gate remains after `classifier-retirement`, so
  there is no gate outcome enum to reshape.
- **Security / safety:** the blast radius shrinks — no auto-approved LLM process runs at the repo
  root inside the session; judgment happens in the operator's own session under their own
  permission mode. Keystroke eligibility becomes stricter (coding surfaces only, every sender), and
  no git-paw process approves a prompt (unchanged from v0.17.0).
- **Dependencies:** none.

## Rebase note (2026-10-19)

v0.17.0 pulled DS-1 forward as `supervisor-boot-wiring`: `cmd_supervisor` appends the governance section to the supervisor's boot context. This change then moves that section into the orchestrator context when the pane goes, so its DS-1 requirement is a MODIFIED of the requirement v0.17.0 archived, built on the archived v0.17.0 text.

## Rebase note (2026-10-20): classifier retirement

v0.17.0 `classifier-retirement` retired the command classifier: no git-paw process approves a prompt, the drive loop escalates every live prompt, the attended auto-approver (`poll.rs`, `approve::auto_approve_pane`) and the Rust `approval_send_gate` are deleted, and `sweep.sh approve` always sends `1` + `Enter`. This change was re-scoped onto it and is built on the archived v0.17.0 text.

- Dropped (v0.17.0 REMOVES them): MODIFIED *Auto-approval keystroke sequence*, *No auto-approval for unsafe or unknown classes*, *Escalation of risky and unknown prompts is non-blocking*; REMOVED *Pane-0 approval must not pollute the supervisor's context*.
- Rebuilt on v0.17.0's text: *Unattended drive loop runs a supervisor wave to completion*, *CLI unavailability is an explicit non-success exit*, *Drive-loop coordination in the supervisor boot context*, *Start flow conditionally creates dashboard pane*.
- Added: MODIFIED *Every live permission prompt is escalated without blocking the wave* (v0.17.0 ADDED) — coding surfaces only, no orchestrator-pane hand-off.
- Renamed by REMOVED + ADDED: *Blind send-keys excludes the supervisor pane 0* (v0.17.0 MODIFIED) becomes *Keystrokes reach only coding-agent surfaces*. GP-46 shrinks to the `sweep.sh approve` roster refusal plus the drive loop's nudges and submits.
- REMOVED requirements that v0.17.0 MODIFIES (allowed): *The drive loop covers the supervisor's own pane 0*, *Judgment calls are handed to the orchestrator pane*, *Orchestration-sweep nudge on a longer cadence*.

## Rebase note (2026-10-25): audit against released v0.17.0

- DS-1 already ships: v0.17.0 renders the governance section into the repo-root sidecar through `skills::supervisor_boot_context` (`skills.rs:928`, called at `commands/supervisor.rs:677`), pinned by `tests/governance_supervisor_boot.rs`. The prose, D8 and task 3.2 now say "keep it in the sidecar when the pane goes" instead of "render it for the first time".
- GP-63 closes here: the governance position rule reads "after the orchestrator skill content", and the orchestrator context is specified to carry no per-agent task content.
- Task 4.1 names the real `RepoOrchestratorEntry` writers (`commands/supervisor.rs:857`, `commands/recover.rs:344`, `restart_from_pause` via `helpers.rs:54-66`, the `add.rs:249` / `remove.rs:162` carry-forwards), not `session.rs`.
- Task 4.4 and *Supervisor self-registration* now match the code: the drive loop publishes escalations (`drive.rs:2808`) and branch-refresh feedback as `supervisor` and the broker seeds that row's CLI (`main.rs:1024`), but only status-kind messages create a roster row (`delivery.rs` `upserts_roster`); the inventory stops mapping the row to pane 0 (`inventory.rs:188`).
- `dashboard_pane = None` follows the earlier (pre-relocation) rules in both task 4.2 and the `session-state` delta (design call 7); every new supervised launch records `Some(0)` so `None` stays unambiguous.
- *Pane-to-agent resolution via pane_current_path* is modified in one place only, by `terminal-backend-seam`; this change does not touch it.
- Test list extended (`discovery_file_pane_indices.rs`, `cli_supervisor_no_config.rs`, `governance_supervisor_boot.rs`); `resolve_supervisor_flags` is also called at `recover.rs:290`; `GIT_PAW_DRIVE_ORCHESTRATION_NUDGE_MS` goes with the nudge.
