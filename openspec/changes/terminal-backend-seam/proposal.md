## Why

tmux's *model*, not just its verbs, has leaked into git-paw's domain. At this tip, outside
`src/tmux/`: about 17 raw `tmux` process spawns in 8 files (`main.rs::submit_prompt_to_pane`,
`commands/start.rs` boot pre-fill and restart-from-pause, `coordination/inventory.rs`,
`supervisor/{keys,permission_prompt,drive}.rs`, `selftest.rs`), none through `CommandRunner`;
the `"{session}:0.{n}"` target hand-built in several modules; agent-owns-pane resolution
implemented six times with three different rules — exact canonical path in
`drive.rs::resolve_pane_agent` (`drive.rs:221`), `tmux::resolve_pane_id_for_worktree` (`remove`),
`tmux::resolve_pane_index_for_worktree` (`tmux/session.rs:571`, `restart_from_pause`'s discovery
rewrite) and `tmux::agents_without_live_pane` (`add`'s reconciliation); basename suffix in
`inventory.rs::match_pane`; longest realpath prefix in `sweep.sh`'s `resolve_agent_for_path` —
plus two label-based surface classifiers (the `@paw_role` filter in the liveness probe,
`tmux/session.rs:188`, and `sweep.sh`'s title-based `discover_coding_panes`); the supervisor seam
traits typed `(session, pane_index)` with their per-trait test fakes; and
`tmux/readiness.rs` importing `supervisor::permission_prompt::capture_pane` — the dependency
pointing the wrong way. `pane_index` is persisted in the per-repo discovery file `sweep.sh` reads.

The one user-visible defect that used to live in this leak — **GP-45**, the attended
auto-approver's positional pane map — is gone with that approver: v0.17.0
`classifier-retirement` deleted the dashboard auto-approve thread and `poll.rs`. This change is
therefore behavior-preserving.

Archive position 12 of the cycle: a `TerminalBackend` port in git-paw's terms, keyed by an
opaque `SurfaceId`, with tmux as its first adapter. It pays off whether or not tmux is ever
replaced, and it makes the tmux / PTY / headless fork a choice between backends rather than a bet.

## What Changes

- **Port.** `src/terminal/` defines `TerminalBackend`, keyed by an opaque `SurfaceId`, with the
  domain operations the code needs: open / close a session from a plan, set session environment,
  `spawn_surface`, `kill`, `list_surfaces` → (`SurfaceId`, owning worktree), label a
  surface, `send_text`, `send_key`, `capture_tail`, `stream_to` (an agent's output to a log),
  `liveness`, `attach` / detach clients, and an optional `layout` for backends that tile. No
  method or type carries tmux vocabulary.
- **Scope: the orchestration path.** The drive loop, the launch paths (bare, spec-driven and
  supervised `start`, recovery, restart from pause), `add`, `remove`, the readiness gate, prompt
  submission (including `launch-prompt-entry-point`'s idle wait before a declared entry point —
  `capture_tail` observations judged by the shared idle predicate — and the entry-point submit as
  `send_text` then `send_key`) and the agent inventory go through the port. The lifecycle commands `pause`, `stop`,
  `attach`, `status` and `doctor` keep calling `src/tmux/` helpers in this change; the
  confinement guard forbids direct `tmux` spawns outside the adapter, not calls into `src/tmux/`.
- **First adapter.** `TmuxBackend` composes the existing `src/tmux/` builders and `CommandRunner`;
  the ad-hoc spawns move into it. The tmux argv, ordering, indices, titles, environment injection
  and every persisted byte are unchanged.
- **One owner resolution for surfaces.** The four Rust exact-match implementations (drive loop,
  remove, restart-from-pause, add reconciliation) become one core function over `list_surfaces`,
  keeping exact canonical-path equality: a surface's owner is its worktree root at launch. The
  dashboard is identified by the layout record (`SessionLayout`'s dashboard position), never by
  its repo-root owner; any other unmatched surface is `Unknown`. Two matchers stay as they are, so
  the refactor changes no behavior: `inventory::match_pane` keeps its basename-suffix match (a
  broker `/status` row carries no worktree path; replacing it needs the roster as a new input),
  reading owners through the port; and `sweep.sh`'s `resolve_agent_for_path` keeps its
  longest-realpath-prefix match, documented as a deliberate difference because it maps arbitrary
  paths, not surfaces, to agents. A parity test pins the surface case (an owner equal to a
  worktree root names the same agent on both sides). The label-based classifiers (`@paw_role`
  liveness filter, `discover_coding_panes`) are unchanged.
- **One fake.** A domain-level fake backend (scripted screens and owners, recorded operations)
  replaces the per-trait fakes for logic tests; real backends are smoke-tested.
- **Dependency direction fixed.** Capture lives in the adapter; `tmux/` imports nothing from
  `supervisor/`.
- **Guards.** Source audits: no `tmux` process outside the adapter (environment probes in
  `doctor` / `selftest` excepted by name); no tmux vocabulary in the port.
- **Persistence.** `pane_index` / `dashboard_pane` keep name, type and bytes as the tmux adapter's
  encoding of a `SurfaceId`; a future backend persists its own encoding additively.
- Zero new dependencies; compile-time, in-tree adapters only; no CLI or config change.

**Not in scope:** dropping tmux, a PTY / headless / zellij backend (the tmux decision is a
decision-only spike this cycle), any behavior change.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `code-architecture`: the terminal port in domain terms keyed by `SurfaceId`; tmux as first
  adapter with byte-identical behavior; the fake backend; confinement guard; persisted-handle
  back-compat.
- `supervisor-unattended-operation`: pane-to-agent resolution asks the port which worktree owns a
  surface (exact canonical match; the dashboard found by the layout record; no supervisor pane).

## Impact

- **Predecessors (implementation order):** archive position 12 of 20, after
  `supervisor-role-typing` (1: `SurfaceRole`, `SessionLayout` live in core; geometry already out
  of `supervisor/`), `supervisor-pane-removal` (6) + `pane-less-session-parity` (9) (no
  supervisor-pane cases; one grid layout; gate on every path), `drive-loop-reattach` (10, the
  drive-loop entry) and `launch-prompt-entry-point` (11, `submit_prompt_to_pane`, the task
  prompt shape, the idle wait and the entry-point submit — this change moves all of them behind
  the port afterwards, so its argv fixtures are taken on the post-11 code). This change is the one place *Pane-to-agent resolution via
  pane_current_path* is modified this cycle (built on the released text; no earlier change in the
  order touches it). v0.17: `bundled-helper-discovery`
  (sweep.sh reads the roster by full path — the bash mirror this change keeps in parity),
  `live-prompt-tail-window` (sweep.sh gate parity), `classifier-retirement` (deletes `poll.rs`,
  `approve.rs`, `approval_send_gate` and the attended approver; moves `KeyDispatcher` /
  `TmuxKeyDispatcher` to `src/supervisor/keys.rs` and prompt detection to
  `src/supervisor/permission_prompt.rs`; the drive loop sends only nudge and buffered-input
  keystrokes — the port is built over that surviving surface, and no header is shared).
- **Successors:** `broker-host-separation` adds a service-surface operation to the port;
  `drive-loop-functional-core` observes and acts only through the port.
- **Code:** new `src/terminal/{mod,tmux,fake}.rs` (`pub mod terminal`); `src/supervisor/{drive,keys,
  approval_gate,permission_prompt,claim}.rs`, `src/coordination/inventory.rs`,
  `src/commands/{start,recover,remove,add,helpers,supervisor}.rs`, `src/main.rs`
  (`submit_prompt_to_pane`, `launch-prompt-entry-point`'s `submit_entry_points`), `src/tmux/readiness.rs`, `src/tmux/session.rs` (the
  `resolve_pane_id_for_worktree` / `resolve_pane_index_for_worktree` / `agents_without_live_pane`
  path comparisons delegate to the core owner match); `src/tmux/` remains the command-building
  substrate. The seams that survive v0.17.0 (`PaneEnumerator` / `PaneCapture` in `drive.rs`,
  `KeyDispatcher` in `keys.rs`) are implemented over the port. `assets/scripts/sweep.sh`: a
  comment on `resolve_agent_for_path` documenting the prefix-match difference, no logic change.
- **Persisted state:** none changes; `sweep.sh`'s discovery contract is untouched.
- **Tests:** existing behavioral suites unchanged; fake-backend logic tests; argv fixtures; two
  source audits; the `sweep.sh` surface-case parity test.
- **Docs:** `docs/src/architecture.md` module list gains `src/terminal/`;
  `docs/src/architecture-direction.md` "Where the code is today" row for Terminal updated.
- **Enum-variant ripple:** none (`BrokerMessage`, `SpecBackendKind` untouched).
- **Security / safety:** no new privilege or input surface; argv still flows through
  `TmuxCommand` / `CommandRunner` with the existing newtype quoting. The drive loop still sends no
  approval keystrokes; its nudge / submit keystrokes go through the port unchanged.
- **Dependencies:** none.

## Rebase note (2026-10-20)

`approval-pane-map-refresh` (GP-45) was dropped from v0.17.0, superseded by `classifier-retirement`, which deletes the attended auto-approver, `poll.rs` and its positional pane map outright. GP-45 is therefore moot: this change's `approval-auto` delta (*The attended approver resolves agent surfaces on every poll*) and its GP-45 tasks were dropped, and the fake-backend drive-loop scenario asserts escalation with no keystrokes instead of an approval.

## Rebase note (2026-10-25): audit against released v0.17.0

- Added the owner-resolution sites the inventory missed: `tmux::resolve_pane_index_for_worktree` (`tmux/session.rs:571`, used by `restart_from_pause`) and `agents_without_live_pane` (`add`'s reconciliation), plus the label-based classifiers (`@paw_role` liveness filter, `sweep.sh discover_coding_panes`), which stay unchanged.
- Design call 1: surfaces match by exact canonical-path equality in Rust, as `drive.rs:221` and `tmux/session.rs:584/:614` already do; `sweep.sh`'s longest-realpath-prefix `resolve_agent_for_path` stays as a documented deliberate difference, and the parity test covers the surface case only.
- Design call 2: `inventory::match_pane` keeps its basename-suffix match (the earlier D4 dropped it, a behavior change, and `StatusAgent` has no worktree path). With both calls applied, "no behavior change" holds.
- The dashboard is identified by the layout record, not by a repo-root owner; an unmatched owner is `Unknown`. This matches `supervisor-role-typing` and `supervisor-pane-removal`.
- Scope narrowed to the orchestration path (drive loop, launch paths, `add` / `remove`, readiness gate, prompt submission, inventory); `pause`, `stop`, `attach`, `status` and `doctor` keep calling `src/tmux/` helpers.
- Predecessors updated to the archive order (position 12, after `drive-loop-reattach` and `launch-prompt-entry-point`); this change is the single place *Pane-to-agent resolution via pane_current_path* is modified.
- `launch-prompt-entry-point` (11) adds an idle wait before typing a declared workflow entry point and a separate entry-point submit; the port carries both (`capture_tail` observations for the idle check, `send_text` + `send_key` for the submit), with a fake-backend test and argv fixtures, so the "no `tmux` process outside the adapter" audit holds after both changes.
