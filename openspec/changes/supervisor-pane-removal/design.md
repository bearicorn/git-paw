## Context

Since `supervisor-as-pane` a supervised session has run an LLM CLI at pane 0 in the repo root,
given it the supervisor skill as context, and had the drive loop sweep, nudge and inject judgment
calls into it. Dogfood across many cycles shows the pane is the persistent friction source: it
self-stalls on its own prompts (hence pane-0 sweeping and the "never pollute pane 0" rules), is
slow to approve, talks to the user in-pane, needs idle nudges, and its 50/50 top row drives every
layout complaint. The v0.17 dogfood ran a relocation rehearsal (`--no-supervisor`, an external
orchestrator session): the external driver worked, and the in-process loop's sub-second,
identity-deduped sweep proved irreplaceable (GP-33) — the split as written.

Load-bearing facts at the tip:

- `supervisor-autonomous-orchestrator` already specifies and tests the no-pane path
  (`orchestrator_present() == false` → broker-only escalation, no injection).
- The supervisor skill is rendered to the repo-root sidecar `.git-paw/AGENTS.local.md`
  (`setup_worktree_agents_md(repo_root, repo_root, …)` in `cmd_supervisor`); the pane's framing
  prompt merely points at it. v0.17.0 already appends the governance section there (DS-1):
  `skills::supervisor_boot_context` (`skills.rs:928`) wraps the drive-loop directive and
  `governance_section_paths`, `cmd_supervisor` calls it (`commands/supervisor.rs:677`), and
  `tests/governance_supervisor_boot.rs` pins it. The assignment carries no spec or file-ownership
  content, which is the only reason the released "before any per-agent task content" position
  rule holds (GP-63).
- `supervisor-role-typing` (step 1) resolves every surface to a `SurfaceRole` over a
  `SessionLayout`, so this deletion is compiler-enumerated.
- GP-46: surface eligibility is decided by pane index, not role. After v0.17.0
  `classifier-retirement` no git-paw process approves a prompt: the attended approver (`poll.rs`),
  `approve::auto_approve_pane`, the Rust `approval_send_gate` and `drive.rs::send_approval` are
  gone. What remains is `sweep.sh approve <pane>` (explicit, always `1` + `Enter`), which refuses
  index 0 ("supervisor pane") in every mode, and the drive loop's nudges, stale-input recovery and
  buffered-input submits, which v0.17.0 also aims at pane 0. In a bare session pane 0 is the
  dashboard; in a broker-less session it is the first coding agent.
- `RepoOrchestratorEntry` records the pane in the discovery file; `sweep.sh` never read it. Its
  writers are the commands (`commands/supervisor.rs:857`, `commands/recover.rs:344`,
  `restart_from_pause` through `helpers.rs::orchestrator_entry_for_mode`, and the
  carry-forwards in `commands/add.rs:249` / `commands/remove.rs:162`).
- The `supervisor` agent id outlives the pane as a broker wire identity: the drive loop publishes
  its escalations (`drive.rs:2808`) and branch-refresh feedback as `supervisor`, the bundled
  helper's verdict verbs do too, and the broker seeds that row's CLI column from
  `[supervisor].cli` / `default_cli` (`main.rs:1024`). Only status-kind messages create a roster
  row (`broker/delivery.rs::upserts_roster`), so the row appears when the orchestrator publishes
  its status. `coordination::inventory::match_pane` maps it to pane 0 (`inventory.rs:188`), which
  becomes the dashboard.

## Goals / Non-Goals

**Goals:**
- Remove the pane, its layout, flags, injection and sweep special-casing — compiler-guided.
- Close GP-46: one eligibility rule (coding surfaces only) for every keystroke sender.
- Make the no-pane path the only path, byte-for-byte where it already existed.
- Keep every config key parseable, every session file loadable, and a pre-relocation session that
  is still alive after upgrade resolvable.
- Keep the orchestrator context at a stable, announced path, with the governance section v0.17.0
  already renders into it (DS-1).

**Non-Goals:**
- Headless escalation delivery (`orchestrator-escalation-handoff`).
- `add`/`remove` on bare sessions and the launch gate on bare/spec launches
  (`pane-less-session-parity`, next).
- The terminal port (`terminal-backend-seam`) and the broker host split (`broker-host-separation`).
- Skill content conversion and the export surface (`orchestrator-skill-export`).
- Retuning the dashboard's share of the screen (downstream of the dashboard-survival question).

## Decisions

### D1 — Delete `SurfaceRole::Supervisor`, follow the compiler

Sites known up front (scoped, not discovered): `drive.rs` (resolver arm, `orchestrator_pane_index`,
`orchestrator_present`, `hand_to_orchestrator`, `orchestrator_ready`, the orchestration nudge,
`SweepContext.orchestrator` / `orchestrator_settle_delay`, `DriveConfig.supervisor_cli`,
`PaneSweepOutcome::OrchestratorCliUnavailable`, the pane-0 escalation and buffered-input paths),
`claim.rs`, `coordination/inventory.rs::match_pane`, `commands/supervisor.rs` (supervisor
`PaneSpec`, framing, `resolve_supervisor_flags`, `build_supervisor_framing`),
`commands/recover.rs::recover_supervisor_session` (and its `resolve_supervisor_flags` call at
`:290`), the core layout record (supervised offset 2 → 1), `tmux/command.rs::build_supervisor_session`
(swap, compensated `-c`), the `RepoOrchestratorEntry` writers in the commands
(`supervisor.rs:857`, `recover.rs:344`, `helpers.rs::orchestrator_entry_for_mode` used by
`restart_from_pause`, the `add.rs` / `remove.rs` carry-forwards), `assets/supervisor-framing.md`,
and the `GIT_PAW_DRIVE_ORCHESTRATION_NUDGE_MS` override that goes with the nudge.

### D2 — Layout: drop the horizontal split, keep the row geometry

The dashboard becomes the full-width top row at the existing 50% height; agent rows keep
`50 / agent_rows`. The swap and the compensated `-c` (a workaround for the swap) are deleted. Whether
the dashboard deserves 50% is left to the dashboard-survival question. *Alternative rejected:*
collapse supervised mode into the bare `main-horizontal` layout — it loses predictable row-major
indices for ≥6 agents; `pane-less-session-parity` moves bare sessions onto this grid instead.

### D3 — GP-46: eligibility is a role, for every keystroke sender

There are two keystroke senders left after v0.17.0: the explicit `sweep.sh approve <pane>` and the
drive loop's nudges, stale-input recovery and buffered-input submits. The drive loop resolves each
surface through `SessionLayout` and acts (detects, escalates, nudges, submits) only on a surface
resolved as `Coding`; it keeps its per-pane claim. `sweep.sh approve <pane>` refuses any pane the
discovery roster does not list as a coding agent; with no readable roster it falls back to refusing
pane 0, today's behavior. Its live re-confirm and option-1 guard are unchanged. The refusal is
reported per surface. Because the dashboard is always a non-coding surface, text it renders (the
broker log can quote a worker's rate-limit banner or a prompt) can never trigger an escalation, a
submit, or a CLI-unavailability decision. The index-named *Blind send-keys excludes the supervisor
pane 0* is replaced (REMOVED + ADDED) by the role-named *Keystrokes reach only coding-agent
surfaces*, so the requirement's name says what it enforces. *Alternative rejected:* keep the
index-0 refusal — it wrongly refuses the first agent of a broker-less session (agents from 0) and
wrongly allows the dashboard of a legacy supervised session (index 1).

### D4 — `--supervisor` keeps its name and means "supervised session"

The flag, `--no-supervisor`, `[supervisor] enabled`, `SessionMode::Supervisor` and the dispatch
chain are unchanged. Renaming would be a breaking CLI change for no behavioral gain; the docs explain
the supervisor *role* is now the operator's own session.

### D5 — Config keys stay; semantics narrow

`[supervisor].cli` becomes optional at launch (it seeded the pane). It still seeds the roster's
`supervisor` CLI column and will name the headless one-shot CLI. `[supervisor].approval` stays
parseable and affects no pane; the hand-off change binds it. No warning: an earlier config launches
with zero edits.

### D6 — Back-compat for a live pre-relocation session

Upgrading while a supervised session from an earlier binary is alive must not break `status`,
`stop`, `pause`, `remove`. `SessionLayout` derives the first agent index from the recorded
`dashboard_pane`: `Some(n)` → `n + 1` (`Some(1)` → 2, `Some(0)` → 1). `None` follows the earlier
(pre-relocation) rules, the ones `supervisor-role-typing` encoded: supervised → agents from 2
(supervisor 0, dashboard 1), bare → 1 with the broker and 0 without, and the dashboard index
defaults to 0 for pause and restart. That keeps a live broker-less supervised session from an older
binary (`None`, agents from 2) resolving. To keep `None` unambiguous, every new supervised launch
records `Some(0)`, broker or not (the supervised layout always has the dashboard at pane 0); only a
broker-less bare launch records `None`. `SessionMode` is consulted only when `dashboard_pane` is
`None`. A legacy session's pane 0 still runs the old supervisor CLI; it resolves to no role the
loop acts on (not `Coding`), so nothing types into it. Recovery of such a stopped session rebuilds
the new layout and re-saves `dashboard_pane = Some(0)`.

### D7 — Escalations: delete the actuation, keep the text builders

`hand_to_orchestrator`, `orchestrator_ready`, the nudge and `SweepContext.orchestrator*` go with
their requirements (REMOVED here; headless delivery is specified next). The pure renderers for
questions and merge decisions stay: they are the escalation context the one-shot reads on stdin.
A permission-prompt escalation has no pane to go to and is never handed to an orchestrator
invocation; it stays on the broker, in the exit summary and in the manual-decision log, where an
orchestrator outside the session reads it and relays it to the human.

### D8 — Orchestrator context: sidecar + governance + printed hint (DS-1)

The sidecar render, governance section included, is the existing v0.17.0 mechanism
(`skills::supervisor_boot_context`); this change keeps it when the pane goes and adds only the
printed sidecar path. Carrying the requirement (rather than retiring it) is the recommended DS-1
option: the orchestrator is exactly the reader the section was written for. The position rule is
reworded to "after the orchestrator skill content" and the orchestrator assignment is specified
to carry no spec or file-ownership content, which closes GP-63 (the released "before any
per-agent task content" wording held only because the supervisor carried none). The durable
discovery surface (init pointer, `SKILL.md` export, MCP `get_skill`) is
`orchestrator-skill-export`'s job.

### D9 — CLI-unavailability: coding agents only

With no orchestrator pane there is nothing for the loop to exit on; a coding agent's banner stays a
deduped `cli-unavailable` escalation and the wave is never reported `completed`. The v0.17
"no orchestrator pane was monitored" summary line becomes unconditional prose that the orchestrator
runs outside the session and its CLI health is not monitored by the loop.

### D10 — The `supervisor` wire identity stays; the inventory gives it no pane

The drive loop keeps publishing escalations and branch-refresh feedback as `supervisor`, the
helper's verdict verbs keep that sender, and the broker keeps seeding the row's CLI. None of those
creates a roster row; the orchestrator's own status publish does. With no supervisor pane, the
inventory reports the `supervisor` row (when present) with no pane index instead of pane 0, which
is now the dashboard.

## Risks / Trade-offs

- [No orchestrator delivery between this change and `orchestrator-escalation-handoff`] → that is the
  already-tested broker-only fallback; land both in this release.
- [Hidden pane-0 assumptions outside the compiler: `sweep.sh`, skill prose, docs] → `sweep.sh`
  approve refusal rewritten and tested; skill prose is `orchestrator-skill-export`'s scope and is
  not contradicted by code meanwhile; docs re-indexed here.
- [A user who typed to the supervisor pane] → relocation framing in docs and changelog.
- [The released *Pane-to-agent resolution via pane_current_path* still says "Pane 0
  (supervisor)" between this change and `terminal-backend-seam`] → accepted interim wording: that
  requirement is modified in one place, by `terminal-backend-seam`; meanwhile this change's sweep
  requirements (coding surfaces only, dashboard never acted on) decide behavior and do not
  contradict its owner-matching rule.
- [Security] → strictly reduced; the role-based eligibility rule is stricter than the index check.

## Migration Plan

Ordered, each step compiling and passing `just check`:

1. Delete `SurfaceRole::Supervisor`; fix every compile error under `src/supervisor/`.
2. One eligibility rule: the drive loop detects, escalates, nudges and submits on `Coding`
   surfaces only; rewrite the `sweep.sh` approve refusal against the roster (the `.git-paw/scripts/` copy is
   gitignored and refreshed by `git paw init`).
3. Launch flow: remove the pane, swap, flags, framing; keep the sidecar render (governance
   included); print the sidecar hint; `dashboard_pane = Some(0)`.
4. Recovery and offset derivation from `dashboard_pane` (`None` → earlier rules); stop writing
   `RepoOrchestratorEntry`; the inventory's `supervisor` row loses its pane.
5. Test deletions/rewrites; docs, README, `--help`; changelog uses the relocation framing.

Rollback: revert the branch. State written by the new binary loads in the old one, which would
rebuild the old layout on recovery.

## Open Questions

- Should `[supervisor].approval` print a one-time "no effect until headless escalation" notice in
  this change? Recommendation: no — the hand-off change lands in the same release and binds it.
