## 1. Grid geometry for every session

- [x] 1.1 Grid geometry takes "has top row" as an input (`supervisor_layout` / `layout_for` in `src/surface.rs`, `SupervisorLayout`); no-dashboard variant (agents from 0, rows share 100%, no top-row resize in `push_supervisor_resize_pass`); unit tests for 1/5/6/10/25 agents with and without a dashboard. (`build_add_agent_commands`, `build_remove_retile_commands` and `rebalance_agent_rows` ALREADY take `first_agent` from `SessionLayout` — landed with `supervisor-pane-removal`; only the layout-record argument they are passed changes.)
- [x] 1.2 `TmuxSessionBuilder::build` (`src/tmux/command.rs`) emits the agent grid, sharing one split+resize routine with `build_supervisor_session`; no `select-layout tiled` / `main-horizontal` for agents; tests for the "Every pane-less session uses the agent grid" scenarios (bare, spec-driven, broker-less). `cmd_start`, `launch_spec_session` and `recover_bare_session` all build through `TmuxSessionBuilder`, so they need no call-site change.
- [x] 1.3 Recovery (`commands/recover.rs`) rebuilds bare sessions as grids and re-saves the layout; test "Recovery rebuilds a grid"

## 2. Layout record

- [x] 2.1 Add the optional `Session` layout field (`serde(default, skip_serializing_if)`), written by every start / recovery / restart; `SessionLayout` reads it (absent + bare = tiled, absent + supervised = grid); round-trip and earlier-file load tests

## 3. Hot-attach on every grid session (GP-49)

- [x] 3.1 Replace `commands/helpers.rs::bare_mode_unsupported` with a legacy-tiled refusal in `cmd_add` / `cmd_remove`, checked before any mutation; message names the tiled layout, tells the user to stop and start again, cites no version; replace the `bare_mode_unsupported` unit tests (`commands/helpers.rs`) that pinned the REMOVED *Bare-session add and remove refusal states the limitation plainly*
- [x] 3.2 `tests/add_remove_e2e.rs`: add to / remove from a bare grid session; add to a broker-less grid session; add to a pre-relocation supervised session (agents from 2); legacy tiled refusal leaves worktrees, panes and session JSON untouched
- [x] 3.3 Re-tiling requirement tests re-indexed (agents from 1; dashboard only when present)

## 4. Passive discovery and the live roster

- [x] 4.1 `drive_loop` (`src/supervisor/drive.rs`) re-reads the coding-agent roster (`AgentPane`) from the session state each sweep through an injected `DriveDeps` port (the loop's `agents: &[AgentPane]` and the `coding_ids` derived at loop start are both fixed today); keep the previous roster on a read failure; completion, branch refresh and the exit summary use the last-read roster
- [x] 4.2 Tests: a live prompt on an agent added mid-run is escalated by the running loop (a `permission-prompt` naming that agent, no answering keystroke); a removed agent gets no further keystrokes and is absent from the summary; an unreadable roster keeps the previous one; add/remove send no keystroke to any other surface; the discovery file lists/omits the agent (already rewritten by `cmd_add` / `cmd_remove` — assert it, do not re-implement)

## 5. Readiness gate on the last ungated path (DS-5)

- [x] 5.1 Gate `restart_from_pause`'s held-prompt submit (`commands/start.rs`, the `has_pending` block around `:1005`, submit at `:1011`) per pending pane; a dialog-stuck pane keeps its held prompt (do NOT `.take()` it before the gate passes — the loop saves the session straight after) and is reported on stderr while the rest of the session restarts. The gate must NOT be `?`-propagated here, unlike the other call sites. Resolve each pending pane's index the way the discovery-file rewrite 20 lines below does (`tmux::resolve_pane_index_for_worktree`), not `offset + idx`, because a resume re-splits the dashboard at whatever index tmux assigns. (Bare `start` at `:430` and `launch_spec_session` at `:861` are already gated by v0.17.0.)
- [x] 5.2 Tests: restart keeps the held prompt for a dialog-stuck pane and names it on stderr (scenario "Restart from pause gates a held prompt", in `tests/pause_e2e.rs` — which today has no pending-prompt harness: its `write_session_json` must grow a `pending_boot_prompt` and its simulated agent pane must run the dialog stub; reuse `write_dialog_stub_cli` and the `GIT_PAW_READINESS_TIMEOUT_MS=3000` override from `tests/start_spec_launch_parity_integration.rs`, without which the gate falls back before the dialog line renders and the test flakes); map "Bare start is gated" and "Spec-driven start stuck on a first-run dialog fails loudly" to the existing `bare_launch_fails_loudly_on_stuck_dialog` / `spec_driven_launch_fails_loudly_on_stuck_dialog` (`tests/start_spec_launch_parity_integration.rs`), adding a bare-shell pre-fill test only if none pins it; unrecognised CLI still falls back to fixed-budget injection

## 6. Docs and gates

- [x] 6.1 `docs/src/user-guide/session-lifecycle.md` (add/remove on every session; the tiled-layout refusal and its fix), `docs/src/architecture.md` (one layout), `user-guide/dashboard.md` (grid below the dashboard), `cli-reference.md` + `add`/`remove` `--help` (no "supervisor-mode only"), README CLI table; `mdbook build docs/` succeeds
- [x] 6.2 Every scenario maps to a named test; `just check` + `just deny` green; full suite green vs merge-base (`cargo test --no-fail-fast`)
